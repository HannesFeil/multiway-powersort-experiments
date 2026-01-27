//! contains structs implementing [`MergingMethod`], which implement various strategies
//! for merging adjacent runs in a slice.

pub mod multi_way;
pub mod two_way;

pub mod util {
    /// Iterates through `iter` and returns the first element `current` with the proceeding element
    /// `next`, such that `f(current, next) == true` and returns `Some(current)`
    ///
    /// If `f(current, next)` is never true, returns `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `iter` returns `None` at the start
    fn find_first_sequentially<T>(
        mut iter: impl Iterator<Item = T>,
        mut f: impl FnMut(&T, &T) -> bool,
    ) -> Result<Option<T>, ()> {
        let mut current = iter.next().ok_or(())?;

        for next in iter {
            if f(&current, &next) {
                return Ok(Some(current));
            } else {
                current = next;
            }
        }

        Ok(None)
    }

    /// Returns the largest `index`, such that `slice[..index]` is weakly increasing
    pub fn weakly_increasing_prefix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate();

        // Find the index of the first element breaking the sequence
        match find_first_sequentially(iter, |(_, current), (_, next)| current > next) {
            // Found the index
            Ok(Some((index, _))) => index + 1,
            // Sequence is not found, split into full and empty slice
            Ok(None) => slice.len(),
            // Slice is empty, split into two empty slices
            Err(()) => 0,
        }
    }

    /// Returns the smallest `index`, such that `slice[index..]` is weakly increasing
    pub fn weakly_increasing_suffix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate().rev();

        // Find the index of the first element breaking the sequence
        match find_first_sequentially(iter, |(_, current), (_, previous)| current < previous) {
            // Found the index
            Ok(Some((index, _))) => index,
            // Sequence is not found, split into full and empty slice
            Ok(None) => slice.len(),
            // Slice is empty, split into two empty slices
            Err(()) => 0,
        }
    }

    /// Returns the largest `index`, such that `slice[..index]` is strictly decreasing
    pub fn strictly_decreasing_prefix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate();

        // Find the index of the first element breaking the sequence
        match find_first_sequentially(iter, |(_, current), (_, next)| current <= next) {
            // Found the index
            Ok(Some((index, _))) => index + 1,
            // Sequence is not found, split into full and empty slice
            Ok(None) => slice.len(),
            // Slice is empty, split into two empty slices
            Err(()) => 0,
        }
    }

    /// Returns the smallest `index`, such that `slice[index..]` is strictly decreasing
    pub fn strictly_decreasing_suffix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate().rev();

        // Find the index of the first element breaking the sequence
        match find_first_sequentially(iter, |(_, current), (_, previous)| current >= previous) {
            // Found the index
            Ok(Some((index, _))) => index,
            // Sequence is not found, split into full and empty slice
            Ok(None) => slice.len(),
            // Slice is empty, split into two empty slices
            Err(()) => 0,
        }
    }

    /// Returns the largest (`index`, `decreasing`), such that `slice[index..]` is weakly increasing or
    /// strictly decreasing. `decreasing` indicating if the found sequence is strictly decreasing.
    pub fn weakly_increasing_or_strictly_decreasing_index<T: Ord>(
        slice: &mut [T],
    ) -> (usize, bool) {
        if slice.len() < 2 {
            return (slice.len(), false);
        }

        let (first, rest) = slice.split_first().unwrap();

        if first > rest.first().unwrap() {
            (strictly_decreasing_prefix_index(rest) + 1, true)
        } else {
            (weakly_increasing_prefix_index(rest) + 1, false)
        }
    }
}

/// Copied from [`std::slice::sort::stable::BufGuard<T>`]
pub trait BufGuard<T> {
    /// Creates new buffer that holds at least `capacity` memory.
    fn with_capacity(capacity: usize) -> Self;
    /// Returns mutable access to uninitialized memory owned by the buffer.
    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>];
}

#[allow(dead_code)]
pub static ALLOC_COUNTER: crate::data::GlobalCounter = crate::data::GlobalCounter::new();

impl<T> BufGuard<T> for Vec<T> {
    fn with_capacity(capacity: usize) -> Self {
        #[cfg(feature = "counters")]
        ALLOC_COUNTER.increase(capacity as u64);

        Vec::with_capacity(capacity)
    }

    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>] {
        self.spare_capacity_mut()
    }
}

#[allow(dead_code)]
pub static MERGE_SLICE_COUNTER: crate::data::GlobalCounter = crate::data::GlobalCounter::new();
#[allow(dead_code)]
pub static MERGE_BUFFER_COUNTER: crate::data::GlobalCounter = crate::data::GlobalCounter::new();

mod slice {
    /// Copies the first `count` elements from `src` to `dst` and returns the slices with the
    /// prefix stripped, e.g. `(&src[count..], &mut dst[count..])`
    pub(super) fn copy_prefix_to_uninit<T>(
        src: &mut &[T],
        dst: &mut &mut [std::mem::MaybeUninit<T>],
        count: usize,
    ) {
        assert!(src.len() >= count && dst.len() >= count);

        // SAFETY: We checked that src and dst are long enough.
        // The writing is valid since MaybeUninit<T> has the same layout, size and ABI as as T and
        // elements in [T] are guaranteed to be laid out sequentially in memory
        // (see https://doc.rust-lang.org/reference/type-layout.html#slice-layout)).
        //
        // Additionally the owner of dst is responsible for not causing UB when reading non Copy
        // elements.
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr() as *mut T, count);
        }

        // Adjust slice sizes
        *src = src.split_off(count..).unwrap();
        *dst = dst.split_off_mut(count..).unwrap();
    }

    /// Copies the first `count` elements from `src` to `dst` and returns the slices with the
    /// prefix stripped, e.g. `(&mut src[count..], &mut dst[count..])`
    pub(super) fn copy_mut_prefix_to_uninit<T>(
        src: &mut &mut [T],
        dst: &mut &mut [std::mem::MaybeUninit<T>],
        count: usize,
    ) {
        assert!(src.len() >= count && dst.len() >= count);

        let temp_src = &mut &(**src);
        copy_prefix_to_uninit(temp_src, dst, count);

        // Adjust src size (dst has been adjusted by copy_prefix_to_uninit)
        *src = src.split_off_mut(count..).unwrap();
    }

    /// Copies the last `count` elements from `src` to `dst` and returns the slices with the
    /// prefix stripped, e.g. `(&src[count..], &mut dst[count..])`
    pub(super) fn copy_suffix_to_uninit<T>(
        src: &mut &[T],
        dst: &mut &mut [std::mem::MaybeUninit<T>],
        count: usize,
    ) {
        assert!(src.len() >= count && dst.len() >= count);
        let src_offset = src.len() - count;
        let dst_offset = dst.len() - count;

        // SAFETY: We checked that src and dst are long enough.
        // The writing is valid since MaybeUninit<T> has the same layout, size and ABI as as T and
        // elements in [T] are guaranteed to be laid out sequentially in memory
        // (see https://doc.rust-lang.org/reference/type-layout.html#slice-layout)).
        //
        // Additionally the owner of dst is responsible for not causing UB when reading non Copy
        // elements.
        unsafe {
            std::ptr::copy_nonoverlapping(
                src.as_ptr().add(src_offset),
                dst.as_mut_ptr().add(dst_offset) as *mut T,
                count,
            );
        }

        // Adjust slice sizes
        *src = src.split_off(..src_offset).unwrap();
        *dst = dst.split_off_mut(..dst_offset).unwrap();
    }

    /// Copies the last `count` elements from `src` to `dst` and returns the slices with the
    /// suffix stripped, e.g. `(&mut src[count..], &mut dst[count..])`
    pub(super) fn copy_mut_suffix_to_uninit<T>(
        src: &mut &mut [T],
        dst: &mut &mut [std::mem::MaybeUninit<T>],
        count: usize,
    ) {
        assert!(src.len() >= count && dst.len() >= count);
        let src_offset = src.len() - count;

        let temp_src = &mut &(**src);
        copy_suffix_to_uninit(temp_src, dst, count);

        // Adjust src size (dst has been adjusted by copy_prefix_to_uninit)
        *src = src.split_off_mut(..src_offset).unwrap();
    }
}

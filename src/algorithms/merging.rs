//! contains structs implementing [`MergingMethod`], which implement various strategies
//! for merging adjacent runs in a slice.

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
pub fn weakly_increasing_prefix_index<T: Ord>(slice: &mut [T]) -> usize {
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
pub fn weakly_increasing_suffix_index<T: Ord>(slice: &mut [T]) -> usize {
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
pub fn strictly_decreasing_prefix_index<T: Ord>(slice: &mut [T]) -> usize {
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
pub fn strictly_decreasing_suffix_index<T: Ord>(slice: &mut [T]) -> usize {
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

/// Copied from [`std::slice::sort::stable::BufGuard<T>`]
pub trait BufGuard<T> {
    /// Creates new buffer that holds at least `capacity` memory.
    fn with_capacity(capacity: usize) -> Self;
    /// Returns mutable access to uninitialized memory owned by the buffer.
    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>];
}

impl<T> BufGuard<T> for Vec<T> {
    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }

    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>] {
        self.spare_capacity_mut()
    }
}

/// Specifies ways to merge two adjacent runs in a slice, given a buffer
pub trait MergingMethod {
    /// Whether the merging method is stable
    const IS_STABLE: bool;

    /// Merge the two sorted runs `0..split_point` and `split_point..slice.len()`, potentially
    /// using `buffer`.
    fn merge<T: Ord>(slice: &mut [T], split_point: usize, buffer: &mut [std::mem::MaybeUninit<T>]);

    /// The required capacity of the buffer, needed for merging slices with length less than
    /// or equal to `size`.
    fn required_capacity(size: usize) -> usize {
        size
    }
}

mod pointer_range {
    /// A sequential pointer range, pointing to a slice
    pub(super) struct PointerRange<'a, T>(
        /// The backing range, start can never be larger then end
        std::ops::Range<*mut T>,
        /// A lifetime marker used to tie this range to a slice reference
        std::marker::PhantomData<&'a mut ()>,
    );

    impl<'a, T> From<&'a mut [T]> for PointerRange<'a, T> {
        fn from(value: &'a mut [T]) -> Self {
            Self(value.as_mut_ptr_range(), std::marker::PhantomData)
        }
    }

    impl<'a, T> PointerRange<'a, T> {
        /// Returns whether this pointer range is empty
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        /// Returns the inclusive start pointer of this range
        pub fn start(&self) -> *mut T {
            self.0.start
        }

        /// Returns the exclusive end pointer of this range
        pub fn end(&self) -> *mut T {
            self.0.end
        }

        /// Returns the length of this range
        pub fn len(&self) -> usize {
            // SAFETY: self.0.end can never be less than self.0.start
            unsafe { self.0.end.offset_from_unsigned(self.0.start) }
        }
    }

    /// Copy `count` elements from `src` to `dst` and advances both ranges (adding `count` to their
    /// start)
    ///
    /// # Safety
    /// The length of both `src` and `dst` has to be greater or equal to `count`
    ///
    /// Additional safety concerns regaring [`std::ptr::copy_nonoverlapping()`] also apply
    pub unsafe fn uninit_copy_prefix_and_advance<T>(
        src: &mut PointerRange<T>,
        dst: &mut PointerRange<std::mem::MaybeUninit<T>>,
        count: usize,
    ) {
        debug_assert!(src.len() >= count && dst.len() >= count);

        // SAFETY: See function documentation. The cast as `*mut T` is allowed because
        // of the safety requirements for [`std::mem::MaybeUninit`]
        unsafe {
            std::ptr::copy_nonoverlapping(src.0.start, dst.0.start as *mut T, count);
            src.0.start = src.0.start.add(count);
            dst.0.start = dst.0.start.add(count);
        }
    }

    /// Copy `count` elements from `src` to `dst` and shrinks both ranges (subtracting `count` from
    /// their ends)
    ///
    /// # Safety
    /// The length of both `src` and `dst` has to be greater or equal to `count`
    ///
    /// Additional safety concerns regaring [`std::ptr::copy_nonoverlapping()`] also apply
    pub unsafe fn uninit_copy_suffix_and_shrink<T>(
        src: &mut PointerRange<T>,
        dst: &mut PointerRange<std::mem::MaybeUninit<T>>,
        count: usize,
    ) {
        debug_assert!(src.len() >= count && dst.len() >= count);

        // SAFETY: See function documentation. The cast as `*mut T` is allowed because
        // of the safety requirements for [`std::mem::MaybeUninit`]
        unsafe {
            src.0.end = src.0.end.sub(count);
            dst.0.end = dst.0.end.sub(count);

            std::ptr::copy_nonoverlapping(src.0.end, dst.0.end as *mut T, count);
        }
    }
}

/// A [`MergingMethod`] implementation via a simple merging procedure
///
/// The `buffer` given in [`Self::merge`] has to have at least the same
/// size as the `slice`.
#[derive(Debug, Clone, Copy)]
pub struct CopyBoth;

impl MergingMethod for CopyBoth {
    const IS_STABLE: bool = true;

    fn merge<T: Ord>(slice: &mut [T], split_point: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.is_empty() {
            return;
        }

        assert!(
            buffer.len() >= slice.len(),
            "Buffer needs to have at least the size of slice"
        );
        assert!(
            (0..slice.len()).contains(&split_point),
            "Split points needs to be in bounds"
        );

        {
            let mut output: pointer_range::PointerRange<_> =
                (&mut (&mut *buffer)[..slice.len()]).into();
            let (left, right) = slice.split_at_mut(split_point);
            let mut left: pointer_range::PointerRange<T> = left.into();
            let mut right: pointer_range::PointerRange<T> = right.into();

            // NOTE: We copy after the merging as opposed to before, to prevent inconsistent
            // state which could occur when panicking on merging into slice

            // SAFETY: All pointers from slice are kept in bounds of their respective range.
            // Since it is assumed that slice.len() <= buffer.len() and in total slice.len()
            // elements are written into buffer one by one, these accesses are guaranteed to be
            // in bounds as well. The writing is valid since MaybeUninit<T> has the same layout,
            // size and ABI as as T and elements in [T] are guaranteed to be laid out sequentially
            // in memory (see https://doc.rust-lang.org/reference/type-layout.html#slice-layout)).
            //
            // Additionally each element is written into buffer exactly once,
            // so that buffer ends up as a permutation of slice.
            unsafe {
                // Repeatedly copy the smaller element of both runs into the buffer
                while !left.is_empty() && !right.is_empty() {
                    if *left.start() <= *right.start() {
                        pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, 1);
                    } else {
                        pointer_range::uninit_copy_prefix_and_advance(&mut right, &mut output, 1);
                    }
                }

                // Copy the rest of the remaining run into the buffer
                if !left.is_empty() {
                    let count = left.len();
                    pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, count);
                }
                if !right.is_empty() {
                    let count = right.len();
                    pointer_range::uninit_copy_prefix_and_advance(&mut right, &mut output, count);
                }
            }
        }

        // SAFETY: Since buffer now contains a permutation of slice, we can safely copy it over to
        // slice, again regarding the same layout invariant for T and MaybeUninit<T>. (see above)
        unsafe {
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr() as *const T,
                slice.as_mut_ptr(),
                slice.len(),
            );
        }
    }
}

// TODO: update description (especially space requirement)
/// A [`MergingMethod`] implementation via a galloping merge procedure
///
/// The `buffer` given in [`Self::merge`] has to have at least the same
/// size as the `slice`.
#[derive(Debug, Clone, Copy)]
pub struct Galloping<const MIN_GALLOP: usize = 7>;

impl<const MIN_GALLOP: usize> MergingMethod for Galloping<MIN_GALLOP> {
    const IS_STABLE: bool = true; // TODO: check this

    fn merge<T: Ord>(slice: &mut [T], split_point: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() < 2 || split_point == 0 {
            return;
        }

        let start = Self::gallop::<T, false>(&slice[split_point], &slice[..split_point], 0);
        if start == split_point {
            return;
        }

        let end = Self::gallop::<T, true>(
            &slice[split_point - 1],
            &slice[split_point..],
            slice.len() - split_point - 1,
        ) + split_point;
        if end == split_point {
            return;
        }

        let mut min_gallop = MIN_GALLOP;

        if split_point - start <= end - split_point {
            Self::merge_low(
                &mut slice[start..end],
                split_point - start,
                buffer,
                &mut min_gallop,
            );
        } else {
            Self::merge_high(
                &mut slice[start..end],
                split_point - start,
                buffer,
                &mut min_gallop,
            );
        }
    }
}

impl<const MIN_GALLOP: usize> Galloping<MIN_GALLOP> {
    // FIXME: fix this comment, more precise
    /// Return the insertion index of `key` in `slice`, assuming `slice` is sorted.
    /// `hint` is the starting index, from which to gallop.
    /// If `LEFT`, gallop left and otherwise gallop right.
    fn gallop<T: Ord, const LEFT: bool>(key: &T, slice: &[T], hint: usize) -> usize {
        debug_assert!(slice.is_sorted());
        assert!((0..slice.len()).contains(&hint));

        let mut last_offset = 0;
        let mut offset = 1;

        // Determine comparison functions depending on galloping direction
        type Comparator<T> = fn(&T, &T) -> bool;
        let (cmp, cmp_negated): (Comparator<T>, Comparator<T>) =
            if LEFT { (T::gt, T::le) } else { (T::ge, T::lt) };

        // check if we're searching slice[..hint] or slice[hint..]
        if cmp(key, &slice[hint]) {
            // Use quadratic search to find the containing interval
            let max_offset = slice.len() - hint;
            while offset < max_offset && cmp(key, &slice[hint + offset]) {
                last_offset = offset;
                // TODO: is this correct wrg. to overflow
                offset = (offset << 1) + 1;
            }
            offset = std::cmp::min(offset, max_offset);

            // Since we searched slice[hint..] we have to add it as starting offset
            last_offset += hint + 1;
            offset += hint;
        } else {
            // Use quadratic search to find the containing interval
            let max_offset = hint + 1;
            while offset < max_offset && cmp_negated(key, &slice[hint - offset]) {
                last_offset = offset;
                // TODO: is this correct wrg. to overflow
                offset = (offset << 1) + 1;
            }
            offset = std::cmp::min(offset, max_offset);

            // Since we searched slice[..hint] backwards, we reverse our offset
            let tmp = last_offset;
            last_offset = hint + 1 - offset;
            offset = hint - tmp;
        }
        assert!(last_offset < offset + 1 && offset <= slice.len());

        // Perform binary search in the found interval
        let result = slice[last_offset..offset].partition_point(|x| cmp(key, x)) + last_offset;

        debug_assert_eq!(result, slice.partition_point(|x| cmp(key, x)),);

        result
    }

    /// Sort the given `slice` assuming `slice[..split_point]` and `slice[split_point..]` are
    /// already sorted.
    fn merge_low<T: Ord>(
        slice: &mut [T],
        split_point: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        assert!(
            buffer.len() >= slice.len(),
            "We need at least slice.len() buffer size"
        );
        assert!(
            (0..slice.len()).contains(&split_point),
            "Split point has to be within slice bounds"
        );

        {
            // TODO: unchecked this?
            let mut output: pointer_range::PointerRange<_> = (&mut buffer[..slice.len()]).into();
            let (left, right) = slice.split_at_mut(split_point);
            let mut left: pointer_range::PointerRange<_> = left.into();
            let mut right: pointer_range::PointerRange<_> = right.into();

            // TODO: safety comment
            // TODO: do I want to count lengths?
            // TODO: write wrapper struct for pointer range maybe?
            unsafe {
                pointer_range::uninit_copy_prefix_and_advance(&mut right, &mut output, 1);

                // Right side only had one element, only need to copy the left side
                if right.is_empty() {
                    let count = left.len();
                    pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, count);

                    // Copy back to slice
                    std::ptr::copy_nonoverlapping(
                        buffer.as_ptr() as *const T,
                        slice.as_mut_ptr(),
                        slice.len(),
                    );
                    return;
                }

                // Left side only has one element, copy the rest of the right side and then the one
                // element from the left side
                if split_point == 1 {
                    let count = right.len();
                    pointer_range::uninit_copy_prefix_and_advance(&mut right, &mut output, count);
                    pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, 1);

                    // Copy back to slice
                    std::ptr::copy_nonoverlapping(
                        buffer.as_ptr() as *const T,
                        slice.as_mut_ptr(),
                        slice.len(),
                    );
                    return;
                }

                'outer: loop {
                    let mut count1 = 0;
                    let mut count2 = 0;

                    while (count1 | count2) < *min_gallop {
                        assert!(left.len() > 1);
                        assert!(!right.is_empty());

                        if *right.start() < *left.start() {
                            // Advance the right side
                            pointer_range::uninit_copy_prefix_and_advance(
                                &mut right,
                                &mut output,
                                1,
                            );
                            count2 += 1;
                            count1 = 0;

                            if right.is_empty() {
                                break 'outer;
                            }
                        } else {
                            // Advance the left side
                            pointer_range::uninit_copy_prefix_and_advance(
                                &mut left,
                                &mut output,
                                1,
                            );
                            count1 += 1;
                            count2 = 0;

                            if left.len() <= 1 {
                                break 'outer;
                            }
                        }
                    }

                    while count1 >= MIN_GALLOP || count2 >= MIN_GALLOP {
                        assert!(left.len() > 1);
                        assert!(!right.is_empty());

                        count1 = Self::gallop::<T, false>(
                            &*right.start(),
                            std::slice::from_raw_parts(left.start(), left.len()),
                            0,
                        );
                        if count1 != 0 {
                            pointer_range::uninit_copy_prefix_and_advance(
                                &mut left,
                                &mut output,
                                count1,
                            );

                            if left.len() <= 1 {
                                break 'outer;
                            }
                        }

                        pointer_range::uninit_copy_prefix_and_advance(&mut right, &mut output, 1);

                        if right.is_empty() {
                            break 'outer;
                        }

                        count2 = Self::gallop::<T, true>(
                            &*left.start(),
                            std::slice::from_raw_parts(right.start(), right.len()),
                            0,
                        );
                        if count2 != 0 {
                            pointer_range::uninit_copy_prefix_and_advance(
                                &mut right,
                                &mut output,
                                count2,
                            );

                            if right.is_empty() {
                                break 'outer;
                            }
                        }

                        pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, 1);

                        if left.len() == 1 {
                            break 'outer;
                        }

                        *min_gallop = min_gallop.saturating_sub(1);
                    }

                    *min_gallop += 2;
                }

                *min_gallop = std::cmp::max(*min_gallop, 1);

                if left.len() == 1 {
                    assert!(!right.is_empty());
                    let count = right.len();
                    pointer_range::uninit_copy_prefix_and_advance(&mut right, &mut output, count);
                    pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, 1);
                } else {
                    assert!(!left.is_empty());
                    assert!(right.is_empty());
                    let count = left.len();
                    pointer_range::uninit_copy_prefix_and_advance(&mut left, &mut output, count);
                }
            }
        }
        // Copy back the merged elements from the buffer
        // TODO: safety comment
        unsafe {
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr() as *const T,
                slice.as_mut_ptr(),
                slice.len(),
            );
        }
    }

    fn merge_high<T: Ord>(
        slice: &mut [T],
        split_point: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        Self::merge_low(slice, split_point, buffer, min_gallop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::{Rng as _, RngCore};

    /// How big the test arrays should be
    const TEST_SIZE: usize = 100;
    /// How many times to run each test
    const TEST_RUNS: usize = 100;

    macro_rules! test_methods {
        ($($method:ident),*) => {
            $(
                paste::paste! {
                    mod [< $method:snake >] {
                        use super::*;

                        test_methods!(@single $method);
                    }
                }
            )*
        };
        (@single $method:ident) => {
            #[test]
            fn test_empty_merges() {
                test_empty_merge::<$method>();
            }

            #[test]
            fn test_correct_merges() {
                test_correct_merge::<$method>();
            }

            #[test]
            fn test_correct_stable_merges() {
                if <$method>::IS_STABLE {
                    test_correct_stable_merge::<$method>();
                }
            }

            #[test]
            fn test_soundness_merges() {
                test_soundness_merge::<$method>();
            }
        };
    }

    test_methods!(CopyBoth, Galloping);

    /// Test merging an empty slice
    fn test_empty_merge<T: MergingMethod>() {
        let mut elements = [(); 0];
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));

        // This should not panic nor cause UB
        T::merge(&mut elements, 0, buffer.as_uninit_slice_mut())
    }

    /// Test that two runs are correctly merged
    fn test_correct_merge<T: MergingMethod>() {
        let mut rng = crate::test::test_rng();
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));

        // Test random runs
        for run in 0..TEST_RUNS {
            let mut elements: Box<[usize]> = (0..TEST_SIZE)
                .map(|_| rng.random_range(0..usize::MAX))
                .collect();
            let split = rng.random_range(0..TEST_SIZE);
            elements[..split].sort();
            elements[split..].sort();

            T::merge(&mut elements, split, buffer.as_uninit_slice_mut());

            assert!(
                elements.is_sorted(),
                "Resulting elements were not sorted by {name} in run {run}",
                name = std::any::type_name::<T>(),
            );
        }

        // Test random runs, split at 0 and n - 1
        for split in [0, TEST_SIZE - 1] {
            let mut elements: Box<[usize]> = (0..TEST_SIZE)
                .map(|_| rng.random_range(0..usize::MAX))
                .collect();
            elements[..split].sort();
            elements[split..].sort();

            T::merge(&mut elements, split, buffer.as_uninit_slice_mut());

            assert!(
                elements.is_sorted(),
                "Resulting elements were not sorted by {name} with split {split}",
                name = std::any::type_name::<T>(),
            );
        }
    }

    /// Test that two runs are correctly merged and the ordering of equal elements remains stable
    fn test_correct_stable_merge<T: MergingMethod>() {
        let mut rng = crate::test::test_rng();
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));

        // Test random runs
        for run in 0..TEST_RUNS {
            let mut elements: Box<[_]> = crate::test::IndexedOrdered::map_iter(
                (0..TEST_SIZE).map(|_| rng.random_range(0..TEST_SIZE / 4)),
            )
            .collect();
            let split = rng.random_range(0..TEST_SIZE);
            elements[..split].sort();
            elements[split..].sort();

            T::merge(&mut elements, split, buffer.as_uninit_slice_mut());

            assert!(
                crate::test::IndexedOrdered::is_stable_sorted(&elements),
                "Resulting elements were not sorted by {name} in run {run}\n{elements:?}",
                name = std::any::type_name::<T>(),
            );
        }

        // Test random runs, split at 0 and n - 1
        for split in [0, TEST_SIZE - 1] {
            let mut elements: Box<[_]> = crate::test::IndexedOrdered::map_iter(
                (0..TEST_SIZE).map(|_| rng.random_range(0..TEST_SIZE / 4)),
            )
            .collect();
            elements[..split].sort();
            elements[split..].sort();

            T::merge(&mut elements, split, buffer.as_uninit_slice_mut());

            assert!(
                crate::test::IndexedOrdered::is_stable_sorted(&elements),
                "Resulting elements were not sorted by {name} with split {split}\n{elements:?}",
                name = std::any::type_name::<T>(),
            );
        }
    }

    /// Run Merging methods with [`crate::test::RandomOrdered`] elements and
    /// [`crate::test::MaybePanickingOrdered`] elements, mostly useful for running under miri
    fn test_soundness_merge<T: MergingMethod>() {
        let mut rng = crate::test::test_rng();
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut maybe_panicking_buffer =
            <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut maybe_panicking_random_buffer =
            <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));

        // Test random runs
        for _ in 0..TEST_RUNS {
            // RandomOrdered elements
            let mut elements: Box<[crate::test::RandomOrdered]> =
                crate::test::RandomOrdered::new_iter(rng.next_u64())
                    .take(TEST_SIZE)
                    .collect();
            let split = rng.random_range(0..TEST_SIZE);

            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(&mut elements, split, buffer.as_uninit_slice_mut());
            }));

            drop(elements);

            // MaybePanickingOrdered elements
            let mut elements: Box<[u32]> = std::iter::repeat_with(|| rng.random())
                .take(TEST_SIZE)
                .collect();
            let split = rng.random_range(0..TEST_SIZE);
            elements[..split].sort();
            elements[split..].sort();

            let mut elements: Box<[crate::test::MaybePanickingOrdered<TEST_SIZE, u32>]> =
                crate::test::MaybePanickingOrdered::map_iter(elements.into_iter(), rng.next_u64())
                    .collect();

            // The types are not actually unwind safe but must not trigger UB anyway
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(
                    &mut elements,
                    split,
                    maybe_panicking_buffer.as_uninit_slice_mut(),
                );
            }));

            // MaybePanickingOrdered RandomOrdered elements
            let mut elements: Box<
                [crate::test::MaybePanickingOrdered<TEST_SIZE, crate::test::RandomOrdered>],
            > = crate::test::MaybePanickingOrdered::map_iter(
                crate::test::RandomOrdered::new_iter(rng.next_u64()).take(TEST_SIZE),
                crate::test::TEST_SEED,
            )
            .collect();

            // The types are not actually unwind safe but must not trigger UB anyway
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(
                    &mut elements,
                    split,
                    maybe_panicking_random_buffer.as_uninit_slice_mut(),
                );
            }));

            drop(elements);
        }
    }
}

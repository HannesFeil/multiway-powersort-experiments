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

// TODO: integrate better?
#[allow(dead_code)]
pub static MERGE_SLICE_COUNTER: crate::data::GlobalCounter = crate::data::GlobalCounter::new();
#[allow(dead_code)]
pub static MERGE_BUFFER_COUNTER: crate::data::GlobalCounter = crate::data::GlobalCounter::new();

#[derive(Debug)]
pub struct Run<T>(std::ops::Range<*mut T>);

impl<T> Clone for Run<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Run<std::mem::MaybeUninit<T>> {
    // Assume all elements in the contained range are initialized.
    pub unsafe fn assume_init(self) -> Run<T> {
        Run(self.0.start as *mut T..self.0.end as *mut T)
    }
}

impl<T> Run<T> {
    pub fn start(&self) -> *mut T {
        self.0.start
    }

    pub fn end(&self) -> *mut T {
        self.0.end
    }

    /// # Safety
    ///
    /// All safety conditions of `<*mut T>::offset_from_unsigned()` must hold for
    /// [`Self::start()`] and [`Self::end()`].
    pub unsafe fn len(&self) -> usize {
        debug_assert!(self.start() <= self.end());

        unsafe { self.0.end.offset_from_unsigned(self.0.start) }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub unsafe fn as_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.start(), self.len()) }
    }

    /// Copies `count` elements from the beginning of this run to the beginning of the other run
    /// and moves both run's starts after the copied elements
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy_nonoverlapping()`] must hold for
    /// [`self.start()`](Self::start()) and [`other.start()`](Self::start()) and `count`.
    pub unsafe fn copy_nonoverlapping_prefix_to(&mut self, other: &mut Self, count: usize) {
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            std::ptr::copy_nonoverlapping(self.0.start, other.0.start, count);

            self.0.start = self.0.start.add(count);
            other.0.start = other.0.start.add(count);
        }
    }

    /// Copies `count` elements from the beginning of this run to the beginning of the other run
    /// and moves both run's starts after the copied elements
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy()`] must hold for
    /// [`self.start()`](Self::start()) and [`other.start()`](Self::start()) and `count`.
    pub unsafe fn copy_prefix_to(&mut self, other: &mut Self, count: usize) {
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            std::ptr::copy(self.0.start, other.0.start, count);

            self.0.start = self.0.start.add(count);
            other.0.start = other.0.start.add(count);
        }
    }

    /// Copies `count` elements from the end of this run to the end of the other run
    /// and moves both run's ends before the copied elements
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy_nonoverlapping()`] must hold for
    /// [`self.end().sub(count)`](Self::end()) and [`other.end().sub(count)`](Self::end()) and `count`.
    pub unsafe fn copy_nonoverlapping_suffix_to(&mut self, other: &mut Self, count: usize) {
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            self.0.end = self.0.end.sub(count);
            other.0.end = other.0.end.sub(count);

            std::ptr::copy_nonoverlapping(self.0.end, other.0.end, count);
        }
    }

    /// Copies `count` elements from the end of this run to the end of the other run
    /// and moves both run's ends before the copied elements
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy()`] must hold for
    /// [`self.end().`](Self::end()) and [`other.end()`](Self::end()) and `count`.
    pub unsafe fn copy_suffix_to(&mut self, other: &mut Self, count: usize) {
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            self.0.end = self.0.end.sub(count);
            other.0.end = other.0.end.sub(count);

            std::ptr::copy(self.0.end, other.0.end, count);
        }
    }
}

pub struct MergingDropGuard<T, const N: usize> {
    pub runs: [Run<T>; N],
    pub output: Run<T>,
    sealed: std::marker::PhantomData<()>,
}

impl<T, const N: usize> MergingDropGuard<T, N> {
    /// Construct a new merging drop guard.
    /// When this struct is dropped, all runs in `runs` which are not empty, will be
    /// written into `output`.
    ///
    /// # Safety
    ///
    /// The sum of the length of the remaining `runs` must be smaller or equal to the length of
    /// `output`. The pointer ranges must be valid to read from and write to respectively.
    /// This invariant must not be invalidated while mutating any of the public fields.
    /// To disarm the guard see [Self::disarm()].
    pub unsafe fn new(runs: [Run<T>; N], output: Run<T>) -> Self {
        Self {
            runs,
            output,
            sealed: std::marker::PhantomData,
        }
    }

    /// Disarms this guard and returns it's components `(runs, output)`.
    ///
    /// This is safe, since no guarantees are given and any unsafe operations during drop are skipped.
    pub fn disarm(self) -> ([Run<T>; N], Run<T>) {
        let dont_drop = std::mem::ManuallyDrop::new(self);
        let runs = dont_drop.runs.clone();
        let output = dont_drop.output.clone();
        (runs, output)
    }

    /// Returns whether all runs are empty and there is nothing to clean up
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(Run::is_empty)
    }
}

impl<T, const N: usize> Drop for MergingDropGuard<T, N> {
    fn drop(&mut self) {
        for run in self.runs.iter_mut() {
            if !run.is_empty() {
                // SAFETY: See condition on [`Self::new()`]
                unsafe {
                    run.copy_prefix_to(&mut self.output, run.len());
                }
            }
        }
    }
}

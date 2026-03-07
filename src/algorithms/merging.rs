//! Contains various implementations for merging adjacent runs in slices.

pub mod multi_way;
pub mod two_way;

pub use multi_way::MultiMergingMethod;
pub use two_way::MergingMethod;

/// Contains various utility methods for the detection of runs.
/// Contains various utility methods for the detection of runs.
pub mod util {
    /// Iterates through `iter` and returns the first element `current` with the proceeding element
    /// `next`, such that `f(current, next) == true` and returns `Some(current)`.
    ///
    /// If `f(current, next)` is never true, returns `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if `iter` instantly returns `None`.
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

    /// Returns the largest `index`, such that `slice[..index]` is weakly increasing.
    pub fn weakly_increasing_prefix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate();

        // Find the index where the next element breaks the run
        match find_first_sequentially(iter, |(_, current), (_, next)| current > next) {
            // Found the index
            Ok(Some((index, _))) => index + 1,
            // Run not broken, return length
            Ok(None) => slice.len(),
            // Slice is empty
            Err(()) => 0,
        }
    }

    /// Returns the smallest `index`, such that `slice[index..]` is weakly increasing.
    pub fn weakly_increasing_suffix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate().rev();

        // Find the index of the first element breaking the run
        match find_first_sequentially(iter, |(_, current), (_, previous)| current < previous) {
            // Found the index
            Ok(Some((index, _))) => index,
            // Run is not broken, return start
            Ok(None) => 0,
            // Slice is empty
            Err(()) => 0,
        }
    }

    /// Returns the largest `index`, such that `slice[..index]` is strictly decreasing
    pub fn strictly_decreasing_prefix_index<T: Ord>(slice: &[T]) -> usize {
        let iter = slice.iter().enumerate();

        // Find the index where the next element breaks the run
        match find_first_sequentially(iter, |(_, current), (_, next)| current <= next) {
            // Found the index
            Ok(Some((index, _))) => index + 1,
            // Run is not broken, return length
            Ok(None) => slice.len(),
            // Slice is empty
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
            // Run is not broken, return start
            Ok(None) => 0,
            // Slice is empty
            Err(()) => 0,
        }
    }

    /// Indicates if a weakly increasing or strictly decreasing run was found, see
    /// [`weakly_increasing_or_strictly_decreasing_index`].
    #[derive(Debug, Clone, Copy)]
    pub enum RunOrdering {
        /// The run is weakly increasing.
        WeaklyIncreasing,
        /// The run is strictly decreasing.
        StrictlyDecreasing,
    }

    /// Returns the largest index such that `slice[..index]` is either weakly increasing or
    /// strictly decreasing.
    ///
    /// Additionally, returns the [`RunOrdering`].
    pub fn weakly_increasing_or_strictly_decreasing_index<T: Ord>(
        slice: &mut [T],
    ) -> (usize, RunOrdering) {
        // Weakly increasing is the default case
        if slice.len() < 2 {
            return (slice.len(), RunOrdering::WeaklyIncreasing);
        }

        // Split of first element
        let (first, rest) = slice.split_first().unwrap();

        // Look at next and choose either weakly increasing or strictly decreasing search.
        if first > rest.first().unwrap() {
            (
                strictly_decreasing_prefix_index(rest) + 1,
                RunOrdering::StrictlyDecreasing,
            )
        } else {
            (
                weakly_increasing_prefix_index(rest) + 1,
                RunOrdering::WeaklyIncreasing,
            )
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

impl<T> BufGuard<T> for Vec<T> {
    fn with_capacity(capacity: usize) -> Self {
        #[cfg(feature = "counters")]
        #[expect(
            clippy::as_conversions,
            reason = "This will always be accurate (capacity will realistically not be too high)"
        )]
        crate::GLOBAL_COUNTERS.merge_alloc.increase(capacity as u64);

        Vec::with_capacity(capacity)
    }

    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>] {
        self.spare_capacity_mut()
    }
}

/// A thin wrapper around a pointer range, offering some convenience methods.
#[derive(Debug)]
pub struct Run<T>(std::ops::Range<*mut T>);

impl<T> Clone for Run<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Run<std::mem::MaybeUninit<T>> {
    /// Assume all elements in the contained range are initialized.
    pub fn assume_init(self) -> Run<T> {
        Run(self.0.start.cast()..self.0.end.cast())
    }
}

impl<T> Run<T> {
    /// Returns the start pointer.
    pub fn start(&self) -> *mut T {
        self.0.start
    }

    /// Returns the end pointer.
    pub fn end(&self) -> *mut T {
        self.0.end
    }

    /// Returns the length of this pointer range (i.e. the number of elements contained in)
    /// [start, end).
    ///
    /// # Safety
    ///
    /// All safety conditions of `<*mut T>::offset_from_unsigned()` must hold for
    /// [`Self::start()`] and [`Self::end()`].
    pub unsafe fn len(&self) -> usize {
        debug_assert!(self.start() <= self.end());

        // SAFETY: see method doc
        unsafe { self.0.end.offset_from_unsigned(self.0.start) }
    }

    /// Returns whether this pointer range is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Creates a temporary slice view into the pointer range.
    ///
    /// # Safety
    ///
    /// The underlying pointer range must be a valid slice, and no aliasing view can exist.
    ///
    /// See [`std::slice::from_raw_parts_mut()`] for more information and conditions.
    pub unsafe fn as_slice(&mut self) -> &mut [T] {
        // SAFETY: see method doc
        unsafe { std::slice::from_raw_parts_mut(self.start(), self.len()) }
    }

    /// Copies `count` elements from the beginning of this run to the beginning of the other run
    /// and moves both run's starts after the copied elements.
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy_nonoverlapping()`] must hold for
    /// [`self.start()`](Self::start()) and [`other.start()`](Self::start()) and `count`.
    pub unsafe fn copy_nonoverlapping_prefix_to(&mut self, other: &mut Self, count: usize) {
        // SAFETY: see method doc
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            std::ptr::copy_nonoverlapping(self.0.start, other.0.start, count);

            self.0.start = self.0.start.add(count);
            other.0.start = other.0.start.add(count);
        }
    }

    /// Copies `count` elements from the beginning of this run to the beginning of the other run
    /// and moves both run's starts after the copied elements.
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy()`] must hold for
    /// [`self.start()`](Self::start()) and [`other.start()`](Self::start()) and `count`.
    pub unsafe fn copy_prefix_to(&mut self, other: &mut Self, count: usize) {
        // SAFETY: see method doc
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            std::ptr::copy(self.0.start, other.0.start, count);

            self.0.start = self.0.start.add(count);
            other.0.start = other.0.start.add(count);
        }
    }

    /// Copies `count` elements from the end of this run to the end of the other run
    /// and moves both run's ends before the copied elements.
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy_nonoverlapping()`] must hold for
    /// [`self.end().sub(count)`](Self::end()) and [`other.end().sub(count)`](Self::end()) and `count`.
    pub unsafe fn copy_nonoverlapping_suffix_to(&mut self, other: &mut Self, count: usize) {
        // SAFETY: see method doc
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            self.0.end = self.0.end.sub(count);
            other.0.end = other.0.end.sub(count);

            std::ptr::copy_nonoverlapping(self.0.end, other.0.end, count);
        }
    }

    /// Copies `count` elements from the end of this run to the end of the other run
    /// and moves both run's ends before the copied elements.
    ///
    /// # Safety
    ///
    /// All safety conditions of [`std::ptr::copy()`] must hold for
    /// [`self.end().`](Self::end()) and [`other.end()`](Self::end()) and `count`.
    pub unsafe fn copy_suffix_to(&mut self, other: &mut Self, count: usize) {
        // SAFETY: see method doc
        unsafe {
            debug_assert!(self.len() >= count && other.len() >= count);

            self.0.end = self.0.end.sub(count);
            other.0.end = other.0.end.sub(count);

            std::ptr::copy(self.0.end, other.0.end, count);
        }
    }
}

/// A drop guard used to write all remaining elements in `runs` are written to `output` when
/// dropped.
pub struct MergingDropGuard<T, const N: usize> {
    /// The runs which will be written to `output`.
    pub runs: [Run<T>; N],
    /// The output run, into which all elements from `runs` are written.
    pub output: Run<T>,
    /// Prevent construction without [`Self::new()`].
    _sealed: std::marker::PhantomData<()>,
}

impl<T, const N: usize> MergingDropGuard<T, N> {
    /// Construct a new merging drop guard.
    /// When this struct is dropped, all runs in `runs` which are not empty, will be
    /// written into `output`.
    ///
    /// # Safety
    ///
    /// The sum of the length of the remaining `runs` must be smaller or equal to the length of
    /// `output`. The pointer ranges must be valid to be read from and written to respectively.
    /// This invariant must not be invalidated when mutating any of the public fields.
    /// To disarm the guard see [Self::disarm()].
    pub unsafe fn new(runs: [Run<T>; N], output: Run<T>) -> Self {
        Self {
            runs,
            output,
            _sealed: std::marker::PhantomData,
        }
    }

    /// Disarms this guard and returns its components `(runs, output)`.
    ///
    /// This is safe, since we only do work on drop.
    pub fn disarm(self) -> ([Run<T>; N], Run<T>) {
        // SAFETY: we make sure never to drop `self`, and since we consume `self` this is the only
        // access to `self.runs` and `self.output`.
        unsafe {
            // Make sure to never drop self
            let dont_drop = std::mem::ManuallyDrop::new(self);

            // Extract the relevant fields
            let runs = std::ptr::read(&raw const dont_drop.runs);
            let output = std::ptr::read(&raw const dont_drop.output);

            (runs, output)
        }
    }

    /// Returns whether all runs are empty and there is nothing to clean up.
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(Run::is_empty)
    }
}

impl<T, const N: usize> Drop for MergingDropGuard<T, N> {
    fn drop(&mut self) {
        // SAFETY: See condition on [`Self::new()`]
        unsafe {
            // Iterate through all runs and write them consecutively into output
            for run in self.runs.iter_mut() {
                if !run.is_empty() {
                    run.copy_prefix_to(&mut self.output, run.len());
                }
            }
        }
    }
}

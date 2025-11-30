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

        let mut output = buffer.as_mut_ptr();
        let (left, right) = slice.split_at(split_point);
        let std::ops::Range {
            start: mut left_start,
            end: left_end,
        } = left.as_ptr_range();
        let std::ops::Range {
            start: mut right_start,
            end: right_end,
        } = right.as_ptr_range();

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
            while left_start != left_end && right_start != right_end {
                if *left_start <= *right_start {
                    output
                        .copy_from_nonoverlapping(left_start as *const std::mem::MaybeUninit<T>, 1);
                    left_start = left_start.add(1);
                } else {
                    output.copy_from_nonoverlapping(
                        right_start as *const std::mem::MaybeUninit<T>,
                        1,
                    );
                    right_start = right_start.add(1);
                }

                output = output.add(1);
            }

            // Copy the rest of the remaining run into the buffer
            while left_start < left_end {
                output.copy_from_nonoverlapping(left_start as *const std::mem::MaybeUninit<T>, 1);
                left_start = left_start.add(1);
                output = output.add(1);
            }
            while right_start < right_end {
                output.copy_from_nonoverlapping(right_start as *const std::mem::MaybeUninit<T>, 1);
                right_start = right_start.add(1);
                output = output.add(1);
            }
        }

        // SAFETY: Since buffer now contains a permutation of slice, we can safely copy it over to
        // slice, again regarding the same layout invariant for T and MaybeUninit<T>. (see above)
        unsafe {
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr() as *mut T,
                slice.as_mut_ptr(),
                slice.len(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::Rng as _;

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
            pub fn test_empty_merges() {
                test_empty_merge::<$method>();
            }

            #[test]
            pub fn test_correct_merges() {
                test_correct_merge::<$method>();
            }

            #[test]
            pub fn test_correct_stable_merges() {
                if $method::IS_STABLE {
                    test_correct_stable_merge::<$method>();
                }
            }

            #[test]
            pub fn test_soundness_merges() {
                test_soundness_merge::<$method>();
            }
        };
    }

    test_methods!(CopyBoth);

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

        // Test random runs
        for _ in 0..TEST_RUNS {
            // RandomOrdered elements
            let mut elements: Box<[crate::test::RandomOrdered]> =
                crate::test::RandomOrdered::new_iter(crate::test::TEST_SEED)
                    .take(TEST_SIZE)
                    .collect();
            let split = rng.random_range(0..TEST_SIZE);

            T::merge(&mut elements, split, buffer.as_uninit_slice_mut());

            drop(elements);

            // MaybePanickingOrdered elements
            let mut elements: Box<
                [crate::test::MaybePanickingOrdered<TEST_SIZE, crate::test::RandomOrdered>],
            > = crate::test::MaybePanickingOrdered::map_iter(
                crate::test::RandomOrdered::new_iter(crate::test::TEST_SEED).take(TEST_SIZE),
                crate::test::TEST_SEED,
            )
            .collect();
            let split = rng.random_range(0..TEST_SIZE);

            // The types are not actually unwind safe but must not trigger UB anyway
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(
                    &mut elements,
                    split,
                    maybe_panicking_buffer.as_uninit_slice_mut(),
                );
            }));

            drop(elements);
        }
    }
}

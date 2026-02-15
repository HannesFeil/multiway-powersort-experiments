/// Specifies ways to merge two adjacent runs in a slice, given a buffer
pub trait MergingMethod {
    /// Whether the merging method is stable
    const IS_STABLE: bool;

    /// String representation of this merging method
    fn display() -> String;

    /// Merge the two sorted runs `0..run_length` and `run_length..slice.len()`, potentially
    /// using `buffer`.
    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]);

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

    fn display() -> String {
        "copy-both".to_string()
    }

    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.is_empty() {
            return;
        }

        #[cfg(feature = "counters")]
        {
            super::MERGE_SLICE_COUNTER.increase(slice.len() as u64);
            super::MERGE_BUFFER_COUNTER.increase(slice.len() as u64);
        }

        assert!(
            buffer.len() >= slice.len(),
            "Buffer needs to have at least the size of slice"
        );
        assert!(
            (0..slice.len()).contains(&run_length),
            "Split points needs to be in bounds"
        );

        let buffer = &mut buffer[..slice.len()];

        // SAFETY: We make sure to copy each element from left and right into buffer exactly once,
        // so that buffer ends up a permutation (sorted) of slice. Therefor at the end we may
        // assume slice.len() elements in buffer are initialized and may be copied back into slice
        // without duplication.
        unsafe {
            // Copy entire slice into buffer
            std::ptr::copy_nonoverlapping(
                slice.as_ptr(),
                buffer.as_mut_ptr() as *mut T,
                slice.len(),
            );

            let ptr_range = buffer.as_mut_ptr_range();
            let runs = [
                super::Run(ptr_range.start..ptr_range.start.add(run_length)).assume_init(),
                super::Run(ptr_range.start.add(run_length)..ptr_range.end).assume_init(),
            ];
            let output = super::Run(slice.as_mut_ptr_range());

            // SAFETY: all runs are readable valid subslices and output is writable and large
            // enough for all elements in slice.
            let mut guard = super::MergingDropGuard::new(runs, output);

            // Destructure bindings for easier access, these are only references and
            // guard is still responsible for cleaning up.
            let &mut [ref mut left, ref mut right] = &mut guard.runs;

            // Repeatedly copy the smaller element of both runs into the slice
            while !left.is_empty() && !right.is_empty() {
                if *left.start() <= *right.start() {
                    left.copy_nonoverlapping_prefix_to(&mut guard.output, 1);
                } else {
                    right.copy_nonoverlapping_prefix_to(&mut guard.output, 1);
                }
            }

            // Copy the rest of the remaining runs into the slice
            if !left.is_empty() {
                left.copy_nonoverlapping_prefix_to(&mut guard.output, left.len());
            }
            if !right.is_empty() {
                right.copy_nonoverlapping_prefix_to(&mut guard.output, right.len());
            }

            // Disarm drop guard, we should be done anyway
            debug_assert!(guard.is_empty());
            guard.disarm();
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

    fn display() -> String {
        format!("galloping (MIN_GALLOP = {MIN_GALLOP})")
    }

    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() < 2 || run_length == 0 {
            return;
        }

        // TODO: improve this?
        #[cfg(feature = "counters")]
        {
            super::MERGE_SLICE_COUNTER.increase(slice.len() as u64);
            super::MERGE_BUFFER_COUNTER.increase(slice.len() as u64);
        }

        let start = Self::gallop::<T, false>(&slice[run_length], &slice[..run_length], 0);
        if start == run_length {
            return;
        }

        let end = Self::gallop::<T, true>(
            &slice[run_length - 1],
            &slice[run_length..],
            slice.len() - run_length - 1,
        ) + run_length;
        if end == run_length {
            return;
        }

        let mut min_gallop = MIN_GALLOP;

        if run_length - start <= end - run_length {
            Self::merge_low(
                &mut slice[start..end],
                run_length - start,
                buffer,
                &mut min_gallop,
            );
        } else {
            Self::merge_high(
                &mut slice[start..end],
                run_length - start,
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

    // FIXME: better doc
    /// Sort the given `slice` assuming `slice[..run_length]` and `slice[run_length..]` are
    /// already sorted.
    fn merge_low<T: Ord>(
        slice: &mut [T],
        run_length: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        assert!(
            buffer.len() >= run_length,
            "We need at least run_length buffer size"
        );
        assert!(
            (1..slice.len()).contains(&run_length),
            "Split point has to be within slice bounds"
        );

        // Set buffer size
        let buffer = &mut buffer[..run_length];

        // TODO: safety comment
        unsafe {
            // Copy start into temporary buffer
            std::ptr::copy_nonoverlapping(
                slice.as_mut_ptr(),
                buffer.as_mut_ptr() as *mut T,
                run_length,
            );

            let slice_ptrs = slice.as_mut_ptr_range();
            let runs = [
                // Left run in buffer
                super::Run(buffer.as_mut_ptr_range()).assume_init(),
                // Right run at the end of slice
                super::Run(slice_ptrs.start.add(run_length)..slice_ptrs.end),
            ];
            let output = super::Run(slice_ptrs);

            let mut guard = super::MergingDropGuard::new(runs, output);

            let &mut [ref mut left, ref mut right] = &mut guard.runs;
            let output = &mut guard.output;

            (move || {
                right.copy_nonoverlapping_prefix_to(output, 1);

                // Right side only had one element, only need to copy the left side
                if right.is_empty() {
                    left.copy_nonoverlapping_prefix_to(output, left.len());

                    return;
                }

                // Left side only has one element, copy the rest of the right side and then the one
                // element from the left side
                if left.len() == 1 {
                    right.copy_prefix_to(output, right.len());
                    left.copy_nonoverlapping_prefix_to(output, 1);

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
                            right.copy_nonoverlapping_prefix_to(output, 1);
                            count2 += 1;
                            count1 = 0;

                            if right.is_empty() {
                                break 'outer;
                            }
                        } else {
                            // Advance the left side
                            left.copy_nonoverlapping_prefix_to(output, 1);
                            count1 += 1;
                            count2 = 0;

                            if left.len() == 1 {
                                break 'outer;
                            }
                        }
                    }

                    while count1 >= MIN_GALLOP || count2 >= MIN_GALLOP {
                        assert!(left.len() > 1);
                        assert!(!right.is_empty());

                        count1 = Self::gallop::<T, false>(&*right.start(), left.as_slice(), 0);
                        if count1 != 0 {
                            left.copy_nonoverlapping_prefix_to(output, count1);

                            if left.len() <= 1 {
                                break 'outer;
                            }
                        }

                        right.copy_nonoverlapping_prefix_to(output, 1);

                        if right.is_empty() {
                            break 'outer;
                        }

                        count2 = Self::gallop::<T, true>(&*left.start(), right.as_slice(), 0);
                        if count2 != 0 {
                            right.copy_prefix_to(output, count2);

                            if right.is_empty() {
                                break 'outer;
                            }
                        }

                        left.copy_nonoverlapping_prefix_to(output, 1);

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
                    right.copy_prefix_to(output, right.len());
                    left.copy_nonoverlapping_prefix_to(output, 1);
                } else {
                    assert!(!left.is_empty());
                    assert!(right.is_empty());
                    left.copy_nonoverlapping_prefix_to(output, left.len());
                }
            })();

            // Guard should be empty at this point
            debug_assert!(guard.is_empty());
            guard.disarm();
        }
    }

    fn merge_high<T: Ord>(
        slice: &mut [T],
        run_length: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        assert!(
            buffer.len() >= slice.len() - run_length,
            "We need at least slice.len() - run_length buffer size"
        );
        assert!(
            (1..slice.len()).contains(&run_length),
            "Split point has to be within slice bounds"
        );

        // Set buffer size
        let buffer = &mut buffer[..slice.len() - run_length];

        // TODO: safety comment
        unsafe {
            // Copy suffix into temporary buffer
            std::ptr::copy_nonoverlapping(
                slice.as_mut_ptr().add(run_length),
                buffer.as_mut_ptr() as *mut T,
                slice.len() - run_length,
            );

            let slice_ptrs = slice.as_mut_ptr_range();
            let runs = [
                // Left run in buffer
                super::Run(slice_ptrs.start..slice_ptrs.start.add(run_length)),
                // Right run at the end of slice
                super::Run(buffer.as_mut_ptr_range()).assume_init(),
            ];
            let output = super::Run(slice_ptrs);

            let mut guard = super::MergingDropGuard::new(runs, output);

            let &mut [ref mut left, ref mut right] = &mut guard.runs;
            let output = &mut guard.output;

            (|| {
                left.copy_nonoverlapping_suffix_to(output, 1);

                // Left side only had one element, only need to copy the left side
                if left.is_empty() {
                    right.copy_nonoverlapping_suffix_to(output, right.len());

                    return;
                }

                // right side only has one element, copy the rest of the left side and then the one
                // element from the right side
                if right.len() == 1 {
                    left.copy_suffix_to(output, left.len());
                    right.copy_nonoverlapping_suffix_to(output, 1);

                    return;
                }

                'outer: loop {
                    let mut count1 = 0;
                    let mut count2 = 0;

                    while (count1 | count2) < *min_gallop {
                        assert!(right.len() > 1);
                        assert!(!left.is_empty());

                        if *right.end().sub(1) < *left.end().sub(1) {
                            // Advance the left side
                            left.copy_nonoverlapping_suffix_to(output, 1);
                            count1 += 1;
                            count2 = 0;

                            if left.is_empty() {
                                break 'outer;
                            }
                        } else {
                            // Advance the right side
                            right.copy_nonoverlapping_suffix_to(output, 1);
                            count1 = 0;
                            count2 += 1;

                            if right.len() == 1 {
                                break 'outer;
                            }
                        }
                    }

                    while count1 >= MIN_GALLOP || count2 >= MIN_GALLOP {
                        assert!(right.len() > 1);
                        assert!(!left.is_empty());

                        let left_len = left.len();
                        count1 = left.len()
                            - Self::gallop::<T, false>(
                                &*right.end().sub(1),
                                left.as_slice(),
                                left_len - 1,
                            );
                        if count1 != 0 {
                            left.copy_suffix_to(output, count1);

                            if left.is_empty() {
                                break 'outer;
                            }
                        }

                        right.copy_nonoverlapping_suffix_to(output, 1);

                        if right.len() == 1 {
                            break 'outer;
                        }

                        let right_len = right.len();
                        count2 = right.len()
                            - Self::gallop::<T, true>(
                                &*left.end().sub(1),
                                right.as_slice(),
                                right_len - 1,
                            );
                        if count2 != 0 {
                            right.copy_nonoverlapping_suffix_to(output, count2);

                            if right.len() <= 1 {
                                break 'outer;
                            }
                        }

                        left.copy_nonoverlapping_suffix_to(output, 1);

                        if left.is_empty() {
                            break 'outer;
                        }

                        *min_gallop = min_gallop.saturating_sub(1);
                    }

                    *min_gallop += 2;
                }

                *min_gallop = std::cmp::max(*min_gallop, 1);

                if right.len() == 1 {
                    assert!(!left.is_empty());
                    left.copy_suffix_to(output, left.len());
                    right.copy_nonoverlapping_suffix_to(output, 1);
                } else {
                    assert!(!right.is_empty());
                    assert!(left.is_empty());
                    right.copy_nonoverlapping_suffix_to(output, right.len());
                }
            })();

            debug_assert!(guard.is_empty());
            guard.disarm();
        }
    }
}

// TODO: refactor pls
#[cfg(test)]
mod tests {
    use super::super::BufGuard;
    use super::*;

    use rand::{Rng as _, RngCore as _};

    /// How big the test arrays should be
    const TEST_SIZE: usize = 1000;
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

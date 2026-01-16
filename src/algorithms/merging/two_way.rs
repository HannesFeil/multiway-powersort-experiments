/// Specifies ways to merge two adjacent runs in a slice, given a buffer
pub trait MergingMethod {
    /// Whether the merging method is stable
    const IS_STABLE: bool;

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

    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.is_empty() {
            return;
        }

        assert!(
            buffer.len() >= slice.len(),
            "Buffer needs to have at least the size of slice"
        );
        assert!(
            (0..slice.len()).contains(&run_length),
            "Split points needs to be in bounds"
        );

        // SAFETY: We make sure to copy each element from left and right into buffer exactly once,
        // so that buffer ends up a permutation (sorted) of slice. Therefor at the end we may
        // assume slice.len() elements in buffer are initialized and may be copied back into slice
        // without duplication.
        unsafe {
            let output = &mut &mut buffer[..slice.len()];
            let (ref mut left, ref mut right) = slice.split_at(run_length);

            // Repeatedly copy the smaller element of both runs into the buffer
            while !left.is_empty() && !right.is_empty() {
                if left.first().unwrap() <= right.first().unwrap() {
                    super::slice::copy_prefix_to_uninit(left, output, 1);
                } else {
                    super::slice::copy_prefix_to_uninit(right, output, 1);
                }
            }

            // Copy the rest of the remaining run into the buffer
            if !left.is_empty() {
                super::slice::copy_prefix_to_uninit(left, output, left.len());
            }
            if !right.is_empty() {
                super::slice::copy_prefix_to_uninit(right, output, right.len());
            }

            // NOTE: We copy after the merging as opposed to before, to prevent inconsistent
            // state which could occur when panicking on merging into slice
            //
            // See the safety comment, since buffer is a permutation we can assume init and copy
            // back
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

    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() < 2 || run_length == 0 {
            return;
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
            buffer.len() >= slice.len(),
            "We need at least slice.len() buffer size"
        );
        assert!(
            (0..slice.len()).contains(&run_length),
            "Split point has to be within slice bounds"
        );

        // Wrapping in closure for early return (ugly?)
        // FIXME: expand unsafe block
        (|| {
            // TODO: unchecked this?
            let output = &mut &mut buffer[..slice.len()];
            let (ref mut left, ref mut right) = slice.split_at_mut(run_length);

            super::slice::copy_mut_prefix_to_uninit(right, output, 1);

            // Right side only had one element, only need to copy the left side
            if right.is_empty() {
                super::slice::copy_mut_prefix_to_uninit(left, output, left.len());

                return;
            }

            // Left side only has one element, copy the rest of the right side and then the one
            // element from the left side
            if left.len() == 1 {
                super::slice::copy_mut_prefix_to_uninit(right, output, right.len());
                super::slice::copy_mut_prefix_to_uninit(left, output, 1);

                return;
            }

            'outer: loop {
                let mut count1 = 0;
                let mut count2 = 0;

                while (count1 | count2) < *min_gallop {
                    assert!(left.len() > 1);
                    assert!(!right.is_empty());

                    if *right.first().unwrap() < *left.first().unwrap() {
                        // Advance the right side
                        super::slice::copy_mut_prefix_to_uninit(right, output, 1);
                        count2 += 1;
                        count1 = 0;

                        if right.is_empty() {
                            break 'outer;
                        }
                    } else {
                        // Advance the left side
                        super::slice::copy_mut_prefix_to_uninit(left, output, 1);
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

                    count1 = Self::gallop::<T, false>(right.first().unwrap(), left, 0);
                    if count1 != 0 {
                        super::slice::copy_mut_prefix_to_uninit(left, output, count1);

                        if left.len() <= 1 {
                            break 'outer;
                        }
                    }

                    super::slice::copy_mut_prefix_to_uninit(right, output, 1);

                    if right.is_empty() {
                        break 'outer;
                    }

                    count2 = Self::gallop::<T, true>(left.first().unwrap(), right, 0);
                    if count2 != 0 {
                        super::slice::copy_mut_prefix_to_uninit(right, output, count2);

                        if right.is_empty() {
                            break 'outer;
                        }
                    }

                    super::slice::copy_mut_prefix_to_uninit(left, output, 1);

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
                super::slice::copy_mut_prefix_to_uninit(right, output, right.len());
                super::slice::copy_mut_prefix_to_uninit(left, output, 1);
            } else {
                assert!(!left.is_empty());
                assert!(right.is_empty());
                super::slice::copy_mut_prefix_to_uninit(left, output, left.len());
            }
        })();
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
        run_length: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        assert!(
            buffer.len() >= slice.len(),
            "We need at least slice.len() buffer size"
        );
        assert!(
            (0..slice.len()).contains(&run_length),
            "Split point has to be within slice bounds"
        );

        (|| {
            // TODO: unchecked this?
            let output = &mut &mut buffer[..slice.len()];
            let (ref mut left, ref mut right) = slice.split_at_mut(run_length);

            super::slice::copy_mut_suffix_to_uninit(left, output, 1);

            // Left side only had one element, only need to copy the left side
            if left.is_empty() {
                super::slice::copy_mut_suffix_to_uninit(right, output, right.len());

                return;
            }

            // right side only has one element, copy the rest of the left side and then the one
            // element from the right side
            if right.len() == 1 {
                super::slice::copy_mut_suffix_to_uninit(left, output, left.len());
                super::slice::copy_mut_suffix_to_uninit(right, output, 1);

                return;
            }

            'outer: loop {
                let mut count1 = 0;
                let mut count2 = 0;

                while (count1 | count2) < *min_gallop {
                    assert!(right.len() > 1);
                    assert!(!left.is_empty());

                    if *right.last().unwrap() < *left.last().unwrap() {
                        // Advance the left side
                        super::slice::copy_mut_suffix_to_uninit(left, output, 1);
                        count1 += 1;
                        count2 = 0;

                        if left.is_empty() {
                            break 'outer;
                        }
                    } else {
                        // Advance the right side
                        super::slice::copy_mut_suffix_to_uninit(right, output, 1);
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

                    count1 = left.len()
                        - Self::gallop::<T, false>(right.last().unwrap(), left, left.len() - 1);
                    if count1 != 0 {
                        super::slice::copy_mut_suffix_to_uninit(left, output, count1);

                        if left.is_empty() {
                            break 'outer;
                        }
                    }

                    super::slice::copy_mut_suffix_to_uninit(right, output, 1);

                    if right.len() == 1 {
                        break 'outer;
                    }

                    count2 = right.len()
                        - Self::gallop::<T, true>(left.last().unwrap(), right, right.len() - 1);
                    if count2 != 0 {
                        super::slice::copy_mut_suffix_to_uninit(right, output, count2);

                        if right.len() <= 1 {
                            break 'outer;
                        }
                    }

                    super::slice::copy_mut_suffix_to_uninit(left, output, 1);

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
                super::slice::copy_mut_suffix_to_uninit(left, output, left.len());
                super::slice::copy_mut_suffix_to_uninit(right, output, 1);
            } else {
                assert!(!right.is_empty());
                assert!(left.is_empty());
                super::slice::copy_mut_suffix_to_uninit(right, output, right.len());
            }
        })();

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
    const TEST_RUNS: usize = 1000;

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

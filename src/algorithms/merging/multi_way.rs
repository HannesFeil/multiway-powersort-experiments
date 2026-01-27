// TODO: sentinel check (move right and continue?)

/// Specifies ways to merge tup to `K` adjacent runs in a slice, given a buffer
pub trait MultiMergingMethod<const K: usize> {
    /// Whether the merging method is stable
    const IS_STABLE: bool;

    /// String representation of this merging method
    fn display() -> String;

    /// Merge the up to `K` sorted runs `0..run_lengths[0]`, `run_lengths[0]..run_lengths[1]`
    /// and so forth, using `buffer`.
    ///
    /// It should hold that `run_lengths.len() <= K`.
    fn merge<T: Ord>(
        slice: &mut [T],
        run_lengths: &[usize],
        buffer: &mut [std::mem::MaybeUninit<T>],
    );

    /// The required capacity of the buffer, needed for merging slices with length less than
    /// or equal to `size`.
    fn required_capacity(size: usize) -> usize {
        size
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DynamicTournamentTree;

impl<const K: usize> MultiMergingMethod<K> for DynamicTournamentTree {
    const IS_STABLE: bool = true;

    fn display() -> String {
        "dynamic-tournament-tree".to_string()
    }

    fn merge<T: Ord>(
        slice: &mut [T],
        run_lengths: &[usize],
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
        let run_slice = &mut &*slice;
        let mut run_slices = Vec::with_capacity(run_lengths.len() + 1);
        for len in run_lengths {
            let next_run = run_slice
                .split_off(..*len)
                .expect("Sum of run_lengths should not be larger than slice.len()");
            run_slices.push(next_run);
        }
        run_slices.push(*run_slice);
        let mut runs: Box<_> = run_slices.iter_mut().collect();
        let output = &mut &mut buffer[..slice.len()];

        // let nodes = Vec::with_capacity(runs.len() - 1);

        unimplemented!("Actually implement a dynamic tournament tree ...");

        // FIXME aaaahhhh

        // TODO: safety comment
        // unsafe {
        //     std::ptr::copy_nonoverlapping(
        //         buffer.as_ptr() as *const T,
        //         slice.as_mut_ptr(),
        //         slice.len(),
        //     );
        // }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MergeRunsIndices4;

impl MultiMergingMethod<4> for MergeRunsIndices4 {
    const IS_STABLE: bool = true;

    fn display() -> String {
        "merge-runs-indices-4".to_string()
    }

    fn merge<T: Ord>(
        slice: &mut [T],
        run_lengths: &[usize],
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
        #[cfg(feature = "counters")]
        {
            super::MERGE_SLICE_COUNTER.increase(slice.len() as u64);
            super::MERGE_BUFFER_COUNTER.increase(slice.len() as u64);
        }

        let run_slice = &mut &*slice;
        let mut run_slices: [&[T]; 4] = [&[]; 4];
        let mut index = 0;
        for len in run_lengths {
            let next_run = run_slice
                .split_off(..*len)
                .expect("Sum of run_lengths should not be larger than slice.len()");
            run_slices[index] = next_run;
            index += 1;
        }
        run_slices[index] = run_slice;
        for slice in run_slices {
            assert!(slice.is_sorted());
        }
        let runs: Box<_> = run_slices.iter_mut().collect();
        let output = &mut &mut *buffer;
        let mut x =
            if runs[0].first().map(std::cmp::Reverse) >= runs[1].first().map(std::cmp::Reverse) {
                0
            } else {
                1
            };
        let mut y =
            if runs[2].first().map(std::cmp::Reverse) >= runs[3].first().map(std::cmp::Reverse) {
                2
            } else {
                3
            };
        let mut z =
            if runs[x].first().map(std::cmp::Reverse) >= runs[y].first().map(std::cmp::Reverse) {
                x
            } else {
                y
            };
        for _ in 0..slice.len() {
            super::slice::copy_prefix_to_uninit(runs[z], output, 1);
            if z <= 1 {
                x = if runs[0].first().map(std::cmp::Reverse)
                    >= runs[1].first().map(std::cmp::Reverse)
                {
                    0
                } else {
                    1
                };
            } else {
                y = if runs[2].first().map(std::cmp::Reverse)
                    >= runs[3].first().map(std::cmp::Reverse)
                {
                    2
                } else {
                    3
                };
            }
            z = if runs[x].first().map(std::cmp::Reverse) >= runs[y].first().map(std::cmp::Reverse)
            {
                x
            } else {
                y
            };
        }

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

    macro_rules! test_multi_methods {
        ($($method:ident: [$($k:expr),+]),+$(,)?) => {
            $(
                paste::paste! {
                    mod [< $method:snake >] {
                        use super::*;

                        test_multi_methods!(@single $method [$($k),*]);
                    }
                }
            )*
        };
        (@single $method:ident [$($k:expr),*]) => {
            #[test]
            fn test_empty_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    test_empty_merge::<$method, K>();
                });
            }

            #[test]
            fn test_correct_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    test_correct_merge::<$method, K>();
                });
            }

            #[test]
            fn test_correct_stable_merges() {
                    test_multi_methods!(@all_k [$($k),*] => K => {
                    if <$method as MultiMergingMethod<K>>::IS_STABLE {
                        test_correct_stable_merge::<$method, K>();
                    }
                });
            }

            #[test]
            fn test_soundness_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    test_soundness_merge::<$method, K>();
                });
            }
        };
        (@all_k [$($value:expr),*] => $k:ident => $code:block) => {
            $(
                {
                    const $k: usize = $value;

                    $code
                }
            );*
        };
    }

    test_multi_methods! {
        DynamicTournamentTree: [2, 3, 4, 5, 6, 7, 8],
        MergeRunsIndices4: [4],
    }

    /// Test merging an empty slice
    fn test_empty_merge<T: MultiMergingMethod<K>, const K: usize>() {
        let mut elements = [(); 0];
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));

        // This should not panic nor cause UB
        T::merge(&mut elements, &[], buffer.as_uninit_slice_mut())
    }

    /// Test that two runs are correctly merged
    fn test_correct_merge<T: MultiMergingMethod<K>, const K: usize>() {
        let mut rng = crate::test::test_rng();
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut splits = Vec::with_capacity(K - 1);

        // Test random runs
        for run in 0..TEST_RUNS {
            let mut elements: Box<[usize]> = (0..TEST_SIZE)
                .map(|_| rng.random_range(0..usize::MAX))
                .collect();

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                elements[last..last + split].sort();
                splits.push(split);
                last += split;
            }
            elements[last..].sort();

            T::merge(&mut elements, &splits, buffer.as_uninit_slice_mut());

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

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                elements[last..last + split].sort();
                splits.push(split);
                last += split;
            }
            elements[last..].sort();

            T::merge(&mut elements, &splits, buffer.as_uninit_slice_mut());

            assert!(
                elements.is_sorted(),
                "Resulting elements were not sorted by {name} with split {split}",
                name = std::any::type_name::<T>(),
            );
        }
    }

    /// Test that two runs are correctly merged and the ordering of equal elements remains stable
    fn test_correct_stable_merge<T: MultiMergingMethod<K>, const K: usize>() {
        let mut rng = crate::test::test_rng();
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut splits = Vec::with_capacity(K - 1);

        // Test random runs
        for run in 0..TEST_RUNS {
            let mut elements: Box<[_]> = crate::test::IndexedOrdered::map_iter(
                (0..TEST_SIZE).map(|_| rng.random_range(0..TEST_SIZE / 4)),
            )
            .collect();

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                elements[last..last + split].sort();
                splits.push(split);
                last += split;
            }
            elements[last..].sort();

            T::merge(&mut elements, &splits, buffer.as_uninit_slice_mut());

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

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                elements[last..last + split].sort();
                splits.push(split);
                last += split;
            }
            elements[last..].sort();

            T::merge(&mut elements, &splits, buffer.as_uninit_slice_mut());

            assert!(
                crate::test::IndexedOrdered::is_stable_sorted(&elements),
                "Resulting elements were not sorted by {name} with split {split}\n{elements:?}",
                name = std::any::type_name::<T>(),
            );
        }
    }

    /// Run Merging methods with [`crate::test::RandomOrdered`] elements and
    /// [`crate::test::MaybePanickingOrdered`] elements, mostly useful for running under miri
    fn test_soundness_merge<T: MultiMergingMethod<K>, const K: usize>() {
        let mut rng = crate::test::test_rng();
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut maybe_panicking_buffer =
            <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut maybe_panicking_random_buffer =
            <Vec<_> as BufGuard<_>>::with_capacity(T::required_capacity(TEST_SIZE));
        let mut splits = Vec::with_capacity(K - 1);

        // Test random runs
        for _ in 0..TEST_RUNS {
            // RandomOrdered elements
            let mut elements: Box<[crate::test::RandomOrdered]> =
                crate::test::RandomOrdered::new_iter(rng.next_u64())
                    .take(TEST_SIZE)
                    .collect();

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                splits.push(split);
                last += split;
            }

            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(&mut elements, &splits, buffer.as_uninit_slice_mut());
            }));

            drop(elements);

            // MaybePanickingOrdered elements
            let mut elements: Box<[u32]> = std::iter::repeat_with(|| rng.random())
                .take(TEST_SIZE)
                .collect();

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                elements[last..last + split].sort();
                splits.push(split);
                last += split;
            }
            elements[last..].sort();

            let mut elements: Box<[crate::test::MaybePanickingOrdered<TEST_SIZE, u32>]> =
                crate::test::MaybePanickingOrdered::map_iter(elements.into_iter(), rng.next_u64())
                    .collect();

            // The types are not actually unwind safe but must not trigger UB anyway
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(
                    &mut elements,
                    &splits,
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

            splits.clear();
            let num_splits = rng.random_range(1..K - 1);
            let mut last = 0;
            for i in 0..num_splits {
                let split = rng.random_range(1..TEST_SIZE - num_splits + i - last);
                splits.push(split);
                last += split;
            }

            // The types are not actually unwind safe but must not trigger UB anyway
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                T::merge(
                    &mut elements,
                    &splits,
                    maybe_panicking_random_buffer.as_uninit_slice_mut(),
                );
            }));

            drop(elements);
        }
    }
}

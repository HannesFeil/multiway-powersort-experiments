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
pub struct TournamentTree;

impl<const K: usize> MultiMergingMethod<K> for TournamentTree {
    const IS_STABLE: bool = true;

    fn display() -> String {
        format!("tournament-tree-{K}")
    }

    fn merge<T: Ord>(
        slice: &mut [T],
        run_lengths: &[usize],
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
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
            (run_lengths).iter().sum::<usize>() <= slice.len(),
            "Split points need to be in bounds"
        );

        let buffer = &mut buffer[..slice.len()];

        // TODO: safety comment
        unsafe {
            // Copy entire slice into buffer
            std::ptr::copy_nonoverlapping(
                slice.as_ptr(),
                buffer.as_mut_ptr() as *mut T,
                slice.len(),
            );

            let ptr_range = buffer.as_mut_ptr_range();
            let mut run_end = ptr_range.start;
            let runs: [_; K] = std::array::from_fn(|i| {
                let run_start = run_end;
                run_end = run_lengths
                    .get(i)
                    .map(|len| run_start.add(*len))
                    .unwrap_or(ptr_range.end);

                super::Run(run_start..run_end).assume_init()
            });
            let output = super::Run(slice.as_mut_ptr_range());

            // SAFETY: all runs are readable valid subslices and output is writable and large
            // enough for all elements in slice.
            let mut guard = super::MergingDropGuard::new(runs, output);

            let runs = &mut guard.runs;
            let output = &mut guard.output;

            Self::tournament_tree_merge(runs, output, slice.len());

            debug_assert!(guard.is_empty());
            guard.disarm();
        }
    }
}

impl TournamentTree {
    unsafe fn tournament_tree_merge<T: Ord, const K: usize>(
        runs: &mut [super::Run<T>; K],
        output: &mut super::Run<T>,
        n: usize,
    ) {
        let compare = |a: &(usize, Option<*mut T>), b: &(usize, Option<*mut T>)| unsafe {
            match (a.1, b.1) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a_first), Some(b_first)) => (*a_first).cmp(&*b_first).then(a.0.cmp(&b.0)),
            }
        };

        // Workaround for const generics
        let mut nodes = [[(0, None); 2]; K];
        let nodes = nodes.as_flattened_mut();

        for (index, run) in runs.iter().enumerate() {
            let projected_index = index + K - 1;

            if !run.is_empty() {
                nodes[projected_index] = (index, Some(run.start()))
            }
        }

        for index in (0..K - 1).rev() {
            let left_child = index * 2 + 1;
            let right_child = index * 2 + 2;

            let min = std::cmp::min_by(nodes[left_child], nodes[right_child], compare);
            nodes[index] = min;
        }

        unsafe {
            for _ in 0..n {
                let run_index = nodes[0].0;
                let run = &mut runs[run_index];
                run.copy_nonoverlapping_prefix_to(output, 1);

                let node = (run_index, (!run.is_empty()).then_some(run.start()));
                let mut node_index = run_index + K - 1;
                nodes[node_index] = node;

                while node_index != 0 {
                    node_index = (node_index - 1) / 2;

                    let left_child = node_index * 2 + 1;
                    let right_child = node_index * 2 + 2;

                    let min = std::cmp::min_by(nodes[left_child], nodes[right_child], compare);

                    nodes[node_index] = min;
                }
            }
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
        TournamentTree: [2, 3, 4, 5, 6, 7, 8],
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
            let num_splits = rng.random_range(1..K);
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
            let num_splits = rng.random_range(1..K);
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
            let num_splits = rng.random_range(1..K);
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
            let num_splits = rng.random_range(1..K);
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
            let num_splits = rng.random_range(1..K);
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
            let num_splits = rng.random_range(1..K);
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
            let num_splits = rng.random_range(1..K);
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

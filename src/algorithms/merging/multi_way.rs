// TODO: sentinel check (move right and continue?)

/// Specifies ways to merge up to `K` adjacent runs in a slice, given a buffer
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
        #[expect(
            clippy::as_conversions,
            reason = "slice.len() will realistically stay way below u64::MAX, so this is lossless"
        )]
        {
            crate::GLOBAL_COUNTERS
                .merge_slice
                .increase(slice.len() as u64);
            crate::GLOBAL_COUNTERS
                .merge_buffer
                .increase(slice.len() as u64);
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

            // SAFETY: all runs are readable valid sub-slices and output is writable and large
            // enough for all elements in slice.
            let mut guard = super::MergingDropGuard::new(runs, output);

            let runs = &mut guard.runs;
            let output = &mut guard.output;

            Self::tournament_tree_merge(runs, output);

            debug_assert!(guard.is_empty());
            guard.disarm();
        }
    }
}

impl TournamentTree {
    unsafe fn tournament_tree_merge<T: Ord, const K: usize>(
        runs: &mut [super::Run<T>; K],
        output: &mut super::Run<T>,
    ) {
        unsafe fn min_run<T: Ord, const K: usize>(
            index_a: usize,
            index_b: usize,
            runs: &[super::Run<T>; K],
        ) -> usize {
            unsafe {
                if runs[index_b].is_empty()
                    || (!runs[index_a].is_empty()
                        && (&*runs[index_a].start(), index_a) <= (&*runs[index_b].start(), index_b))
                {
                    index_a
                } else {
                    index_b
                }
            }
        }

        // Workaround for const generics
        let mut nodes = [[0; 2]; K];
        let nodes = nodes.as_flattened_mut();

        for index in 0..runs.len() {
            let projected_index = index + K - 1;

            nodes[projected_index] = index;
        }

        unsafe {
            for index in (0..K - 1).rev() {
                let left_child = index * 2 + 1;
                let right_child = index * 2 + 2;

                let min = min_run(nodes[left_child], nodes[right_child], runs);
                nodes[index] = min;
            }

            for _ in 0..output.len() {
                let run_index = nodes[0];
                runs[run_index].copy_nonoverlapping_prefix_to(output, 1);

                let mut node_index = run_index + K - 1;

                while node_index != 0 {
                    node_index = (node_index - 1) / 2;

                    let left_child = node_index * 2 + 1;
                    let right_child = node_index * 2 + 2;

                    let min = min_run(nodes[left_child], nodes[right_child], runs);

                    nodes[node_index] = min;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Fourway;

impl MultiMergingMethod<4> for Fourway {
    const IS_STABLE: bool = true;

    fn display() -> String {
        "fourway".to_string()
    }

    fn merge<T: Ord>(
        slice: &mut [T],
        run_lengths: &[usize],
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
        if slice.is_empty() {
            return;
        }

        #[expect(
            clippy::as_conversions,
            reason = "slice.len() will realistically stay way below u64::MAX, so this is lossless"
        )]
        {
            crate::GLOBAL_COUNTERS
                .merge_slice
                .increase(slice.len() as u64);
            crate::GLOBAL_COUNTERS
                .merge_buffer
                .increase(slice.len() as u64);
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
            let runs: [_; 4] = std::array::from_fn(|i| {
                let run_start = run_end;
                run_end = run_lengths
                    .get(i)
                    .map(|len| run_start.add(*len))
                    .unwrap_or(ptr_range.end);

                super::Run(run_start..run_end).assume_init()
            });
            let output = super::Run(slice.as_mut_ptr_range());

            // SAFETY: all runs are readable valid sub-slices and output is writable and large
            // enough for all elements in slice.
            let mut guard = super::MergingDropGuard::new(runs, output);

            let runs = &mut guard.runs;
            let output = &mut guard.output;

            Self::merge(runs, output);

            debug_assert!(guard.is_empty());
            guard.disarm();
        }
    }
}

impl Fourway {
    unsafe fn merge<T: Ord>(runs: &mut [super::Run<T>; 4], output: &mut super::Run<T>) {
        unsafe fn min_run<T: Ord>(
            index_a: usize,
            index_b: usize,
            runs: &[super::Run<T>; 4],
        ) -> usize {
            unsafe {
                if runs[index_b].is_empty()
                    || (!runs[index_a].is_empty()
                        && *runs[index_a].start() <= *runs[index_b].start())
                {
                    index_a
                } else {
                    index_b
                }
            }
        }

        unsafe {
            let mut left = min_run(0, 1, runs);
            let mut right = min_run(2, 3, runs);
            let mut root = min_run(left, right, runs);

            for _ in 0..output.len() {
                runs[root].copy_nonoverlapping_prefix_to(output, 1);

                if root < 2 {
                    left = min_run(0, 1, runs);
                } else {
                    right = min_run(2, 3, runs);
                }
                root = min_run(left, right, runs);
            }
        }
    }
}

impl<M: super::two_way::MergingMethod> MultiMergingMethod<2> for M {
    const IS_STABLE: bool = M::IS_STABLE;

    fn display() -> String {
        M::display()
    }

    fn merge<T: Ord>(
        slice: &mut [T],
        run_lengths: &[usize],
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
        if run_lengths.is_empty() {
            return;
        }

        M::merge(slice, run_lengths[0], buffer);
    }
}

// TODO: refactor please
#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_multi_methods {
        ($($module_name:ident : $method:ident [$($k:expr),+]),+$(,)?) => {
            $(
                mod $module_name {
                    use super::*;

                    test_multi_methods!(@single $method [$($k),*]);
                }
            )*
        };
        (@single $method:ident [$($k:expr),*]) => {
            #[test]
            fn test_empty_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    crate::test::merging::test_empty_merge::<$method, K>();
                });
            }

            #[test]
            fn test_correct_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    crate::test::merging::test_correct_merge::<$method, K>();
                });
            }

            #[test]
            fn test_correct_stable_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    crate::test::merging::test_correct_stable_merge::<$method, K>();
                });
            }

            #[test]
            fn test_soundness_merges() {
                test_multi_methods!(@all_k [$($k),*] => K => {
                    crate::test::merging::test_soundness_merge::<$method, K>();
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
        tournament_tree: TournamentTree [2, 3, 4, 5, 6, 7, 8],
        fourway: Fourway [4],
    }
}

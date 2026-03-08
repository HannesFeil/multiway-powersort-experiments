//! Defines methods to merge multiple adjacent runs in a slice, see [`MultiMergingMethod`].

/// Specifies ways to merge up to `K` adjacent runs in a slice, given a buffer.
pub trait MultiMergingMethod<const K: usize> {
    /// Whether the merging method is stable.
    const IS_STABLE: bool;

    /// Returns the string representation of this merging method.
    fn display() -> String;

    /// Merge the up to `K` sorted runs `slice[0..run_lengths[0]]`,
    /// `slice[run_lengths[0]..run_lengths[1]]`, ... using `buffer`.
    ///
    /// It must hold that `run_lengths.len() <= K`.
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

/// Merges multiple runs using a tournament tree.
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
            "Run length sum must be smaller or equal to slice.len()"
        );

        let buffer = &mut buffer[..slice.len()];

        // SAFETY: We copy each element into buffer and back exactly once, such that slice ends up
        // permuted. Since we have exclusive access to slice and buffer, the constructed pointer
        // ranges are safe to read from and write to.
        unsafe {
            // Copy entire slice into buffer
            std::ptr::copy_nonoverlapping(slice.as_ptr(), buffer.as_mut_ptr().cast(), slice.len());

            let ptr_range = buffer.as_mut_ptr_range();
            let mut run_end = ptr_range.start;

            // Construct the runs from run_lengths
            let runs: [_; K] = std::array::from_fn(|i| {
                let run_start = run_end;
                run_end = run_lengths
                    .get(i)
                    .map(|len| run_start.add(*len))
                    .unwrap_or(ptr_range.end);

                // Assume init, since we just copied the elements into buffer
                super::Run(run_start..run_end).assume_init()
            });

            // We write back output into slice
            let output = super::Run(slice.as_mut_ptr_range());

            // We know all runs and output are valid by construction.
            // This guard ensures all elements end up copied back, even if a comparison panics.
            let mut guard = super::MergingDropGuard::new(runs, output);

            // References for easier access, guard is still responsible for cleaning up
            let runs = &mut guard.runs;
            let output = &mut guard.output;

            // Perform the actual merge
            Self::tournament_tree_merge(runs, output);

            debug_assert!(guard.is_empty());

            // At this point we are done, so this guard is unnecessary
            guard.disarm();
        }
    }
}

impl TournamentTree {
    /// Merges `runs` into `output` using a tournament tree.
    ///
    /// # Safety
    ///
    /// `runs` have to be valid to read and `output` has to be valid to write to.
    /// The sum of run lengths has to be equal to the length of output.
    /// Additionally, no memory regions of `runs` must overlap with `output`.
    unsafe fn tournament_tree_merge<T: Ord, const K: usize>(
        runs: &mut [super::Run<T>; K],
        output: &mut super::Run<T>,
    ) {
        /// Returns the index of the run with the smaller first element.
        ///
        /// Guaranteed to always return the index of an inhabited run unless both are empty.
        ///
        /// # Safety
        /// each run in `runs` has to be valid to read from.
        unsafe fn min_run<T: Ord, const K: usize>(
            index_a: usize,
            index_b: usize,
            runs: &[super::Run<T>; K],
        ) -> usize {
            // SAFETY: see method doc.
            unsafe {
                // NOTE: We could construct a perfect binary tree instead but that would also have
                // some overhead cost...
                //
                // We use the index as a second parameter of comparison to ensure stability.
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

        // Workaround for const generics, since we need <= 2 * K nodes
        let mut nodes = [[0; 2]; K];
        let nodes = nodes.as_flattened_mut();

        // SAFETY: We know each run in `runs` is valid to read from and `output` is valid to write
        // to (see method doc.). `min_run()` always returns an occupied run if it exists, and for
        // all `output.len()` elements there exists at least one of these runs.
        unsafe {
            // Fill in the run nodes (leaves)
            for index in 0..runs.len() {
                let projected_index = index + K - 1;

                nodes[projected_index] = index;
            }

            // Populate the tournament tree
            for index in (0..K - 1).rev() {
                let left_child = index * 2 + 1;
                let right_child = index * 2 + 2;

                let min = min_run(nodes[left_child], nodes[right_child], runs);
                nodes[index] = min;
            }

            // Copy all elements into output
            for _ in 0..output.len() {
                // Copy the current minimum
                let run_index = nodes[0];
                runs[run_index].copy_nonoverlapping_prefix_to(output, 1);

                let mut node_index = run_index + K - 1;

                // Update tournament tree
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

/// A four-way tournament tree implementation.
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
            "Run length sum must be smaller or equal to slice.len()"
        );

        let buffer = &mut buffer[..slice.len()];

        // SAFETY: We copy each element into buffer and back exactly once, such that slice ends up
        // permuted. Since we have exclusive access to slice and buffer, the constructed pointer
        // ranges are safe to read from and write to.
        unsafe {
            // Copy entire slice into buffer
            std::ptr::copy_nonoverlapping(slice.as_ptr(), buffer.as_mut_ptr().cast(), slice.len());

            // Construct the runs from run_lengths
            let ptr_range = buffer.as_mut_ptr_range();
            let mut run_end = ptr_range.start;

            let runs: [_; 4] = std::array::from_fn(|i| {
                let run_start = run_end;
                run_end = run_lengths
                    .get(i)
                    .map(|len| run_start.add(*len))
                    .unwrap_or(ptr_range.end);

                // Assume init, since we just copied the elements over
                super::Run(run_start..run_end).assume_init()
            });

            // Construct the `output` pointer range
            let output = super::Run(slice.as_mut_ptr_range());

            // We know all runs and output are valid by construction.
            // This guard ensures all elements end up copied back, even if a comparison panics.
            let mut guard = super::MergingDropGuard::new(runs, output);

            // References for easier access, guard is still responsible for cleaning up
            let runs = &mut guard.runs;
            let output = &mut guard.output;

            // Perform the actual merge
            Self::merge(runs, output);

            debug_assert!(guard.is_empty());

            // We are done, so this guard is no longer required
            guard.disarm();
        }
    }
}

impl Fourway {
    /// Merges `runs` into `output` using a four-way tournament tree.
    unsafe fn merge<T: Ord>(runs: &mut [super::Run<T>; 4], output: &mut super::Run<T>) {
        /// Returns the index of the run with the smaller first element.
        ///
        /// Guaranteed to always return the index of an inhabited run unless both are empty.
        ///
        /// # Safety
        /// each run in `runs` has to be valid to read from.
        unsafe fn min_run<T: Ord>(
            index_a: usize,
            index_b: usize,
            runs: &[super::Run<T>; 4],
        ) -> usize {
            // SAFETY: see method doc.
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

        // SAFETY: We know each run in `runs` is valid to read from and `output` is valid to write
        // to (see method doc.). `min_run()` always returns an inhabited run if it exists, and for
        // all `output.len()` elements there exists at least one of these runs.
        unsafe {
            // Construct initial tournament tree
            let mut left = min_run(0, 1, runs);
            let mut right = min_run(2, 3, runs);
            let mut root = min_run(left, right, runs);

            for _ in 0..output.len() {
                // Copy minimum run
                runs[root].copy_nonoverlapping_prefix_to(output, 1);

                // Update tournament tree
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

// Each `MergingMethod` is also a `MultiMergingMethod`
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

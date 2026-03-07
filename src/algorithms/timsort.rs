//! The Timsort implementation.

use super::merging::BufGuard as _;

/// The default insertion sort to use.
pub type DefaultInsertionSort = super::insertionsort::InsertionSort<true>;

/// The default [`super::merging::MergingMethod`] to use.
pub type DefaultMergingMethod = super::merging::two_way::Galloping;

/// The default [`super::BufGuardFactory`] to use.
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `MIN_MERGE` to use.
pub const DEFAULT_MIN_MERGE: usize = 32;

/// The Timsort [`super::Sort`].
///
/// - `I` is the insertion sort used for small slices.
/// - `M` is the [`super::merging::MergingMethod`] used to merge slices.
/// - `B` is the [`super::BufGuardFactory`] used to create the merging buffer.
/// - `MIN_MERGE` determines the maximum slice length threshold to be sorted with `I`.
pub struct TimSort<
    I: super::PostfixSort = DefaultInsertionSort,
    M: super::merging::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const MIN_MERGE: usize = DEFAULT_MIN_MERGE,
>(
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_MERGE: usize,
> super::Sort for TimSort<I, M, B, MIN_MERGE>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    const BASE_NAME: &str = "timsort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![
            ("i-sort", crate::cli::display_inline::<I>()),
            ("merging", M::display()),
            ("min-merge", MIN_MERGE.to_string()),
        ]
        .into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = B::Guard::with_capacity(slice.len());

        // Delegate to helper function
        Self::timsort(slice, buffer.as_uninit_slice_mut());
    }
}

/// A single continuous run starting at `start` followed by `len` weakly increasing elements.
#[derive(Debug, Clone, Copy)]
struct Run {
    /// The start index of the run.
    start: usize,
    /// The length of the run.
    len: usize,
}

impl<
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_MERGE: usize,
> TimSort<I, M, B, MIN_MERGE>
{
    /// The actual Timsort implementation.
    fn timsort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() < MIN_MERGE {
            let split_point = Self::count_run_and_make_ascending(slice);
            I::sort_with_sorted_prefix(slice, split_point);
            return;
        }

        // Stack of pending runs
        let mut pending_runs: Vec<Run> = vec![];

        // Calculate the minimum run length to use for merging
        let min_run_length = Self::min_run_length(slice.len());

        // Tracking remaining length seems to optimize well
        let mut start = 0;
        let mut remaining_length = slice.len();

        while start < slice.len() {
            // Find the current run length
            let mut run_length = Self::count_run_and_make_ascending(&mut slice[start..]);

            // Make sure we have at least run length `min_run_length`
            if run_length < min_run_length {
                let forced_run_length = std::cmp::min(remaining_length, min_run_length);
                I::sort_with_sorted_prefix(
                    &mut slice[start..start + forced_run_length],
                    run_length,
                );
                run_length = forced_run_length;
            }

            // Add current run to stack
            pending_runs.push(Run {
                start,
                len: run_length,
            });

            // Merge top runs according to Timsort rules
            Self::merge_collapse(slice, buffer, &mut pending_runs);

            start += run_length;
            remaining_length -= run_length;
        }

        // Merge the rest of the runs
        Self::merge_force_collapse(slice, buffer, &mut pending_runs);

        debug_assert!(pending_runs.len() == 1, "There should only be one run left");
    }

    /// Find the first index `i`, such that `slice[..i]` is weakly increasing.
    ///
    /// If `slice` starts with a strictly decreasing run `slice[..i]`, it will be reversed and `i`
    /// will be returned.
    fn count_run_and_make_ascending<T: Ord>(slice: &mut [T]) -> usize {
        if slice.len() < 2 {
            return slice.len();
        }

        if slice[0] > slice[1] {
            let run_end = super::merging::util::strictly_decreasing_prefix_index(slice);

            slice[..run_end].reverse();

            run_end
        } else {
            super::merging::util::weakly_increasing_prefix_index(slice)
        }
    }

    /// Calculates the minimum run length for a given `n`.
    fn min_run_length(mut n: usize) -> usize {
        let mut r = 0;
        while n >= MIN_MERGE {
            r |= n & 1;
            n >>= 1;
        }
        n + r
    }

    /// Merges runs from the top of the stack to uphold the following invariants:
    ///
    /// - `pending_runs[top].len > pending_runs[top - 1].len + pending_runs[top - 2].len`
    /// - `pending_runs[top - 1].len > pending_runs[top - 2].len`
    fn merge_collapse<T: Ord>(
        slice: &mut [T],
        buffer: &mut [std::mem::MaybeUninit<T>],
        pending_runs: &mut Vec<Run>,
    ) {
        while pending_runs.len() > 1 {
            let mut n = pending_runs.len() - 2;

            if (n > 0 && pending_runs[n - 1].len <= pending_runs[n].len + pending_runs[n + 1].len)
                || (n > 1
                    && pending_runs[n - 2].len <= pending_runs[n - 1].len + pending_runs[n].len)
            {
                if pending_runs[n - 1].len < pending_runs[n + 1].len {
                    n -= 1;
                }

                Self::merge_at(slice, buffer, pending_runs, n);
            } else if pending_runs[n].len <= pending_runs[n + 1].len {
                Self::merge_at(slice, buffer, pending_runs, n);
            } else {
                break;
            }
        }
    }

    /// Merges runs from the top of the `pending_runs` stack, until there is only one left.
    fn merge_force_collapse<T: Ord>(
        slice: &mut [T],
        buffer: &mut [std::mem::MaybeUninit<T>],
        pending_runs: &mut Vec<Run>,
    ) {
        while pending_runs.len() > 1 {
            let mut n = pending_runs.len() - 2;

            if n > 0 && pending_runs[n - 1].len < pending_runs[n + 1].len {
                n -= 1;
            }

            Self::merge_at(slice, buffer, pending_runs, n);
        }
    }

    /// Merge the runs `pending_runs[index]` and `pending_runs[index + 1]`.
    ///
    /// # Panics
    ///
    /// if `index` is not the last or second to last element of `pending_runs`.
    fn merge_at<T: Ord>(
        slice: &mut [T],
        buffer: &mut [std::mem::MaybeUninit<T>],
        pending_runs: &mut Vec<Run>,
        index: usize,
    ) {
        // Check we are merging the last or second to last element
        let stack_size = pending_runs.len();
        assert!(stack_size >= 2);
        assert!(index == stack_size - 2 || index == stack_size - 3);

        // Check the runs are valid
        let run1 = pending_runs[index];
        let run2 = pending_runs[index + 1];
        assert!(run1.len > 0 && run2.len > 0);
        assert!(run1.start + run1.len == run2.start);

        // Merge the run markers
        pending_runs[index].len += run2.len;
        if index != stack_size - 2 {
            pending_runs[index + 1] = pending_runs[index + 2]
        }
        pending_runs.pop();

        // Merge the actual runs
        M::merge(
            &mut slice[run1.start..run1.start + run1.len + run2.len],
            run1.len,
            buffer,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = crate::test::DEFAULT_RUNS;
    const TEST_SIZE: usize = crate::test::DEFAULT_TEST_SIZE;

    #[test]
    fn empty() {
        crate::test::test_empty::<TimSort>();
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, TimSort>();
    }

    #[test]
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, TimSort>();
    }
}

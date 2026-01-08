//! The timsort implementation

use crate::algorithms::merging::strictly_decreasing_prefix_index;

use super::merging::BufGuard as _;

/// The default insertion sort to use
pub type DefaultInsertionSort = super::insertionsort::InsertionSort::<true>;

/// The default [`super::merging::MergingMethod`] to use
pub type DefaultMergingMethod = super::merging::Galloping;

/// The default BufGuardFactory to use
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `MIN_MERGE` to use
pub const DEFAULT_MIN_MERGE: usize = 32;

/// The timsort [`super::Sort`]
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

    fn sort<T: Ord>(slice: &mut [T]) {
        let mut buffer = B::Guard::with_capacity(slice.len());

        Self::timsort(slice, buffer.as_uninit_slice_mut());
    }
}

/// A single continuous run starting at `start` followed by `len` weakly increasing elements
#[derive(Debug, Clone, Copy)]
struct Run {
    start: usize,
    len: usize,
}

impl<
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_MERGE: usize,
> TimSort<I, M, B, MIN_MERGE>
{
    /// Actual timsort implementation
    fn timsort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() < 2 {
            return;
        }

        if slice.len() < MIN_MERGE {
            let split_point = Self::count_run_and_make_ascending(slice);
            I::sort(slice, split_point);
            return;
        }

        let mut pending_runs: Vec<Run> = vec![];

        let min_run = Self::min_run_length(slice.len());

        let mut n = slice.len();
        let mut start = 0;

        while n != 0 {
            let mut run_length = Self::count_run_and_make_ascending(&mut slice[start..]);

            if run_length < min_run {
                let force = std::cmp::min(n, min_run);
                I::sort(&mut slice[start..start + force], run_length);
                run_length = force;
            }

            pending_runs.push(Run {
                start,
                len: run_length,
            });
            Self::merge_collapse(slice, buffer, &mut pending_runs);

            start += run_length;
            n -= run_length;
        }

        assert!(start == slice.len());
        Self::merge_force_collapse(slice, buffer, &mut pending_runs);
        assert!(pending_runs.len() == 1);
    }

    /// Find the first weakly increasing run and return it's end index.
    ///
    /// If `slice` starts with a strictly decreasing run, it is found and reversed.
    fn count_run_and_make_ascending<T: Ord>(slice: &mut [T]) -> usize {
        if slice.len() < 2 {
            return slice.len();
        }

        if slice[0] > slice[1] {
            let run_end = strictly_decreasing_prefix_index(slice);

            slice[..run_end].reverse();

            run_end
        } else {
            super::merging::weakly_increasing_prefix_index(slice)
        }
    }

    // TODO: I have no clue how and why this does
    fn min_run_length(mut n: usize) -> usize {
        let mut r = 0;
        while n >= MIN_MERGE {
            r |= n & 1;
            n >>= 1;
        }
        n + r
    }

    // TODO: Hope this is correct?
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

    // TODO: Again hope this is correct?
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

    // TODO: add description
    fn merge_at<T: Ord>(
        slice: &mut [T],
        buffer: &mut [std::mem::MaybeUninit<T>],
        pending_runs: &mut Vec<Run>,
        index: usize,
    ) {
        let stack_size = pending_runs.len();
        assert!(stack_size >= 2);
        assert!(index == stack_size - 2 || index == stack_size - 3);

        let run1 = pending_runs[index];
        let run2 = pending_runs[index + 1];
        assert!(run1.len > 0 && run2.len > 0);
        assert!(run1.start + run1.len == run2.start);

        pending_runs[index].len += run2.len;
        if index != stack_size - 2 {
            pending_runs[index + 1] = pending_runs[index + 2]
        }
        pending_runs.pop();

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

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100_000;

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

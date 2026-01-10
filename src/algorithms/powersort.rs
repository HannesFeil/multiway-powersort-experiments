//! The powersort implementation

use crate::algorithms::merging::BufGuard as _;

/// The default insertion sort to use
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use
pub type DefaultMergingMethod = super::merging::CopyBoth;

/// The default BufGuardFactory to use
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `MIN_RUN_LENGTH` to use
pub const DEFAULT_MIN_RUN_LENGTH: usize = 24;

/// The default `ONLY_INCREASING_RUNS` to use
pub const DEFAULT_ONLY_INCREASING_RUNS: bool = false;

/// The default `POWER_INDEXED_STACK` to use
pub const DEFAULT_USE_POWER_INDEXED_STACK: bool = false;

// TODO: missing node power implementation
/// The powersort [`super::Sort`]
pub struct PowerSort<
    I: super::PostfixSort = DefaultInsertionSort,
    M: super::merging::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const MIN_RUN_LENGTH: usize = DEFAULT_MIN_RUN_LENGTH,
    const ONLY_INCREASING_RUNS: bool = DEFAULT_ONLY_INCREASING_RUNS,
    const USE_POWER_INDEXED_STACK: bool = DEFAULT_USE_POWER_INDEXED_STACK,
>(
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
    const USE_POWER_INDEXED_STACK: bool,
> super::Sort
    for PowerSort<I, M, B, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS, USE_POWER_INDEXED_STACK>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        Self::powersort(slice, buffer.as_uninit_slice_mut());
    }
}

impl<
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
    const USE_POWER_INDEXED_STACK: bool,
> PowerSort<I, M, B, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS, USE_POWER_INDEXED_STACK>
{
    fn powersort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        // TODO: unwrap?
        let log_n_plus_2 = usize::try_from(slice.len().ilog2()).unwrap() + 2;
        let mut stack: Box<[Option<std::ops::Range<usize>>]> =
            std::iter::repeat_n(None, log_n_plus_2).collect();
        let mut top = 0;
        let mut run_a = 0..Self::extend_run(slice);
        if run_a.len() < MIN_RUN_LENGTH {
            let end_a = std::cmp::min(slice.len(), MIN_RUN_LENGTH);
            I::sort(&mut slice[..end_a], run_a.len());
            run_a.end = end_a;
        }
        assert!(slice[run_a.clone()].is_sorted());

        while run_a.end != slice.len() {
            let mut run_b = run_a.end..Self::extend_run(&mut slice[run_a.end..]) + run_a.end;
            if run_b.len() < MIN_RUN_LENGTH {
                let end_b = std::cmp::min(slice.len(), run_b.start + MIN_RUN_LENGTH);
                I::sort(&mut slice[run_b.start..end_b], run_b.len());
                run_b.end = end_b;
            }
            assert!(slice[run_b.clone()].is_sorted());

            let node_power = Self::node_power(slice.len(), run_a.clone(), run_b.clone());
            assert!(node_power != top);

            if node_power < top {
                for possible_run in stack[node_power..=top].iter_mut().rev() {
                    let Some(run) = possible_run else {
                        continue;
                    };

                    run_a.start = run.start;
                    M::merge(&mut slice[run_a.clone()], run.len(), buffer);
                    // TODO: keep these assertions as debug invariants? (other sorts?)
                    assert!(slice[run_a.clone()].is_sorted());

                    *possible_run = None;
                }
            }

            top = node_power;
            stack[node_power] = Some(run_a);
            run_a = run_b;
        }

        for possible_run in stack[..=top].iter().rev() {
            let Some(run) = possible_run else {
                continue;
            };

            M::merge(&mut slice[run.start..], run.len(), buffer);
        }
    }

    fn extend_run<T: Ord>(slice: &mut [T]) -> usize {
        if ONLY_INCREASING_RUNS {
            super::merging::weakly_increasing_prefix_index(slice)
        } else {
            match super::merging::weakly_increasing_or_strictly_decreasing_index(slice) {
                (index, false) => index,
                (index, true) => {
                    slice[..index].reverse();
                    index
                }
            }
        }
    }

    fn node_power(n: usize, run_a: std::ops::Range<usize>, run_b: std::ops::Range<usize>) -> usize {
        let a = (run_a.start as f64 + run_a.len() as f64 / 2.0) / n as f64;
        let b = (run_b.start as f64 + run_b.len() as f64 / 2.0) / n as f64;
        let mut k = 0;
        loop {
            k += 1;
            let power = 1 << k;
            if (a * (power as f64)).floor() < (b * (power as f64)).floor() {
                break k;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100_000;

    type PowerSortOnlyIncreasing = PowerSort<
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        true,
        DEFAULT_USE_POWER_INDEXED_STACK,
    >;

    #[test]
    fn empty() {
        crate::test::test_empty::<PowerSort>();
        crate::test::test_empty::<PowerSortOnlyIncreasing>();
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSort>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortOnlyIncreasing>();
    }

    #[test]
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSort>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortOnlyIncreasing>();
    }
}

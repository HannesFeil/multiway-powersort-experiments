//! The powersort implementation

use crate::algorithms::merging::BufGuard as _;

/// The default node power calculation method
pub type DefaultNodePowerMethod = node_power::MostSignificantSetBit;

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

/// Type used to represent runs of sorted elements
type Run = std::ops::Range<usize>;

// TODO: missing node power implementation
/// The powersort [`super::Sort`]
pub struct PowerSort<
    N: node_power::NodePowerMethod = DefaultNodePowerMethod,
    I: super::PostfixSort = DefaultInsertionSort,
    M: super::merging::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const MIN_RUN_LENGTH: usize = DEFAULT_MIN_RUN_LENGTH,
    const ONLY_INCREASING_RUNS: bool = DEFAULT_ONLY_INCREASING_RUNS,
    const USE_POWER_INDEXED_STACK: bool = DEFAULT_USE_POWER_INDEXED_STACK,
>(
    std::marker::PhantomData<N>,
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    N: node_power::NodePowerMethod,
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
    const USE_POWER_INDEXED_STACK: bool,
> super::Sort
    for PowerSort<N, I, M, B, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS, USE_POWER_INDEXED_STACK>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        if USE_POWER_INDEXED_STACK {
            Self::powersort::<T, PowerIndexedStack>(slice, buffer.as_uninit_slice_mut());
        } else {
            Self::powersort::<T, Stack>(slice, buffer.as_uninit_slice_mut());
        }
    }
}

impl<
    N: node_power::NodePowerMethod,
    I: super::PostfixSort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
    const USE_POWER_INDEXED_STACK: bool,
> PowerSort<N, I, M, B, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS, USE_POWER_INDEXED_STACK>
{
    fn powersort<T: Ord, S: RunStack>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        // TODO: unwrap?
        let max_stack_height = usize::try_from(slice.len().ilog2()).unwrap() + 2;
        let mut stack = S::new(max_stack_height);
        let mut run_a = 0..Self::extend_run(slice);
        if run_a.len() < MIN_RUN_LENGTH {
            let end_a = std::cmp::min(slice.len(), MIN_RUN_LENGTH);
            I::sort(&mut slice[..end_a], run_a.len());
            run_a.end = end_a;
        }

        while run_a.end != slice.len() {
            let mut run_b = run_a.end..Self::extend_run(&mut slice[run_a.end..]) + run_a.end;
            if run_b.len() < MIN_RUN_LENGTH {
                let end_b = std::cmp::min(slice.len(), run_b.start + MIN_RUN_LENGTH);
                I::sort(&mut slice[run_b.start..end_b], run_b.len());
                run_b.end = end_b;
            }

            assert!(run_a.end == run_b.start);
            let node_power = N::node_power(slice.len(), run_a.clone(), run_b.clone());
            assert!(node_power != stack.top_power());

            if node_power < stack.top_power() {
                for run in stack.pop_runs(node_power) {
                    run_a.start = run.start;
                    M::merge(&mut slice[run_a.clone()], run.len(), buffer);
                    // TODO: keep these assertions as debug invariants? (other sorts?)
                    debug_assert!(slice[run_a.clone()].is_sorted());
                }
            }

            stack.push(run_a, node_power);
            run_a = run_b;
        }

        for run in stack.pop_runs(0) {
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
}

trait RunStack {
    /// Create a new stack with the given capacity
    fn new(capacity: usize) -> Self;

    /// Returns a power greater or equal to the highest power of a run in the stack
    fn top_power(&self) -> usize;

    /// Push a new run onto the stack
    ///
    /// power must be greater than or equal to [`RunStack::top_power()`].
    /// After this call, [`RunStack::top_power()`] will be equal to `power`.
    fn push(&mut self, run: Run, power: usize);

    /// Pop runs from the top until [`RunStack::top_power()`] is less than `power`
    ///
    /// power must be smaller than or equal to [`RunStack::top_power()`].
    fn pop_runs<'this>(&'this mut self, power: usize) -> impl Iterator<Item = Run> + 'this;
}

/// A simple stack
#[derive(Debug)]
struct Stack(Box<[Option<Run>]>, usize);

impl RunStack for Stack {
    fn new(capacity: usize) -> Self {
        Self(std::iter::repeat_n(None, capacity).collect(), 0)
    }

    fn top_power(&self) -> usize {
        self.1
    }

    fn push(&mut self, run: Run, power: usize) {
        assert!(power >= self.1);
        assert!(power < self.0.len());

        self.0[power] = Some(run);
        self.1 = power;
    }

    fn pop_runs<'this>(&'this mut self, power: usize) -> impl Iterator<Item = Run> + 'this {
        assert!(power <= self.top_power());

        let top_power = self.top_power();
        self.1 = power;
        (power..=top_power)
            .rev()
            .filter_map(|i| dbg!(&mut self.0[i]).take())
    }
}

/// A power indexed stack
#[derive(Debug)]
struct PowerIndexedStack(Vec<(usize, Run)>);

impl RunStack for PowerIndexedStack {
    fn new(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    fn top_power(&self) -> usize {
        self.0.last().map(|(power, _)| *power).unwrap_or(0)
    }

    fn push(&mut self, run: Run, power: usize) {
        assert!(power >= self.top_power());
        assert!(!self.0.spare_capacity_mut().is_empty());

        self.0.push((power, run));
    }

    fn pop_runs<'this>(&'this mut self, power: usize) -> impl Iterator<Item = Run> + 'this {
        assert!(power <= self.top_power());

        std::iter::from_fn(move || {
            if self.top_power() >= power {
                self.0.pop().map(|(_, run)| run)
            } else {
                None
            }
        })
    }
}

/// Node power calculation
pub mod node_power {
    pub trait NodePowerMethod {
        // TODO: accurate?
        /// Calculate the node power of run b?
        fn node_power(
            n: usize,
            run_a: std::ops::Range<usize>,
            run_b: std::ops::Range<usize>,
        ) -> usize;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Trivial;

    impl NodePowerMethod for Trivial {
        fn node_power(
            n: usize,
            run_a: std::ops::Range<usize>,
            run_b: std::ops::Range<usize>,
        ) -> usize {
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

    #[derive(Debug, Clone, Copy)]
    pub struct DivisionLoop;

    impl NodePowerMethod for DivisionLoop {
        fn node_power(
            n: usize,
            run_a: std::ops::Range<usize>,
            run_b: std::ops::Range<usize>,
        ) -> usize {
            let n2 = n * 2;
            let mut a = 2 * run_a.start + run_a.len();
            let mut b = 2 * run_b.start + run_b.len();
            let mut k = 0;

            while b - a <= n2 && a / n2 == b / n2 {
                k += 1;
                a *= 2;
                b *= 2;
            }

            k
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct BitwiseLoop;

    impl NodePowerMethod for BitwiseLoop {
        fn node_power(
            n: usize,
            run_a: std::ops::Range<usize>,
            run_b: std::ops::Range<usize>,
        ) -> usize {
            const MAX_N: usize = 1 << (usize::BITS - 1);
            assert!(n <= MAX_N);

            let mut l2 = run_a.start + run_a.end;
            let mut r2 = run_b.start + run_b.end;

            let mut common_bits = 0;
            let (mut digit_a, mut digit_b) = (l2 >= n, r2 >= n);

            while digit_a == digit_b {
                common_bits += 1;

                if digit_a {
                    l2 -= n;
                    r2 -= n;
                }

                l2 <<= 1;
                r2 <<= 1;

                (digit_a, digit_b) = (l2 >= n, r2 >= n)
            }

            common_bits + 1
        }
    }

    // TODO: what about node_power_clz_unconstrained

    #[derive(Debug, Clone, Copy)]
    pub struct MostSignificantSetBit;

    impl NodePowerMethod for MostSignificantSetBit {
        fn node_power(
            n: usize,
            run_a: std::ops::Range<usize>,
            run_b: std::ops::Range<usize>,
        ) -> usize {
            const MAX_N: usize = 1 << (usize::BITS / 2 - 1);
            const HALF_MASK: usize = usize::MAX >> (usize::BITS / 2);

            assert!(n <= MAX_N);

            let l2 = run_a.start + run_a.end;
            let r2 = run_b.start + run_b.end;

            let a = ((l2 << 30) / n) & HALF_MASK;
            let b = ((r2 << 30) / n) & HALF_MASK;

            ((a ^ b).leading_zeros() - usize::BITS / 2) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 20;
    const TEST_SIZE: usize = 100_000;

    type PowerSortTrivialPowerIndexedStack = PowerSort<
        node_power::Trivial,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        true,
    >;

    type PowerSortTrivial = PowerSort<
        node_power::Trivial,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        DEFAULT_USE_POWER_INDEXED_STACK,
    >;

    type PowerSortDivisionLoop = PowerSort<
        node_power::DivisionLoop,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        DEFAULT_USE_POWER_INDEXED_STACK,
    >;

    type PowerSortBitwiseLoop = PowerSort<
        node_power::BitwiseLoop,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        DEFAULT_USE_POWER_INDEXED_STACK,
    >;

    type PowerSortMostSignificantBit = PowerSort<
        node_power::MostSignificantSetBit,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        DEFAULT_USE_POWER_INDEXED_STACK,
    >;

    #[test]
    fn empty() {
        crate::test::test_empty::<PowerSortTrivial>();
        crate::test::test_empty::<PowerSortTrivialPowerIndexedStack>();
        crate::test::test_empty::<PowerSortDivisionLoop>();
        crate::test::test_empty::<PowerSortBitwiseLoop>();
        crate::test::test_empty::<PowerSortMostSignificantBit>();
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortTrivial>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortTrivialPowerIndexedStack>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortDivisionLoop>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortBitwiseLoop>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortMostSignificantBit>();
    }

    #[test]
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortTrivial>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortTrivialPowerIndexedStack>(
        );
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortDivisionLoop>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortBitwiseLoop>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortMostSignificantBit>();
    }
}

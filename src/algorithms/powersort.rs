//! The powersort implementation

use crate::algorithms::merging::BufGuard as _;

/// The default node power calculation method
pub type DefaultNodePowerMethod = node_power::MostSignificantSetBit;

/// The default insertion sort to use
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use
pub type DefaultMergingMethod = super::merging::CopyBoth;

/// The default [`super::merging::MultiMergingMethod`] to use
pub type DefaultMultiMergingMethod = super::merging::multi::CopyAll;

/// The default BufGuardFactory to use
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `MERGE_K_RUNS` to use for multiway powersort
pub const DEFAULT_MERGE_K_RUNS: usize = 4;

/// The default `MIN_RUN_LENGTH` to use
pub const DEFAULT_MIN_RUN_LENGTH: usize = 24;

/// The default `ONLY_INCREASING_RUNS` to use
pub const DEFAULT_ONLY_INCREASING_RUNS: bool = false;

/// The default `POWER_INDEXED_STACK` to use
pub const DEFAULT_USE_POWER_INDEXED_STACK: bool = false;

/// Type used to represent runs of sorted elements
type Run = std::ops::Range<usize>;

/// The powersort [`super::Sort`]
pub struct PowerSort<
    N: node_power::NodePowerMethod<2> = DefaultNodePowerMethod,
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
    N: node_power::NodePowerMethod<2>,
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
    N: node_power::NodePowerMethod<2>,
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
        let mut run_a = Self::next_run(slice, 0);

        while run_a.end != slice.len() {
            let run_b = Self::next_run(slice, run_a.end);

            assert!(run_a.end == run_b.start);
            let node_power = N::node_power(slice.len(), run_a.clone(), run_b.clone());
            assert!(node_power != stack.top_power());

            if node_power < stack.top_power() {
                for (_, run) in stack.pop_runs(node_power) {
                    run_a.start = run.start;
                    M::merge(&mut slice[run_a.clone()], run.len(), buffer);
                    // TODO: keep these assertions as debug invariants? (other sorts?)
                    debug_assert!(slice[run_a.clone()].is_sorted());
                }
            }

            stack.push(run_a, node_power);
            run_a = run_b;
        }

        for (_, run) in stack.pop_runs(0) {
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

    fn next_run<T: Ord>(slice: &mut [T], start: usize) -> Run {
        let run = start..Self::extend_run(&mut slice[start..]);

        if run.len() < MIN_RUN_LENGTH {
            let end = std::cmp::min(slice.len(), start + MIN_RUN_LENGTH);

            I::sort(&mut slice[start..end], run.len());

            start..end
        } else {
            run
        }
    }
}

/// The powersort [`super::Sort`]
pub struct MultiwayPowerSort<
    N: node_power::NodePowerMethod<MERGE_K_RUNS> = DefaultNodePowerMethod,
    I: super::PostfixSort = DefaultInsertionSort,
    M: super::merging::multi::MultiMergingMethod<MERGE_K_RUNS> = DefaultMultiMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const MERGE_K_RUNS: usize = DEFAULT_MERGE_K_RUNS,
    const MIN_RUN_LENGTH: usize = DEFAULT_MIN_RUN_LENGTH,
    const ONLY_INCREASING_RUNS: bool = DEFAULT_ONLY_INCREASING_RUNS,
>(
    std::marker::PhantomData<N>,
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    N: node_power::NodePowerMethod<MERGE_K_RUNS>,
    I: super::PostfixSort,
    M: super::merging::multi::MultiMergingMethod<MERGE_K_RUNS>,
    B: super::BufGuardFactory,
    const MERGE_K_RUNS: usize,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
> super::Sort
    for MultiwayPowerSort<N, I, M, B, MERGE_K_RUNS, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        Self::multiway_powersort(slice, buffer.as_uninit_slice_mut());
    }
}

impl<
    N: node_power::NodePowerMethod<MERGE_K_RUNS>,
    I: super::PostfixSort,
    M: super::merging::multi::MultiMergingMethod<MERGE_K_RUNS>,
    B: super::BufGuardFactory,
    const MERGE_K_RUNS: usize,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
> MultiwayPowerSort<N, I, M, B, MERGE_K_RUNS, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>
{
    fn multiway_powersort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        // TODO: unwrap?
        let max_stack_height =
            (MERGE_K_RUNS - 1) * (usize::try_from(slice.len().ilog(MERGE_K_RUNS)).unwrap() + 2);
        let mut stack = PowerIndexedStack::new(max_stack_height);
        // NOTE: We technically only need `MERGE_K_RUNS - 1` but that is feature gated
        let mut split_points = [0; MERGE_K_RUNS];
        let mut split_points_index = MERGE_K_RUNS;
        let mut run_a = Self::next_run(slice, 0);
        assert!(slice[run_a.clone()].is_sorted());

        while run_a.end != slice.len() {
            let run_b = Self::next_run(slice, run_a.end);
            assert!(slice[run_b.clone()].is_sorted());

            let node_power = N::node_power(slice.len(), run_a.clone(), run_b.clone());

            let mut top_power = stack.top_power();
            if node_power < top_power {
                for (power, run) in stack.pop_runs(node_power) {
                    if top_power != power {
                        M::merge(
                            &mut slice[run_a.clone()],
                            &split_points[split_points_index..],
                            buffer,
                        );
                        split_points_index = MERGE_K_RUNS;
                        top_power = power;
                    }

                    split_points_index -= 1;
                    split_points[split_points_index] = run.len();
                    run_a.start = run.start;
                }

                assert!(!split_points.is_empty());
                M::merge(
                    &mut slice[run_a.clone()],
                    &split_points[split_points_index..],
                    buffer,
                );
                split_points_index = MERGE_K_RUNS;
            }

            stack.push(run_a, node_power);
            run_a = run_b;
        }

        let mut remaining = stack.pop_runs(0);
        while run_a.start != 0 {
            for _ in 0..MERGE_K_RUNS - 1 {
                let Some((_, run)) = remaining.next() else {
                    break;
                };

                split_points_index -= 1;
                split_points[split_points_index] = run.len();
                run_a.start = run.start;
            }

            M::merge(
                &mut slice[run_a.clone()],
                &split_points[split_points_index..],
                buffer,
            );
            split_points_index = MERGE_K_RUNS;
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

    fn next_run<T: Ord>(slice: &mut [T], start: usize) -> Run {
        let run = start..Self::extend_run(&mut slice[start..]);

        if run.len() < MIN_RUN_LENGTH {
            let end = std::cmp::min(slice.len(), start + MIN_RUN_LENGTH);

            I::sort(&mut slice[start..end], run.len());

            start..end
        } else {
            run
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
    /// After this call, [`RunStack::top_power()`] will be less than or equal to `power`.
    fn push(&mut self, run: Run, power: usize);

    /// Pop runs from the top until [`RunStack::top_power()`] is less than `power`
    ///
    /// power must be smaller than or equal to [`RunStack::top_power()`].
    /// After this call, [`RunStack::top_power()`] will be less than or equal to `power`.
    fn pop_runs<'this>(&'this mut self, power: usize)
    -> impl Iterator<Item = (usize, Run)> + 'this;
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
        assert!(self.0[power].is_none());

        self.0[power] = Some(run);
        self.1 = power;
    }

    fn pop_runs<'this>(
        &'this mut self,
        power: usize,
    ) -> impl Iterator<Item = (usize, Run)> + 'this {
        assert!(power <= self.top_power());

        let top_power = self.top_power();
        self.1 = power;
        (power..=top_power)
            .rev()
            .filter_map(|i| self.0[i].take().map(|run| (i, run)))
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

    fn pop_runs<'this>(
        &'this mut self,
        power: usize,
    ) -> impl Iterator<Item = (usize, Run)> + 'this {
        assert!(power <= self.top_power());

        std::iter::from_fn(move || {
            if self.top_power() >= power {
                self.0.pop()
            } else {
                None
            }
        })
    }
}

/// Node power calculation
pub mod node_power {
    pub trait NodePowerMethod<const K: usize> {
        /// The max `n` up to which this method words correctly
        const MAX_N: usize;

        // TODO: accurate?
        /// Calculate the node power of run b?
        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Trivial;

    impl<const K: usize> NodePowerMethod<K> for Trivial {
        // NOTE: Not sure when accuracy fails, but practically it should not matter
        const MAX_N: usize = usize::MAX;

        #[expect(clippy::absurd_extreme_comparisons)]
        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);

            // NOTE: This is correct under the assumption, that usize is not larger than f64?
            let a = (run_a.start as f64 + run_a.len() as f64 / 2.0) / n as f64;
            let b = (run_b.start as f64 + run_b.len() as f64 / 2.0) / n as f64;
            let mut power = 0;

            loop {
                power += 1;

                let k_to_p = (K as f64).powi(power.try_into().unwrap());

                if (a * k_to_p).floor() < (b * k_to_p).floor() {
                    break power;
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct DivisionLoop;

    impl<const K: usize> NodePowerMethod<K> for DivisionLoop {
        // FIXME: what is correct here?
        const MAX_N: usize = usize::MAX.isqrt();

        // TODO: check if this is correct
        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);

            let n2 = n * 2;
            let mut a = 2 * run_a.start + run_a.len();
            let mut b = 2 * run_b.start + run_b.len();
            let mut power = 0;

            // FIXME: how should this handle overflows?
            while b - a <= n2 && a / n2 == b / n2 {
                power += 1;
                a *= K;
                b *= K;
            }

            power
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct BitwiseLoop;

    impl<const K: usize> NodePowerMethod<K> for BitwiseLoop {
        const MAX_N: usize = {
            // TODO: is this correct?
            assert!(K > 1);
            assert!(K.count_ones() == 1, "K has to be a power of 2");

            1 << (usize::BITS - 1)
        };

        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);

            let factor: usize = K.trailing_zeros().try_into().unwrap();

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

            common_bits / factor + 1
        }
    }

    // TODO: what about node_power_clz_unconstrained

    #[derive(Debug, Clone, Copy)]
    pub struct MostSignificantSetBit;

    impl<const K: usize> NodePowerMethod<K> for MostSignificantSetBit {
        const MAX_N: usize = {
            // TODO: is this correct?
            assert!(K > 1);
            assert!(K.count_ones() == 1, "K has to be a power of 2");

            1 << (usize::BITS / 2 - 1)
        };

        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);

            const HALF_MASK: usize = usize::MAX >> (usize::BITS / 2);

            let factor: usize = K.trailing_zeros().try_into().unwrap();

            let l2 = run_a.start + run_a.end;
            let r2 = run_b.start + run_b.end;

            let a = ((l2 << 30) / n) & HALF_MASK;
            let b = ((r2 << 30) / n) & HALF_MASK;

            (((a ^ b).leading_zeros() - usize::BITS / 2) as usize - 1) / factor + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    const RUNS: usize = 20;
    const TEST_SIZE: usize = 100_000;

    // Test under the assumption, that node_power::Trivial is correct

    type PowerSortTrivial = PowerSort<
        node_power::Trivial,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        DEFAULT_USE_POWER_INDEXED_STACK,
    >;

    type PowerSortTrivialPowerIndexedStack = PowerSort<
        node_power::Trivial,
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
        true,
    >;

    type PowerSortTrivialMulti4 = MultiwayPowerSort<
        node_power::Trivial,
        DefaultInsertionSort,
        DefaultMultiMergingMethod,
        DefaultBufGuardFactory,
        4,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
    >;

    type PowerSortTrivialMulti8 = MultiwayPowerSort<
        node_power::Trivial,
        DefaultInsertionSort,
        DefaultMultiMergingMethod,
        DefaultBufGuardFactory,
        8,
        DEFAULT_MIN_RUN_LENGTH,
        DEFAULT_ONLY_INCREASING_RUNS,
    >;

    macro_rules! test_powers {
        ([$($power:expr),*]: $k:ident => $code:expr) => {
            $(
                {
                    const $k: usize = $power;

                    $code;
                }
            );*
        };
    }

    #[test]
    fn empty() {
        crate::test::test_empty::<PowerSortTrivial>();
        crate::test::test_empty::<PowerSortTrivialPowerIndexedStack>();
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortTrivial>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortTrivialPowerIndexedStack>();
    }

    #[test]
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortTrivial>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortTrivialPowerIndexedStack>(
        );
    }

    #[test]
    fn multi_empty() {
        crate::test::test_empty::<PowerSortTrivialMulti4>();
        crate::test::test_empty::<PowerSortTrivialMulti8>();
    }

    #[test]
    fn multi_random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortTrivialMulti4>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PowerSortTrivialMulti8>();
    }

    #[test]
    fn multi_random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortTrivialMulti4>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PowerSortTrivialMulti8>();
    }

    #[test]
    fn node_power_division_loop() {
        test_powers!(
            [2, 3, 4, 5, 6, 7, 8]:
            K => test_node_power_calculations::<node_power::DivisionLoop, K>()
        );
    }

    #[test]
    fn node_power_bitwise_loop() {
        test_powers!(
            [2, 4, 8, 16]:
            K => test_node_power_calculations::<node_power::BitwiseLoop, K>()
        );
    }

    #[test]
    fn node_power_most_significant_bit_set() {
        test_powers!(
            [2, 4, 8, 16]:
            K => test_node_power_calculations::<node_power::MostSignificantSetBit, K>()
        );
    }

    fn test_node_power_calculations<N: node_power::NodePowerMethod<K>, const K: usize>() {
        use node_power::*;

        let mut rng = crate::test::test_rng();

        for _ in 0..RUNS {
            let n = rng.random_range(2..N::MAX_N);
            let start = rng.random_range(0..(n - 2));
            let middle = rng.random_range(start + 1..n - 1);
            let end = rng.random_range(middle + 1..n);

            let run_a = start..middle;
            let run_b = middle..end;

            let correct_power =
                <Trivial as NodePowerMethod<K>>::node_power(n, run_a.clone(), run_b.clone());
            let test_power = N::node_power(n, run_a, run_b);

            assert_eq!(correct_power, test_power);
        }
    }
}

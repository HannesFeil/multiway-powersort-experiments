//! The Powersort implementation.

use crate::algorithms::merging::BufGuard as _;

/// The default [`node_power::NodePowerMethod`] to use.
pub type DefaultNodePowerMethod = node_power::MostSignificantSetBit;

/// The default insertion sort to use.
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use.
pub type DefaultMergingMethod = super::merging::two_way::CopyBoth;

/// The default [`super::merging::MultiMergingMethod`] to use.
pub type DefaultMultiMergingMethod = super::merging::multi_way::TournamentTree;

/// The default [`super::BufGuardFactory`] to use.
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `MERGE_K_RUNS` to use.
pub const DEFAULT_MERGE_K_RUNS: usize = 4;

/// The default `MIN_RUN_LENGTH` to use.
pub const DEFAULT_MIN_RUN_LENGTH: usize = 24;

/// The default `ONLY_INCREASING_RUNS` to use.
pub const DEFAULT_ONLY_INCREASING_RUNS: bool = false;

/// The default `USE_POWER_INDEXED_STACK` to use.
pub const DEFAULT_USE_POWER_INDEXED_STACK: bool = false;

/// The Powersort [`super::Sort`].
///
/// - `N` is the [`noder_power::NodePowerMethod`] used to calculate the node power of runs.
/// - `I` is the insertion sort used to extend small runs.
/// - `M` is the [`super::merging::MergingMethod`] used to merge runs.
/// - `B` is the [`super::BufGuardFactory`] used to create the buffer for merging.
/// - `MIN_RUN_LENGTH` determines the minimum length up to which runs will be manually extended.
/// - `ONLY_INCREASING_RUNS` indicates whether only to use preexisting weakly increasing runs.
/// - `USE_POWER_INDEXED_STACK` indicates whether to use a power indexed stack.
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

    const BASE_NAME: &str = "powersort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![
            ("node-power", N::display()),
            ("i-sort", crate::cli::display_inline::<I>()),
            ("merging", M::display()),
            ("min-run-len", MIN_RUN_LENGTH.to_string()),
            ("only-increasing", ONLY_INCREASING_RUNS.to_string()),
            ("power-indexed", USE_POWER_INDEXED_STACK.to_string()),
        ]
        .into_iter()
    }

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

/// Type used to represent runs of sorted elements
type Run = std::ops::Range<usize>;

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
    /// The actual Powersort implementation.
    fn powersort<T: Ord, S: RunStack>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        // Create the run stack
        let max_stack_height =
            usize::try_from(slice.len().ilog2()).expect("This can not panic") + 2;
        let mut stack = S::new(max_stack_height);

        // Find current run
        let mut current_run = next_run::<_, I, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>(slice, 0);

        // Iterate until we reach the end
        while current_run.end != slice.len() {
            // Find next run
            let next_run =
                next_run::<_, I, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>(slice, current_run.end);

            // Calculate the node power of the current run
            assert!(current_run.end == next_run.start);
            let current_node_power =
                N::node_power(slice.len(), current_run.clone(), next_run.clone());
            assert!(current_node_power != stack.top_power());

            // Pop and merge runs with higher power from the stack with the current run.
            for (_, run) in stack.pop_runs_with_greater_power(current_node_power) {
                current_run.start = run.start;

                M::merge(&mut slice[current_run.clone()], run.len(), buffer);
            }

            // Push current run onto the stack
            stack.push(current_run, current_node_power);
            current_run = next_run;
        }

        // Merge all remaining runs with the rest of the slice
        for (_, run) in stack.pop_all() {
            M::merge(&mut slice[run.start..], run.len(), buffer);
        }
    }
}

/// The Multiway Powersort [`super::Sort`].
///
/// - `N` is the [`noder_power::NodePowerMethod`] used to calculate the node power of runs.
/// - `I` is the insertion sort used to extend small runs.
/// - `M` is the [`super::merging::MultiMergingMethod`] used to merge runs.
/// - `B` is the [`super::BufGuardFactory`] used to create the buffer for merging.
/// - `MERGE_K_RUNS` determines how many runs are merged together.
/// - `MIN_RUN_LENGTH` determines the minimum length up to which runs will be manually extended.
/// - `ONLY_INCREASING_RUNS` indicates whether only to use preexisting weakly increasing runs.
pub struct MultiwayPowerSort<
    N: node_power::NodePowerMethod<MERGE_K_RUNS> = DefaultNodePowerMethod,
    I: super::PostfixSort = DefaultInsertionSort,
    M: super::merging::MultiMergingMethod<MERGE_K_RUNS> = DefaultMultiMergingMethod,
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
    M: super::merging::MultiMergingMethod<MERGE_K_RUNS>,
    B: super::BufGuardFactory,
    const MERGE_K_RUNS: usize,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
> super::Sort
    for MultiwayPowerSort<N, I, M, B, MERGE_K_RUNS, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    const BASE_NAME: &str = "multiway-powersort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![
            ("K", MERGE_K_RUNS.to_string()),
            ("node-power", N::display()),
            ("i-sort", crate::cli::display_inline::<I>()),
            ("merging", M::display()),
            ("min-run-len", MIN_RUN_LENGTH.to_string()),
            ("only-increasing", ONLY_INCREASING_RUNS.to_string()),
        ]
        .into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        Self::multiway_powersort(slice, buffer.as_uninit_slice_mut());
    }
}

impl<
    N: node_power::NodePowerMethod<MERGE_K_RUNS>,
    I: super::PostfixSort,
    M: super::merging::MultiMergingMethod<MERGE_K_RUNS>,
    B: super::BufGuardFactory,
    const MERGE_K_RUNS: usize,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
> MultiwayPowerSort<N, I, M, B, MERGE_K_RUNS, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>
{
    // The actual Multiway Powersort implementation.
    fn multiway_powersort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        // Create run stack
        let max_stack_height = (MERGE_K_RUNS - 1)
            * (usize::try_from(slice.len().ilog(MERGE_K_RUNS)).expect("This can not fail") + 2);
        let mut stack = Stack::new(max_stack_height);

        // NOTE: We technically only need `MERGE_K_RUNS - 1` but that is unstable (const generics)
        // `run_lengths[run_lengths_index..]` forms the stack of merging split points use by `M`.
        // We build the stack from the back, since that is the order `M` expects.
        let mut run_lengths = [0; MERGE_K_RUNS];
        let mut run_lengths_index = MERGE_K_RUNS;

        // Find current run
        let mut current_run = next_run::<_, I, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>(slice, 0);

        // Iterate until we reach the end
        while current_run.end != slice.len() {
            // Find next run
            let next_run =
                next_run::<_, I, MIN_RUN_LENGTH, ONLY_INCREASING_RUNS>(slice, current_run.end);

            // Calculate the node power of the current run
            let node_power = N::node_power(slice.len(), current_run.clone(), next_run.clone());

            // Pop runs from the stack until the current node power is the highest
            let mut top_power = stack.top_power();
            if node_power < top_power {
                // Pop runs and collect run lengths of runs of equal power in `run_lengths`
                for (power, run) in stack.pop_runs_with_greater_power(node_power) {
                    // Run power drops, merge all previously collected runs
                    if top_power != power {
                        M::merge(
                            &mut slice[current_run.clone()],
                            &run_lengths[run_lengths_index..],
                            buffer,
                        );

                        // Empty `run_lengths` stack and update last power
                        run_lengths_index = MERGE_K_RUNS;
                        top_power = power;
                    }

                    // Push run onto the `run_lengths` stack
                    run_lengths_index -= 1;
                    run_lengths[run_lengths_index] = run.len();

                    // Expand current run start (since we will merge)
                    current_run.start = run.start;
                }

                // There will be at least one run left to merge at this point
                assert!(run_lengths_index < MERGE_K_RUNS);
                M::merge(
                    &mut slice[current_run.clone()],
                    &run_lengths[run_lengths_index..],
                    buffer,
                );

                // Empty `run_lengths` stack
                run_lengths_index = MERGE_K_RUNS;
            }

            stack.push(current_run, node_power);
            current_run = next_run;
        }

        let stack_size = stack.len();
        let remainder = stack_size % (MERGE_K_RUNS - 1);

        // Pop all remaining runs
        let mut remaining_runs = stack.pop_runs_with_greater_power(0);

        // Merge runs so we have a multiple of `MERGE_K_RUNS - 1` runs left
        if remainder > 0 {
            // Collect run lengths
            for (_, run) in remaining_runs.by_ref().take(remainder) {
                run_lengths_index -= 1;
                run_lengths[run_lengths_index] = run.len();
                current_run.start = run.start;
            }

            M::merge(
                &mut slice[current_run.clone()],
                &run_lengths[run_lengths_index..],
                buffer,
            );
        }

        // Repeatedly merge `MERGE_K_RUNS - 1` top runs and the current run
        for _ in 0..stack_size / (MERGE_K_RUNS - 1) {
            // Collect run lengths
            for i in (1..MERGE_K_RUNS).rev() {
                let run = remaining_runs.next().unwrap().1;
                run_lengths[i] = run.len();
                current_run.start = run.start;
            }

            M::merge(&mut slice[current_run.clone()], &run_lengths[1..], buffer);
        }
    }
}

/// Finds the maximum index `i` such that `slice[..i]` is weakly increasing.
///
/// If `ONLY_INCREASING_RUNS` is `false`, and `slice[..j]` contains a strictly decreasing run,
/// reverses that run and returns `j`.
fn find_run<T: Ord, const ONLY_INCREASING_RUNS: bool>(slice: &mut [T]) -> usize {
    if ONLY_INCREASING_RUNS {
        super::merging::util::weakly_increasing_prefix_index(slice)
    } else {
        match super::merging::util::weakly_increasing_or_strictly_decreasing_index(slice) {
            (index, super::merging::util::RunOrdering::WeaklyIncreasing) => index,
            (index, super::merging::util::RunOrdering::StrictlyDecreasing) => {
                slice[..index].reverse();
                index
            }
        }
    }
}

/// Creates the next run, by finding the longest existing run and potentially extending it using
/// `I` such that it is at least `MIN_RUN_LENGTH` elements long.
fn next_run<
    T: Ord,
    I: super::PostfixSort,
    const MIN_RUN_LENGTH: usize,
    const ONLY_INCREASING_RUNS: bool,
>(
    slice: &mut [T],
    start: usize,
) -> Run {
    // Find longest existing run
    let run = start..start + find_run::<_, ONLY_INCREASING_RUNS>(&mut slice[start..]);

    // Extend run if too short
    if run.len() < MIN_RUN_LENGTH {
        let end = std::cmp::min(slice.len(), start + MIN_RUN_LENGTH);

        I::sort_with_sorted_prefix(&mut slice[start..end], run.len());

        start..end
    } else {
        run
    }
}

/// Unifies behavior of run stack implementations.
trait RunStack {
    /// Creates a new stack with the given capacity.
    fn new(capacity: usize) -> Self;

    /// Returns a power greater or equal to the highest power of a run in the stack.
    fn top_power(&self) -> usize;

    /// Push a new run onto the stack.
    ///
    /// `power` must be greater than or equal to [`RunStack::top_power()`].
    /// After this call, [`RunStack::top_power()`] will be equal to `power`.
    fn push(&mut self, run: Run, power: usize);

    /// Pop runs from the top of the stack until the highest power is less than or equal to `power`.
    ///
    /// After this call, [`RunStack::top_power()`] will be less than or equal to `power`.
    ///
    /// # Note
    ///
    /// If the returned iterator is not fully consumed, the resulting state of the stack is left
    /// unspecified.
    fn pop_runs_with_greater_power<'this>(
        &'this mut self,
        power: usize,
    ) -> impl Iterator<Item = (usize, Run)> + 'this;

    /// Pops all remaining runs from this stack.
    fn pop_all(self) -> impl Iterator<Item = (usize, Run)>;

    /// Returns the number of runs left on the stack.
    fn len(&self) -> usize;
}

/// A power indexed stack, cannot be used for [`MultiwayPowerSort`] since it can only store one run
/// of each power.
#[derive(Debug)]
struct PowerIndexedStack(Box<[Option<Run>]>, usize);

impl RunStack for PowerIndexedStack {
    fn new(capacity: usize) -> Self {
        Self(std::iter::repeat_n(None, capacity).collect(), 0)
    }

    fn top_power(&self) -> usize {
        self.1
    }

    fn push(&mut self, run: Run, power: usize) {
        assert!(power >= self.1);
        assert!(power < self.0.len());
        assert!(self.0[power].is_none(), "Power slot is already occupied");

        self.0[power] = Some(run);
        self.1 = power;
    }

    fn pop_runs_with_greater_power<'this>(
        &'this mut self,
        power: usize,
    ) -> impl Iterator<Item = (usize, Run)> + 'this {
        let top_power = self.top_power();
        self.1 = power;
        (power + 1..=top_power)
            .rev()
            .filter_map(|i| self.0[i].take().map(|run| (i, run)))
    }

    fn pop_all(mut self) -> impl Iterator<Item = (usize, Run)> {
        (0..=self.top_power())
            .rev()
            .filter_map(move |i| self.0[i].take().map(|run| (i, run)))
    }

    fn len(&self) -> usize {
        self.0[..=self.top_power()]
            .iter()
            .filter(|r| r.is_some())
            .count()
    }
}

/// A simple [`RunStack`] implementation, storing each stack with its power.
#[derive(Debug)]
struct Stack(Vec<(usize, Run)>);

impl RunStack for Stack {
    fn new(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    fn top_power(&self) -> usize {
        self.0.last().map(|(power, _)| *power).unwrap_or(0)
    }

    fn push(&mut self, run: Run, power: usize) {
        assert!(power >= self.top_power());
        assert!(
            !self.0.spare_capacity_mut().is_empty(),
            "We should not exceed the initial capacity"
        );

        self.0.push((power, run));
    }

    fn pop_runs_with_greater_power<'this>(
        &'this mut self,
        power: usize,
    ) -> impl Iterator<Item = (usize, Run)> + 'this {
        std::iter::from_fn(move || {
            if self.top_power() > power {
                self.0.pop()
            } else {
                None
            }
        })
    }

    fn pop_all(self) -> impl Iterator<Item = (usize, Run)> {
        self.0.into_iter().rev()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Node power calculation methods.
pub mod node_power {
    /// Defines a node power calculation method, for `K` way merges.
    pub trait NodePowerMethod<const K: usize> {
        /// The max `n` up to which this method words correctly
        const MAX_N: usize;

        /// The string representation of this node power method
        fn display() -> String;

        /// Calculate the node power of `run_a`.
        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize;
    }

    /// Trivial [`NodePowerMethod`] using floating point calculations.
    #[allow(dead_code, reason = "Currently not used for experiments")]
    #[derive(Debug, Clone, Copy)]
    pub struct Trivial;

    impl<const K: usize> NodePowerMethod<K> for Trivial {
        // NOTE: Not sure when accuracy fails, but practically it should not matter
        const MAX_N: usize = usize::MAX;

        fn display() -> String {
            "trivial".to_string()
        }

        #[expect(
            clippy::as_conversions,
            reason = "The accuracy should not matter for very large values"
        )]
        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            #[expect(clippy::absurd_extreme_comparisons)]
            {
                assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);
            }

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

    /// A [`NodePowerMethod`] using a simple division loop.
    #[allow(dead_code, reason = "Currently not used for experiments")]
    #[derive(Debug, Clone, Copy)]
    pub struct DivisionLoop;

    impl<const K: usize> NodePowerMethod<K> for DivisionLoop {
        // NOTE: is this correct?
        const MAX_N: usize = usize::MAX.isqrt();

        fn display() -> String {
            "division-loop".to_string()
        }

        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);

            let n2 = n * 2;
            let mut a = 2 * run_a.start + run_a.len();
            let mut b = 2 * run_b.start + run_b.len();
            let mut power = 0;

            // TODO: Investigate with regards to overflows?
            while b - a <= n2 && a / n2 == b / n2 {
                power += 1;
                a *= K;
                b *= K;
            }

            power
        }
    }

    /// A [`NodePowerMethod`] using a loop with bitwise operations.
    ///
    /// # Note
    ///
    /// This method only works for `K` being a power of 2.
    #[allow(dead_code, reason = "Currently not used for experiments")]
    #[derive(Debug, Clone, Copy)]
    pub struct BitwiseLoop;

    impl<const K: usize> NodePowerMethod<K> for BitwiseLoop {
        const MAX_N: usize = {
            assert!(K > 1);
            assert!(K.count_ones() == 1, "K has to be a power of 2");

            1 << (usize::BITS - 1)
        };

        fn display() -> String {
            "bitwise-loop".to_string()
        }

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

    /// A [`NodePowerMethod`] utilizing the most significant set bit, without use of a loop.
    ///
    /// # Note
    ///
    /// This method only works for `K` being a power of 2.
    #[derive(Debug, Clone, Copy)]
    pub struct MostSignificantSetBit;

    impl<const K: usize> NodePowerMethod<K> for MostSignificantSetBit {
        const MAX_N: usize = {
            assert!(K > 1);
            assert!(K.count_ones() == 1, "K has to be a power of 2");

            1 << (usize::BITS / 2 - 1)
        };

        fn display() -> String {
            "most-significant-set-bit".to_string()
        }

        fn node_power(n: usize, run_a: super::Run, run_b: super::Run) -> usize {
            assert!(n <= <Self as NodePowerMethod<K>>::MAX_N);

            const HALF_MASK: usize = usize::MAX >> (usize::BITS / 2);
            const HALF_BITS: u32 = usize::BITS / 2;

            let factor: usize = K.trailing_zeros().try_into().unwrap();

            let l2 = run_a.start + run_a.end;
            let r2 = run_b.start + run_b.end;

            let a = ((l2 << (HALF_BITS - 2)) / n) & HALF_MASK;
            let b = ((r2 << (HALF_BITS - 2)) / n) & HALF_MASK;

            (usize::try_from((a ^ b).leading_zeros() - usize::BITS / 2).unwrap() - 1) / factor + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    const RUNS: usize = crate::test::DEFAULT_TEST_RUNS;
    const TEST_SIZE: usize = crate::test::DEFAULT_TEST_SIZE;

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

    mod two_way {
        use crate::generate_test_suite;

        generate_test_suite! {
            TEST_SIZE: super::TEST_SIZE;
            TEST_RUNS: super::RUNS;

            super::PowerSortTrivial,
            super::PowerSortTrivialPowerIndexedStack,
        }
    }

    mod multi_way {
        use crate::generate_test_suite;

        generate_test_suite! {
            TEST_SIZE: super::TEST_SIZE;
            TEST_RUNS: super::RUNS;

            super::PowerSortTrivialMulti4,
            super::PowerSortTrivialMulti8,
        }
    }

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

//! Contains various structs and functions intended for testing purposes.

use rand::{Rng as _, SeedableRng as _, seq::SliceRandom as _};

/// The default test size to use.
pub const DEFAULT_TEST_SIZE: usize = 10_000;
/// The default runs to use.
pub const DEFAULT_TEST_RUNS: usize = 100;

/// The seed shared by all tests.
pub const TEST_SEED: u64 = 0xa8bf17eb656f828d;
/// The RNG used by each test.
pub type Rng = rand::rngs::SmallRng;

/// Generates the `Rng` for a test.
pub fn test_rng() -> Rng {
    Rng::seed_from_u64(TEST_SEED)
}

/// A unit struct that returns a random ordering when compared.
///
/// Intended to simulate a badly behaving [`Ord`] implementation.
#[derive(Debug, Clone)]
pub struct RandomOrdered(std::rc::Rc<std::cell::RefCell<rand::rngs::SmallRng>>);

impl RandomOrdered {
    /// Creates a new endless [`Iterator`] of RandomOrdered, created with a shared.
    /// [`rand::rngs::SmallRng`].
    pub fn new_iter(seed: u64) -> impl Iterator<Item = Self> {
        let rng = std::rc::Rc::new(std::cell::RefCell::new(
            rand::rngs::SmallRng::seed_from_u64(seed),
        ));

        std::iter::repeat_with(move || RandomOrdered(rng.clone()))
    }
}

// The following implementations are intentionally 'bad' (see RandomOrdered)

impl PartialEq for RandomOrdered {
    fn eq(&self, _other: &Self) -> bool {
        self.0.borrow_mut().random()
    }
}

impl Eq for RandomOrdered {}

impl PartialOrd for RandomOrdered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RandomOrdered {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        match rand::random_range(0..3) {
            0 => std::cmp::Ordering::Less,
            1 => std::cmp::Ordering::Equal,
            2 => std::cmp::Ordering::Greater,
            _ => unreachable!(),
        }
    }
}

/// A Wrapper that panics with the likelihood of `1 / LIKELIHOOD` when being compared.
///
/// Intended to check for undefined behavior when panicking occurs during merging.
#[derive(Debug, Clone)]
pub struct MaybePanickingOrdered<const LIKELIHOOD: usize, T: Ord>(
    std::rc::Rc<std::cell::RefCell<rand::rngs::SmallRng>>,
    T,
);

impl<const LIKELIHOOD: usize, T: Ord> MaybePanickingOrdered<LIKELIHOOD, T> {
    /// Maps an [`Iterator`] of `T` to `Self` with a shared [`rand::rngs::SmallRng`].
    pub fn map_iter(iter: impl Iterator<Item = T>, seed: u64) -> impl Iterator<Item = Self> {
        let rng = std::rc::Rc::new(std::cell::RefCell::new(
            rand::rngs::SmallRng::seed_from_u64(seed),
        ));

        iter.map(move |element| Self(rng.clone(), element))
    }

    /// Consumes the wrapper and returns the inner `T`.
    pub fn into_inner(self) -> T {
        self.1
    }
}

// The following implementations are intentionally 'bad' (see RandomOrdered)

impl<const LIKELIHOOD: usize, T: Ord> PartialEq for MaybePanickingOrdered<LIKELIHOOD, T> {
    fn eq(&self, other: &Self) -> bool {
        match self.0.borrow_mut().random_range(0..LIKELIHOOD) {
            0 => panic!("MaybePanickingOrdered panicked during comparison"),
            _ => self.1.eq(&other.1),
        }
    }
}

impl<const LIKELIHOOD: usize, T: Ord> Eq for MaybePanickingOrdered<LIKELIHOOD, T> {}

impl<const LIKELIHOOD: usize, T: Ord> PartialOrd for MaybePanickingOrdered<LIKELIHOOD, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const LIKELIHOOD: usize, T: Ord> Ord for MaybePanickingOrdered<LIKELIHOOD, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.0.borrow_mut().random_range(0..LIKELIHOOD) {
            0 => panic!("MaybePanickingOrdered panicked during comparison"),
            _ => self.1.cmp(&other.1),
        }
    }
}

/// A Wrapper struct that tracks an original index with an ordered element.
///
/// Intended to test sort results for stability.
///
/// When compared, the call is intentionally forwarded to the implementation of `T`.
/// To check for stable sorting, see [`Self::is_stable_sorted()`]
#[derive(Debug, Clone)]
pub struct IndexedOrdered<T: Ord>(usize, T);

impl<T: Ord> IndexedOrdered<T> {
    /// Creates a new iterator of `IndexedOrdered`, tracking the position of each element in `iter`.
    pub fn map_iter(iter: impl Iterator<Item = T>) -> impl Iterator<Item = Self> {
        iter.enumerate()
            .map(|(index, element)| Self(index, element))
    }

    /// Checks that `iter` is sorted and check for stability, e.g. equal elements keeping their
    /// initial relative ordering.
    ///
    /// Returns `Ok(result)` if `iter` is sorted with regards to `T` where `result` indicates if
    /// the sort is stable. Otherwise, returns `Err(())` if `iter` was not sorted with regards to
    /// `T`.
    pub fn is_stable_sorted<'a>(mut iter: impl Iterator<Item = &'a Self>) -> Result<bool, ()>
    where
        T: 'a,
    {
        let Some(mut previous) = iter.next() else {
            return Ok(true);
        };

        for current in iter {
            match current.cmp(previous) {
                // Slice is not sorted
                std::cmp::Ordering::Less => return Err(()),
                // Elements are not stable
                std::cmp::Ordering::Equal if current.0 < previous.0 => return Ok(false),
                _ => {}
            }

            previous = current;
        }

        Ok(true)
    }
}

impl<T: Ord> PartialEq for IndexedOrdered<T> {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl<T: Ord> Eq for IndexedOrdered<T> {}

impl<T: Ord> PartialOrd for IndexedOrdered<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord> Ord for IndexedOrdered<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.1.cmp(&other.1)
    }
}

/// Generates a sequence of random test functions, to test a [`crate::algorithms::Sort`].
///
/// # Example usage
///
/// ```
/// #[cfg(test)]
/// mod tets {
///     generate_test_suite! {
///         TEST_SIZE: $size:expr;
///         TEST_RUNS: $runs:expr;
///
///         SortToTest,
///         AnotherSortToTest,
///         // ...
///     }
/// }
/// ```
#[macro_export]
macro_rules! generate_test_suite {
    (
        TEST_SIZE: $size:expr;
        TEST_RUNS: $runs:expr;

        $(
            $algorithm:ty
        ),+
        $(,)?
    ) => {
        const TEST_SIZE: usize = $size;
        const TEST_RUNS: usize = $runs;

        #[test]
        fn test_empty() {
            $(
                $crate::test::test_empty::<$algorithm>();
            )+
        }

        #[test]
        fn test_random_sorted() {
            $(
                $crate::test::test_random_sorted::<TEST_RUNS, TEST_SIZE, $algorithm>();
            )+
        }

        #[test]
        fn test_random_stable_sorted() {
            $(
                $crate::test::test_random_stable_sorted::<TEST_RUNS, TEST_SIZE, $algorithm>();
            )+
        }
    };
}

/// Tests the sort on an empty slice.
pub fn test_empty<S: crate::algorithms::Sort>() {
    S::sort::<usize>(&mut []);
}

/// Tests the sort on some random ordered slices and check they are sorted afterwords.
pub fn test_random_sorted<const RUNS: usize, const TEST_SIZE: usize, S: crate::algorithms::Sort>() {
    let mut rng = test_rng();

    // Random permutations
    let permutation_values: Box<[usize]> = (0..TEST_SIZE).collect();
    // Random permutations with repeat elements
    let repeat_permutation_values: Box<[usize]> =
        std::iter::repeat_n(0..TEST_SIZE / 4, 4).flatten().collect();

    for mut values in [permutation_values, repeat_permutation_values] {
        // Check slices of size TEST_SIZE
        for run in 0..RUNS {
            values.shuffle(&mut rng);

            S::sort(&mut values);

            assert!(values.is_sorted(), "Run {run} was not sorted");
        }

        // Check smaller slices
        for run in 0..RUNS {
            let values = &mut values[..rng.random_range(0..TEST_SIZE)];
            values.shuffle(&mut rng);

            S::sort(values);

            assert!(values.is_sorted(), "Run {run} was not sorted");
        }
    }
}

/// Like [`test_random_sorted`] but additionally checks that the sort was stable or unstable
/// depending on [`S::IS_STABLE`](crate::algorithms::Sort::IS_STABLE).
pub fn test_random_stable_sorted<
    const RUNS: usize,
    const TEST_SIZE: usize,
    S: crate::algorithms::Sort,
>() {
    let mut rng = test_rng();

    // Random permutations with repeat elements
    let mut values: Box<[usize]> = std::iter::repeat_n(0..TEST_SIZE / 4, 4).flatten().collect();
    let mut ordered_values: Box<[IndexedOrdered<usize>]>;

    for run in 0..RUNS {
        values.shuffle(&mut rng);
        ordered_values = IndexedOrdered::map_iter(values.iter().copied()).collect();
        S::sort(&mut ordered_values);

        match IndexedOrdered::is_stable_sorted(ordered_values.iter()) {
            Ok(false) if !S::IS_STABLE => return, // Correctly determined that `S` is not stable
            Ok(stable) => assert!(stable, "Elements in {run} were not sorted stable"),
            Err(()) => panic!("Elements in run {run} were not sorted at all"),
        }
    }

    for run in 0..RUNS {
        let values = &mut values[..rng.random_range(0..TEST_SIZE)];
        values.shuffle(&mut rng);
        ordered_values = IndexedOrdered::map_iter(values.iter().copied()).collect();
        S::sort(&mut ordered_values);

        match IndexedOrdered::is_stable_sorted(ordered_values.iter()) {
            Ok(false) if !S::IS_STABLE => return, // Correctly determined that `S` is not stable
            Ok(stable) => assert!(stable, "Elements in {run} were not sorted stable"),
            Err(()) => panic!("Elements in run {run} were not sorted at all"),
        }
    }

    assert!(
        S::IS_STABLE,
        "Sort should be stable otherwise this test should return earlier"
    );
}

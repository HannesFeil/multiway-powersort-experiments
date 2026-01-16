//! Contains various structs intended for testing purposes

use rand::{Rng as _, SeedableRng as _, seq::SliceRandom as _};

/// The default test size to use
pub const DEFAULT_TEST_SIZE: usize = 10_000;
/// The default runs to use
pub const DEFAULT_RUNS: usize = 100;

/// The seed shared by all tests
pub const TEST_SEED: u64 = 0xa8bf17eb656f828d;
/// The rng used by each test
pub type Rng = rand::rngs::SmallRng;

/// Generate the `Rng` for a test
pub fn test_rng() -> Rng {
    Rng::seed_from_u64(TEST_SEED)
}

/// A unit struct that returns a random ordering when compared
#[derive(Debug, Clone)]
pub struct RandomOrdered(std::rc::Rc<std::cell::RefCell<rand::rngs::SmallRng>>);

impl RandomOrdered {
    /// Create a new [`Iterator`] of RandomOrdered, created with a shared [`rand::rngs::SmallRng`]
    pub fn new_iter(seed: u64) -> impl Iterator<Item = Self> {
        let rng = std::rc::Rc::new(std::cell::RefCell::new(
            rand::rngs::SmallRng::seed_from_u64(seed),
        ));

        std::iter::repeat_with(move || RandomOrdered(rng.clone()))
    }
}

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

/// A struct that panicks with the likelihood of `1 / LIKELIHOOD` when compared.
#[derive(Debug, Clone)]
pub struct MaybePanickingOrdered<const LIKELIHOOD: usize, T: Ord>(
    std::rc::Rc<std::cell::RefCell<rand::rngs::SmallRng>>,
    T,
);

impl<const LIKELIHOOD: usize, T: Ord> MaybePanickingOrdered<LIKELIHOOD, T> {
    /// Map an [`Iterator`] of `T` to `Self` with a shared [`rand::rngs::SmallRng`]
    pub fn map_iter(iter: impl Iterator<Item = T>, seed: u64) -> impl Iterator<Item = Self> {
        let rng = std::rc::Rc::new(std::cell::RefCell::new(
            rand::rngs::SmallRng::seed_from_u64(seed),
        ));

        iter.map(move |element| Self(rng.clone(), element))
    }
}

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

/// A Wrapper struct that tracks an original index with an ordered element,
/// used to test sort results for stability
#[derive(Debug, Clone)]
pub struct IndexedOrdered<T: Ord>(usize, T);

impl<T: Ord> IndexedOrdered<T> {
    /// Create a new iterator of `IndexedOrdered`, tracking the position of each element in `iter`
    pub fn map_iter(iter: impl Iterator<Item = T>) -> impl Iterator<Item = Self> {
        iter.enumerate()
            .map(|(index, element)| Self(index, element))
    }

    /// Check `slice` is sorted and check for stability, e.g. equal elements keeping initial ordering.
    pub fn is_stable_sorted(slice: &[Self]) -> bool {
        if slice.len() < 2 {
            return true;
        }

        let mut previous = &slice[0];
        for current in slice[1..].iter() {
            match current.cmp(previous) {
                // Slice is not sorted
                std::cmp::Ordering::Less => return false,
                // Elements are not stable
                std::cmp::Ordering::Equal if current.0 < previous.0 => return false,
                _ => {}
            }

            previous = current;
        }

        true
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

/// Test the sort on an empty slice
pub fn test_empty<S: crate::algorithms::Sort>() {
    S::sort::<usize>(&mut []);
}

/// Test the sort on some random ordered slices and check they are sorted afterwords
pub fn test_random_sorted<const RUNS: usize, const TEST_SIZE: usize, S: crate::algorithms::Sort>() {
    let mut rng = test_rng();

    let mut values: Box<[usize]> = (0..TEST_SIZE).collect();

    for run in 0..RUNS {
        values.shuffle(&mut rng);
        S::sort(&mut values);
        assert!(values.is_sorted(), "Run {run} was not sorted");
    }

    let mut values: Box<[usize]> = std::iter::repeat_n(0..TEST_SIZE / 4, 4).flatten().collect();
    for run in 0..RUNS {
        values.shuffle(&mut rng);
        S::sort(&mut values);
        assert!(values.is_sorted(), "Run {run} was not sorted");
    }
}

/// Like [`test_random_sorted`] but additionally checks that the sort was stable
pub fn test_random_stable_sorted<
    const RUNS: usize,
    const TEST_SIZE: usize,
    S: crate::algorithms::Sort,
>() {
    assert!(S::IS_STABLE);

    let mut rng = test_rng();
    let mut values: Box<[usize]> = std::iter::repeat_n(0..TEST_SIZE / 4, 4).flatten().collect();
    let mut ordered_values: Box<[IndexedOrdered<usize>]>;

    for run in 0..RUNS {
        values.shuffle(&mut rng);
        ordered_values = IndexedOrdered::map_iter(values.iter().copied()).collect();
        S::sort(&mut ordered_values);
        assert!(
            IndexedOrdered::is_stable_sorted(&ordered_values),
            "Run {run} was not stable sorted"
        );
    }
}

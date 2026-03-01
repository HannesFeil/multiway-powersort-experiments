//! Contains various structs used to measure differences when being sorted

use rand::{distr::Distribution as _, seq::SliceRandom};

/// Used to define ways to compare [`Blobs`](Blob)
pub trait BlobComparisonMethod<T: Ord, const N: usize>: std::fmt::Debug {
    /// Compares the data of two [`Blobs`](Blob)
    fn compare(a: &[T; N], b: &[T; N]) -> std::cmp::Ordering;
}

/// Compares two [`Blobs`](Blob) by comparing only their first entry
#[derive(Debug, Clone, Copy)]
pub struct CompareFirstEntry;

impl<T: Ord, const N: usize> BlobComparisonMethod<T, N> for CompareFirstEntry {
    fn compare(a: &[T; N], b: &[T; N]) -> std::cmp::Ordering {
        a.first().cmp(&b.first())
    }
}

/// Compares two [`Blobs`](Blob) by comparing them in lexicographical order, e.g. comparing their
/// first element and falling back to later elements on equality.
#[derive(Debug, Clone, Copy)]
pub struct CompareLexicographical;

impl<T: Ord, const N: usize> BlobComparisonMethod<T, N> for CompareLexicographical {
    fn compare(a: &[T; N], b: &[T; N]) -> std::cmp::Ordering {
        for (a, b) in a.iter().zip(b.iter()) {
            match a.cmp(b) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }

        std::cmp::Ordering::Equal
    }
}

/// Compares two [`Blobs`](Blob) by calculating and then comparing their hashes.
///
/// # Note
///
/// This method only works for [`Blobs`](Blob) of `u32`.
#[derive(Debug, Clone, Copy)]
pub struct CompareHash;

impl<const N: usize> BlobComparisonMethod<u32, N> for CompareHash {
    fn compare(a: &[u32; N], b: &[u32; N]) -> std::cmp::Ordering {
        fn hash<const N: usize>(blob: &[u32; N]) -> u32 {
            const P: u32 = 2147483659;
            const A: u32 = 3952532;
            const B: u32 = 23895293;

            blob.iter().map(|i| (A * i + B) % P).sum()
        }

        hash(a).cmp(&hash(b))
    }
}

/// A data blob, being a small wrapper over and array along with a [`BlobComparisonMethod`].
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct Blob<T: Ord, C: BlobComparisonMethod<T, N>, const N: usize>(
    [T; N],
    std::marker::PhantomData<C>,
);

/// Prime numbers used for the creation of blobs (directly taken from the C++ implementation)
const BLOB_PRIMES: [u32; 64] = [
    1073741827, 1073741831, 1073741833, 1073741839, 1073741843, 1073741857, 1073741891, 1073741909,
    1073741939, 1073741953, 1073741969, 1073741971, 1073741987, 1073741993, 1073742037, 1073742053,
    1073742073, 1073742077, 1073742091, 1073742113, 1073742169, 1073742203, 1073742209, 1073742223,
    1073742233, 1073742259, 1073742277, 1073742289, 1073742343, 1073742353, 1073742361, 1073742391,
    1073742403, 1073742463, 1073742493, 1073742517, 1073742583, 1073742623, 1073742653, 1073742667,
    1073742671, 1073742673, 1073742707, 1073742713, 1073742721, 1073742731, 1073742767, 1073742773,
    1073742811, 1073742851, 1073742853, 1073742881, 1073742889, 1073742913, 1073742931, 1073742937,
    1073742959, 1073742983, 1073743007, 1073743037, 1073743049, 1073743051, 1073743079, 1073743091,
];

impl<T: Ord + From<u32>, C: BlobComparisonMethod<T, N>, const N: usize> From<u32>
    for Blob<T, C, N>
{
    fn from(value: u32) -> Self {
        assert!(N <= 64, "Cannot create blobs with size greater than 64");

        Blob(
            std::array::from_fn(|i| (value % BLOB_PRIMES[i]).into()),
            std::marker::PhantomData,
        )
    }
}

impl<T: Ord + TryFrom<usize>, C: BlobComparisonMethod<T, N>, const N: usize> TryFrom<usize>
    for Blob<T, C, N>
{
    type Error = T::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        assert!(N <= 64, "Cannot create blobs with size greater than 64");

        let mut elements: Vec<T> = vec![];
        for prime in BLOB_PRIMES.iter().copied().take(N) {
            elements.push(
                (value % usize::try_from(prime).expect("u32 should fit in a usize")).try_into()?,
            )
        }

        Ok(Blob(
            std::array::from_fn(|_| elements.remove(0)),
            std::marker::PhantomData,
        ))
    }
}

impl<T: Ord, C: BlobComparisonMethod<T, N>, const N: usize> PartialEq for Blob<T, C, N> {
    fn eq(&self, other: &Self) -> bool {
        C::compare(&self.0, &other.0).is_eq()
    }
}

impl<T: Ord, C: BlobComparisonMethod<T, N>, const N: usize> Eq for Blob<T, C, N> {}

impl<T: Ord, C: BlobComparisonMethod<T, N>, const N: usize> PartialOrd for Blob<T, C, N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord, C: BlobComparisonMethod<T, N>, const N: usize> Ord for Blob<T, C, N> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        C::compare(&self.0, &other.0)
    }
}

/// A simple wrapper around an atomic u64, used to keep track of various metrics during sorts.
///
/// See [`crate::GLOBAL_COUNTERS`].
#[derive(Debug)]
pub struct GlobalCounter(std::sync::atomic::AtomicU64);

impl GlobalCounter {
    /// Constructs a new global counter with initial value `0`
    pub const fn new() -> Self {
        Self(std::sync::atomic::AtomicU64::new(0))
    }

    /// Increases the counter by `amount`.
    pub fn increase(&self, amount: u64) {
        self.0
            .fetch_add(amount, std::sync::atomic::Ordering::Relaxed);
    }

    /// Returns the current value of the counter and resets it to `0`
    pub fn read_and_reset(&self) -> u64 {
        self.0.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

/// A generic wrapper around a comparable elements, that tracks the number of times the element
/// has been compared.
///
/// # Note
///
/// All comparisons are tracked together in the single counter `crate::GLOBAL_COUNTERS.comparisons`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct CountComparisons<T>(T);

impl<T> CountComparisons<T> {
    /// Increases the comparison counter by `amount`
    fn increase_counter(amount: u64) {
        crate::GLOBAL_COUNTERS.comparisons.increase(amount);
    }
}

impl<T: PartialEq> PartialEq for CountComparisons<T> {
    fn eq(&self, other: &Self) -> bool {
        Self::increase_counter(1);

        self.0 == other.0
    }
}

impl<T: Eq> Eq for CountComparisons<T> {}

impl<T: PartialOrd> PartialOrd for CountComparisons<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Self::increase_counter(1);

        self.0.partial_cmp(&other.0)
    }
}

impl<T: Ord> Ord for CountComparisons<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Self::increase_counter(1);

        self.0.cmp(&other.0)
    }
}

impl<T: TryFrom<usize>> TryFrom<usize> for CountComparisons<T> {
    type Error = T::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        T::try_from(value).map(Self)
    }
}

/// A random permutation data distribution
#[derive(Debug, Clone, Copy, Default)]
pub struct PermutationData;

/// A permutation with random runs of a certain expected length
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomRunsData(usize);

/// A permutation with random runs of expected length `n.isqrt()`
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomRunsSqrtData;

/// A permutation with random runs of expected and constant length `LENGTH`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomRunsConstData<const LENGTH: usize>;

/// Used to generate the data to be sorted.
pub trait DataGenerator<T: Ord + std::fmt::Debug>: Default {
    /// Initialize a vector of the given size
    fn initialize(&mut self, size: usize, rng: &mut impl rand::Rng) -> Vec<T>;

    /// Reinitialize the given slice of elements
    fn reinitialize(&mut self, slice: &mut [T], rng: &mut impl rand::Rng);
}

impl<T> DataGenerator<T> for PermutationData
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(&mut self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        let mut values: Vec<_> = (0..size).map(|i| T::try_from(i).unwrap()).collect();

        self.reinitialize(&mut values, rng);

        values
    }

    fn reinitialize(&mut self, slice: &mut [T], rng: &mut impl rand::Rng) {
        slice.shuffle(rng);
    }
}

impl<T> DataGenerator<T> for RandomRunsData
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(&mut self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        let mut values = PermutationData.initialize(size, rng);

        self.reinitialize(&mut values, rng);

        values
    }

    fn reinitialize(&mut self, slice: &mut [T], rng: &mut impl rand::Rng) {
        PermutationData.reinitialize(slice, rng);

        #[expect(
            clippy::as_conversions,
            reason = "length should be small enough so precision errors should not be a concern"
        )]
        let geometric = rand_distr::Geometric::new(1.0 / self.0 as f64).unwrap();

        let mut start = 0;
        while start < slice.len() {
            let len = std::cmp::min(
                geometric.sample(rng).try_into().unwrap_or(usize::MAX),
                slice.len() - start,
            );
            slice[start..start + len].sort();
            start += len;
        }
    }
}

impl<T> DataGenerator<T> for RandomRunsSqrtData
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(&mut self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        RandomRunsData(size.isqrt()).initialize(size, rng)
    }

    fn reinitialize(&mut self, slice: &mut [T], rng: &mut impl rand::Rng) {
        RandomRunsData(slice.len().isqrt()).reinitialize(slice, rng);
    }
}

impl<T, const LENGTH: usize> DataGenerator<T> for RandomRunsConstData<LENGTH>
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(&mut self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        RandomRunsData(LENGTH).initialize(size, rng)
    }

    fn reinitialize(&mut self, slice: &mut [T], rng: &mut impl rand::Rng) {
        RandomRunsData(LENGTH).reinitialize(slice, rng);
    }
}

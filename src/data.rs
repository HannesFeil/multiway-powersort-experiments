//! Contains various structs used to measure differences and memory effects when being sorted

use rand::{distr::Distribution as _, seq::SliceRandom};

pub trait BlobComparisonMethod<const N: usize> {
    fn compare(a: &[u32; N], b: &[u32; N]) -> std::cmp::Ordering;
}

#[derive(Debug, Clone, Copy)]
pub struct CompareFirstEntry;

impl<const N: usize> BlobComparisonMethod<N> for CompareFirstEntry {
    fn compare(a: &[u32; N], b: &[u32; N]) -> std::cmp::Ordering {
        a.first().cmp(&b.first())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompareLexicographically;

impl<const N: usize> BlobComparisonMethod<N> for CompareLexicographically {
    fn compare(a: &[u32; N], b: &[u32; N]) -> std::cmp::Ordering {
        for (a, b) in a.iter().zip(b.iter()) {
            match a.cmp(b) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }

        std::cmp::Ordering::Equal
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompareHash;

impl<const N: usize> BlobComparisonMethod<N> for CompareHash {
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

#[derive(Debug, Clone)]
pub struct Blob<C: BlobComparisonMethod<N>, const N: usize>([u32; N], std::marker::PhantomData<C>);

impl<C: BlobComparisonMethod<N>, const N: usize> From<u32> for Blob<C, N> {
    fn from(value: u32) -> Self {
        const PRIMES: [u32; 64] = [
            1073741827, 1073741831, 1073741833, 1073741839, 1073741843, 1073741857, 1073741891,
            1073741909, 1073741939, 1073741953, 1073741969, 1073741971, 1073741987, 1073741993,
            1073742037, 1073742053, 1073742073, 1073742077, 1073742091, 1073742113, 1073742169,
            1073742203, 1073742209, 1073742223, 1073742233, 1073742259, 1073742277, 1073742289,
            1073742343, 1073742353, 1073742361, 1073742391, 1073742403, 1073742463, 1073742493,
            1073742517, 1073742583, 1073742623, 1073742653, 1073742667, 1073742671, 1073742673,
            1073742707, 1073742713, 1073742721, 1073742731, 1073742767, 1073742773, 1073742811,
            1073742851, 1073742853, 1073742881, 1073742889, 1073742913, 1073742931, 1073742937,
            1073742959, 1073742983, 1073743007, 1073743037, 1073743049, 1073743051, 1073743079,
            1073743091,
        ];
        assert!(N <= 64, "Cannot create blobs with size greater than 64");

        Blob(
            std::array::from_fn(|i| value % PRIMES[i]),
            std::marker::PhantomData,
        )
    }
}

impl<C: BlobComparisonMethod<N>, const N: usize> PartialEq for Blob<C, N> {
    fn eq(&self, other: &Self) -> bool {
        C::compare(&self.0, &other.0).is_eq()
    }
}

impl<C: BlobComparisonMethod<N>, const N: usize> Eq for Blob<C, N> {}

impl<C: BlobComparisonMethod<N>, const N: usize> PartialOrd for Blob<C, N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<C: BlobComparisonMethod<N>, const N: usize> Ord for Blob<C, N> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        C::compare(&self.0, &other.0)
    }
}

static COMPARISON_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct CountComparisons<T>(T);

impl<T> CountComparisons<T> {
    fn increase_counter(amount: u64) {
        COMPARISON_COUNTER.fetch_add(amount, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn read_and_reset_counter() -> u64 {
        COMPARISON_COUNTER.swap(0, std::sync::atomic::Ordering::Relaxed)
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

impl<T: rand::distr::uniform::SampleUniform> rand::distr::uniform::SampleUniform
    for CountComparisons<T>
{
    type Sampler = CountComparisonsSampler<T::Sampler>;
}

pub struct CountComparisonsSampler<T>(T);

impl<T: rand::distr::uniform::UniformSampler> rand::distr::uniform::UniformSampler
    for CountComparisonsSampler<T>
where
    T::X: rand::distr::uniform::SampleUniform,
{
    type X = CountComparisons<T::X>;

    fn new<B1, B2>(low: B1, high: B2) -> Result<Self, rand::distr::uniform::Error>
    where
        B1: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        T::new(&low.borrow().0, &high.borrow().0).map(Self)
    }

    fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, rand::distr::uniform::Error>
    where
        B1: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        T::new_inclusive(&low.borrow().0, &high.borrow().0).map(Self)
    }

    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        CountComparisons(self.0.sample(rng))
    }
}

impl<T: Extremes> Extremes for CountComparisons<T> {
    const MIN: Self = Self(T::MIN);

    const MAX: Self = Self(T::MAX);
}

trait Extremes {
    const MIN: Self;
    const MAX: Self;
}

/// A uniform data distribution
#[derive(Debug, Clone, Copy, Default)]
pub struct UniformData;

/// A random permutation data distribution
#[derive(Debug, Clone, Copy, Default)]
pub struct PermutationData;

/// A permutation with random runs
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomRunsData(usize);

/// A permutation with random runs
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomRunsSqrtData;

/// A trait for generalizing sorting data creation
pub trait Data<T: Sized + Ord + std::fmt::Debug>: Default {
    /// Initialize a vector of the given size
    fn initialize(self, size: usize, rng: &mut impl rand::Rng) -> Vec<T>;
}

impl<T> Data<T> for UniformData
where
    T: Ord + Extremes + rand::distr::uniform::SampleUniform + std::fmt::Debug,
{
    fn initialize(self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        rand::distr::Uniform::new(T::MIN, T::MAX)
            .unwrap()
            .sample_iter(rng)
            .take(size)
            .collect()
    }
}

impl<T> Data<T> for PermutationData
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        let mut values: Vec<_> = (0..size).map(|i| T::try_from(i).unwrap()).collect();
        values.shuffle(rng);
        values
    }
}

impl<T> Data<T> for RandomRunsData
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        let mut values = PermutationData.initialize(size, rng);
        let geometric = rand_distr::Geometric::new(1.0 / self.0 as f64).unwrap();

        let mut start = 0;
        while start < values.len() {
            let len = std::cmp::min(geometric.sample(rng) as usize, values.len() - start);
            values[start..start + len].sort();
            start += len;
        }

        values
    }
}

impl<T> Data<T> for RandomRunsSqrtData
where
    T: Ord + TryFrom<usize> + std::fmt::Debug,
    <T as TryFrom<usize>>::Error: std::fmt::Debug,
{
    fn initialize(self, size: usize, rng: &mut impl rand::Rng) -> Vec<T> {
        RandomRunsData(size.isqrt()).initialize(size, rng)
    }
}

/// Implement distribution data for the given integer types
macro_rules! impl_for_integers {
    ($($type:ty),*) => {
        $(
            impl_for_integers!(@single $type);
        )*
    };
    (@single $type:ty) => {
        impl Extremes for $type {
            const MIN: Self = Self::MIN;
            const MAX: Self = Self::MAX;
        }
    }
}

// Implement the Data trait for the default integer types
impl_for_integers!(u8, u16, u32, u64, u128);

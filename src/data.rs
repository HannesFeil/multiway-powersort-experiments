//! Contains various structs used to measure differences and memory effects when being sorted

use rand::distr::Distribution as _;

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

/// A uniform data distribution set
#[derive(Debug)]
pub struct UniformData<T>(std::marker::PhantomData<T>);

/// A random permutation data distribution
#[derive(Debug)]
pub struct PermutationData<T>(std::marker::PhantomData<T>);

/// A trait for generalizing sorting data creation
pub trait Data<T: Sized + Ord + std::fmt::Debug> {
    /// Initialize a vector of the given size
    fn initialize(size: usize, rng: &mut impl rand::Rng) -> Vec<T>;
}

/// Implement distribution data for the given integer types
macro_rules! impl_for_integers {
    ($($type:ty),*) => {
        $(
            impl_for_integers!(@single $type);
        )*
    };
    (@single $type:ty) => {
        impl Data<$type> for UniformData<$type> {
            fn initialize(size: usize, rng: &mut impl rand::Rng) -> Vec<$type> {
                rand::distr::Uniform::new(<$type>::MIN, <$type>::MAX)
                    .unwrap()
                    .sample_iter(rng)
                    .take(size)
                    .collect()
            }
        }

        impl Data<$type> for PermutationData<$type> {
            fn initialize(size: usize, rng: &mut impl rand::Rng) -> Vec<$type> {
                use rand::seq::SliceRandom as _;

                let mut result: Vec<$type> = (0..size as $type).collect();
                result.shuffle(rng);

                result
            }
        }
    }
}

// Implement the Data trait for the default integer types
impl_for_integers!(u8, u16, u32, u64, u128);

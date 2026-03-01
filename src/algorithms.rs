//! Contains various sorting algorithms see e.g. [`Sort`] and [`Algorithm`].

pub mod insertionsort;
pub mod mergesort;
pub mod merging;
pub mod peeksort;
pub mod powersort;
pub mod quicksort;
pub mod timsort;

/// A sorting algorithm
pub trait Sort {
    /// Whether [`Self::sort`] preserves the order of equal elements
    const IS_STABLE: bool;

    /// The base algorithm name
    const BASE_NAME: &str;

    /// Returns an iterator over the algorithm parameters and their values.
    fn parameters() -> impl Iterator<Item = (&'static str, String)>;

    /// Sorts the given slice.
    fn sort<T: Ord>(slice: &mut [T]);
}

/// A sorting algorithm that takes slices with a prefix partition already sorted
pub trait PostfixSort: Sort {
    /// Sort the given slice under the assumption, that `slice[..split_point]` is already sorted.
    ///
    /// # Invalid Input
    ///
    /// If `slice[..split_point]` is not sorted, this call may or may not panic and the result will
    /// probably not be correctly sorted.
    fn sort_with_sorted_prefix<T: Ord>(slice: &mut [T], split_point: usize);
}

/// The Standard library sort
pub struct StdSort<const STABLE: bool = true>;

impl<const STABLE: bool> Sort for StdSort<STABLE> {
    const IS_STABLE: bool = STABLE;

    const BASE_NAME: &str = "std";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![("stable", STABLE.to_string())].into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        if STABLE {
            <[T]>::sort(slice);
        } else {
            <[T]>::sort_unstable(slice);
        }
    }
}

/// A trait to parameterize random number generation
pub trait RandomFactory {
    /// The [`rand::Rng`] type produced by this factory
    type Rng: rand::Rng;

    /// Produces a new [`Self::Rng`].
    fn produce() -> Self::Rng;
}

/// A factory producing the default [`rand::Rng`]
pub struct DefaultRngFactory;

impl RandomFactory for DefaultRngFactory {
    type Rng = rand::rngs::ThreadRng;

    fn produce() -> Self::Rng {
        rand::rng()
    }
}

/// A trait to parameterize the creation of [`BufGuards`](merging::BufGuard).
///
/// This trait serves as a type level function to get a type implementing
/// [`Bufguard<T>`](merging::Bufguard) for a given type `T`.
pub trait BufGuardFactory {
    /// The associated guard type
    type Guard<T>: merging::BufGuard<T>;
}

/// The [`BufGuardFactory`] producing `Vec<T>` types
pub struct DefaultBufGuardFactory;

impl BufGuardFactory for DefaultBufGuardFactory {
    type Guard<T> = Vec<T>;
}

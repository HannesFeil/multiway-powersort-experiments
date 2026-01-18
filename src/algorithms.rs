//! Contains various sorting algorithms see e.g. [`Sort`] and [`Algorithm`].

pub mod insertionsort;
pub mod mergesort;
pub mod merging;
pub mod peeksort;
pub mod powersort;
pub mod quicksort;
pub mod timsort;

/// Return the multi line display representation of a sort
pub fn display<S: Sort>() -> String {
    format!(
        "{base}\n{parameters}",
        base = S::BASE_NAME,
        parameters = S::parameters()
            .map(|(key, value)| format!("\t{key} = {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Return the inline display representation of a sort
pub fn display_inline<S: Sort>() -> String {
    format!(
        "{base} {parameters}",
        base = S::BASE_NAME,
        parameters = S::parameters()
            .map(|(key, value)| format!("({key} = {value})"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// A trait to simplify the algorithm definitions
pub trait Sort {
    /// Whether [`Self::sort`] preserves the order of equal elements
    const IS_STABLE: bool;

    /// The base algorithm name
    const BASE_NAME: &str;

    /// String representation of the parameters
    fn parameters() -> impl Iterator<Item = (&'static str, String)>;

    /// Sort the given slice
    fn sort<T: Ord>(slice: &mut [T]);
}

/// Defines a Sort that expects slices with a first partition already sorted
pub trait PostfixSort {
    /// Whether [`Self::sort`] preserves the order of equal elements
    const IS_STABLE: bool;

    /// The base algorithm name
    const BASE_NAME: &str;

    /// String representation of the parameters
    fn parameters() -> impl Iterator<Item = (&'static str, String)>;

    /// Sort the given slice under the assumption, that `slice[..split_point]` is already sorted
    fn sort<T: Ord>(slice: &mut [T], split_point: usize);
}

impl<S: PostfixSort> Sort for S {
    const IS_STABLE: bool = Self::IS_STABLE;

    const BASE_NAME: &str = Self::BASE_NAME;

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        Self::parameters()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.is_empty() {
            return;
        }

        Self::sort(slice, 1);
    }
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

/// A trait for modulizing random number generation
pub trait RandomFactory {
    /// The [`rand::Rng`] produced by this factory
    type Rng: rand::Rng;

    /// Produce [`Self::Rng`]
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

/// A trait for modulizing [`merging::BufGuard`]s
pub trait BufGuardFactory {
    /// The corresponding guard type
    type Guard<T>: merging::BufGuard<T>;
}

/// The factory producing `Vec<T>` types
pub struct DefaultBufGuardFactory;

impl BufGuardFactory for DefaultBufGuardFactory {
    type Guard<T> = Vec<T>;
}

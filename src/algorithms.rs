//! Contains various sorting algorithms see e.g. [`Sort`] and [`Algorithm`].

use clap::ValueEnum as _;

use crate::algorithms::merging::BufGuard;

mod insertionsort;
mod merging;
mod peeksort;
mod quicksort;

/// A trait to simplify the algorithm definitions
pub trait Sort {
    /// Whether [`Self::sort`] preserves the order of equal elements
    const IS_STABLE: bool;

    /// Sort the given slice
    fn sort<T: Ord>(slice: &mut [T]);
}

/// The Standard library sort
struct StdSort<const STABLE: bool = true>;

impl<const STABLE: bool> Sort for StdSort<STABLE> {
    const IS_STABLE: bool = STABLE;

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
    type Guard<T>: BufGuard<T>;
}

/// The factory producing `Vec<T>` types
pub struct DefaultBufGuardFactory;

impl BufGuardFactory for DefaultBufGuardFactory {
    type Guard<T> = Vec<T>;
}

/// Define the sorting algorithms available to the user
macro_rules! algorithms {
    (
        $(
            $(
                #[$attr:meta]
            )*
            $name:ident : $sort:ty
        ),*
        $(,)?
    ) => {
        /// The different sorting algorithms
        #[derive(Debug, Clone, Copy, clap::ValueEnum, Hash, PartialEq, Eq)]
        pub enum Algorithm {
            $(
                $(
                    #[$attr]
                )*
                $name,
            )*
        }

        // Delegate each variant to the corresponding Sort type
        impl Algorithm {
            /// The sort function
            pub fn sorter<T: Ord>(self) -> fn(&mut [T]) {
                match self {
                    $(
                        Self::$name => <$sort>::sort,
                    )*
                }
            }

            /// Return whether the sort is stable
            pub fn is_stable(self) -> bool {
                match self {
                    $(
                        Self::$name => <$sort>::IS_STABLE,
                    )*
                }
            }
        }
    };
}

algorithms! {
    /// The algorithm used by the rust std library
    Std: StdSort,
    /// The unstable algorithm used by the rust std library
    StdUnstable: StdSort<false>,
    /// Insertion sort
    Insertion: insertionsort::InsertionSort,
    /// Binary Insertion sort
    BinaryInsertion: insertionsort::InsertionSort<true>,
    /// Quicksort
    Quicksort: quicksort::QuickSort,
    /// Peeksort
    Peeksort: peeksort::PeekSort,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}

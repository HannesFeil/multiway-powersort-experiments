//! Contains various sorting algorithms, unified under [`Algorithm`].

mod insertionsort;
mod merging;
mod peeksort;
mod quicksort;

/// The different sorting algorithms
#[derive(Debug, Clone, Copy, clap::ValueEnum, Hash, PartialEq, Eq)]
pub enum Algorithm {
    /// The algorithm used by the rust std library
    Std,
    /// The unstable algorithm used by the rust std library
    StdUnstable,
    /// Insertion sort
    Insertion,
    /// Binary Insertion sort
    BinaryInsertion,
    /// Quicksort
    Quicksort,
    /// Peeksort
    Peeksort,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Algorithm::Std => "Standard library sort",
            Algorithm::StdUnstable => "Standard library unstable sort",
            Algorithm::Insertion => "Insertion sort",
            Algorithm::BinaryInsertion => "Binary insertion sort",
            Algorithm::Quicksort => "Quicksort",
            Algorithm::Peeksort => "Peeksort",
        })
    }
}

impl Algorithm {
    /// The sort function
    pub fn sorter<T: Ord>(self) -> fn(&mut [T]) {
        match self {
            Algorithm::Std => <[T]>::sort,
            Algorithm::StdUnstable => <[T]>::sort_unstable,
            Algorithm::Insertion => insertionsort::insertion_sort,
            Algorithm::BinaryInsertion => insertionsort::binary_insertion_sort,
            Algorithm::Quicksort => quicksort::default_quicksort,
            Algorithm::Peeksort => peeksort::default_peeksort,
        }
    }

    /// Return whether the sort is stable
    pub fn is_stable(self) -> bool {
        match self {
            Algorithm::Std => true,
            Algorithm::StdUnstable => false,
            Algorithm::Insertion => true,
            Algorithm::BinaryInsertion => true,
            Algorithm::Quicksort => false,
            Algorithm::Peeksort => true,
        }
    }
}

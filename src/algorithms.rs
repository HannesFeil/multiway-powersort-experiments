mod insertionsort;

/// The different sorting algorithms
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Algorithm {
    /// The algorithm used by the rust std library
    Std,
    /// The unstable algorithm used by the rust std library
    StdUnstable,
    /// Insertion sort
    Insertion,
    /// Binary Insertion sort
    BinaryInsertion,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Algorithm::Std => "Standard library sort",
            Algorithm::StdUnstable => "Standard library unstable sort",
            Algorithm::Insertion => "Insertion sort",
            Algorithm::BinaryInsertion => "Binary insertion sort",
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
        }
    }

    /// Return whether the sort is stable
    pub fn is_stable(self) -> bool {
        match self {
            Algorithm::Std => true,
            Algorithm::StdUnstable => false,
            Algorithm::Insertion => true,
            Algorithm::BinaryInsertion => true,
        }
    }
}

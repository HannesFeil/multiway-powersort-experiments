//! Command line input handling

/// Run sorting algorithms on random data and measure their performance
#[derive(clap::Parser)]
#[command(
    version,
    subcommand_value_name = "sort",
    subcommand_help_heading = "Sorts",
    disable_help_subcommand = true
)]
pub struct Args {
    /// The sorting algorithm to run
    #[arg()]
    pub algorithm: Algorithm,
    /// The datatype and distribution to use for sorting
    #[arg(short, long, default_value_t = DataType::RandomRunsSqrtU32)]
    pub data: DataType,
    /// The algorithm variant, use `-v=''` to print available options
    #[arg(short, long, default_value_t = { "default".to_string() })]
    pub variant: String,
    /// The number of runs to do
    #[arg(short, long, default_value_t = 1_000)]
    pub runs: usize,
    /// The size of the data slices to sort
    #[arg(short, long, default_value_t = 1_000_000)]
    pub size: usize,
    /// Seed for the RNG
    #[arg(long)]
    pub seed: Option<u64>,
    /// An optional output file to write the samples to (formatted as CSV)
    pub output: Option<std::path::PathBuf>,
}

/// Returns the multiline string representation of a sorting algorithm.
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

/// Returns the inline string representation of a sorting algorithm.
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

/// A trait for dispatching on data type `T` and distribution type `D`.
pub trait DataTypeDispatcher {
    /// The output of the dispatch.
    type Output;

    /// Dispatches with `T` and `D`.
    fn dispatch<
        T: Ord + std::fmt::Debug,
        D: crate::data::DataGenerator<T>
            + crate::data::DataGenerator<crate::data::CountComparisons<T>>,
    >(
        self,
    ) -> Self::Output;
}

/// A trait for dispatching on [`crate::algorithm::Sort`] type `T`.
pub trait SortDispatcher {
    /// The output of the dispatch.
    type Output;

    /// Dispatches with `T`.
    fn dispatch<T: crate::algorithms::Sort>(self) -> Self::Output;
}

/// Declare the available algorithms and variants.
///
/// # Example usage
///
/// ```rust
/// define_algorithms! {
///     Algorithm {
///         Std => {
///             "default":  StdSort,
///             "unstable": StdSort<false>,
///         },
///         // ...
///     }
/// }
/// ```
macro_rules! define_algorithms {
    (
        $name:ident {
            $(
                $(
                    #[$attr:meta]
                )*
                $top_algorithm:ident => {
                    $(
                        $id:literal: $variant:ty
                    ),*
                    $(,)?
                }
            ),*
            $(,)?
        }
    ) => {
        /// The available top level sorting algorithms
        #[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
        pub enum $name {
            $(
                $(
                    #[$attr]
                )*
                $top_algorithm
            ),*
        }

        impl $name {
            /// Dispatches on the [`crate::algorithm::Sort`] type for `variant`.
            fn dispatch<D: SortDispatcher>(self, variant: &str, dispatcher: D) -> Option<D::Output> {
                match (self, variant) {
                    $(
                        $(
                            ($name::$top_algorithm, $id) => Some(dispatcher.dispatch::<$variant>()),
                        )*
                    )*
                    _ => None,
                }
            }

            fn variants(self) -> &'static [&'static str] {
                match self {
                    $(
                        Self::$top_algorithm => &[
                            $(
                                $id
                            ),*
                        ],
                    )*
                }
            }
        }
    };
}

// Use namespace to reduce declaration verbosity
use crate::algorithms::*;

// Statically declare all available algorithm variants
define_algorithms! {
    Algorithm {
        /// The default sort in [`std`]
        Std => {
            "default": StdSort,
            "unstable": StdSort<false>,
        },
        /// Insertionsort
        Insertionsort => {
            "default": insertionsort::InsertionSort,
            "binary": insertionsort::InsertionSort<true>,
        },
        /// Quicksort
        Quicksort => {
            "default": quicksort::QuickSort,
            "check-sorted": quicksort::QuickSort<
                quicksort::DefaultRngFactory,
                quicksort::DefaultInsertionSort,
                { quicksort::DEFAULT_INSERTION_THRESHOLD },
                { quicksort::DEFAULT_NINTHER_THRESHOLD },
                true,
            >,
        },
        /// Peeksort
        Peeksort => {
            "default": peeksort::PeekSort<
                peeksort::DefaultInsertionSort,
                peeksort::DefaultMergingMethod,
                peeksort::DefaultBufGuardFactory,
                { peeksort::DEFAULT_INSERTION_THRESHOLD },
                false,
            >,
        },
        /// Mergesort
        Mergesort => {
            "default": mergesort::MergeSort,
            "i1": mergesort::MergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                { mergesort::DEFAULT_BOTTOM_UP },
                1,
                false,
            >,
            "i1-check-sorted": mergesort::MergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                { mergesort::DEFAULT_BOTTOM_UP },
                1,
                true,
            >,
            "bottom-up-check-sorted": mergesort::MergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                true,
                { mergesort::DEFAULT_INSERTION_THRESHOLD },
                true,
            >,
        },
        /// Timsort
        Timsort => {
            "default": timsort::TimSort,
            "copy-both": timsort::TimSort<
                timsort::DefaultInsertionSort,
                merging::two_way::CopyBoth,
                timsort::DefaultBufGuardFactory,
                { timsort::DEFAULT_MIN_MERGE },
            >,
            "no-binary-copy-both": timsort::TimSort<
                insertionsort::InsertionSort<false>,
                merging::two_way::CopyBoth,
                timsort::DefaultBufGuardFactory,
                { timsort::DEFAULT_MIN_MERGE },
            >,
        },
        /// Powersort
        Powersort => {
            "default": powersort::PowerSort,
        },
        /// Multiway Powersort
        MultiwayPowersort => {
            "default": powersort::MultiwayPowerSort,
            "specific-4-way-merge": powersort::MultiwayPowerSort<
                powersort::DefaultNodePowerMethod,
                powersort::DefaultInsertionSort,
                merging::multi_way::Fourway,
                powersort::DefaultBufGuardFactory,
                4,
                { powersort::DEFAULT_MIN_RUN_LENGTH },
                { powersort::DEFAULT_ONLY_INCREASING_RUNS },
            >,
        },
    }
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
    }
}

impl Algorithm {
    /// Checks if `variant` is valid.
    pub fn validate(self, variant: &str) -> bool {
        self.variants().contains(&variant)
    }

    /// Returns the available variants and their descriptions.
    pub fn variant_descriptions(self) -> impl Iterator<Item = (&'static str, String)> {
        struct AlgorithmDisplayDispatcher;
        impl SortDispatcher for AlgorithmDisplayDispatcher {
            type Output = String;

            fn dispatch<T: crate::algorithms::Sort>(self) -> Self::Output {
                display::<T>()
            }
        }

        self.variants().iter().map(move |variant| {
            (
                *variant,
                self.dispatch(variant, AlgorithmDisplayDispatcher).unwrap(),
            )
        })
    }

    /// Returns whether the given `variant` is stable.
    pub fn is_stable(self, variant: &str) -> Option<bool> {
        struct AlgorithmStableDispatcher;
        impl SortDispatcher for AlgorithmStableDispatcher {
            type Output = bool;

            fn dispatch<T: crate::algorithms::Sort>(self) -> Self::Output {
                T::IS_STABLE
            }
        }

        self.dispatch(variant, AlgorithmStableDispatcher)
    }

    /// Returns the sorting function for `variant`.
    pub fn sorter<T: Ord>(self, variant: &str) -> Option<fn(&mut [T])> {
        struct AlgorithmSorterDispatcher<T>(std::marker::PhantomData<T>);
        impl<T: Ord> SortDispatcher for AlgorithmSorterDispatcher<T> {
            type Output = fn(&mut [T]);

            fn dispatch<S: crate::algorithms::Sort>(self) -> Self::Output {
                S::sort
            }
        }

        self.dispatch(variant, AlgorithmSorterDispatcher(std::marker::PhantomData))
    }
}

/// Declare the available data types and distributions variants.
///
/// We the input to generate a macro that statically dispatches on the type, given a value.
///
/// # Example usage
///
/// ```rust
/// declare_data_types! {
///     /// Datatype and distribution description
///     Name = DataType : DistributionType,
///     /// A random permutation of u32 values
///     PermutationU32 = u32 : PermutationData,
///     // ...
/// }
/// ```
macro_rules! declare_data_types {
    (
        $(
            $(
                #[$attribute:meta]
            )*
            $name:ident = $type:ty : $d_type:ty
        ),*
        $(,)?
    ) => {
        /// Available data types and distributions for sorting.
        #[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
        pub enum DataType {
            $(
                $(
                    #[$attribute]
                )*
                $name
            ),*
        }

        impl DataType {
            pub fn dispatch<U: DataTypeDispatcher>(self, dispatcher: U) -> U::Output {
                match self {
                    $(
                        DataType::$name => dispatcher.dispatch::<$type, $d_type>(),
                    )*
                }
            }
        }
    };
}

/// L+P datatype, should be equivalent to the C++ original definition
pub type Blob2U64CmpFirst = crate::data::Blob<u64, crate::data::CompareFirstEntry, 2>;

// Declare the available data types
declare_data_types! {
    /// A random permutation of u32 values
    PermutationU32       = u32 : crate::data::PermutationData,
    /// Random runs with average length of `n.isqrt()` of u32 values
    RandomRunsSqrtU32    = u32 : crate::data::RandomRunsSqrtData,
    /// Random runs with average length of `3` of u32 values
    RandomRuns3U32       = u32 : crate::data::RandomRunsConstData<3>,
    /// Random runs with average length of `30` of u32 values
    RandomRuns30U32      = u32 : crate::data::RandomRunsConstData<30>,
    /// Random runs with average length of `300` of u32 values
    RandomRuns300U32     = u32 : crate::data::RandomRunsConstData<300>,
    /// Random runs with average length of `3000` of u32 values
    RandomRuns3000U32    = u32 : crate::data::RandomRunsConstData<3000>,
    /// Random runs with average length of `30000` of u32 values
    RandomRuns30000U32   = u32 : crate::data::RandomRunsConstData<30000>,
    /// Random runs with average length of `300000` of u32 values
    RandomRuns300000U32  = u32 : crate::data::RandomRunsConstData<300000>,
    /// Random runs with average length of `3000000` of u32 values
    RandomRuns3000000U32 = u32 : crate::data::RandomRunsConstData<3000000>,

    /// A random permutation of L+P blobs
    PermutationLP    = Blob2U64CmpFirst : crate::data::PermutationData,
    /// Random runs with average length of `n.isqrt()` of L+P blobs
    RandomRunsSqrtLP = Blob2U64CmpFirst : crate::data::RandomRunsSqrtData,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
    }
}

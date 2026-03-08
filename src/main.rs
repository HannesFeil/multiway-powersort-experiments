#![warn(
    clippy::as_conversions,
    clippy::missing_safety_doc,
    reason = "Check for scrutiny"
)]

use std::io::Write;

use clap::Parser as _;
use rand::SeedableRng as _;

mod algorithms;
mod cli;
mod data;

#[cfg(test)]
mod test;

/// Executable entry point
fn main() {
    let cli::Args {
        algorithm,
        variant,
        runs,
        size,
        data,
        seed,
        output,
    } = cli::Args::parse();

    // Validate the given algorithm variant
    let Some(variant) = cli::AlgorithmVariants::validate(algorithm, variant) else {
        println!("Invalid variant {variant} for algorithm {algorithm}");
        println!("Possible variants:");
        for (index, variant) in cli::AlgorithmVariants::variants(algorithm).enumerate() {
            println!("{index:>3}: {variant}");
        }
        return;
    };

    println!(
        "Running measurements for the following (stable: {stable}) algorithm:\n{alg}",
        alg = cli::AlgorithmVariants::variants(algorithm)
            .nth(variant)
            .unwrap(),
        stable = cli::AlgorithmVariants::is_stable(algorithm, variant).unwrap(),
    );
    println!("Runs: {runs}, Slice size: {size}, Data type: {data}");

    // Create RNG for data generation
    let mut rng = match seed {
        Some(partial_seed) => rand::rngs::StdRng::seed_from_u64(partial_seed),
        None => {
            println!("No seed provided, generating one using system rng");
            rand::rngs::StdRng::from_os_rng()
        }
    };

    let (samples, stats);

    // Run the experiment with the given algorithm and data
    //
    // This macro generates a match, dispatching for each single type, since generics can not be
    // resolved statically.
    with_match_type! {
        data;
        T, D => {
            // Get the sort function pointer (data type can be inferred at this point)
            let sorter = cli::AlgorithmVariants::sorter(algorithm, variant).unwrap();

            // Measure running times
            #[cfg(not(feature = "counters"))]
            {
                (samples, stats) =
                    perform_time_experiment::<T, D>(sorter, runs, size, &mut rng);

                println!("Run times in ms:\n{stats:#?}")
            }

            // Measure comparisons and merge costs
            #[cfg(feature = "counters")]
            {
                (samples, stats) = perform_counters_experiment::<T, D>(sorter, runs, size, &mut rng);

                println!("Comparisons:\n{stats:#?}")
            };
        }
    };

    // Write samples to output file if given
    if let Some(output) = output {
        write_output(&output, samples).unwrap_or_else(|error| {
            eprintln!("An error occurred while trying to write output at {output:?}: {error}");
        });
    }
}

/// Writes `samples` to a file at `path`, which is created in case it does not exist.
///
/// Returns IO error if writing to the file is not possible.
fn write_output<S: Samples<N>, const N: usize>(
    path: impl AsRef<std::path::Path>,
    samples: S,
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    // Write the CSV header
    file.write_all(S::headers().join(",").as_bytes())?;
    file.write_all(b"\n")?;

    // Write the individual lines (escaping should not be necessary since we only write integers)
    for line in samples.csv_lines() {
        file.write_all(line.join(",").as_bytes())?;
        file.write_all(b"\n")?;
    }

    Ok(())
}

/// A trait for encoding samples as CSV with `N` columns
trait Samples<const N: usize> {
    /// Returns the column headers for this data
    fn headers() -> [String; N];

    /// Returns the individual CSV lines, with one [`String`] per column
    fn csv_lines(self) -> impl Iterator<Item = [String; N]>;
}

impl Samples<1> for Vec<std::time::Duration> {
    fn headers() -> [std::string::String; 1] {
        ["ns".to_string()]
    }

    fn csv_lines(self) -> impl Iterator<Item = [String; 1]> {
        self.into_iter()
            .map(|duration| [duration.as_nanos().to_string()])
    }
}

impl Samples<4> for Vec<CounterSample> {
    fn headers() -> [std::string::String; 4] {
        ["comparisons", "alloc", "slice", "buffer"].map(str::to_string)
    }

    fn csv_lines(self) -> impl Iterator<Item = [String; 4]> {
        self.into_iter().map(|sample| {
            [
                sample.comparisons.to_string(),
                sample.merge_alloc_cost.to_string(),
                sample.merge_slice_cost.to_string(),
                sample.merge_buffer_cost.to_string(),
            ]
        })
    }
}

/// The global counters used during the experiment
pub static GLOBAL_COUNTERS: GlobalCounters = GlobalCounters {
    comparisons: data::GlobalCounter::new(),
    merge_alloc: data::GlobalCounter::new(),
    merge_slice: data::GlobalCounter::new(),
    merge_buffer: data::GlobalCounter::new(),
};

/// Container for global counters used during the experiment
pub struct GlobalCounters {
    pub comparisons: data::GlobalCounter,
    pub merge_alloc: data::GlobalCounter,
    pub merge_slice: data::GlobalCounter,
    pub merge_buffer: data::GlobalCounter,
}

impl GlobalCounters {
    /// Reset all global counters
    pub fn reset(&self) {
        self.comparisons.read_and_reset();
        self.merge_alloc.read_and_reset();
        self.merge_slice.read_and_reset();
        self.merge_buffer.read_and_reset();
    }
}

/// A single sample point for measuring comparisons and merge costs
#[derive(Debug)]
struct CounterSample {
    /// The number of comparisons
    comparisons: u64,
    /// The number of elements needed as additional merge allocation
    merge_alloc_cost: u64,
    /// The number of elements written to the original slice during merging
    merge_slice_cost: u64,
    /// The number of elements written to the buffer during merging
    merge_buffer_cost: u64,
}

/// Performs a time sampling experiment on the given sorting algorithm
///
/// - `sorter`: The function used for sorting
/// - `runs`: The number of samples to measure
/// - `size`: The size of the slices to sort
/// - `rng`: The RNG used for sampling the data
#[allow(dead_code, reason = "Unused when feature 'counters' is active")]
fn perform_time_experiment<T: Ord + std::fmt::Debug, D: data::DataGenerator<T>>(
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<std::time::Duration>, rolling_stats::Stats<f64>) {
    let mut samples = Vec::with_capacity(runs);
    let mut stats: rolling_stats::Stats<f64> = rolling_stats::Stats::new();

    perform_experiment::<_, T, D>(
        |elapsed| {
            samples.push(elapsed);
            #[expect(
                clippy::as_conversions,
                reason = "Millis should not get high enough for this cast to become inaccurate"
            )]
            stats.update(elapsed.as_millis() as f64);
        },
        sorter,
        runs,
        size,
        rng,
    );

    (samples, stats)
}

/// Performs a sampling experiment on the given sorting algorithm.
///
/// Records comparisons, as well as different merge costs, see [`CounterSample`].
///
/// - `sorter`: The function used for sorting
/// - `runs`: The number of samples to measure
/// - `size`: The size of the slices to sort
/// - `rng`: The RNG used for sampling the data
#[allow(dead_code, reason = "Unused when feature 'counters' is inactive")]
fn perform_counters_experiment<
    T: Ord + std::fmt::Debug,
    D: data::DataGenerator<crate::data::CountComparisons<T>>,
>(
    sorter: fn(&mut [crate::data::CountComparisons<T>]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<CounterSample>, rolling_stats::Stats<f64>) {
    let mut samples = Vec::with_capacity(runs);
    let mut stats = rolling_stats::Stats::<f64>::new();

    perform_experiment::<_, crate::data::CountComparisons<T>, D>(
        |_| {
            let comparisons = GLOBAL_COUNTERS.comparisons.read_and_reset();
            let merge_alloc_cost = GLOBAL_COUNTERS.merge_alloc.read_and_reset();
            let merge_slice_cost = GLOBAL_COUNTERS.merge_slice.read_and_reset();
            let merge_buffer_cost = GLOBAL_COUNTERS.merge_buffer.read_and_reset();

            let sample = CounterSample {
                comparisons,
                merge_alloc_cost,
                merge_slice_cost,
                merge_buffer_cost,
            };

            samples.push(sample);

            #[expect(
                clippy::as_conversions,
                reason = "Comparisons should not get high enough for this cast to become inaccurate"
            )]
            stats.update(comparisons as f64);
        },
        sorter,
        runs,
        size,
        rng,
    );

    (samples, stats)
}

/// Perform a generic sampling experiment on the given sorting algorithm.
///
/// - `sampler`: The function used for sampling, receiving the running time of each sort iteration
/// - `sorter`: The function used for sorting
/// - `runs`: The number of samples to measure
/// - `size`: The size of the slices to sort
/// - `rng`: The RNG used for sampling the data
fn perform_experiment<
    F: FnMut(std::time::Duration),
    T: Ord + std::fmt::Debug,
    D: data::DataGenerator<T>,
>(
    mut sampler: F,
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) {
    #[expect(
        clippy::as_conversions,
        reason = "Realistically runs is not gonna be higher than u64::MAX"
    )]
    let bar = indicatif::ProgressBar::new(runs as u64);
    let mut generator = D::default();
    let mut data = generator.initialize(size, rng);

    for run in 0..=runs {
        let now = std::time::Instant::now();
        sorter(std::hint::black_box(&mut data));
        let elapsed = now.elapsed();

        assert!(
            data.is_sorted(),
            "Data was not sorted after algorithm run: {run}"
        );

        // Skip first sample (behavior taken from original codebase)
        if run != 0 {
            sampler(elapsed);
            bar.inc(1);
        }

        generator.reinitialize(&mut data, rng);

        #[cfg(feature = "counters")]
        GLOBAL_COUNTERS.reset();
    }
}

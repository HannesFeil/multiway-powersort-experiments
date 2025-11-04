use std::time::Duration;

use clap::Parser;
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    algorithms::Algorithm,
    data::{Data, UniformData},
};

mod algorithms;
mod data;

/// Command line arguments
#[derive(clap::Parser)]
struct Args {
    /// The sorting algorithm to run
    #[arg()]
    algorithm: Algorithm,
    /// The number of runs to do
    #[arg()]
    runs: usize,
    /// The size of the slices to sort
    #[arg()]
    size: usize,
    /// The data type to use for sorting
    #[arg()]
    data: DataType,
    /// Seed for the rng
    #[arg(long)]
    seed: Option<u128>,
}

/// Available data types for sorting
#[derive(Clone, Copy, clap::ValueEnum)]
enum DataType {
    UniformU32,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DataType::UniformU32 => "Uniform u32",
        })
    }
}

fn main() {
    let Args {
        algorithm,
        runs,
        size,
        data,
        seed,
    } = Args::parse();

    println!("Running experiment with sort: {algorithm}");
    println!("Runs: {runs}, Slice size: {size}, Data type: {data}");

    // Create rng
    // FIXME: this is probably bad, but have to look into rng anyway
    let mut rng = match seed {
        Some(partial_seed) => {
            let mut seed = [0; 32];
            seed[16..].copy_from_slice(&partial_seed.to_le_bytes());
            StdRng::from_seed(seed)
        }
        None => {
            println!("No seed provided, generating one using system rng");
            StdRng::from_os_rng()
        }
    };

    let samples = match data {
        DataType::UniformU32 => {
            perform_experiment::<u32, UniformData<u32>>(algorithm, runs, size, &mut rng)
        }
    };

    println!("Time samples: {samples:?}");
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
fn perform_experiment<T: Ord, D: Data<T>>(
    algorithm: Algorithm,
    runs: usize,
    size: usize,
    rng: &mut StdRng,
) -> Vec<Duration> {
    let sorter = algorithm.sorter();
    let mut samples = Vec::with_capacity(runs);

    for run in 0..=runs {
        let mut data = D::initialize(size, rng);

        let now = std::time::Instant::now();
        sorter(&mut data);
        let elapsed = now.elapsed();

        debug_assert!(data.is_sorted());

        // NOTE: Skip first sample (taken from original codebase)
        if run != 0 {
            samples.push(elapsed);
        }
    }

    samples
}

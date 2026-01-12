use clap::Parser as _;
use rand::SeedableRng as _;

mod algorithms;
mod cli;
mod data;

#[cfg(test)]
mod test;

// General TODO: check for overflows and stuff?

/// Program entry point
fn main() {
    let cli::Args {
        algorithm,
        runs,
        size,
        data,
        seed,
    } = cli::Args::parse();

    println!(
        "Running measurements for the following algorithm:\n{algorithm:?} (stable: {stable})",
        stable = algorithm.is_stable(),
    );
    println!("Runs: {runs}, Slice size: {size}, Data type: {data}");

    // Create rng
    // FIXME: this is probably bad, but i have to look into rng anyway
    let mut rng = match seed {
        Some(partial_seed) => rand::rngs::StdRng::seed_from_u64(partial_seed),
        None => {
            println!("No seed provided, generating one using system rng");
            rand::rngs::StdRng::from_os_rng()
        }
    };

    let (samples, stats) = match data {
        cli::DataType::UniformU32 => {
            perform_experiment::<u32, data::UniformData<u32>>(algorithm, runs, size, &mut rng)
        }
        cli::DataType::PermutationU32 => {
            perform_experiment::<u32, data::PermutationData<u32>>(algorithm, runs, size, &mut rng)
        }
    };

    println!("Stats: {stats:?}");
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
fn perform_experiment<T: Ord + std::fmt::Debug, D: data::Data<T>>(
    algorithm: cli::Algorithm,
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<std::time::Duration>, rolling_stats::Stats<f64>) {
    let sorter = algorithm.sorter();
    let mut samples = Vec::with_capacity(runs);

    let mut stats: rolling_stats::Stats<f64> = rolling_stats::Stats::new();

    let bar = indicatif::ProgressBar::new(runs as u64);

    for run in 0..=runs {
        let mut data = D::initialize(size, rng);

        let now = std::time::Instant::now();
        sorter(std::hint::black_box(&mut data));
        let elapsed = now.elapsed();

        debug_assert!(
            data.is_sorted(),
            "{data:?} is not sorted after algorithm run"
        );

        // NOTE: Skip first sample (behavior taken from original codebase)
        if run != 0 {
            samples.push(elapsed);
            // TODO: is this cast fine?
            stats.update(elapsed.as_millis() as f64);

            bar.inc(1);
        }
    }

    (samples, stats)
}

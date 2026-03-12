# Accompanying code for paper "Multiway Powersort in Rust"

This is the accompanying Rust code for my Bachelors Thesis "Multiway Powersort in Rust".
It contains sorting implementations from [Powersort](https://github.com/sebawild/powersort),
rewritten in Rust.

## Data

The `experiments.sh` script can be used to generate the data, evalutated in the paper's study.
It requires `cargo` and a valid rust toolchain.
The one described in `rust-toolchain.toml` can be used to run tests under Miri.
Samples will be written into their respective files in a "results" subdirectory.

## Figures

The figures were created using combined datasets from the rust implementation and the C++
implementation.
The script makes use of `python` and it's `matplotlib`, `pandas` and `numpy` libraries.
Following these steps one can recreate the used figures:

1. Obtain the data for this code as explained in the **Data** section.
2. Move the rust results in a folder called "rust".
3. Obtain the data for the C++ implementation as explained in it's repository
   [Powersort fork](https://github.com/HannesFeil/powersort-fork-for-experiments)
4. Move the C++ results into a folder called "cpp"
5. Execute the `figures.py` python script to generate the figures

## Dev shell

The `flake.nix` file offers a devshell with all required dependencies.

## Code overview

- `main.rs` is the main entry point for the binary, it contains the code for running the
  experiments.
- `cli.rs` handles the command line interface.
- `data.rs` defines different datatypes used for sorting.
- `test.rs` contains utility structs and functions used for testing purposes.

- `algorithms.rs` contains the `Sort` trait, which unifies sorting behavior.
- `algorithms/<sort>.rs` implements the specific sort, often supporting multiple generic parameters.
- `algorithms/merging.rs` contains utility structs and functions used for implementing the specific
  merging procedues in `algorithms/merging/two_way.rs` and `algorithms/merging/multi_way.rs`.

#! /bin/sh
set -euo pipefail

cargo build --release
SEED=439569436534

BINARY=target/release/multiway-powersort-experiments
OUTPUT=results
mkdir -p ${OUTPUT}

run_sorts() {
  local runs=$1
  local n=$2
  local d=$3
  local file_suffix=$4

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} std ${d} "${OUTPUT}/std-${file_suffix}"
}

echo "Experiment 1: int, random runs, various n"

run_sorts 1001     10000 random-runs-sqrt-u32  times-runs-10k-int >> ${OUTPUT}/times-runs-int.out
run_sorts 1001    100000 random-runs-sqrt-u32 times-runs-100k-int >> ${OUTPUT}/times-runs-int.out
run_sorts 1001   1000000 random-runs-sqrt-u32   times-runs-1m-int >> ${OUTPUT}/times-runs-int.out
run_sorts  101  10000000 random-runs-sqrt-u32  times-runs-10m-int >> ${OUTPUT}/times-runs-int.out
run_sorts  101 100000000 random-runs-sqrt-u32 times-runs-100m-int >> ${OUTPUT}/times-runs-int.out

echo "Experiment 2a: 10^7 ints distribution, random runs"

for runs in 3 30 300 3000 30000 300000 3000000
do
  # FIXME: ?
  run_sorts 3  10000000 runs$runs '*' ${SEED} times-runs$runs-10m-int-dist    >> ${OUTPUT}/times-runs$runs-int.out
done

echo "Experiment 2: 10^7 ints distribution, random runs"

run_sorts 1001 10000000 random-runs-sqrt-u32 times-runs-3k-10m-int-dist >> ${OUTPUT}/times-runs3k-int.out

echo "Experiment 3: long+pointer, random runs, various n"

run_sorts 1001     10000 random-runs-sqrt-lp  times-runs-10k-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts 1001    100000 random-runs-sqrt-lp times-runs-100k-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts 1001   1000000 random-runs-sqrt-lp   times-runs-1m-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts  101  10000000 random-runs-sqrt-lp  times-runs-10m-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts  101 100000000 random-runs-sqrt-lp times-runs-100m-l+p >> ${OUTPUT}/times-runs-l+p.out

echo "Experiment 4: int, random permutations, various n"

run_sorts 1001     10000 permutation-u32  times-runs-10k-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts 1001    100000 permutation-u32 times-runs-100k-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts 1001   1000000 permutation-u32   times-runs-1m-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts  101  10000000 permutation-u32  times-runs-10m-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts  101 100000000 permutation-u32 times-runs-100m-int-rp >> ${OUTPUT}/times-rp-int.out

echo "Experiment 5: count comparisons and merge cost, random runs, various n"

cargo build --release --features counters

run_sorts 1001     10000 random-runs-sqrt-u32  times-runs-10k-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts 1001    100000 random-runs-sqrt-u32 times-runs-100k-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts 1001   1000000 random-runs-sqrt-u32   times-runs-1m-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts  101  10000000 random-runs-sqrt-u32  times-runs-10m-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts  101 100000000 random-runs-sqrt-u32 times-runs-100m-int-cmp >> ${OUTPUT}/times-runs-cmps.out

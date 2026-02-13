#! /bin/sh
set -eu

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

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} std ${d} -v 0 "${OUTPUT}/std-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} std ${d} -v 1 "${OUTPUT}/std-unstable-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} quicksort ${d} -v 0 "${OUTPUT}/quicksort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} quicksort ${d} -v 1 "${OUTPUT}/quicksort-check-sorted-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} peeksort ${d} -v 0 "${OUTPUT}/peeksort-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} mergesort ${d} -v 0 "${OUTPUT}/mergesort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} mergesort ${d} -v 1 "${OUTPUT}/mergesort-i1-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} mergesort ${d} -v 2 "${OUTPUT}/mergesort-i1-check-sorted-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} mergesort ${d} -v 3 "${OUTPUT}/mergesort-bottom-up-check-sorted-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} timsort ${d} -v 0 "${OUTPUT}/timsort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} timsort ${d} -v 1 "${OUTPUT}/trotsort-binary-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} timsort ${d} -v 2 "${OUTPUT}/trotsort-simple-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} powersort ${d} -v 0 "${OUTPUT}/powersort-${file_suffix}"

  # ${BINARY} --runs ${runs} --size ${n} --seed ${SEED} multiway-powersort ${d} -v 0 "${OUTPUT}/multiway-powersort-${file_suffix}"
}

echo "Experiment 1: int, random runs, various n"

run_sorts 1001     10000 random-runs-sqrt-u32  times-runs-e4-int >> ${OUTPUT}/times-runs-int.out
run_sorts 1001    100000 random-runs-sqrt-u32 times-runs-e5-int >> ${OUTPUT}/times-runs-int.out
run_sorts 1001   1000000 random-runs-sqrt-u32   times-runs-e6-int >> ${OUTPUT}/times-runs-int.out
run_sorts  101  10000000 random-runs-sqrt-u32  times-runs-e7-int >> ${OUTPUT}/times-runs-int.out
run_sorts  101 100000000 random-runs-sqrt-u32 times-runs-e8-int >> ${OUTPUT}/times-runs-int.out

echo "Experiment 2: 10^7 ints distribution, random runs"

for runs in 3 30 300 3000 30000 300000 3000000
do
  run_sorts 101 10000000 random-runs${runs}-u32 times-runs${runs}-e7-int >> ${OUTPUT}/times-runs${runs}-int.out
done

echo "Experiment 3: long+pointer, random runs, various n"

run_sorts 1001     10000 random-runs-sqrt-lp  times-runs-e4-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts 1001    100000 random-runs-sqrt-lp times-runs-e5-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts 1001   1000000 random-runs-sqrt-lp   times-runs-e6-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts  101  10000000 random-runs-sqrt-lp  times-runs-e7-l+p >> ${OUTPUT}/times-runs-l+p.out
run_sorts  101 100000000 random-runs-sqrt-lp times-runs-e8-l+p >> ${OUTPUT}/times-runs-l+p.out

echo "Experiment 4: int, random permutations, various n"

run_sorts 1001     10000 permutation-u32  times-runs-e4-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts 1001    100000 permutation-u32 times-runs-e5-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts 1001   1000000 permutation-u32   times-runs-e6-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts  101  10000000 permutation-u32  times-runs-e7-int-rp >> ${OUTPUT}/times-rp-int.out
run_sorts  101 100000000 permutation-u32 times-runs-e8-int-rp >> ${OUTPUT}/times-rp-int.out

echo "Experiment 5: count comparisons and merge cost, random runs, various n"

cargo build --release --features counters

run_sorts 1001     10000 random-runs-sqrt-u32  times-runs-e4-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts 1001    100000 random-runs-sqrt-u32 times-runs-e5-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts 1001   1000000 random-runs-sqrt-u32   times-runs-e6-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts  101  10000000 random-runs-sqrt-u32  times-runs-e7-int-cmp >> ${OUTPUT}/times-runs-cmps.out
run_sorts  101 100000000 random-runs-sqrt-u32 times-runs-e8-int-cmp >> ${OUTPUT}/times-runs-cmps.out

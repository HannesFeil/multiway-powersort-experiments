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

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 std "${OUTPUT}/std-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 1 std "${OUTPUT}/std-unstable-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 quicksort "${OUTPUT}/quicksort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 1 quicksort "${OUTPUT}/quicksort-check-sorted-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 peeksort "${OUTPUT}/peeksort-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 mergesort "${OUTPUT}/mergesort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 1 mergesort "${OUTPUT}/mergesort-i1-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 2 mergesort "${OUTPUT}/mergesort-i1-check-sorted-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 3 mergesort "${OUTPUT}/mergesort-bottom-up-check-sorted-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 timsort "${OUTPUT}/timsort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 1 timsort "${OUTPUT}/trotsort-binary-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 2 timsort "${OUTPUT}/trotsort-simple-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 powersort "${OUTPUT}/powersort-${file_suffix}"

  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 0 multiway-powersort "${OUTPUT}/multiway-powersort-${file_suffix}"
  ${BINARY} --runs ${runs} --size ${n} --data ${d} --seed ${SEED} -v 1 multiway-powersort "${OUTPUT}/multiway-powersort-4-${file_suffix}"
}

echo "Experiment 1: int, random runs, various n"

run_sorts 1000     10000 random-runs-sqrt-u32 times-runs-int-e4.csv >> ${OUTPUT}/times-runs-int.out
run_sorts 1000    100000 random-runs-sqrt-u32 times-runs-int-e5.csv >> ${OUTPUT}/times-runs-int.out
run_sorts 1000   1000000 random-runs-sqrt-u32 times-runs-int-e6.csv >> ${OUTPUT}/times-runs-int.out
run_sorts  100  10000000 random-runs-sqrt-u32 times-runs-int-e7.csv >> ${OUTPUT}/times-runs-int.out
run_sorts  100 100000000 random-runs-sqrt-u32 times-runs-int-e8.csv >> ${OUTPUT}/times-runs-int.out

echo "Experiment 2: 10^7 ints distribution, random runs"

for runs in 3 30 300 3000 30000 300000 3000000
do
  run_sorts 100 10000000 random-runs${runs}-u32 times-runs${runs}-int-e7.csv >> ${OUTPUT}/times-runs${runs}-int.out
done

echo "Experiment 3: long+pointer, random runs, various n"

run_sorts 1000     10000 random-runs-sqrt-lp times-runs-l+p-e4.csv >> ${OUTPUT}/times-runs-l+p.out
run_sorts 1000    100000 random-runs-sqrt-lp times-runs-l+p-e5.csv >> ${OUTPUT}/times-runs-l+p.out
run_sorts 1000   1000000 random-runs-sqrt-lp times-runs-l+p-e6.csv >> ${OUTPUT}/times-runs-l+p.out
run_sorts  100  10000000 random-runs-sqrt-lp times-runs-l+p-e7.csv >> ${OUTPUT}/times-runs-l+p.out
run_sorts  100 100000000 random-runs-sqrt-lp times-runs-l+p-e8.csv >> ${OUTPUT}/times-runs-l+p.out

echo "Experiment 4: int, random permutations, various n"

run_sorts 1000     10000 permutation-u32 times-rp-int-e4.csv >> ${OUTPUT}/times-rp-int.out
run_sorts 1000    100000 permutation-u32 times-rp-int-e5.csv >> ${OUTPUT}/times-rp-int.out
run_sorts 1000   1000000 permutation-u32 times-rp-int-e6.csv >> ${OUTPUT}/times-rp-int.out
run_sorts  100  10000000 permutation-u32 times-rp-int-e7.csv >> ${OUTPUT}/times-rp-int.out
run_sorts  100 100000000 permutation-u32 times-rp-int-e8.csv >> ${OUTPUT}/times-rp-int.out

echo "Experiment 5: count comparisons and merge cost, random runs, various n"

cargo build --release --features counters

run_sorts 1000     10000 random-runs-sqrt-u32 times-runs-int-cmp-e4.csv >> ${OUTPUT}/times-runs-cmps.out
run_sorts 1000    100000 random-runs-sqrt-u32 times-runs-int-cmp-e5.csv >> ${OUTPUT}/times-runs-cmps.out
run_sorts 1000   1000000 random-runs-sqrt-u32 times-runs-int-cmp-e6.csv >> ${OUTPUT}/times-runs-cmps.out
run_sorts  100  10000000 random-runs-sqrt-u32 times-runs-int-cmp-e7.csv >> ${OUTPUT}/times-runs-cmps.out
run_sorts  100 100000000 random-runs-sqrt-u32 times-runs-int-cmp-e8.csv >> ${OUTPUT}/times-runs-cmps.out

run_sorts 1000     10000 permutation-u32 times-rp-int-cmp-e4.csv >> ${OUTPUT}/times-rp-cmps.out
run_sorts 1000    100000 permutation-u32 times-rp-int-cmp-e5.csv >> ${OUTPUT}/times-rp-cmps.out
run_sorts 1000   1000000 permutation-u32 times-rp-int-cmp-e6.csv >> ${OUTPUT}/times-rp-cmps.out
run_sorts  100  10000000 permutation-u32 times-rp-int-cmp-e7.csv >> ${OUTPUT}/times-rp-cmps.out
run_sorts  100 100000000 permutation-u32 times-rp-int-cmp-e8.csv >> ${OUTPUT}/times-rp-cmps.out

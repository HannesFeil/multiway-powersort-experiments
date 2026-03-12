import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import math

SHOW = False

algorithms = {
    "mergesort": {
        "cpp": "TopDownMergesort+iscutoff=24+checkSorted=1+mergingMethod=COPY_BOTH",
        "style": ".-r",
    },
    "mergesort-bottom-up-check-sorted": {
        "cpp": "BottomUpMergesort+minRunLen=24+checkSorted=1+mergingMethod=COPY_BOTH",
        "style": "v--r",
    },
    "mergesort-i1-check-sorted": {
        "cpp": "TopDownMergesort+iscutoff=1+checkSorted=1+mergingMethod=COPY_BOTH",
        "style": "s-.r",
    },
    "mergesort-i1": {
        "cpp": "TopDownMergesort+iscutoff=1+checkSorted=0+mergingMethod=COPY_BOTH",
        "style": "p:r",
    },
    "peeksort": {
        "cpp": "PeekSort+iscutoff=24+onlyIncRuns=0+mergingMethod=COPY_BOTH",
        "style": ".-m",
    },
    "powersort": {
        "cpp": "PowerSort+minRunLen=24+onlyIncRuns=0+mergingMethod=COPY_BOTH",
        "style": ".-g",
    },
    # TODO: adjust?
    "multiway-powersort-4": {
        "cpp": "PowerSort4Way+minRunLen=24+mergeMethod=GENERAL_INDICES+onlyIncRuns=0",
        "style": ".-y",
    },
    # "multiway-powersort-4": {
    #     "cpp": "PowerSort4Way+minRunLen=24+mergeMethod=GENERAL_BY_STAGES+onlyIncRuns=0",
    #     "style": "v--y",
    # },
    "quicksort": {
        "cpp": "QuickSort+iscutoff=24+ninthercutiff=128+checkSorted=0",
        "style": ".-b",
    },
    "quicksort-check-sorted": {
        "cpp": "QuickSort+iscutoff=24+ninthercutiff=128+checkSorted=1",
        "style": "v--b",
    },
    "std": {
        "cpp": "std::stable_sort",
        "style": ".-k",
    },
    "std-unstable": {
        "cpp": "std::sort",
        "style": "v--k",
    },
    "timsort": {
        "cpp": "Timsort-gfx",
        "style": ".-c",
    },
    "trotsort-binary": {
        "cpp": "TimsortTrot-useBinaryInsertionsort=1",
        "style": "v--c",
    },
    "trotsort-simple": {
        "cpp": "TimsortTrot-useBinaryInsertionsort=0",
        "style": "s:c",
    },
}

time_tests = {
    "times-runs-int": range(4, 9),
    "times-rp-int": range(4, 9),
    "times-runs-l+p": range(4, 9),
    "times-runs3-int": [7],
    "times-runs30-int": [7],
    "times-runs300-int": [7],
    "times-runs3000-int": [7],
    "times-runs30000-int": [7],
    "times-runs300000-int": [7],
    "times-runs3000000-int": [7],
}

comparison_tests = {"times-runs-int-cmp": range(4, 9), "times-rp-int-cmp": range(4, 9)}


def load_data():
    result = dict()

    for test, sizes in time_tests.items():
        result[test] = dict()

        for size in sizes:
            rust_data = load_rust_data(test, size)
            cpp_data = load_cpp_data(test, size)

            result[test][size] = {"rust": rust_data, "cpp": cpp_data}

    for test, sizes in comparison_tests.items():
        result[test] = dict()
        for size in sizes:
            rust_data = load_rust_cmp_data(test, size)
            cpp_data = load_cpp_cmp_data(test, size)

            result[test][size] = {"rust": rust_data, "cpp": cpp_data}

    return result


def load_rust_data(test, size):
    result = dict()

    for alg in algorithms.keys():
        result[alg] = np.loadtxt(f"rust/{alg}-{test}-e{size}.csv", skiprows=1)

    return result


def load_rust_cmp_data(test, size):
    result = dict()

    for alg in algorithms.keys():
        data = np.loadtxt(f"rust/{alg}-{test}-e{size}.csv", delimiter=",", skiprows=1)

        result[alg] = {
            "comparisons": data[:, 0],
            "alloc_cost": data[:, 1],
            "slice_merge_cost": data[:, 2],
            "buffer_merge_cost": data[:, 3],
        }

    return result


def read_cpp_data(file):
    data = (
        (
            pd.read_csv(
                file,
                index_col=False,
                skipfooter=1,
                engine="python",
            )
        )
        .set_index("algo")
        .groupby("algo")
        .agg(list)
    )

    return data


def load_cpp_data(test, size):
    result = dict()

    data = read_cpp_data(f"cpp/{test}-e{size}.csv")

    for alg, algo_data in algorithms.items():
        result[alg] = np.array(data.loc[algo_data["cpp"], "nano"])

    return result


def load_cpp_cmp_data(test, size):
    result = dict()

    data = read_cpp_data(f"cpp/{test}-e{size}.csv")

    for alg, algo_data in algorithms.items():
        result[alg] = {
            "comparisons": np.array(data.loc[algo_data["cpp"], "comparisons"])
        }

    return result


data = load_data()


def error_plot_comparisons(test):
    fig, axs = plt.subplots(1, 2, sharey=True, sharex=True)
    fig: plt.Figure = fig
    axs: list[plt.Axes] = axs

    current_sizes = comparison_tests[test]

    for i, lang in enumerate(["rust", "cpp"]):
        for algo, algo_data in algorithms.items():
            algo_data_per_size = [
                1
                / (10**size * math.log2(10**size))
                * data[test][size][lang][algo]["comparisons"]
                for size in current_sizes
            ]
            axs[i].errorbar(
                current_sizes,
                [arr.mean() for arr in algo_data_per_size],
                yerr=[arr.std() for arr in algo_data_per_size],
                fmt=algo_data["style"],
                label=algo,
            )
        axs[i].set_xticks(current_sizes)
        axs[i].set_xlabel("n = 10^x")
        axs[i].set_ylabel("mean comparisons over n * log2(n)")
        axs[i].set_title(f"{lang}")

    plt.legend(bbox_to_anchor=(1.04, 0.5), loc="center left", borderaxespad=0)

    if SHOW:
        plt.show()
    else:
        plt.savefig(f"error_plot_comparisons_{test}.svg", bbox_inches="tight")


def error_plot(test):
    fig, axs = plt.subplots(1, 2, sharey=True, sharex=True)
    fig: plt.Figure = fig
    axs: list[plt.Axes] = axs

    for i, lang in enumerate(["rust", "cpp"]):
        for algo, algo_data in algorithms.items():
            algo_data_per_size = [
                1 / (10**size * math.log2(10**size)) * data[test][size][lang][algo]
                for size in time_tests[test]
            ]
            axs[i].errorbar(
                time_tests[test],
                [arr.mean() for arr in algo_data_per_size],
                yerr=[arr.std() for arr in algo_data_per_size],
                fmt=algo_data["style"],
                label=algo,
            )
        axs[i].set_xticks(time_tests[test])
        axs[i].set_xlabel("n = 10^x")
        axs[i].set_ylabel("mean ns over n * log2(n)")
        axs[i].set_title(f"{lang}")

    plt.legend(bbox_to_anchor=(1.04, 0.5), loc="center left", borderaxespad=0)

    if SHOW:
        plt.show()
    else:
        plt.savefig(f"error_plot_runtimes_{test}.svg", bbox_inches="tight")


def error_plot_per_runlen():
    fig, axs = plt.subplots(1, 2, sharey=True, sharex=True)
    fig: plt.Figure = fig
    axs: list[plt.Axes] = axs

    powers = range(6)
    lengths = [3 * 10**x for x in powers]
    tests = [f"times-runs{len}-int" for len in lengths]
    size = 7

    for i, lang in enumerate(["rust", "cpp"]):
        for algo, algo_data in algorithms.items():
            algo_data_per_runlen = [data[test][size][lang][algo] for test in tests]
            axs[i].errorbar(
                powers,
                [arr.mean() for arr in algo_data_per_runlen],
                yerr=[arr.std() for arr in algo_data_per_runlen],
                fmt=algo_data["style"],
                label=algo,
            )
        axs[i].set_xticks(powers)
        axs[i].set_xlabel("avarage run length of 3 * 10^x")
        axs[i].set_ylabel("mean ns")

    plt.legend(bbox_to_anchor=(1.04, 0.5), loc="center left", borderaxespad=0)

    if SHOW:
        plt.show()
    else:
        plt.savefig("error_plot_runtimes_per_runlen.svg", bbox_inches="tight")


def percentage_plot(test):
    fig, axs = plt.subplots()
    fig: plt.Figure = fig
    axs: plt.Axes = axs

    def value_for_algo_and_size(algo, size):
        rust = data[test][size]["rust"][algo]
        cpp = data[test][size]["cpp"][algo]

        return rust.mean() / cpp.mean() - 1

    def color_for_fmt(fmt):
        color_char = fmt[-1]
        return color_char

    for algo, algo_data in algorithms.items():
        algo_data_per_size = [
            value_for_algo_and_size(algo, size) for size in time_tests[test]
        ]
        axs.plot(
            time_tests[test],
            algo_data_per_size,
            algo_data["style"],
            label=algo,
        )

    axs.axhline(0, color="black", linestyle=":", alpha=0.5)
    axs.set_xticks(time_tests[test])
    axs.set_xlabel("n = 10^x")
    axs.set_ylabel("relative change in mean runtime")

    plt.legend(bbox_to_anchor=(1.04, 0.5), loc="center left", borderaxespad=0)

    if SHOW:
        plt.show()
    else:
        plt.savefig(f"percentage_plot_runtime_{test}.svg", bbox_inches="tight")


def plot_tests_violin(test, size):
    fig, axs = plt.subplots(1, 2, sharey=True, sharex=True)

    handles = []
    labels = []
    for i, lang in enumerate(["rust", "cpp"]):
        for algo_index, algo in enumerate(algorithms.keys()):
            plot = axs[i].violinplot(
                data[test][size][lang][algo],
                widths=1,
                showmeans=True,
                showextrema=True,
                positions=[algo_index],
            )
            handles += [plot["cmeans"]]
            labels += [algo]

        axs[i].set_xticks(
            range(0, len(algorithms.keys())), labels=algorithms.keys(), rotation=90
        )

        axs[i].set_xlabel("algorithm")
        axs[i].set_ylabel("Mean runtime (ns)")
        axs[i].set_title(f"{lang}")

    if SHOW:
        plt.show()
    else:
        plt.savefig(f"violin_plot_{test}_{size}.svg", bbox_inches="tight")


error_plot_comparisons("times-runs-int-cmp")
error_plot_comparisons("times-rp-int-cmp")

for test in ["times-runs-int", "times-runs-l+p", "times-rp-int"]:
    error_plot(test)

error_plot_per_runlen()

for test in ["times-runs-int", "times-runs-l+p", "times-rp-int"]:
    percentage_plot(test)

test = "times-runs-int"
plot_tests_violin(test, time_tests[test][-2])

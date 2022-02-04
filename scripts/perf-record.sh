#!/usr/bin/env bash
set -euo pipefail

# The output file is stored on an in-memory filesystem to minimize disk I/O
# while `perf` is writing samples to it. My trusty development machine has a
# Winchester (HDD) and disk writes can affect performance measurements
# significantly even if the game files have been fully cached into RAM by the
# kernel.
perf_file="$(mktemp -t -p /dev/shm perf.XXXXXXXXXX)"
finish() {
  rm -rfv "$perf_file"
}
trap finish EXIT

perf record --call-graph=dwarf --output="$perf_file" --compression-level=1 -- "$@"
mv "$perf_file" perf.data
# <https://github.com/jonhoo/inferno>
perf script | inferno-collapse-perf | inferno-flamegraph > perf-flamegraph.svg

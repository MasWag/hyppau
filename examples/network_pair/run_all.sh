#!/bin/sh -ue
################################################################
# Name
#  run_all.sh
#
# Description
#  Script to run all experiments on Network Pair benchmark.
#
# Example
#  ./run_all.sh
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

readonly MODES="naive fjs naive-filtered fjs-filtered"
readonly INPUT_LENGTHS="1000 2000 3000 4000 5000 6000 7000 8000 9000 10000"
readonly REPETITIONS=10

cd "$(dirname "$0")" || exit 1

for mode in $MODES; do
    for input_length in $INPUT_LENGTHS; do
        for _ in $(seq 1 $REPETITIONS); do
            ./run.sh "$input_length" "$mode"
        done
    done
done

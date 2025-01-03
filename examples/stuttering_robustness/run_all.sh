#!/bin/sh -ue
################################################################
# Name
#  run_all.sh
#
# Description
#  Script to run all experiments on Stuttering Robustness benchmark.
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
readonly INPUTS='a,b'
readonly OUTPUTS='0,1'
readonly INPUT_LENGTHS="200 400 600 800 1000 1200 1400 1600 1800 2000"
readonly REPETITIONS=10

cd "$(dirname "$0")" || exit 1

for input_length in $INPUT_LENGTHS; do
    for mode in $MODES; do
        for _ in $(seq 1 $REPETITIONS); do
            ./run.sh "$INPUTS" "$OUTPUTS" "$input_length" "$mode"
        done
    done
done

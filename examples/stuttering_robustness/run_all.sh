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
readonly INPUT_LENGTHS="50 100 150 200 250 300 350 400 450 500"
readonly REPETITIONS=10

cd "$(dirname "$0")" || exit 1

for mode in $MODES; do
    for input_length in $INPUT_LENGTHS; do
        for _ in $(seq 1 $REPETITIONS); do
            ./run.sh "$INPUTS" "$OUTPUTS" "$input_length" "$mode"
        done
    done
done

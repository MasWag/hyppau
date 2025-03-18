#!/bin/sh -ue
################################################################
# Name
#  run_all.sh
#
# Description
#  Script to run all experiments on Interference benchmark.
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
readonly ACTIONS='a,b'
readonly OUTPUTS='0,1'
readonly INPUT_LENGTHS="100 200 300 400 500 600 700 800 900 1000"
readonly REPETITIONS=10

cd "$(dirname "$0")" || exit 1

for mode in $MODES; do
    for input_length in $INPUT_LENGTHS; do
        for _ in $(seq 1 $REPETITIONS); do
            ./run.sh "$ACTIONS" "$OUTPUTS" "$input_length" "$mode"
        done
    done
done

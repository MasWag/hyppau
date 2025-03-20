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
readonly INPUT_LENGTHS="500 1000 1500 2000 2500 3000 3500 4000 4500 5000"
readonly REPETITIONS=10

cd "$(dirname "$0")" || exit 1

for input_length in $INPUT_LENGTHS; do
    for mode in $MODES; do
        for _ in $(seq 1 $REPETITIONS); do
            ./run.sh "$ACTIONS" "$OUTPUTS" "$input_length" "$mode"
        done
    done
done

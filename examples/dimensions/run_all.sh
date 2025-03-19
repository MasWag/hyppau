#!/bin/sh -ue
################################################################
# Name
#  run_all.sh
#
# Description
#  Script to run experiments on Dimension benchmark.
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
readonly DIMENSIONS="2 3 4 5"
readonly REPETITIONS=10

for mode in $MODES; do
    for dimensions in $DIMENSIONS; do
        for _ in $(seq 1 $REPETITIONS); do
        ./run.sh "$dimensions" "$mode"
        done
    done
done

#!/bin/sh -ue
################################################################
# Name
#  run_all.sh
#
# Description
#  Script to run all experiments on Small benchmark.
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

readonly MODES="naive fjs online naive-filtered fjs-filtered online-filtered"
readonly REPETITIONS=10

for mode in $MODES; do
    for _ in $(seq 1 $REPETITIONS); do
        ./run.sh "$mode"
    done
done
#!/bin/sh -ue
################################################################
# Name
#  run_multiple_all.sh
#
# Description
#  Script to run all experiments on Stuttering Robustness benchmark giving multiple words.
#
# Example
#  ./run_multiple.sh
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

readonly MODES="naive fjs naive-filtered fjs-filtered"
readonly WORD_SIZES="2 3 4 5 6 7 8 9 10"
readonly REPETITIONS=10

cd "$(dirname "$0")" || exit 1

for word_size in $WORD_SIZES; do
    for mode in $MODES; do
        for _ in $(seq 1 $REPETITIONS); do
            ./run_multiple.sh "$word_size" "$mode"
        done
    done
done

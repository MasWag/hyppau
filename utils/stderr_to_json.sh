#!/bin/sh -ue
################################################################
# Name
#  stderr_to_json.sh
#
# Description
#  Generate a JSON file from the logs of HypPAu.
#
# Prerequisites
#  - jo (https://jpmens.net/2016/03/05/a-shell-command-to-create-json-jo/)
#    - You can install jo with brew install jo on macOS (with Homebrew)
#  - jq
#
# Example
#  ./stderr_to_json.sh
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

cd "$(dirname "$0")" || exit 1

for file in ../logs/*.stderr.log; do
    benchmark=$(basename "$file" | cut -d - -f 1)
    id=$(basename "$file" | sed 's/.stderr.log//;')
    case $benchmark in
        network_pair)
            length=$(basename "$file" | cut -d - -f 2)
            mode=$(basename "$file" | cut -d - -f 3,4 | sed 's/-[0-9]\+//;')
            ;;
        *)
            length=$(basename "$file" | cut -d - -f 3)
            mode=$(basename "$file" | cut -d - -f 4,5 | sed 's/-[0-9]\+//;')
    esac
    if [ "$mode" = fjs ] || [ "$mode" = fjs-filtered ]; then
        kmp_time=$(grep kmp "$file" | tr -d ')' | sed 's/^.* //;')
        qs_time=$(grep quick "$file" | tr -d ')' | sed 's/^.* //;')
    else
        kmp_time=""
        qs_time=""
    fi
    if [ "$benchmark" = dimensions ]; then
        dimension=$(basename "$file" | cut -d - -f 2)
        jo -p id="$id" benchmark="$benchmark" length="$length" mode="$mode" dimension="$dimension" kmp_time="$kmp_time" qs_time="$qs_time"
    elif [ "$benchmark" != network_pair ]; then
        alphabet=$(basename "$file" | cut -d - -f 2)
        jo -p id="$id" benchmark="$benchmark" length="$length" mode="$mode" alphabet="$alphabet" kmp_time="$kmp_time" qs_time="$qs_time"
    else
        jo -p id="$id" benchmark="$benchmark" length="$length" mode="$mode" kmp_time="$kmp_time" qs_time="$qs_time"
    fi
done | jq -s .

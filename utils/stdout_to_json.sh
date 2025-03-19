#!/bin/sh -ue
################################################################
# Name
#  stdout_to_json.sh
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
#  ./stdout_to_json.sh
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

cd "$(dirname "$0")" || exit 1

for file in ../logs/*.output.log; do
    id=$(basename "$file" | grep -o '2025[0-9]\+-[0-9]\+')
    match_size="$(sort "$file" | uniq | wc -l)"
    jo -p id="$id" match_size="$match_size"
done | jq -s .

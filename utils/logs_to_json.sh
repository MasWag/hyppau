#!/bin/bash -ue
################################################################
# Name
#  logs_to_json.sh
#
# Description
#  Generate a JSON file from the logs.
#
# Prerequisites
#  - jc (https://kellyjonbrazil.github.io/jc/docs/)
#    - You can install jc with brew install jc on macOS (with Homebrew)
#  - jo (https://jpmens.net/2016/03/05/a-shell-command-to-create-json-jo/)
#    - You can install jo with brew install jo on macOS (with Homebrew)
#  - jq
#
# Example
#  ./logs_to_json.sh
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

cd "$(dirname "$0")" || exit 1

jq -s 'map(.[]) | group_by(.id) | map(add)' <(./stdout_to_json.sh ) <(./stderr_to_json.sh ) <(./gtime_to_json.sh) > ../logs/summary.json




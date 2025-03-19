#!/bin/sh
################################################################
# Name
#  gtime_to_json.sh
#
# Description
#  Generate a JSON file from the logs of GNU time.
#
# Example
#  ./gtime_to_json.sh
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

# Move to the 
cd "$(dirname "$0")" || exit 1

for file in ../logs/*.gtime.log; do
    jc --time < "$file" |
        # extract 2025[0-9]\+-[0-9]\+ from .command_being_timed and use it as "id"
        jq -r '. + {id: (.command_being_timed | match("2025[0-9]+-[0-9]+").string)}'
done | jq -s .

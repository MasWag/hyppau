#!/bin/sh -ue
################################################################
# Name
#  run.sh
#
# Description
#  Script to run experiments on Dimension benchmark.
#
# Prerequisites
# - Timeout in GNU coreutils
#
# Synopsis
#  ./run.sh [dimensions] [mode]
#
# Example
#  ./run.sh 3 filtered-naive
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

chr () {
   while read -r c; do
       printf "\\$(printf '%03o' "$c")\n"
   done
}

timestamp=$(date "+%Y%m%d-%H%M%S.%3N")
readonly TIMEOUT=600 # We set the timeout to 10 minutes

measure_time () {
    log_name="${LOG_DIR}/${timestamp}.gtime.log"
    # Add a brief delay to prevent I/O related errors
    sleep 1
    # find gtime on macOS
    if [ "$(uname)" = 'Darwin' ]; then
        gtime -v -o "$log_name" timeout $TIMEOUT "$@"
    else
        /usr/bin/time -v -o "$log_name" timeout $TIMEOUT "$@"
    fi
}

# Constants
readonly INPUT_LENGTH=200
PROJECT_ROOT=$(cd "$(dirname "$0")" && cd ../.. && pwd)
LOG_DIR="${PROJECT_ROOT}/logs"
EXAMPLE_DIR="${PROJECT_ROOT}/examples"

# Check the arguments
if [ $# -ne 2 ]; then
  echo "Usage: $0 [dimensions] [mode]"
  exit 1
fi
readonly dimensions="$1"
readonly mode="$2"

# Generate models and logs
actions="$(seq 97 $((97 + dimensions - 1)) | chr | tr '\n' ',' | sed 's/,$//')"
seq $INPUT_LENGTH | "${EXAMPLE_DIR}/dimensions/gen_log.awk" -v ACTIONS="$actions" > "/tmp/dimensions-$dimensions-${INPUT_LENGTH}.input"
"${EXAMPLE_DIR}/dimensions/gen_dimensions.py" --dimensions "$dimensions" > "/tmp/dimensions_${dimensions}.json"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

measure_time "$PROJECT_ROOT/target/release/hyppau" -f "/tmp/dimensions_${dimensions}.json" -i "/tmp/dimensions-$dimensions-${INPUT_LENGTH}.input" -m "$mode" -o "${LOG_DIR}/dimensions-${dimensions}-${INPUT_LENGTH}-${mode}-${timestamp}.output.log" 2> "${LOG_DIR}/dimensions-${dimensions}-${INPUT_LENGTH}-${mode}-${timestamp}.stderr.log"

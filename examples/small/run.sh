#!/bin/sh -ue
################################################################
# Name
#  run.sh
#
# Description
#  Script to run experiments on Small benchmark.
#
# Synopsis
#  ./run.sh [mode]
#
# Example
#  ./run.sh fjs
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

timestamp=$(date "+%Y%m%d-%H%M%S")

measure_time () {
    log_name="${LOG_DIR}/${timestamp}.gtime.log"
    # find gtime on macOS
    if [ "$(uname)" = 'Darwin' ]; then
        gtime -v -o "$log_name" "$@"
    else
        /usr/bin/time -v -o "$log_name" "$@"
    fi
}

# Constants
PROJECT_ROOT=$(cd "$(dirname "$0")" && cd ../.. && pwd)
LOG_DIR="${PROJECT_ROOT}/logs"
EXAMPLE_DIR="${PROJECT_ROOT}/examples"

# Check the arguments
if [ $# -ne 1 ]; then
  echo "Usage: $0 [mode]"
  exit 1
fi
readonly mode="$1"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

measure_time "$PROJECT_ROOT/target/release/hyppau" -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt" -m "$mode" -o "${LOG_DIR}/small-${mode}-${timestamp}.output.log" 2> "${LOG_DIR}/small-${mode}-${timestamp}.stderr.log"

# Verify the result
sort "${LOG_DIR}/small-${mode}-${timestamp}.output.log" | uniq > "${LOG_DIR}/small-${mode}-${timestamp}.output.sorted.log"
diff "${LOG_DIR}/small-${mode}-${timestamp}.output.sorted.log" "${EXAMPLE_DIR}/small.expected"
if [ $? -eq 0 ]; then
    echo "Result matches the expected output."
else
    echo "Result does not match the expected output!"
    exit 1
fi
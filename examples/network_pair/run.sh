#!/bin/sh -ue
################################################################
# Name
#  run.sh
#
# Description
#  Script to run experiments on Network Pair benchmark.
#
# Synopsis
#  ./run.sh [input_length] [mode]
#
# Example
#  ./run.sh 1000 fjs
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

timestamp=$(date "+%Y%m%d-%H%M%S.%3N")

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
if [ $# -ne 2 ]; then
  echo "Usage: $0 [input_length] [mode]"
  exit 1
fi
readonly input_length="$1"
readonly mode="$2"

# Generate logs
"${EXAMPLE_DIR}/network_pair/gen_log.sh" "$input_length" > "/tmp/network_pair-${input_length}.input"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

measure_time "$PROJECT_ROOT/target/release/hyppau" -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "/tmp/network_pair-${input_length}.input" -m "$mode" -o "${LOG_DIR}/network_pair-${input_length}-${mode}-${timestamp}.output.log" 2> "${LOG_DIR}/network_pair-${input_length}-${mode}-${timestamp}.stderr.log"
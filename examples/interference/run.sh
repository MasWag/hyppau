#!/bin/bash -ue
################################################################
# Name
#  run.sh
#
# Description
#  Script to run experiments on Interference benchmark.
#
# Synopsis
#  ./run.sh [actions] [outputs] [input_length] [mode]
#
# Example
#  ./run.sh a,b 0,1 200 naive-filtered
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

measure_time () {
    log_name="${LOG_DIR}/${timestamp}.gtime.log"
    # Add a brief delay to prevent I/O related errors
    sleep 1
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
if [ $# -ne 4 ]; then
  echo "Usage: $0 [actions] [outputs] [input_length] [mode]"
  exit 1
fi
readonly actions="$1"
readonly outputs="$2"
readonly input_length="$3"
readonly mode="$4"

# Convert comma-separated actions to space-separated for Python script
py_actions=$(echo "$actions" | tr ',' ' ')
py_outputs=$(echo "$outputs" | tr ',' ' ')

# Generate models and logs
seq "$input_length" | "${EXAMPLE_DIR}/interference/gen_log.awk" -v ACTIONS="$actions" -v OUTPUTS="$outputs" > "/tmp/interference_${actions/,/}_${outputs/,/}-${input_length}.input"
"${EXAMPLE_DIR}/interference/gen_interference.py" --actions $py_actions --outputs $py_outputs > "/tmp/interference_${actions/,/}_${outputs/,/}.json"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

measure_time "$PROJECT_ROOT/target/release/hyppau" -f "/tmp/interference_${actions/,/}_${outputs/,/}.json" -i "/tmp/interference_${actions/,/}_${outputs/,/}-${input_length}.input" -m "$mode" -o "${LOG_DIR}/interference-${actions/,/}_${outputs/,/}-${input_length}-${mode}-${timestamp}.output.log" 2> "${LOG_DIR}/interference-${actions/,/}_${outputs/,/}-${input_length}-${mode}-${timestamp}.stderr.log"

#!/bin/bash -ue
################################################################
# Name
#  run.sh
#
# Description
#  Script to run experiments on Stuttering Robustness benchmark.
#
# Synopsis
#  ./run.sh [inputs] [outputs] [input_length] [mode]
#
# Example
#  ./run.sh a,b 0,1 100 fjs
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
  echo "Usage: $0 [inputs] [outputs] [input_length] [mode]"
  exit 1
fi
readonly inputs="$1"
readonly outputs="$2"
readonly input_length="$3"
readonly mode="$4"

# Convert comma-separated inputs to space-separated for Python script
py_inputs=$(echo "$inputs" | tr ',' ' ')
py_outputs=$(echo "$outputs" | tr ',' ' ')

# Generate models and logs
seq "$input_length" | "${EXAMPLE_DIR}/stuttering_robustness/gen_log.awk" -v ACTIONS="$inputs" -v OUTPUTS="$outputs" > "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}-${input_length}.input"
"${EXAMPLE_DIR}/stuttering_robustness/gen_stuttering_robustness.py" --inputs $py_inputs --outputs $py_outputs > "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}.json"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

measure_time "$PROJECT_ROOT/target/release/hyppau" -f "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}.json" -i "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}-${input_length}.input" -m "$mode" -o "${LOG_DIR}/stuttering_robustness-${inputs/,/}_${outputs/,/}-${input_length}-${mode}-${timestamp}.output.log" 2> "${LOG_DIR}/stuttering_robustness-${inputs/,/}_${outputs/,/}-${input_length}-${mode}-${timestamp}.stderr.log"

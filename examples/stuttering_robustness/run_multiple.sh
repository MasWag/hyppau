#!/bin/bash -ue
################################################################
# Name
#  run_multiple.sh
#
# Description
#  Script to run experiments on Stuttering Robustness benchmark giving multiple words.
#
# Synopsis
#  ./run_multiple.sh [inputs] [outputs] [word_size] [mode]
#
# Example
#  ./run_multiple.sh a,b 0,1 3 naive-filtered
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
readonly input_length=200
readonly input_size="$3"
readonly total_input_length="$((input_length * input_size))"
readonly mode="$4"

# Convert comma-separated inputs to space-separated for Python script
py_inputs=$(echo "$inputs" | tr ',' ' ')
py_outputs=$(echo "$outputs" | tr ',' ' ')

# Generate models and logs
seq "$total_input_length" | "${EXAMPLE_DIR}/stuttering_robustness/gen_log.awk" -v ACTIONS="$inputs" -v OUTPUTS="$outputs" > "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}-${total_input_length}.input"
input_opt=""
for i in $(seq 1 "$input_size"); do
    head -n "$(( input_length * i ))" "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}-${total_input_length}.input" |
        tail -n "$input_length" > "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}-${input_length}-${i}.input"
    input_opt="${input_opt} -i /tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}-${input_length}-${i}.input"
done
"${EXAMPLE_DIR}/stuttering_robustness/gen_stuttering_robustness.py" --inputs $py_inputs --outputs $py_outputs > "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}.json"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

log_name="stuttering_robustness-${inputs/,/}_${outputs/,/}-${input_length}-${mode}-${input_size}-${timestamp}"
measure_time "$PROJECT_ROOT/target/release/hyppau" -f "/tmp/stuttering_robustness_${inputs/,/}_${outputs/,/}.json" $input_opt -m "$mode" -o "${LOG_DIR}/${log_name}.output.log" 2> "${LOG_DIR}/${log_name}.stderr.log"

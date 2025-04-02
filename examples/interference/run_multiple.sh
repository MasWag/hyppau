#!/bin/bash -ue
################################################################
# Name
#  run_multiple.sh
#
# Description
#  Script to run experiments on Interference benchmark giving multiple words.
#
# Synopsis
#  ./run_multiple.sh [actions] [outputs] [word_size] [mode]
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
  echo "Usage: $0 [actions] [outputs] [input_length] [mode]"
  exit 1
fi
readonly actions="$1"
readonly outputs="$2"
readonly input_length=500
readonly input_size="$3"
readonly total_input_length="$((input_length * input_size))"
readonly mode="$4"

# Convert comma-separated actions to space-separated for Python script
py_actions=$(echo "$actions" | tr ',' ' ')
py_outputs=$(echo "$outputs" | tr ',' ' ')

# Generate models and logs
seq "$total_input_length" | "${EXAMPLE_DIR}/interference/gen_log.awk" -v ACTIONS="$actions" -v OUTPUTS="$outputs" > "/tmp/interference_${actions/,/}_${outputs/,/}-${total_input_length}.input"
input_opt=""
for i in $(seq 1 "$input_size"); do
    head -n "$(( input_length * i ))" "/tmp/interference_${actions/,/}_${outputs/,/}-${total_input_length}.input" |
        tail -n "$input_length" > "/tmp/interference_${actions/,/}_${outputs/,/}-${input_length}-${i}.input"
    input_opt="${input_opt} -i /tmp/interference_${actions/,/}_${outputs/,/}-${input_length}-${i}.input"
done
"${EXAMPLE_DIR}/interference/gen_interference.py" --actions $py_actions --outputs $py_outputs > "/tmp/interference_${actions/,/}_${outputs/,/}.json"

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

log_name="interference-${actions/,/}_${outputs/,/}-${input_length}-${mode}-${input_size}-${timestamp}"
measure_time "$PROJECT_ROOT/target/release/hyppau" -f "/tmp/interference_${actions/,/}_${outputs/,/}.json" $input_opt -m "$mode" -o "${LOG_DIR}/${log_name}.output.log" 2> "${LOG_DIR}/${log_name}.stderr.log"

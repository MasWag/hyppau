#!/bin/bash -ue
################################################################
# Name
#  run_multiple.sh
#
# Description
#  Script to run experiments on Network Pair benchmark giving multiple words.
#
# Synopsis
#  ./run_multiple.sh [word_size] [mode]
#
# Example
#  ./run_multiple.sh 3 naive-filtered
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
readonly input_length=1000
readonly input_size="$1"
readonly total_input_length="$((input_length * input_size))"
readonly mode="$2"

# Generate logs
"${EXAMPLE_DIR}/network_pair/gen_log.sh" "$total_input_length" > "/tmp/network_pair-${total_input_length}.input"
input_opt=""
for i in $(seq 1 "$input_size"); do
    head -n "$(( input_length * i ))" "/tmp/network_pair-${total_input_length}.input" |
        tail -n "$input_length" > "/tmp/network_pair-${input_length}-${i}.input"
    input_opt="${input_opt} -i /tmp/network_pair-${input_length}-${i}.input"
done

# Build the project
cargo build --release

# Run the experiments
mkdir -p "$LOG_DIR"

log_name="network_pair-${input_length}-${mode}-${input_size}-${timestamp}"
measure_time "$PROJECT_ROOT/target/release/hyppau" -f "${EXAMPLE_DIR}/network_pair/network_pair.json" $input_opt -m "$mode" -o "${LOG_DIR}/${log_name}.output.log" 2> "${LOG_DIR}/${log_name}.stderr.log"

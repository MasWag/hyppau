#!/bin/bash -ue
################################################################
# Name
#  gen_log.sh
#
# Description
#  Script to generate a random sequence for the Netowrk example.
#
# Example
#  ./gen_log.sh 100
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

# Constants for the script
readonly REQ_STOP_PROBABILITY=0.2
readonly RES_STOP_PROBABILITY=0.1

# Current status
request_active=0
response_active=0

# Check the number of arguments
if [ $# -ne 1 ]; then
    echo "Usage: $0 <length of logs>"
    exit 1
fi

# Generate logs
for _i in $(seq 1 "$1"); do
    # decide if we work on the request or the response by a random choice
    if (( RANDOM % 2 == 0 )); then
        # Generate a log on Request
        if [ "$request_active" == 0 ]; then
            # Start letter
            echo 'sq'
            request_active=1
        elif [ "$(echo "$RANDOM / 32767 < $REQ_STOP_PROBABILITY" | bc -l)" == 1 ]; then
            # End letter
            echo 'eq'
            request_active=0
        else
            # Data letter
            echo 'q'
        fi
    else
        # Generate a log on Response
        if [ "$response_active" == 0 ]; then
            # Start letter
            echo 'sp'
            response_active=1
        elif [ "$(echo "$RANDOM / 32767 < $RES_STOP_PROBABILITY" | bc -l)" == 1 ]; then
            # End letter
            echo 'ep'
            response_active=0
        else
            # Data letter
            echo 'p'
        fi
    fi
done

#!/usr/bin/awk -f
################################################################
# Name
#  gen_log.awk
#
# Description
#  Script to generate a random sequence for the stuttering robustness example.
#
# Example
#  seq 100 | ./gen_log.awk -v ACTIONS="a,b" -v OUTPUTS="0,1"
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

BEGIN {
    # Error handling
    if (ACTIONS == "") {
        print("Error: ACTIONS is not set. Usage: ./gen_log.awk -v ACTIONS=\"a,b\" -v OUTPUTS=\"0,1\"")
        exit 1
    }
    if (OUTPUTS == "") {
        print("Error: OUTPUTS is not set. Usage: ./gen_log.awk -v ACTIONS=\"a,b\" -v OUTPUTS=\"0,1\"")
        exit 1
    }
    # make an array of actions and outputs
    split(ACTIONS, actions, ",")
    split(OUTPUTS, outputs, ",")

    n_actions = 0
    for (i in actions) { n_actions++ }

    n_outputs = 0
    for (i in outputs) { n_outputs++ }
}

{
    action_index = int(rand() * n_actions + 1)
    output_index = int(rand() * n_outputs + 1)
    printf("%s_%s\n", actions[action_index], outputs[output_index])
}

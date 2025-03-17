#!/usr/bin/awk -f
################################################################
# Name
#  gen_log.awk
#
# Description
#  Script to generate a random sequence for the ManyDimensions example.
#
# Example
#  seq 100 | ./gen_log.awk -v ACTIONS="a,b" 
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
        print("Error: ACTIONS is not set. Usage: ./gen_log.awk -v ACTIONS=\"a,b\"")
        exit 1
    }
    active = 0

    # make an array of actions and outputs
    split(ACTIONS, actions, ",")

    n_actions = 0
    for (i in actions) { n_actions++ }
}

active {
    if (rand() < 0.3) {
        active = 0
    } else {
        printf("%s\n", actions[action_index])
    }
}

!active {
    print("@")
    action_index = int(rand() * n_actions + 1)
    active = 1
}

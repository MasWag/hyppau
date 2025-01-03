#!/usr/bin/env python3
################################################################
# Name
#  gen_stuttering_robustness.py
#
# Description
#  Generates stuttering robustness examples in JSON representation.
#
# Options
#  *  -h, --help                        Show the help message and exit
#  *  --inputs ACTIONS [ACTIONS ...]   List of inputs (e.g., a b)
#  *  --outputs OUTPUTS [OUTPUTS ...]   List of outputs (e.g., 0 1)
#  *  -o, --output-file OUTPUT_FILE     Output JSON file name (if omitted, prints to stdout)
#
# Examples
#  * ./gen_stuttering_robustness.py --inputs a b --outputs 0 1
#  * ./gen_stuttering_robustness.py --inputs x y z --outputs 0 1 2
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

import json
import itertools
import argparse

def generate_instance(inputs, outputs):
    dimensions = 2
    # Initial/final states + initial branching + waiting for 1 and  waiting for 2 (but -1 to prevent nondeterminism)
    states = (len(inputs) * len(outputs)) * 2 + len(inputs) * len(outputs) * (len(inputs) * len(outputs) - 1) + 2
    instance = {
        "dimensions": dimensions,
        "states": [{"id": i, "is_initial": (i == 0), "is_final": (i == states - 1)} for i in range(states)],
        "transitions": []
    }
    alphabet_size = len(inputs) * len(outputs)
    final_state = states - 1
    # Map a letter for each state
    letter_map = {}


    for idx, (act, val) in enumerate(itertools.product(inputs, outputs), start=1):
        instance["transitions"].append({
            "from": 0,
            "to": idx,
            "label": [f"{act}_{val}", 0]
        })
        letter_map[idx] = (f"{act}_{val}", 0)
        instance["transitions"].append({
            "from": idx,
            "to": idx,
            "label": [f"{act}_{val}", 0]
        })
        instance["transitions"].append({
            "from": idx,
            "to": idx + alphabet_size,
            "label": [f"{act}_{val}", 1]
        })
        for val2 in outputs:
            if val2 != val:
                instance["transitions"].append({
                    "from": idx,
                    "to": final_state,
                    "label": [f"{act}_{val2}", 1]
                })
        instance["transitions"].append({
            "from": idx + alphabet_size,
            "to": idx + alphabet_size,
            "label": [f"{act}_{val}", 1]
        })
        letter_map[idx + alphabet_size] = (f"{act}_{val}", 1)
    for idx, (act, val) in enumerate(itertools.product(inputs, outputs), start=1):
        i = 1
        for (act2, val2) in itertools.product(inputs, outputs):
            if act2 != act or val2 != val:
                instance["transitions"].append({
                    "from": idx + alphabet_size,
                    "to": idx + alphabet_size * (1 + i),
                    "label": [f"{act2}_{val2}", 0]
                })
                letter_map[idx + alphabet_size * (1 + i)] = (f"{act2}_{val2}", 0)
                # add a self-loop for the same action and value
                instance["transitions"].append({
                    "from": idx + alphabet_size * (1 + i),
                    "to": idx + alphabet_size * (1 + i),
                    "label": [f"{act2}_{val2}", 0]
                })
                # Jump to the state with f"{act2}_{val2}", 1 in letter_map
                for j in letter_map.keys():
                    if letter_map[j] == (f"{act2}_{val2}", 1):
                        instance["transitions"].append({
                            "from": idx + alphabet_size * (1 + i),
                            "to": j,
                            "label": [f"{act2}_{val2}", 1]
                        })
                        break
                # or jump to the final state with other val
                for val3 in outputs:
                    if val3 != val2:
                        instance["transitions"].append({
                            "from": idx + alphabet_size * (1 + i),
                            "to": final_state,
                            "label": [f"{act2}_{val3}", 1]
                        })
                i += 1

    return instance

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate a JSON file for a state transition system.")
    parser.add_argument("--inputs", nargs='+', required=True, help="List of inputs (e.g., a b)")
    parser.add_argument("--outputs", nargs='+', type=int, required=True, help="List of outputs (e.g., 0 1)")
    parser.add_argument("-o", "--output-file", type=str, help="Output JSON file name (if omitted, prints to stdout)")

    args = parser.parse_args()

    instance_data = generate_instance(args.inputs, args.outputs)

    if args.output_file:
        with open(args.output_file, "w") as f:
            json.dump(instance_data, f, indent=2)
        print(f"JSON file generated: {args.output_file}")
    else:
        print(json.dumps(instance_data, indent=2))

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
    states = len(inputs) * len(outputs) + 2
    instance = {
        "dimensions": dimensions,
        "states": [{"id": i, "is_initial": (i == 0), "is_final": (i == states - 1)} for i in range(states)],
        "transitions": []
    }

    for idx, (act, val) in enumerate(itertools.product(inputs, outputs), start=1):
        instance["transitions"].append({
            "from": 0,
            "to": idx,
            "label": [f"{act}_{val}", 0]
        })
        instance["transitions"].append({
            "from": idx,
            "to": idx,
            "label": [f"{act}_{val}", 0]
        })
        instance["transitions"].append({
            "from": idx,
            "to": idx,
            "label": [f"{act}_{val}", 1]
        })
        instance["transitions"].append({
            "from": idx,
            "to": 0,
            "label": [f"{act}_{val}", 1]
        })
        instance["transitions"].append({
            "from": idx,
            "to": states - 1,
            "label": [f"{act}_{outputs[1 - outputs.index(val)]}", 1]
        })

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

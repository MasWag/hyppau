#!/usr/bin/env python3
################################################################
# Name
#  gen_interference.py
#
# Description
#  Generates interference examples in JSON representation.
#
# Options
#  *  -h, --help                        Show the help message and exit
#  *  --actions ACTIONS [ACTIONS ...]   List of actions (e.g., a b)
#  *  --outputs OUTPUTS [OUTPUTS ...]   List of outputs (e.g., 0 1)
#  *  -o, --output-file OUTPUT_FILE     Output JSON file name (if omitted, prints to stdout)
#
# Examples
#  * ./gen_interference.py --actions a b --outputs 0 1
#  * ./gen_interference.py --actions x y z --outputs 0 1 2
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

def generate_instance(actions, outputs):
    dimensions = 2
    states = len(actions) * len(outputs) * 2 + 3
    instance = {
        "dimensions": dimensions,
        "states": [{"id": i, "is_initial": (i == 0), "is_final": (i == states - 1)} for i in range(states)],
        "transitions": []
    }

    for idx, (act, val) in enumerate(itertools.product(actions, outputs)):
        instance["transitions"].append({
            "from": 0,
            "to": idx + 1,
            "label": [f"{act}_{val}", 0]
        })
        instance["transitions"].append({
            "from": idx + 1,
            "to": len(actions) * len(outputs) + 1,
            "label": [f"{act}_{val}", 1]
        })

    for idx, (act, val) in enumerate(itertools.product(actions, outputs), start=len(actions) * len(outputs) + 2):
        instance["transitions"].append({
            "from": len(actions) * len(outputs) + 1,
            "to": idx,
            "label": [f"{act}_{val}", 0]
        })
        instance["transitions"].append({
            "from": idx,
            "to": len(actions) * len(outputs) + 1,
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
    parser.add_argument("--actions", nargs='+', required=True, help="List of actions (e.g., a b)")
    parser.add_argument("--outputs", nargs='+', type=int, required=True, help="List of outputs (e.g., 0 1)")
    parser.add_argument("-o", "--output-file", type=str, help="Output JSON file name (if omitted, prints to stdout)")

    args = parser.parse_args()

    instance_data = generate_instance(args.actions, args.outputs)

    if args.output_file:
        with open(args.output_file, "w") as f:
            json.dump(instance_data, f, indent=2)
        print(f"JSON file generated: {args.output_file}")
    else:
        print(json.dumps(instance_data, indent=2))

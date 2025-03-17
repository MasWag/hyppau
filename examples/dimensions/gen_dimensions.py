#!/usr/bin/env python3
################################################################
# Name
#  gen_many_dimensions.py
#
# Description
#  Generates ManyDimensions examples in JSON representation.
#
# Options
#  *  -h, --help                        Show the help message and exit
#  *  --dimensions DIMENSIONS           The dimensions of the instance
#  *  -o, --output-file OUTPUT_FILE     Output JSON file name (if omitted, prints to stdout)
#
# Examples
#  * ./gen_many_dimensions.py --dimensions=2
#
# Author
#  Masaki Waga
#
# License
#  MIT License
################################################################

import json
import argparse

def generate_instance(dimensions):
    actions = [chr(ord('a') + i) for i in range(dimensions)]

    states = dimensions * 2 + 1
    instance = {
        "dimensions": dimensions,
        "states": [{"id": i, "is_initial": (i == 0), "is_final": (i == states - 1)} for i in range(states)],
        "transitions": []
    }

    # The initial transitions
    for idx in range(dimensions):
        instance["transitions"].append({
            "from": idx,
            "to": idx + 1,
            "label": ['@', idx]
        })

    # The loop transitions
    for idx in range(dimensions):
        instance["transitions"].append({
            "from": idx + dimensions,
            "to": (idx + 1) % dimensions + dimensions,
            "label": [actions[idx], idx]
        })

    # The final transitions
    instance["transitions"].append({
        "from": 2 * dimensions,
        "to": 2 * dimensions + 1,
        "label": ['@', 0]
    })

    return instance

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate a JSON file for a state transition system.")
    parser.add_argument("--dimensions", type=int, required=True, help="The dimensions of the instance")
    parser.add_argument("-o", "--output-file", type=str, help="Output JSON file name (if omitted, prints to stdout)")

    args = parser.parse_args()

    instance_data = generate_instance(args.dimensions)

    if args.output_file:
        with open(args.output_file, "w") as f:
            json.dump(instance_data, f, indent=2)
        print(f"JSON file generated: {args.output_file}")
    else:
        print(json.dumps(instance_data, indent=2))

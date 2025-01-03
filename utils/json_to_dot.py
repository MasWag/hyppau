#!/usr/bin/env python3
import json
import argparse
import sys

def json_to_dot(data, name="G"):
    """
    Convert a graph in the given JSON format into a DOT graph.
    """
    lines = []
    lines.append(f"digraph {name} {{")
    lines.append("    rankdir=LR;")
    lines.append("    node [fontname=\"Helvetica\"];")
    lines.append("")

    # 1) invisible start node
    lines.append("    __start__ [shape=point style=invis];")

    # 2) render all states
    for st in data.get("states", []):
        sid = st["id"]
        attrs = []

        # final states as doublecircle, else circle
        if st.get("is_final", False):
            attrs.append("shape=doublecircle")
        else:
            attrs.append("shape=circle")

        # label is just the ID (you could change to something else)
        attrs.append(f"label=\"{sid}\"")

        lines.append(f"    {sid} [{', '.join(attrs)}];")

    lines.append("")

    # 3) connect __start__ to each initial state
    for st in data.get("states", []):
        if st.get("is_initial", False):
            sid = st["id"]
            lines.append(f"    __start__ -> {sid};")

    lines.append("")

    # 4) render all transitions
    for tr in data.get("transitions", []):
        src = tr["from"]
        dst = tr["to"]
        lbl = tr.get("label", [])
        # join label components by comma (e.g. "a_2,1")
        if isinstance(lbl, list) and len(lbl) >= 2:
            # escape quotes if needed
            labstr = f"{lbl[0]},{lbl[1]}"
        else:
            labstr = str(lbl)
        # quote the label
        labstr = labstr.replace("\"", "\\\"")
        lines.append(f"    {src} -> {dst} [label=\"{labstr}\"];")

    lines.append("}")
    return "\n".join(lines)

def main():
    parser = argparse.ArgumentParser(
        description="Translate a JSON‐encoded graph into Graphviz (DOT) format."
    )
    parser.add_argument(
        "input", nargs="?", type=argparse.FileType("r"), default=sys.stdin,
        help="Path to the JSON file (defaults to stdin)."
    )
    parser.add_argument(
        "-o", "--output", type=argparse.FileType("w"), default=sys.stdout,
        help="Write DOT output to this file (defaults to stdout)."
    )
    parser.add_argument(
        "--name", type=str, default="G",
        help="Name of the DOT graph (default: G)."
    )

    args = parser.parse_args()
    try:
        data = json.load(args.input)
    except json.JSONDecodeError as e:
        sys.exit(f"Error parsing JSON: {e}")

    dot = json_to_dot(data, name=args.name)
    args.output.write(dot)
    if args.output is not sys.stdout:
        args.output.write("\n")

if __name__ == "__main__":
    main()

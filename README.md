Hyper Pattern Matching
======================

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)

This is the source code repository for hyper pattern matching — a prototype tool for Hyper Pattern Matching.

Usage
-----

### Options

- **-h**, **--help**: Print a help message.
- **-q**, **--quiet**: Quiet mode. Causes any results to be suppressed.
- **-V**, **--version**: Print the version.
- **-i** *file*, **--input** *file*: Read the log from the *file*. The i-th input file is labeled with `i` in the output.
- **-f** *file*, **--automaton** *file*: Read an automaton written in JSON format from *file*.
- **-g**, **--graphviz**: Print the automaton in Graphviz DOT format.
- **-o** *file*, **--output** *file*: Write the output to *file* instead of stdout.
- **-m** *mode*, **--mode** *mode*: Choose the matching mode: naive, online, fjs, or naive-filtered (default: naive).
- **-v**, **--verbose**: Increase verbosity. Use `-v` for debug-level messages and `-vv` for trace-level messages.

### Synopsis

```bash
hyper-pattern-matching [OPTIONS] -f FILE [FILE...]
```

### Options

- **-h**, **--help**: Print a help message.
- **-q**, **--quiet**: Quiet mode. Causes any results to be suppressed.
- **-V**, **--version**: Print the version.
- **-i** *file*, **--input** *file*: Read the log from the *file*. The i-th input file is labeled with `i` in the output.
- **-f** *file*, **--automaton** *file*: Read an automaton written in JSON format from *file*.
- **-g**, **--graphviz**: Print the automaton in Graphviz DOT format.
- **-o** *file*, **--output** *file*: Write the output to *file* instead of stdout.
- **-v**, **--verbose**: Increase verbosity. Use `-v` for debug-level logging and `-vv` for trace-level logging.

### Automaton JSON Format

The JSON format for the input automaton is as follows:

```json
{
  "dimensions": 2,
  "states": [
    { "id": 0, "is_initial": true, "is_final": false },
    { "id": 1, "is_initial": false, "is_final": true },
    { "id": 2, "is_initial": false, "is_final": false }
  ],
  "transitions": [
    { "from": 0, "to": 1, "label": ["a", 0] },
    { "from": 0, "to": 2, "label": ["b", 1] },
    { "from": 1, "to": 2, "label": ["c", 0] },
    { "from": 2, "to": 0, "label": ["d", 1] }
  ]
}
```

- **dimensions**: The number of dimensions in the automaton.
- **states**: A list of states where each state has an `id`, a boolean indicating if it is initial (`is_initial`), and a boolean indicating if it is final (`is_final`).
- **transitions**: A list of transitions where each transition specifies the source state (`from`), target state (`to`), and the label associated with the transition.

Installation
------------

### Requirements

- **Rust & Cargo:** Ensure you have Rust (and its package manager, Cargo) installed.
  - If not, install them via [rustup](https://rustup.rs).

### Installing from Source

1. **Clone the Repository:**
   ```bash
   git clone https://github.com/MasWag/hyper-pattern-matching.git
   cd hyper-pattern-matching
   ```
2. **Build the Project:**
   Build the project in release mode for an optimized binary:
   ```bash
   cargo build --release
   ```
3. **Run the Binary:**
   The compiled executable will be located in `target/release/`. You can run it by `cargo run --release`, or directly:
   ```bash
   ./target/release/hyper-pattern-matching
   ```
   For easier access, you might copy it to a directory in your PATH:
   ```bash
   cp target/release/hyper-pattern-matching /usr/local/bin/
   ```

Examples
--------

```bash
hyper-pattern-matching -f automaton.json -i logfile1.txt -i logfile2.txt
```

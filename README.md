Hyper Pattern Matching
======================

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)

This is the source code repository for hyper pattern matching --- A prototype tool for Hyper Pattern Matching.

Usage
-----

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

# STELE
Plain-ASCII recipe files embedded in MCU Flash, read by a browser over serial, and passed to an AI agent to generate a GUI.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)

## Quick Start

### 1. Write a recipe

Create a plain-ASCII `.txt` file describing the GUI.


### 2. Generate the C header

```bash
cargo build

# .txt input → stele.h (identifiers: stele / STELE)
cargo run -- path/to/recipe.txt -o output_dir
cargo run -- path/to/recipe.txt --stdout > stele.h

# .json input → pktdef.h (identifiers: pktdef / PKTDEF)
cargo run -- path/to/definition.json -o output_dir
cargo run -- path/to/definition.json --stdout > pktdef.h
```

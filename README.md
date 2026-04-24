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

## stele CLI

```
stele v0.2.0 -- recipe text / packet-definition JSON to C header converter

Usage:
  stele <input.txt|input.json> -o <out_dir> [--chunk-size 248] [--max-chunks 120] [--minify]
  stele <input.txt|input.json> --stdout [--chunk-size 248] [--minify]

Input extension determines the C identifier prefix:
  .txt   -> stele / STELE   (output: stele.h,  load function: load_stele)
  .json  -> pktdef / PKTDEF (output: pktdef.h, load function: load_pktdef)

Options:
  -o, --out-dir <dir>   Output directory (default: out)
  --stdout              Print header to stdout (no files written)
  --chunk-size <n>      Chunk size in bytes, 1..=251 (default: 248)
  --max-chunks <n>      Maximum number of chunks (default: 120)
  --minify              Strip comment lines and collapse blank lines
  -h, --help            Show this help
```

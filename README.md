# Panoramix-RS

High-performance EVM decompiler written in Rust, inspired by the original [Panoramix](https://github.com/eveem-org/panoramix) by palkeo.

## Overview

Panoramix-RS is a Rust port of the Panoramix Python decompiler, designed to decompile Ethereum Virtual Machine (EVM) bytecode back into readable Solidity-like code. This implementation aims for better performance and memory safety while maintaining the core decompilation logic.

## Status

This is a work in progress. The core decompilation engine is functional but may still have bugs and missing features compared to the original Python version.

## Installation

### From Source

```bash
git clone https://github.com/samsheff/panoramix_rs.git
cd panoramix_rs
cargo build --release
```

The binary will be at `target/release/panoramix_rs`.

## Usage

```bash
# Decompile from hex bytecode
./target/release/panoramix_rs 6004600d60003960046000f30011223344

# Or pass via stdin
echo "6004600d60003960046000f30011223344" | ./target/release/panoramix_rs
```

## Project Structure

- `src/core/` - Core decompilation logic (types, arithmetic, masks, memory locations)
- `src/function/` - Function detection and analysis
- `src/vm/` - EVM bytecode interpretation
- `src/decompiler/` - Main decompilation entry point
- `src/folder/` - Expression folding and simplification
- `src/prettify/` - Code formatting and beautification
- `src/sparser/` - Smart contract parsing
- `src/matcher/` - Pattern matching for opcodes
- `src/loader/` - Contract bytecode loading

## Credits

- Original Panoramix by [palkeo](https://github.com/palkeo/panoramix)
- Forked from the unmaintained [eveem-org/panoramix](https://github.com/eveem-org/panoramix)

## License

MIT

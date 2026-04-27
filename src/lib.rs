//! Panoramix-RS: High-performance EVM decompiler written in Rust
//!
//! This is a port of the Panoramix Python decompiler, reimplemented in Rust
//! for significantly better performance on large contracts.
//!
//! ## Architecture
//!
//! The decompiler works in several stages:
//!
//! 1. **Loading & Disassembly**: Parse EVM bytecode into opcodes
//! 2. **Function Detection**: Identify function entry points via symbolic execution
//! 3. **VM Execution**: Symbolic EVM execution to build a trace
//! 4. **Loop Detection**: Convert jumps to structured while loops
//! 5. **Simplification**: Algebraic simplification of expressions
//! 6. **Pretty Printing**: Convert to human-readable Solidity-like syntax

pub mod core;
pub mod loader;
pub mod vm;
pub mod stack;
pub mod matcher;
pub mod function;
pub mod contract;
pub mod prettify;
pub mod folder;
pub mod sparser;
pub mod whiles;

pub mod decompiler;
pub use decompiler::{Decompiler, decompile_bytecode, pretty_contract};

//! Panoramix-RS Binary
//!
//! Command-line interface to the EVM decompiler.

use panoramix_rs::decompiler::{self, pretty_contract};
use std::env;
use std::process;

fn main() {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <bytecode_hex>", args[0]);
        eprintln!();
        eprintln!("Decompile EVM bytecode to Solidity-like pseudocode.");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} 0x60606040523415600e576c5d73c3e1514a600a6000506000818155", args[0]);
        eprintln!("  {} $(cat contract.hex)", args[0]);
        process::exit(1);
    }

    let bytecode_hex = &args[1];

    println!("Decompiling bytecode...");
    println!();

    match decompiler::decompile_bytecode(bytecode_hex) {
        Ok(contract) => {
            let output = pretty_contract(&contract);
            println!("{}", output);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

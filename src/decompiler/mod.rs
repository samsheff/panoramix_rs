//! Main decompiler interface
//!
//! Entry point for decompiling EVM bytecode.

use crate::contract::Contract;
use crate::folder;
use crate::function::Function;
use crate::loader::Bytecode;
use crate::vm::VM;
use crate::whiles;
use std::collections::HashMap;

/// Decompiler configuration
#[derive(Debug, Clone)]
pub struct DecompilerConfig {
    /// Timeout in seconds
    pub timeout: u64,
    /// Only decompile specific function
    pub only_func_name: Option<String>,
    /// Web3 API URL (for address decompilation)
    pub rpc_url: Option<String>,
}

impl Default for DecompilerConfig {
    fn default() -> Self {
        Self {
            timeout: 180,
            only_func_name: None,
            rpc_url: None,
        }
    }
}

/// Decompiler instance
#[derive(Debug)]
pub struct Decompiler {
    config: DecompilerConfig,
}

impl Decompiler {
    /// Create a new Decompiler with the given configuration
    pub fn new(config: DecompilerConfig) -> Self {
        Self { config }
    }

    /// Decompile from bytecode (hex string)
    pub fn decompile_bytecode(&self, code: &str) -> Result<Contract, String> {
        // Load bytecode
        let bytecode = Bytecode::load(code)?;

        // Find function destinations using VM in fdests mode
        let func_infos = self.find_functions(&bytecode)?;

        // If no functions found, treat the entire bytecode as a single function
        if func_infos.is_empty() {
            return self.decompile_single_function(&bytecode, 0, 0);
        }

        // Process each function
        let mut functions: HashMap<u64, Function> = HashMap::new();

        for (pos, hash, _stack) in &func_infos {
            // Run VM to get trace for this function
            let trace = self.run_function_tracing(&bytecode, *pos)?;

            // Apply make_whiles to convert gotos to loops
            let trace_with_whiles = whiles::make_whiles(trace, self.config.timeout);

            // Apply folder.fold to simplify
            let trace_folded = folder::fold(trace_with_whiles);

            // Create Function with hash and trace
            let func = Function::new(*hash as u32, trace_folded);

            functions.insert(*hash as u64, func);
        }

        // Create Contract with all functions
        let mut contract = Contract::new();
        for (_, func) in functions {
            contract.add_function(func);
        }

        // Post-process contract (storage layout, AST generation)
        contract.postprocess();

        Ok(contract)
    }

    /// Find function destinations in bytecode
    fn find_functions(&self, bytecode: &Bytecode) -> Result<Vec<(u64, u128, Vec<crate::core::types::Exp>)>, String> {
        let mut vm = VM::with_fdests_mode(bytecode.clone());
        let functions = vm.find_functions();

        // If no functions found (stub implementation), return just the entry point
        if functions.is_empty() {
            // Return entry point as a default function with hash 0
            return Ok(vec![(0, 0, vec![])]);
        }

        Ok(functions)
    }

    /// Run VM tracing for a specific function position
    fn run_function_tracing(&self, bytecode: &Bytecode, start_pos: u64) -> Result<crate::core::types::Trace, String> {
        let timeout_ms = self.config.timeout * 1000;
        let mut vm = VM::with_timeout(bytecode.clone(), timeout_ms);
        let trace = vm.run_from(start_pos, vec![], None);
        Ok(trace)
    }

    /// Decompile a single function at the given position
    fn decompile_single_function(&self, bytecode: &Bytecode, start_pos: u64, hash: u64) -> Result<Contract, String> {
        // Run VM to get trace
        let trace = self.run_function_tracing(bytecode, start_pos)?;

        // Apply make_whiles to convert gotos to loops
        let trace_with_whiles = whiles::make_whiles(trace, self.config.timeout);

        // Apply folder.fold to simplify
        let trace_folded = folder::fold(trace_with_whiles);

        // Create Function with hash and trace
        let func = Function::new(hash as u32, trace_folded);

        // Create Contract with the function
        let mut contract = Contract::new();
        contract.add_function(func);

        // Post-process contract
        contract.postprocess();

        Ok(contract)
    }

    /// Decompile from contract address
    pub fn decompile_address(&self, _address: &str) -> Result<Contract, String> {
        // TODO: implement Web3 lookup
        Err("Address decompilation requires Web3 RPC URL. Please provide bytecode directly.".to_string())
    }
}

impl Default for Decompiler {
    fn default() -> Self {
        Self::new(DecompilerConfig::default())
    }
}

/// Convenience function to decompile bytecode hex string
pub fn decompile_bytecode(code: &str) -> Result<Contract, String> {
    Decompiler::default().decompile_bytecode(code)
}

/// Pretty print a contract to string
pub fn pretty_contract(contract: &Contract) -> String {
    let mut output = String::new();

    // Print storage definitions
    if !contract.stor_defs().is_empty() {
        output.push_str("## Storage Layout\n\n");
        for var in contract.stor_defs() {
            output.push_str(&format!("  {} {}: {}\n", var.type_name, var.name, var.slot));
        }
        output.push('\n');
    }

    // Print functions
    output.push_str("## Functions\n\n");

    for func in contract.functions_list() {
        output.push_str(&func.print());
        output.push_str("\n\n");
    }

    output.trim_end().to_string()
}

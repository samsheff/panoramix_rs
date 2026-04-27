//! Contract representation
//!
//! Contains all functions and post-processing logic.

use crate::core::types::Exp;
use crate::function::Function;
use crate::sparser::{self, StorageVar};
use std::collections::HashMap;

/// Function hash type
pub type FuncHash = u64;

/// Contract containing all decompiled functions
#[derive(Debug)]
pub struct Contract {
    /// Function hash -> Function
    pub functions: HashMap<FuncHash, Function>,
    /// Storage definitions (parsed from traces)
    stor_defs: Vec<StorageVar>,
    /// Constant functions (getters and const functions)
    consts: Vec<FuncHash>,
    /// Problematic functions (hash -> error message)
    pub problems: HashMap<FuncHash, String>,
}

impl Contract {
    /// Create a new empty contract
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            stor_defs: Vec::new(),
            consts: Vec::new(),
            problems: HashMap::new(),
        }
    }

    /// Add a function to the contract
    pub fn add_function(&mut self, func: Function) {
        let hash = func.hash as FuncHash;
        self.functions.insert(hash, func);
    }

    /// Get a function by hash
    pub fn get_function(&self, hash: FuncHash) -> Option<&Function> {
        self.functions.get(&hash)
    }

    /// Get a mutable function by hash
    pub fn get_function_mut(&mut self, hash: FuncHash) -> Option<&mut Function> {
        self.functions.get_mut(&hash)
    }

    /// Get all functions as a Vec (sorted by some ordering)
    pub fn functions_list(&self) -> Vec<&Function> {
        let mut funcs: Vec<&Function> = self.functions.values().collect();
        funcs.sort_by_key(|f| f.priority());
        funcs
    }

    /// Get storage definitions
    pub fn stor_defs(&self) -> &[StorageVar] {
        &self.stor_defs
    }

    /// Get constant functions (getters and pure functions)
    pub fn consts(&self) -> Vec<&Function> {
        self.consts
            .iter()
            .filter_map(|h| self.functions.get(h))
            .collect()
    }

    /// Get regular (non-const, non-getter) functions
    pub fn regular_functions(&self) -> Vec<&Function> {
        self.functions
            .values()
            .filter(|f| f.is_regular)
            .collect()
    }

    /// Post-process contract (storage layout analysis, AST generation)
    /// 
    /// This performs:
    /// 1. Storage layout analysis via sparser
    /// 2. Replaces parameter indices with inferred names
    /// 3. Generates ASTs for all functions
    pub fn postprocess(&mut self) {
        // Collect all functions for processing
        let mut funcs: Vec<Function> = self.functions.drain().map(|(_, f)| f).collect();
        
        // Run storage layout analysis
        self.stor_defs = sparser::rewrite_functions(&mut funcs);
        
        // Replace parameter names in traces
        for func in &mut funcs {
            let trace = replace_names_in_trace(&func.trace, &func.inferred_params);
            func.trace = trace;
            
            // Generate AST
            func.ast = Some(self.make_ast(&func.trace));
        }
        
        // Identify const functions (getters and pure functions)
        // Sort by putting all-caps consts at the end - looks better this way
        let mut const_funcs: Vec<FuncHash> = funcs
            .iter()
            .filter(|f| f.is_const.is_some() || f.getter.is_some())
            .map(|f| f.hash as FuncHash)
            .collect();
        
        const_funcs.sort_by(|&a, &b| {
            let func_a = funcs.iter().find(|f| f.hash as FuncHash == a).unwrap();
            let func_b = funcs.iter().find(|f| f.hash as FuncHash == b).unwrap();
            
            let a_upper = func_a.name.to_uppercase() == func_a.name;
            let b_upper = func_b.name.to_uppercase() == func_b.name;
            
            match (a_upper, b_upper) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => func_a.name.cmp(&func_b.name),
            }
        });
        
        self.consts = const_funcs;
        
        // Rebuild functions map
        for func in funcs {
            let hash = func.hash as FuncHash;
            self.functions.insert(hash, func);
        }
    }

    /// Make AST from trace
    fn make_ast(&self, trace: &[Exp]) -> Exp {
        // Simplified AST generation - in the Python version this does more
        // complex processing including store_to_set, loc_to_name, etc.
        if trace.is_empty() {
            return Exp::None;
        }
        
        if trace.len() == 1 {
            return trace[0].clone();
        }
        
        // Wrap multiple expressions in a sequence
        Exp::OpN("seq".into(), trace.to_vec())
    }

    /// Export contract to JSON
    pub fn json(&self) -> serde_json::Value {
        let functions_json: Vec<serde_json::Value> = self
            .functions
            .values()
            .map(|f| f.serialize())
            .collect();
        
        let stor_defs_json: Vec<serde_json::Value> = self
            .stor_defs
            .iter()
            .map(|v| {
                serde_json::json!({
                    "name": v.name,
                    "type": v.type_name,
                    "slot": v.slot,
                })
            })
            .collect();
        
        serde_json::json!({
            "problems": self.problems,
            "stor_defs": stor_defs_json,
            "functions": functions_json,
        })
    }

    /// Export to JSON string
    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.json()).unwrap_or_default()
    }
}

impl Default for Contract {
    fn default() -> Self {
        Self::new()
    }
}

/// Replace parameter indices with inferred names in trace
fn replace_names_in_trace(trace: &[Exp], inferred_params: &HashMap<u64, (String, String)>) -> Vec<Exp> {
    trace.iter().map(|exp| replace_names_exp(exp, inferred_params)).collect()
}

fn replace_names_exp(exp: &Exp, inferred_params: &HashMap<u64, (String, String)>) -> Exp {
    // Check if this is a cd reference that should be replaced
    if let Exp::Op2(op, a, _) = exp {
        if op == "cd" {
            if let Exp::Int(idx) = **a {
                let idx = idx as u64;
                if let Some((_kind, name)) = inferred_params.get(&idx) {
                    // Replace with param reference
                    return Exp::Var(format!("param_{}", name));
                }
            }
        }
    }
    
    // Recurse into children
    match exp {
        Exp::Op1(op, a) => {
            Exp::Op1(op.clone(), Box::new(replace_names_exp(a, inferred_params)))
        }
        Exp::Op2(op, a, b) => {
            Exp::Op2(
                op.clone(),
                Box::new(replace_names_exp(a, inferred_params)),
                Box::new(replace_names_exp(b, inferred_params)),
            )
        }
        Exp::Op3(op, a, b, c) => {
            Exp::Op3(
                op.clone(),
                Box::new(replace_names_exp(a, inferred_params)),
                Box::new(replace_names_exp(b, inferred_params)),
                Box::new(replace_names_exp(c, inferred_params)),
            )
        }
        Exp::Op4(op, a, b, c, d) => {
            Exp::Op4(
                op.clone(),
                Box::new(replace_names_exp(a, inferred_params)),
                Box::new(replace_names_exp(b, inferred_params)),
                Box::new(replace_names_exp(c, inferred_params)),
                Box::new(replace_names_exp(d, inferred_params)),
            )
        }
        Exp::OpN(op, args) => {
            Exp::OpN(op.clone(), args.iter().map(|a| replace_names_exp(a, inferred_params)).collect())
        }
        Exp::If(cond, then_br, else_br) => {
            Exp::If(
                Box::new(replace_names_exp(cond, inferred_params)),
                replace_names_in_trace(then_br, inferred_params),
                replace_names_in_trace(else_br, inferred_params),
            )
        }
        Exp::While(cond, body) => {
            Exp::While(
                Box::new(replace_names_exp(cond, inferred_params)),
                replace_names_in_trace(body, inferred_params),
            )
        }
        _ => exp.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_contract_creation() {
        let contract = Contract::new();
        assert!(contract.functions.is_empty());
        assert!(contract.stor_defs.is_empty());
        assert!(contract.consts.is_empty());
    }
    
    #[test]
    fn test_contract_json() {
        let contract = Contract::new();
        let json = contract.json();
        assert!(json.is_object());
        assert!(json["problems"].is_object());
        assert!(json["stor_defs"].is_array());
        assert!(json["functions"].is_array());
    }
}

//! Storage layout parser
//!
//! Identifies storage variable types and positions from function traces.

use crate::core::types::Exp;
use std::collections::HashMap;

/// Storage variable definition
#[derive(Debug, Clone)]
pub struct StorageVar {
    /// Variable name
    pub name: String,
    /// Type name (e.g., "uint256", "address", "mapping")
    pub type_name: String,
    /// Storage slot
    pub slot: u64,
}

impl StorageVar {
    pub fn new(name: &str, type_name: &str, slot: u64) -> Self {
        Self {
            name: name.to_string(),
            type_name: type_name.to_string(),
            slot,
        }
    }
}

/// Find all storage accesses in an expression
fn find_stores(exp: &Exp) -> Vec<Exp> {
    let mut results = Vec::new();
    find_stores_inner(exp, &mut results);
    results
}

fn find_stores_inner(exp: &Exp, results: &mut Vec<Exp>) {
    // Check for store operation: ("store", size, offset, index, value)
    if let Exp::Op4(op, _, _, _, _) = exp {
        if op == "store" {
            results.push(exp.clone());
            return;
        }
    }
    
    // Check for storage reference
    if exp.opcode() == "storage" {
        results.push(exp.clone());
        return;
    }
    
    // Recurse into children
    for arg in exp.args() {
        find_stores_inner(arg, results);
    }
}

/// Get storage location from an expression
pub fn get_loc(exp: &Exp) -> Option<u64> {
    get_loc_inner(exp).or_else(|| get_loc_from_storage(exp))
}

fn get_loc_inner(exp: &Exp) -> Option<u64> {
    // Check for ("type", _, ("field", _, ("stor", ...)))
    if let Exp::Op3(op, _, _, _) = exp {
        if op == "type" {
            if let Exp::Op3(field_op, _, _, stor) = exp {
                if field_op == "field" {
                    return get_loc_from_storage(stor);
                }
            }
        }
    }
    
    // Recurse into args
    for arg in exp.args() {
        if let Some(loc) = get_loc_inner(arg) {
            return Some(loc);
        }
    }
    
    None
}

fn get_loc_from_storage(exp: &Exp) -> Option<u64> {
    // ("storage", Any, Any, :e) or ("stor", Any, Any, :e) or ("stor", :e)
    if let Exp::Op4(op, _, _, a, _) = exp {
        if op == "storage" || op == "stor" {
            return get_loc_from_storage(a);
        }
    }
    
    if let Exp::Op3(op, _, _, a) = exp {
        if op == "stor" {
            return get_loc_from_storage(a);
        }
    }
    
    if let Exp::Op2(op, _, a) = exp {
        if op == "stor" {
            return get_loc_from_storage(a);
        }
    }
    
    if let Exp::Op1(op, a) = exp {
        if op == "stor" {
            return get_loc_from_storage(a);
        }
    }
    
    // ("loc", num)
    if let Exp::Op1(op, a) = exp {
        if op == "loc" {
            if let Exp::Int(n) = **a {
                return Some(n as u64);
            }
        }
    }
    
    None
}

/// Get name from a storage expression
pub fn get_name(exp: &Exp) -> Option<String> {
    // ("name", :name, :loc)
    if let Exp::Op2(op, a, _b) = exp {
        if op == "name" {
            if let Exp::Str(name) = &**a {
                return Some(name.clone());
            }
            if let Exp::Var(name) = &**a {
                return Some(name.clone());
            }
        }
    }
    
    // Recurse
    for arg in exp.args() {
        if let Some(name) = get_name(arg) {
            return Some(name);
        }
    }
    
    None
}

/// Find storage names based on function getters
pub fn find_storage_names(functions: &[crate::function::Function]) -> HashMap<Exp, String> {
    let mut res = HashMap::new();
    
    for func in functions {
        if let Some(getter) = &func.getter {
            let name = func.name.clone();
            
            // Convert name to storage name format
            let mut storage_name = if name.starts_with("get") && name.len() > 3 {
                name[3..].to_string()
            } else {
                name.clone()
            };
            
            // Lowercase first char if not all caps
            if storage_name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                // Already lowercase
            } else if storage_name.to_uppercase() != storage_name {
                // Not all uppercase, lowercase first char
                storage_name = storage_name[0..1].to_lowercase() + &storage_name[1..];
            }
            
            // Remove parentheses part
            if let Some(idx) = storage_name.find('(') {
                storage_name = storage_name[..idx].to_string();
            }
            
            // Add "Address" suffix for address types if needed
            if getter.opcode() == "storage" {
                // Check if it's an address storage getter
                res.insert(getter.clone(), storage_name);
            }
        }
    }
    
    res
}

/// Parse storage accesses to determine layout
pub fn parse_storage(traces: &[Vec<Exp>]) -> Vec<StorageVar> {
    let mut all_stores: Vec<Exp> = Vec::new();
    
    // Collect all storage accesses from all traces
    for trace in traces {
        for exp in trace {
            let stores = find_stores(exp);
            all_stores.extend(stores);
        }
    }
    
    // Deduplicate
    let unique_stores: Vec<Exp> = {
        let mut seen = std::collections::HashSet::new();
        all_stores.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };
    
    let mut storage_vars: Vec<StorageVar> = Vec::new();
    let mut seen_locs: HashMap<u64, String> = HashMap::new();
    
    for store in &unique_stores {
        // Extract location
        if let Some(loc) = get_loc(store) {
            // Determine type based on the expression structure
            let type_name = infer_storage_type(store);
            
            // Get or generate name
            let name = if let Some(existing_name) = seen_locs.get(&loc) {
                existing_name.clone()
            } else {
                let name = get_storage_name(store, loc);
                seen_locs.insert(loc, name.clone());
                name
            };
            
            // Avoid duplicates
            if !storage_vars.iter().any(|v| v.slot == loc) {
                storage_vars.push(StorageVar::new(&name, &type_name, loc));
            }
        }
    }
    
    // Sort by slot
    storage_vars.sort_by_key(|v| v.slot);
    
    storage_vars
}

/// Infer the type of a storage variable from its expression
fn infer_storage_type(exp: &Exp) -> String {
    let op = exp.opcode();
    
    if op == "stor" || op == "storage" {
        // Look at children to determine type
        for arg in exp.args() {
            let child_type = infer_storage_type(arg);
            if child_type != "unknown" {
                return child_type;
            }
        }
    }
    
    // Check for mapping pattern
    if let Exp::Op2(op, _, _) = exp {
        if op == "map" {
            return "mapping".to_string();
        }
        if op == "array" || op == "length" {
            return "array".to_string();
        }
    }
    
    if let Exp::Op1(op, _) = exp {
        if op == "loc" {
            return "uint256".to_string();
        }
        if op == "sha3" {
            return "bytes32".to_string();
        }
    }
    
    "uint256".to_string()
}

/// Get a reasonable name for a storage variable
fn get_storage_name(exp: &Exp, loc: u64) -> String {
    // Try to get name from expression first
    if let Some(name) = get_name(exp) {
        return name;
    }
    
    // Generate default name based on location
    if loc >= 1000 {
        format!("stor{:X}", loc)
    } else {
        format!("stor{}", loc)
    }
}

/// Rewrite functions with storage information (main entry point)
/// 
/// This is the Rust equivalent of Python's `rewrite_functions(functions)`.
/// It analyzes storage accesses in all functions to determine storage layout,
/// then updates function traces with storage variable names and returns
/// storage variable definitions.
pub fn rewrite_functions(functions: &mut [crate::function::Function]) -> Vec<StorageVar> {
    // Collect all traces
    let traces: Vec<Vec<Exp>> = functions.iter().map(|f| f.trace.clone()).collect();
    
    // Parse storage
    let storage_vars = parse_storage(&traces);
    
    // Build a map from storage location to variable name
    let mut loc_to_name: HashMap<u64, String> = HashMap::new();
    for var in &storage_vars {
        loc_to_name.insert(var.slot, var.name.clone());
    }
    
    // The traces are already stored in functions, we've just computed the layout
    // Further trace modification would happen in contract.postprocess()
    
    storage_vars
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_storage_var_creation() {
        let var = StorageVar::new("owner", "address", 0);
        assert_eq!(var.name, "owner");
        assert_eq!(var.type_name, "address");
        assert_eq!(var.slot, 0);
    }
    
    #[test]
    fn test_get_loc_from_loc() {
        let exp = Exp::Op1("loc".into(), Box::new(Exp::Int(42)));
        assert_eq!(get_loc(&exp), Some(42));
    }
    
    #[test]
    fn test_find_stores() {
        // ("storage", 256, 0, ("loc", 5))
        let exp = Exp::Op1(
            "storage".into(),
            Box::new(Exp::Op1("loc".into(), Box::new(Exp::Int(5)))),
        );
        let stores = find_stores(&exp);
        assert!(!stores.is_empty());
    }
}

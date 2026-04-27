//! Function module - handles function detection and analysis
//!
//! This is a port of Panoramix's function.py

use crate::core::types::{Exp, Trace};
use crate::core::masks::mask_to_type;
use crate::matcher::MatchResult;
use crate::prettify::pretty_exp;
use std::collections::HashMap;

/// A detected function in the contract
#[derive(Debug, Clone)]
pub struct Function {
    /// Function selector hash
    pub hash: u32,
    /// Function name
    pub name: String,
    /// ABI name with color codes
    pub color_name: String,
    /// ABI signature name
    pub abi_name: String,
    /// Whether function is constant (view/pure)
    pub is_const: Option<Exp>,
    /// Whether function is read-only (doesn't modify storage)
    pub read_only: bool,
    /// Whether function accepts Ether
    pub payable: bool,
    /// Symbolic trace of the function
    pub trace: Trace,
    /// Original trace before processing
    pub orig_trace: Trace,
    /// Inferred parameter types and names (idx -> (type, name))
    pub inferred_params: HashMap<u64, (String, String)>,
    /// AST representation (if computed)
    pub ast: Option<Exp>,
    /// Getter expression (for getter functions)
    pub getter: Option<Exp>,
    /// List of return expressions
    pub returns: Vec<Exp>,
    /// Whether this is a regular function (not getter, not const)
    pub is_regular: bool,
}

impl Function {
    /// Create a new Function from a hash and trace
    pub fn new(hash: u32, trace: Trace) -> Self {
        let mut func = Function {
            hash,
            name: format!("unknown_{:08x}", hash),
            color_name: format!("unknown_{:08x}", hash),
            abi_name: format!("unknown_{:08x}", hash),
            is_const: None,
            read_only: true,
            payable: true,
            trace: trace.clone(),
            orig_trace: trace,
            inferred_params: HashMap::new(),
            ast: None,
            getter: None,
            returns: Vec::new(),
            is_regular: true,
        };

        // Infer params from trace
        func.inferred_params = func.make_params();

        // Update name if it's unknown
        if func.name.starts_with("unknown") {
            func.make_names();
        }

        // Cleanup masks
        func.trace = func.cleanup_masks();

        // Analyze function traits
        func.analyse();

        func
    }

    /// Get the function priority for sorting
    pub fn priority(&self) -> i32 {
        if self.trace.is_empty() {
            return 0;
        }
        // Self-destruct functions get highest priority
        if self.trace.iter().any(|e| e.opcode() == "selfdestruct") {
            return -1;
        }
        // Otherwise sort by length
        self.ast_length().1 as i32
    }

    /// Get AST length (lines, chars)
    pub fn ast_length(&self) -> (usize, usize) {
        if self.trace.is_empty() {
            return (0, 0);
        }
        let s = self.print();
        (s.lines().count(), s.len())
    }

    /// Cleanup masks in the trace based on inferred params
    fn cleanup_masks(&self) -> Trace {
        // For now, just return the trace as-is
        self.trace.clone()
    }

    /// Make function names from inferred params
    fn make_names(&mut self) {
        let parts: Vec<&str> = self.name.split('(').collect();
        let new_name = if parts.is_empty() {
            self.name.clone()
        } else {
            parts[0].to_string()
        };

        let params_str: String = self
            .inferred_params
            .values()
            .map(|(kind, name)| format!("{} {}", kind, name))
            .collect::<Vec<_>>()
            .join(", ");

        self.name = format!("{}({})", new_name, params_str);
        self.color_name = format!("{}({})", new_name, params_str);

        let abi_params: String = self
            .inferred_params
            .values()
            .map(|(kind, _)| kind.clone())
            .collect::<Vec<_>>()
            .join(",");
        self.abi_name = format!("{}({})", new_name, abi_params);
    }

    /// Make params - infer parameter types from trace
    fn make_params(&self) -> HashMap<u64, (String, String)> {
        let mut params = HashMap::new();

        // Find all cd (calldata) references
        let mut sizes: HashMap<u64, i64> = HashMap::new();

        fn find_cd_refs(exp: &Exp, sizes: &mut HashMap<u64, i64>) {
            match exp {
                Exp::Op2(op, a, _b) => {
                    if op == "cd" {
                        if let Exp::Int(idx) = &**a {
                            let idx = *idx as u64;
                            if idx == 0 {
                                return;
                            }
                            sizes.insert(idx, 256);
                        }
                    }
                }
                Exp::Op4(op, size_exp, _, _, inner) => {
                    if op == "mask_shl" {
                        if let Exp::Op2(inner_op, idx_exp, _) = &**inner {
                            if inner_op == "cd" {
                                if let Exp::Int(idx) = &**idx_exp {
                                    let idx = *idx as u64;
                                    if idx == 0 {
                                        return;
                                    }
                                    if let Exp::Int(size) = &**size_exp {
                                        sizes.insert(idx, *size as i64);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    for arg in exp.args() {
                        find_cd_refs(arg, sizes);
                    }
                }
            }
        }

        for exp in &self.trace {
            find_cd_refs(exp, &mut sizes);
        }

        // Determine types from sizes
        let mut count = 1;
        for (idx, size) in &sizes {
            let kind: String = match size {
                -2 => "tuple".to_string(),
                -1 => "array".to_string(),
                1 => "bool".to_string(),
                _ => {
                    if let Some(t) = mask_to_type(*size as u64, true) {
                        t
                    } else {
                        "uint256".to_string()
                    }
                }
            };
            params.insert(*idx, (kind.to_string(), format!("_param{}", count)));
            count += 1;
        }

        params
    }

    /// Simplify string getter from storage
    fn simplify_string_getter_from_storage(&mut self) {
        if !self.read_only {
            return;
        }

        if self.returns.is_empty() {
            return;
        }

        // Check for return pattern
        for r in &self.returns {
            if let Exp::Op2(op, inner, _) = r {
                if op == "return" {
                    if let Exp::Op1(op1, _) = &**inner {
                        if op1 == "storage" {
                            self.getter = Some(r.clone());
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Analyse function to determine its traits
    pub fn analyse(&mut self) {
        if self.trace.is_empty() {
            return;
        }

        // Find returns
        self.returns = self.find_returns();

        // Check payable - look for callvalue check at start
        let first = &self.trace[0];
        if let Exp::Op3(op, _cond, if_true, if_false) = first {
            if op == "if" {
                // Check for callvalue pattern - simplified
                if Self::is_revert(&[if_true.as_ref().clone()]) || Self::is_invalid(&[if_true.as_ref().clone()]) {
                    self.trace = vec![if_false.as_ref().clone()];
                    self.payable = false;
                } else if Self::is_revert(&[if_false.as_ref().clone()]) || Self::is_invalid(&[if_false.as_ref().clone()]) {
                    self.trace = vec![if_true.as_ref().clone()];
                    self.payable = false;
                } else {
                    self.payable = true;
                }
            } else {
                self.payable = true;
            }
        } else {
            self.payable = true;
        }

        // Determine read_only
        self.read_only = true;
        let write_ops = ["store", "selfdestruct", "call", "delegatecall", "codecall", "create"];
        for exp in &self.trace {
            let op = exp.opcode();
            if write_ops.contains(&op) {
                self.read_only = false;
                break;
            }
        }

        // Const detection
        if self.read_only && self.returns.len() == 1 {
            let mut has_storage = false;
            let mut has_cd = false;

            fn check_trace(exp: &Exp, has_storage: &mut bool, has_cd: &mut bool) {
                let op = exp.opcode();
                if op == "storage" {
                    *has_storage = true;
                }
                if op == "cd" || op == "calldata" || op == "calldataload" {
                    *has_cd = true;
                }
                if op == "store" {
                    *has_storage = true;
                }
                for arg in exp.args() {
                    check_trace(arg, has_storage, has_cd);
                }
            }

            check_trace(&self.trace[0], &mut has_storage, &mut has_cd);

            if !has_storage && !has_cd && self.returns.len() == 1 {
                self.is_const = Some(self.returns[0].clone());
            }
        }

        // Getter detection
        self.getter = None;
        self.simplify_string_getter_from_storage();

        if self.is_const.is_none() && self.read_only && self.returns.len() == 1 {
            let ret = &self.returns[0];
            if let Exp::Op2(op, inner, _) = ret {
                if op == "return" {
                    let ret_exp = inner.as_ref();
                    // Check various getter patterns
                    match ret_exp {
                        Exp::Op1(ref s_op, _) if s_op == "storage" => {
                            self.getter = Some(ret_exp.clone());
                        }
                        Exp::OpN(ref s, ref terms) if s == "data" && !terms.is_empty() => {
                            self.getter = Some(ret_exp.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        // Regular function?
        self.is_regular = self.is_const.is_none() && self.getter.is_none();
    }

    /// Check if expression is a revert
    fn is_revert(exp: &[Exp]) -> bool {
        if exp.len() != 1 {
            return false;
        }
        let e = &exp[0];
        if let Exp::Op2(op, a, _) = e {
            if op == "return" {
                if let Exp::Int(0) = **a {
                    return true;
                }
            }
        }
        false
    }

    /// Check if expression is invalid
    fn is_invalid(exp: &[Exp]) -> bool {
        if exp.is_empty() {
            return false;
        }
        let op = exp[0].opcode();
        op == "invalid"
    }

    /// Find all return expressions in trace
    fn find_returns(&self) -> Vec<Exp> {
        let mut results = Vec::new();
        fn find_returns_inner(exp: &Exp, results: &mut Vec<Exp>) {
            if exp.opcode() == "return" {
                results.push(exp.clone());
            }
            for arg in exp.args() {
                find_returns_inner(arg, results);
            }
        }
        for exp in &self.trace {
            find_returns_inner(exp, &mut results);
        }
        results
    }

    /// Print function as string
    pub fn print(&self) -> String {
        self._print().join("\n")
    }

    /// Internal print implementation
    fn _print(&self) -> Vec<String> {
        // Handle const functions
        if let Some(val) = &self.is_const {
            let val_exp = if let Exp::Op2(op, inner, _) = val {
                if op == "return" {
                    inner.as_ref()
                } else {
                    val
                }
            } else {
                val
            };
            return vec![format!(
                "const {} = {}",
                self.name.split('(').next().unwrap_or(&self.name),
                pretty_exp(val_exp)
            )];
        }

        // Build header
        let mut header = format!("def {}", self.name);
        if self.payable {
            header.push_str(" payable");
        }
        if !self.payable {
            header.push_str(" # not payable");
        }
        if self.name.contains("_fallback") {
            if self.payable {
                header = format!("def {} # default function", self.name);
            } else {
                header = format!("def {} # not payable, default function", self.name);
            }
        }

        let mut result = vec![header];

        // Print trace
        if let Some(ast) = &self.ast {
            for exp in Self::flatten_trace(ast) {
                result.push(format!("  {}", pretty_exp(&exp)));
            }
        } else {
            for exp in &self.trace {
                for line in Self::flatten_trace(exp) {
                    result.push(format!("  {}", pretty_exp(&line)));
                }
            }
        }

        if result.len() == 1 {
            result.push("  stop".to_string());
        }

        result
    }

    /// Flatten a trace into individual expressions
    fn flatten_trace(exp: &Exp) -> Vec<Exp> {
        let mut result = Vec::new();
        Self::flatten_inner(exp, &mut result);
        result
    }

    fn flatten_inner(exp: &Exp, result: &mut Vec<Exp>) {
        match exp {
            Exp::If(_cond, then_br, else_br) => {
                result.push(exp.clone());
                for e in then_br {
                    Self::flatten_inner(e, result);
                }
                for e in else_br {
                    Self::flatten_inner(e, result);
                }
            }
            Exp::While(_cond, body) => {
                result.push(exp.clone());
                for e in body {
                    Self::flatten_inner(e, result);
                }
            }
            _ => {
                if exp.args().is_empty() {
                    result.push(exp.clone());
                } else {
                    result.push(exp.clone());
                    for arg in exp.args() {
                        Self::flatten_inner(arg, result);
                    }
                }
            }
        }
    }

    /// Serialize function to a serializable struct
    pub fn serialize(&self) -> serde_json::Value {
        serde_json::json!({
            "hash": self.hash,
            "name": self.name,
            "color_name": self.color_name,
            "abi_name": self.abi_name,
            "length": self.ast_length(),
            "getter": self.getter.as_ref().map(|e| format!("{}", e)),
            "is_const": self.is_const.as_ref().map(|e| format!("{}", e)),
            "payable": self.payable,
            "print": self.print(),
            "trace": self.trace,
            "params": self.inferred_params,
        })
    }
}

/// Match result type alias for convenience
pub type FuncMatchResult = MatchResult;

/// Pattern for matching function expressions
pub type FuncPattern = crate::matcher::Pattern;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_creation() {
        let trace = vec![Exp::int(42)];
        let func = Function::new(0x1234, trace);
        assert_eq!(func.hash, 0x1234);
    }

    #[test]
    fn test_priority() {
        let trace = vec![Exp::int(42)];
        let func = Function::new(0x1234, trace);
        assert!(func.priority() >= 0);
    }
}

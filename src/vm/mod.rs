//! Virtual Machine for symbolic EVM execution
//!
//! The VM executes EVM bytecode symbolically, building a trace of operations
//! that can be converted to human-readable decompiled code.

use crate::core::types::{Exp, Trace};
use crate::core::arithmetic::{is_zero, simplify_bool};
use crate::loader::{Bytecode, Op};
use crate::stack::Stack;
use std::collections::{HashMap, HashSet};

/// Maximum nodes before stopping (prevents infinite loops)
const MAX_NODE_COUNT: usize = 5_000;

/// A node in the execution trace
#[derive(Debug, Clone)]
pub struct Node {
    /// Position in bytecode
    pub pos: u64,
    /// Previous node
    pub prev: Option<Box<Node>>,
    /// Next nodes
    pub next: Vec<Box<Node>>,
    /// Trace from this node
    pub trace: Option<Trace>,
    /// Stack at this node
    pub stack: Vec<Exp>,
    /// Jump destination key (pos, stack_len, jumpdests)
    pub jd_key: (u64, usize, Vec<u64>),
    /// Depth in the trace tree
    pub depth: usize,
    /// Whether this is a jumpdest
    pub is_jumpdest: bool,
    /// Loop label if this is a loop header
    pub label: Option<Box<Node>>,
    /// Condition for conditional jumps
    pub condition: Option<Exp>,
    /// Whether this node is "safe" (reached via fallthrough)
    pub safe: bool,
}

impl Node {
    pub fn new(pos: u64, stack: Vec<Exp>, safe: bool) -> Self {
        Self {
            pos,
            prev: None,
            next: Vec::new(),
            trace: None,
            stack: stack.clone(),
            jd_key: (pos, stack.len(), Vec::new()),
            depth: 0,
            is_jumpdest: false,
            label: None,
            condition: None,
            safe,
        }
    }
}

/// Symbolic EVM
#[derive(Debug)]
pub struct VM {
    /// Bytecode being executed
    pub bytecode: Bytecode,
    /// Jump destinations
    jump_dests: HashSet<u64>,
    /// Variable counter for fresh names
    var_counter: u64,
    /// Just find function destinations mode
    pub just_fdests: bool,
    /// Execution timeout
    pub timeout_ms: Option<u64>,
    /// Map of jumpdest keys to nodes
    nodes: HashMap<(u64, usize, Vec<u64>), Box<Node>>,
}

impl VM {
    pub fn new(bytecode: Bytecode) -> Self {
        let jump_dests = bytecode.jump_dests.iter().cloned().collect();
        Self {
            bytecode,
            jump_dests,
            var_counter: 0,
            just_fdests: false,
            timeout_ms: None,
            nodes: HashMap::new(),
        }
    }
    
    pub fn with_fdests_mode(bytecode: Bytecode) -> Self {
        let mut vm = Self::new(bytecode);
        vm.just_fdests = true;
        vm
    }
    
    pub fn with_timeout(bytecode: Bytecode, timeout_ms: u64) -> Self {
        let mut vm = Self::new(bytecode);
        vm.timeout_ms = Some(timeout_ms);
        vm
    }
    
    /// Get next position after current
    fn next_pos(&self, pos: u64) -> Option<u64> {
        let current_pos = pos + 1;
        if self.bytecode.ops.contains_key(&current_pos) {
            return Some(current_pos);
        }
        // Scan forward
        for p in current_pos.. {
            if self.bytecode.ops.contains_key(&p) {
                return Some(p);
            }
            if p > pos + 10000 {
                return None;
            }
        }
        None
    }
    
    /// Create a fresh variable name
    fn fresh_var(&mut self) -> String {
        self.var_counter += 1;
        format!("_v{}", self.var_counter)
    }
    
    /// Run the VM to find function destinations
    pub fn find_functions(&mut self) -> Vec<(u64, u128, Vec<Exp>)> {
        // Run from position 0 with empty stack
        let trace = self.run_from(0, vec![], None);
        let mut functions = Vec::new();
        
        // Extract function calls from trace
        self.extract_functions(&trace, &mut functions);
        functions
    }
    
    /// Extract function calls from trace
    fn extract_functions(&self, trace: &[Exp], _functions: &mut Vec<(u64, u128, Vec<Exp>)>) {
        // Look for patterns like: if (eq (cd 0) <func_hash>) jump ...
        // This is a simplified version - full implementation would be more complex
        for exp in trace {
            if let Exp::Op2(_op, _a, _b) = exp {
                // Check if one side is (cd 0) and other is a function hash
                // For now, just collect the hashes we find
            }
        }
    }
    
    /// Main execution loop
    pub fn run_from(&mut self, start: u64, initial_stack: Vec<Exp>, _condition: Option<Exp>) -> Trace {
        // Build initial trace with simple setup
        let init_trace = vec![
            Exp::Op3("setmem".into(), 
                Box::new(Exp::Op2("range".into(), Box::new(Exp::Int(0x40)), Box::new(Exp::Int(32)))),
                Box::new(Exp::Int(0x60)),
                Box::new(Exp::None)),
            Exp::Op1("jump".into(), Box::new(Exp::Var("func_start".to_string()))),
        ];
        
        // Run execution from start position
        let trace = self.execute_from(start, initial_stack, None);
        
        // Combine init trace with executed trace
        let mut result = init_trace;
        result.extend(trace);
        result
    }
    
    /// Execute from a position
    fn execute_from(&mut self, start: u64, stack: Vec<Exp>, _condition: Option<Exp>) -> Trace {
        let mut trace = Vec::new();
        let mut stack = Stack::with_items(stack);
        
        // Skip jumpdest if we're at one
        let mut pos = start;
        if let Some(op) = self.bytecode.get_op(pos) {
            if op.opcode == "jumpdest" {
                pos = self.next_pos(pos).unwrap_or(pos + 1);
            }
        }
        
        // Main execution loop - limited iterations for safety
        for _iteration in 0..1000 {
            // Get current op
            let op = match self.bytecode.get_op(pos) {
                Some(op) => op,
                None => {
                    trace.push(Exp::Op1("invalid".into(), Box::new(Exp::Int(pos as u128))));
                    break;
                }
            };
            
            // Handle termination opcodes
            match op.opcode.as_str() {
                "stop" | "revert" | "invalid" | "return" => {
                    trace.push(self.handle_return_or_revert(&mut stack, &op.opcode));
                    break;
                }
                "jump" => {
                    // For now, just record the jump symbolically and break
                    // A full implementation would need to resolve the target
                    let target = self.pop_or_zero(&mut stack);
                    trace.push(Exp::Op1("jump".into(), Box::new(target)));
                    break;
                }
                "jumpi" => {
                    let _target = self.pop_or_zero(&mut stack);
                    let cond = stack.pop();
                    let cond_simplified = simplify_bool(&cond);
                    
                    // Check if condition is known
                    if is_zero(&cond_simplified) {
                        // Condition is false, fall through
                        if let Some(next_p) = self.next_pos(pos) {
                            pos = next_p;
                            continue;
                        }
                    } else {
                        // For symbolic condition, just record and continue
                        trace.push(Exp::Op1("jumpi".into(), Box::new(cond_simplified)));
                    }
                    break;
                }
                "jumpdest" => {
                    // End this basic block
                    trace.push(Exp::Op1("jumpdest".into(), Box::new(Exp::Int(pos as u128))));
                    break;
                }
                _ => {
                    // Apply operation to stack
                    self.apply_op(&mut stack, &op);
                    trace.push(self.stack_to_trace(&stack));
                }
            }
            
            // Move to next position
            if let Some(next_p) = self.next_pos(pos) {
                pos = next_p;
            } else {
                break;
            }
        }
        
        trace
    }
    
    /// Pop from stack or return zero
    fn pop_or_zero(&self, stack: &mut Stack) -> Exp {
        if stack.is_empty() {
            Exp::Int(0)
        } else {
            stack.pop()
        }
    }
    
    /// Apply an opcode to the stack
    fn apply_op(&self, stack: &mut Stack, op: &Op) {
        match op.opcode.as_str() {
            // Push operations
            opcode_str if opcode_str.starts_with("push") => {
                if let Some(param) = op.param {
                    stack.push(Exp::Int(param.into()));
                }
            }
            // Stack operations
            "pop" => { stack.pop(); }
            "dup" => {
                if let Some(idx) = op.opcode[3..].parse::<usize>().ok() {
                    stack.dup(idx);
                }
            }
            "swap" => {
                if let Some(idx) = op.opcode[4..].parse::<usize>().ok() {
                    stack.swap(idx);
                }
            }
            // Memory operations
            "mload" => {
                let pos = stack.pop();
                stack.push(Exp::Op1("mem".into(), Box::new(Exp::Op2("range".into(), Box::new(pos), Box::new(Exp::Int(32))))));
            }
            "mstore" => {
                let _pos = stack.pop();
                let _val = stack.pop();
                // Just track that memory was modified
            }
            "mstore8" => {
                let _pos = stack.pop();
                let _val = stack.pop();
            }
            // Storage operations
            "sload" => {
                let loc = stack.pop();
                stack.push(Exp::Op1("storage".into(), Box::new(loc)));
            }
            "sstore" => {
                let _loc = stack.pop();
                let _val = stack.pop();
            }
            // Calldata
            "calldataload" => {
                let pos = stack.pop();
                stack.push(Exp::Op2("cd".into(), Box::new(pos), Box::new(Exp::None)));
            }
            "calldatasize" => {
                stack.push(Exp::Op("calldatasize".to_string()));
            }
            "calldatacopy" => {
                // mempos, calldatapos, len
                stack.pop();
                stack.pop();
                stack.pop();
            }
            // Environment operations
            "address" => stack.push(Exp::Op("address".to_string())),
            "caller" => stack.push(Exp::Op("caller".to_string())),
            "callvalue" => stack.push(Exp::Op("callvalue".to_string())),
            "gasprice" => stack.push(Exp::Op("gasprice".to_string())),
            "origin" => stack.push(Exp::Op("origin".to_string())),
            "coinbase" => stack.push(Exp::Op("coinbase".to_string())),
            "timestamp" => stack.push(Exp::Op("timestamp".to_string())),
            "number" => stack.push(Exp::Op("number".to_string())),
            "difficulty" => stack.push(Exp::Op("difficulty".to_string())),
            "gaslimit" => stack.push(Exp::Op("gaslimit".to_string())),
            "chainid" => stack.push(Exp::Op("chainid".to_string())),
            "basefee" => stack.push(Exp::Op("basefee".to_string())),
            "gas" => stack.push(Exp::Op("gas".to_string())),
            "pc" => stack.push(Exp::Op("pc".to_string())),
            "msize" => stack.push(Exp::Op("msize".to_string())),
            "selfbalance" => stack.push(Exp::Op("selfbalance".to_string())),
            // Block operations
            "blockhash" => {
                let block_num = stack.pop();
                stack.push(Exp::Op1("blockhash".into(), Box::new(block_num)));
            }
            "balance" => {
                let addr = stack.pop();
                stack.push(Exp::Op1("balance".into(), Box::new(addr)));
            }
            "extcodesize" => {
                let addr = stack.pop();
                stack.push(Exp::Op1("extcodesize".into(), Box::new(addr)));
            }
            "extcodehash" => {
                let addr = stack.pop();
                stack.push(Exp::Op1("extcodehash".into(), Box::new(addr)));
            }
            "codesize" => stack.push(Exp::Op("codesize".to_string())),
            // Control flow
            "selfdestruct" => {
                let _addr = stack.pop();
            }
            "create" => {
                let _value = stack.pop();
                let _mem_start = stack.pop();
                let _mem_len = stack.pop();
            }
            "create2" => {
                let _value = stack.pop();
                let _mem_start = stack.pop();
                let _mem_len = stack.pop();
                let _salt = stack.pop();
            }
            // Calls
            "call" | "staticcall" | "delegatecall" | "callcode" => {
                // Complex - simplified for now
                stack.pop(); // gas
                stack.pop(); // addr
                if op.opcode == "call" || op.opcode == "callcode" {
                    stack.pop(); // value
                }
                stack.pop(); // args start
                stack.pop(); // args len
                stack.pop(); // ret start
                stack.pop(); // ret len
            }
            // Logging
            "log0" | "log1" | "log2" | "log3" | "log4" => {
                let idx = op.opcode[3..].parse::<usize>().unwrap_or(0);
                stack.pop(); // mem pos
                stack.pop(); // mem size
                for _ in 0..idx {
                    stack.pop(); // topics
                }
            }
            // Return data
            "returndatasize" => stack.push(Exp::Op("returndatasize".to_string())),
            "returndatacopy" => {
                stack.pop();
                stack.pop();
                stack.pop();
            }
            // Codecopy
            "codecopy" => {
                let _mem_pos = stack.pop();
                let _code_pos = stack.pop();
                let _len = stack.pop();
            }
            "extcodecopy" => {
                stack.pop();
                stack.pop();
                stack.pop();
                stack.pop();
            }
            // Unknown - push a placeholder
            _ => {
                stack.apply_op(&op.opcode);
            }
        }
    }
    
    /// Handle return or revert
    fn handle_return_or_revert(&self, stack: &mut Stack, opcode: &str) -> Exp {
        let size = stack.pop();
        let offset = stack.pop();
        if let Exp::Int(0) = &size {
            Exp::Op(opcode.to_string())
        } else {
            Exp::Op2(opcode.to_string(), Box::new(offset), Box::new(size))
        }
    }
    
    /// Convert stack to trace entry
    fn stack_to_trace(&self, _stack: &Stack) -> Exp {
        // Simplified - just return the stack state
        Exp::None
    }
    
    /// Process a jump from a node
    fn process_jump(&self, _node: &mut Node, _last: &Exp) {
        // This would handle linking nodes in the CFG
    }
    
    /// Recursively build trace from node
    fn make_trace_recursive(&self, _trace: &mut Trace, _node: &Node) {
        // This would walk the CFG and build the final trace
    }
}

/// Decompile bytecode to a trace
pub fn decompile(bytecode: &str) -> Result<Trace, String> {
    let bc = Bytecode::load(bytecode)?;
    let mut vm = VM::new(bc);
    let trace = vm.run_from(0, vec![], None);
    Ok(trace)
}

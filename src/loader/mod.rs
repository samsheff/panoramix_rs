//! Bytecode loader and disassembler

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An opcode with its position and optional parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    pub pos: u64,
    pub opcode: String,
    pub param: Option<u64>,
}

impl Op {
    pub fn new(pos: u64, opcode: &str, param: Option<u64>) -> Self {
        Self { pos, opcode: opcode.to_string(), param }
    }
}

/// Opcode dictionary - maps byte values to opcode names
#[derive(Debug, Clone)]
pub struct OpcodeDict {
    /// Maps bytecode byte -> opcode name
    byte_to_op: HashMap<u8, String>,
    /// Maps opcode name -> bytecode byte
    op_to_byte: HashMap<String, u8>,
}

impl OpcodeDict {
    pub fn new() -> Self {
        let mut byte_to_op = HashMap::new();
        let mut op_to_byte = HashMap::new();
        
        // Push operations
        for i in 1..33 {
            let name = format!("push{}", i);
            let byte = 0x5f + i; // PUSH1 = 0x60, PUSH32 = 0x7f
            byte_to_op.insert(byte, name.clone());
            op_to_byte.insert(name, byte);
        }
        
        // Dup operations
        for i in 1..17 {
            let name = format!("dup{}", i);
            let byte = 0x7f + i; // DUP1 = 0x80, DUP16 = 0x8f
            byte_to_op.insert(byte, name.clone());
            op_to_byte.insert(name, byte);
        }
        
        // Swap operations  
        for i in 1..17 {
            let name = format!("swap{}", i);
            let byte = 0x8f + i; // SWAP1 = 0x90, SWAP16 = 0x9f
            byte_to_op.insert(byte, name.clone());
            op_to_byte.insert(name, byte);
        }
        
        // Log operations
        for i in 0..5 {
            let name = format!("log{}", i);
            let byte = 0xa0 + i; // LOG0 = 0xa0, LOG4 = 0xa4
            byte_to_op.insert(byte, name.clone());
            op_to_byte.insert(name, byte);
        }
        
        // Single-byte opcodes
        let single_byte = [
            (0x00, "stop"),
            (0x01, "add"),
            (0x02, "mul"),
            (0x03, "sub"),
            (0x04, "div"),
            (0x05, "sdiv"),
            (0x06, "mod"),
            (0x07, "smod"),
            (0x08, "addmod"),
            (0x09, "mulmod"),
            (0x0a, "exp"),
            (0x0b, "signextend"),
            (0x10, "lt"),
            (0x11, "gt"),
            (0x12, "slt"),
            (0x13, "sgt"),
            (0x14, "eq"),
            (0x15, "iszero"),
            (0x16, "and"),
            (0x17, "or"),
            (0x18, "xor"),
            (0x19, "not"),
            (0x1a, "byte"),
            (0x1b, "shl"),
            (0x1c, "shr"),
            (0x1d, "sar"),
            (0x20, "sha3"),
            (0x30, "address"),
            (0x31, "balance"),
            (0x32, "origin"),
            (0x33, "caller"),
            (0x34, "callvalue"),
            (0x35, "calldataload"),
            (0x36, "calldatasize"),
            (0x37, "calldatacopy"),
            (0x38, "codesize"),
            (0x39, "codecopy"),
            (0x3a, "gasprice"),
            (0x3b, "extcodesize"),
            (0x3c, "extcodecopy"),
            (0x3d, "returndatasize"),
            (0x3e, "returndatacopy"),
            (0x3f, "extcodehash"),
            (0x40, "blockhash"),
            (0x41, "coinbase"),
            (0x42, "timestamp"),
            (0x43, "number"),
            (0x44, "difficulty"),
            (0x45, "gaslimit"),
            (0x46, "chainid"),
            (0x47, "basefee"),
            (0x50, "pop"),
            (0x51, "mload"),
            (0x52, "mstore"),
            (0x53, "mstore8"),
            (0x54, "sload"),
            (0x55, "sstore"),
            (0x56, "jump"),
            (0x57, "jumpi"),
            (0x58, "pc"),
            (0x59, "msize"),
            (0x5a, "gas"),
            (0x5b, "jumpdest"),
            (0xf0, "create"),
            (0xf1, "call"),
            (0xf2, "callcode"),
            (0xf3, "return"),
            (0xf4, "delegatecall"),
            (0xf5, "create2"),
            (0xfa, "staticcall"),
            (0xfd, "revert"),
            (0xfe, "invalid"),
            (0xff, "selfdestruct"),
        ];
        
        for (byte, name) in single_byte {
            byte_to_op.insert(byte, name.to_string());
            op_to_byte.insert(name.to_string(), byte);
        }
        
        Self { byte_to_op, op_to_byte }
    }
    
    pub fn lookup(&self, byte: u8) -> &str {
        self.byte_to_op.get(&byte).map(|s| s.as_str()).unwrap_or("UNKNOWN")
    }
    
    pub fn reverse_lookup(&self, name: &str) -> Option<u8> {
        self.op_to_byte.get(name).copied()
    }
}

impl Default for OpcodeDict {
    fn default() -> Self {
        Self::new()
    }
}

/// Disassembled bytecode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    /// Map from position to Op
    pub ops: HashMap<u64, Op>,
    /// Jump destinations
    pub jump_dests: Vec<u64>,
    /// Raw binary bytes
    pub binary: Vec<u8>,
}

impl Bytecode {
    pub fn load(hex: &str) -> Result<Self, String> {
        // Remove 0x prefix
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        
        // Parse hex string to bytes
        let mut binary = Vec::new();
        for i in (0..hex.len()).step_by(2) {
            let byte = u8::from_str_radix(&hex[i..i+2], 16)
                .map_err(|_| format!("Invalid hex at position {}", i))?;
            binary.push(byte);
        }
        
        let mut ops = HashMap::new();
        let mut jump_dests = Vec::new();
        let dict = OpcodeDict::new();
        
        let mut pos = 0u64;
        let mut i = 0usize;
        
        while i < binary.len() {
            let byte = binary[i];
            let opcode = dict.lookup(byte);
            
            if opcode == "jumpdest" {
                jump_dests.push(pos);
            }
            
            let mut param = None::<u64>;
            
            if opcode.starts_with("push") {
                if let Ok(num_words) = opcode[4..].parse::<u8>() {
                    let num_words = num_words as usize;
                    if i + 1 + num_words <= binary.len() {
                        let mut val = 0u64;
                        for j in 0..num_words {
                            val = val * 256 + binary[i + 1 + j] as u64;
                        }
                        param = Some(val);
                        i += num_words;
                    }
                }
            } else if opcode.starts_with("dup") || opcode.starts_with("swap") {
                // These are single byte opcodes with no parameter
            } else if opcode.starts_with("log") {
                // Log opcodes have no parameter
            }
            
            ops.insert(pos, Op::new(pos, opcode, param));
            
            pos += 1;
            i += 1;
        }
        
        Ok(Self { ops, jump_dests, binary })
    }
    
    pub fn get_op(&self, pos: u64) -> Option<&Op> {
        self.ops.get(&pos)
    }
    
    pub fn next_pos(&self, pos: u64) -> Option<u64> {
        // Find the next position that has an op
        let current_pos = pos + 1;
        if self.ops.contains_key(&current_pos) {
            return Some(current_pos);
        }
        // Scan forward
        for p in current_pos.. {
            if self.ops.contains_key(&p) {
                return Some(p);
            }
            if p > pos + 1000 {
                return None; // Safety limit
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_simple() {
        // Simple stop bytecode
        let bc = Bytecode::load("0x00").unwrap();
        assert_eq!(bc.ops.len(), 1);
        assert_eq!(bc.ops[&0].opcode, "stop");
    }
    
    #[test]
    fn test_push() {
        // PUSH1 42 STOP
        let bc = Bytecode::load("0x602a00").unwrap();
        let push_op = bc.ops.get(&0).unwrap();
        assert_eq!(push_op.opcode, "push1");
        assert_eq!(push_op.param, Some(42));
    }
}

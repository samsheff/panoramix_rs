//! Symbolic stack for EVM execution

use crate::core::types::Exp;
use crate::core::algebra::{add_op, mul_op, sub_op, div_op, mod_op};

/// A stack of symbolic expressions
#[derive(Debug, Clone)]
pub struct Stack {
    items: Vec<Exp>,
}

impl Stack {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    
    pub fn with_items(items: Vec<Exp>) -> Self {
        Self { items }
    }
    
    pub fn push(&mut self, exp: Exp) {
        self.items.push(exp);
    }
    
    pub fn pop(&mut self) -> Exp {
        self.items.pop().unwrap_or(Exp::None)
    }
    
    pub fn dup(&mut self, idx: usize) {
        if idx > 0 && idx <= self.items.len() {
            let item = self.items[self.items.len() - idx].clone();
            self.items.push(item);
        }
    }
    
    pub fn swap(&mut self, idx: usize) {
        if idx > 0 && idx < self.items.len() {
            let len = self.items.len();
            self.items.swap(len - 1, len - 1 - idx);
        }
    }
    
    pub fn len(&self) -> usize {
        self.items.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    
    pub fn get(&self, idx: usize) -> Option<&Exp> {
        self.items.get(idx)
    }
    
    pub fn peek(&self) -> Option<&Exp> {
        self.items.last()
    }
    
    pub fn items(&self) -> &[Exp] {
        &self.items
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &Exp> {
        self.items.iter()
    }
    
    /// Apply an operation to the stack
    pub fn apply_op(&mut self, opcode: &str) {
        match opcode {
            "add" => {
                let b = self.pop();
                let a = self.pop();
                self.push(add_op(a, b));
            }
            "mul" => {
                let b = self.pop();
                let a = self.pop();
                self.push(mul_op(a, b));
            }
            "sub" => {
                let b = self.pop();
                let a = self.pop();
                self.push(sub_op(a, b));
            }
            "div" => {
                let b = self.pop();
                let a = self.pop();
                self.push(div_op(a, b));
            }
            "mod" => {
                let b = self.pop();
                let a = self.pop();
                self.push(mod_op(a, b));
            }
            "and" => {
                let b = self.pop();
                let a = self.pop();
                self.push(crate::core::algebra::and_op(a, b));
            }
            "or" => {
                let b = self.pop();
                let a = self.pop();
                self.push(crate::core::algebra::or_op(a, b));
            }
            "xor" => {
                let b = self.pop();
                let a = self.pop();
                self.push(crate::core::algebra::xor_op(a, b));
            }
            "eq" => {
                let b = self.pop();
                let a = self.pop();
                self.push(Exp::eq(a, b));
            }
            "lt" => {
                let b = self.pop();
                let a = self.pop();
                self.push(Exp::lt(a, b));
            }
            "iszero" => {
                let a = self.pop();
                self.push(Exp::iszero(a));
            }
            "not" => {
                let a = self.pop();
                self.push(Exp::Op1("not".into(), Box::new(a)));
            }
            "shl" => {
                let shift = self.pop();
                let val = self.pop();
                if let Exp::Int(s) = &shift {
                    if let Exp::Int(v) = &val {
                        self.push(Exp::Int(v << s));
                        return;
                    }
                }
                self.push(Exp::Op2("shl".into(), Box::new(val), Box::new(shift)));
            }
            "shr" => {
                let shift = self.pop();
                let val = self.pop();
                if let Exp::Int(s) = &shift {
                    if let Exp::Int(v) = &val {
                        self.push(Exp::Int(v >> s));
                        return;
                    }
                }
                self.push(Exp::Op2("shr".into(), Box::new(val), Box::new(shift)));
            }
            "sar" => {
                let shift = self.pop();
                let val = self.pop();
                // Signed right shift is complex - simplified for now
                self.push(Exp::Op2("sar".into(), Box::new(val), Box::new(shift)));
            }
            "byte" => {
                let pos = self.pop();
                let val = self.pop();
                self.push(Exp::Op2("byte".into(), Box::new(pos), Box::new(val)));
            }
            "signextend" => {
                let bits = self.pop();
                let val = self.pop();
                self.push(Exp::Op2("signextend".into(), Box::new(bits), Box::new(val)));
            }
            _ => {
                // Unknown opcode - push a placeholder
                self.push(Exp::Op(opcode.to_string()));
            }
        }
    }
    
    /// Check if all items are concrete
    pub fn all_concrete(&self) -> bool {
        self.items.iter().all(|e| e.is_concrete())
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", item)?;
        }
        write!(f, "]")
    }
}

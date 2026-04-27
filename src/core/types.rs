//! Core types for the EVM decompiler

use serde::{Deserialize, Serialize};

/// A symbolic expression - matches Panoramix's tuple-based approach
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Exp {
    Var(String),
    Int(u128),
    Str(String),
    None,
    Jd(u64),
    Op(String),
    Op1(String, Box<Exp>),
    Op2(String, Box<Exp>, Box<Exp>),
    Op3(String, Box<Exp>, Box<Exp>, Box<Exp>),
    Op4(String, Box<Exp>, Box<Exp>, Box<Exp>, Box<Exp>),
    OpN(String, Vec<Exp>),
    Label(Box<Exp>, Vec<(u64, u64, Exp)>),
    If(Box<Exp>, Vec<Exp>, Vec<Exp>),
    While(Box<Exp>, Vec<Exp>),
    Goto(Box<Exp>, Vec<(u64, u64, Exp)>),
    Loop(Box<Exp>, Vec<Exp>, Vec<Exp>, Vec<(u64, u64, Exp)>),
    Continue,
    Break,
}

impl Exp {
    pub fn opcode(&self) -> &str {
        match self {
            Exp::Var(_) => "var",
            Exp::Int(_) => "int",
            Exp::Str(_) => "str",
            Exp::None => "none",
            Exp::Jd(_) => "jd",
            Exp::Op(op) => op,
            Exp::Op1(op, _) => op,
            Exp::Op2(op, _, _) => op,
            Exp::Op3(op, _, _, _) => op,
            Exp::Op4(op, _, _, _, _) => op,
            Exp::OpN(op, _) => op,
            Exp::Label(..) => "label",
            Exp::If(..) => "if",
            Exp::While(..) => "while",
            Exp::Goto(..) => "goto",
            Exp::Loop(..) => "loop",
            Exp::Continue => "continue",
            Exp::Break => "break",
        }
    }
    
    pub fn is_concrete(&self) -> bool {
        match self {
            Exp::Int(_) | Exp::Str(_) | Exp::None => true,
            Exp::Op1(_, a) => a.is_concrete(),
            Exp::Op2(_, a, b) => a.is_concrete() && b.is_concrete(),
            Exp::Op3(_, a, b, c) => a.is_concrete() && b.is_concrete() && c.is_concrete(),
            _ => false,
        }
    }
    
    pub fn args(&self) -> Vec<&Exp> {
        match self {
            Exp::Op1(_, a) => vec![a],
            Exp::Op2(_, a, b) => vec![a, b],
            Exp::Op3(_, a, b, c) => vec![a, b, c],
            Exp::Op4(_, a, b, c, d) => vec![a, b, c, d],
            Exp::OpN(_, args) => args.iter().collect(),
            _ => vec![],
        }
    }
    
    pub fn cd(idx: u64) -> Exp { Exp::Op2("cd".into(), Box::new(Exp::Int(idx as u128)), Box::new(Exp::None)) }
    pub fn storage(loc: Exp) -> Exp { Exp::Op1("storage".into(), Box::new(loc)) }
    pub fn add(a: Exp, b: Exp) -> Exp { Exp::Op2("add".into(), Box::new(a), Box::new(b)) }
    pub fn sub(a: Exp, b: Exp) -> Exp { Exp::Op2("sub".into(), Box::new(a), Box::new(b)) }
    pub fn mul(a: Exp, b: Exp) -> Exp { Exp::Op2("mul".into(), Box::new(a), Box::new(b)) }
    pub fn eq(a: Exp, b: Exp) -> Exp { Exp::Op2("eq".into(), Box::new(a), Box::new(b)) }
    pub fn lt(a: Exp, b: Exp) -> Exp { Exp::Op2("lt".into(), Box::new(a), Box::new(b)) }
    pub fn iszero(a: Exp) -> Exp { Exp::Op1("iszero".into(), Box::new(a)) }
    pub fn and(a: Exp, b: Exp) -> Exp { Exp::Op2("and".into(), Box::new(a), Box::new(b)) }
    pub fn or(a: Exp, b: Exp) -> Exp { Exp::Op2("or".into(), Box::new(a), Box::new(b)) }
    pub fn sha3(a: Exp) -> Exp { Exp::Op1("sha3".into(), Box::new(a)) }
    pub fn var(name: &str) -> Exp { Exp::Var(name.to_string()) }
    pub fn int(val: u128) -> Exp { Exp::Int(val) }
}

pub type Trace = Vec<Exp>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position(pub u64);

impl Position {
    pub fn new(pos: u64) -> Self { Position(pos) }
    pub fn as_u64(&self) -> u64 { self.0 }
}

impl std::fmt::Display for Exp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Exp::Var(s) => write!(f, "{}", s),
            Exp::Int(n) => write!(f, "{}", n),
            Exp::Str(s) => write!(f, "\"{}\"", s),
            Exp::None => write!(f, "none"),
            Exp::Jd(n) => write!(f, "jd({})", n),
            Exp::Op(op) => write!(f, "({})", op),
            Exp::Op1(op, a) => write!(f, "({} {})", op, a),
            Exp::Op2(op, a, b) => write!(f, "({} {} {})", op, a, b),
            Exp::Op3(op, a, b, c) => write!(f, "({} {} {} {})", op, a, b, c),
            Exp::Op4(op, a, b, c, d) => write!(f, "({} {} {} {} {})", op, a, b, c, d),
            Exp::OpN(op, args) => { write!(f, "({}", op)?; for arg in args { write!(f, " {}", arg)?; } write!(f, ")") }
            Exp::If(cond, then_br, else_br) => { write!(f, "(if {} [", cond)?; for e in then_br { write!(f, "{} ", e)?; } write!(f, "] [")?; for e in else_br { write!(f, "{} ", e)?; } write!(f, "])") }
            Exp::While(cond, body) => { write!(f, "(while {} [", cond)?; for e in body { write!(f, "{} ", e)?; } write!(f, "])") }
            Exp::Goto(target, _) => write!(f, "(goto {})", target),
            Exp::Loop(..) => write!(f, "(loop ...)"),
            Exp::Label(target, _) => write!(f, "(label {})", target),
            Exp::Continue => write!(f, "(continue)"),
            Exp::Break => write!(f, "(break)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_cd() { let e = Exp::cd(4); assert_eq!(e.opcode(), "cd"); }
    #[test] fn test_concrete() { assert!(Exp::int(42).is_concrete()); assert!(!Exp::var("x").is_concrete()); }
}

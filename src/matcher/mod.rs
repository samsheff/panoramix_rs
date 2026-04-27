//! Pattern matching for expressions
//!
//! This module provides pattern matching functionality similar to Panoramix's matcher.py.
//! Patterns can include:
//! - `:name` - capture a subexpression
//! - `Any` - match anything
//! - `...` (Ellipsis) - match zero or more elements
//! - Concrete values - match exactly

use crate::core::types::Exp;
use std::any::Any as AnyTrait;
use std::collections::HashMap;

/// Special marker for matching anything
#[derive(Debug, Clone)]
pub struct PatAny;

impl PatAny {
    pub fn new() -> Self { PatAny }
}

/// Match result container
#[derive(Debug)]
pub struct MatchResult {
    bindings: HashMap<String, Box<dyn AnyTrait>>,
}

impl MatchResult {
    pub fn new() -> Self {
        Self { bindings: HashMap::new() }
    }
    
    pub fn bind(&mut self, name: &str, value: Box<dyn AnyTrait>) {
        self.bindings.insert(name.to_string(), value);
    }
    
    pub fn get<T: 'static>(&self, name: &str) -> Option<&T> {
        self.bindings.get(name)?.downcast_ref::<T>()
    }
    
    pub fn get_exp(&self, name: &str) -> Option<&Exp> {
        self.get::<Exp>(name)
    }
}

/// Attempt to match an expression against a pattern
pub fn match_exp(expr: &Exp, pattern: &Pattern) -> Option<MatchResult> {
    match (expr, pattern) {
        // Any pattern matches anything
        ( _, Pattern::PatAny) => Some(MatchResult::new()),
        
        // Concrete value patterns
        (Exp::Int(n), Pattern::Int(m)) if n == m => Some(MatchResult::new()),
        (Exp::Str(s), Pattern::Str(m)) if s == m => Some(MatchResult::new()),
        (Exp::None, Pattern::None) => Some(MatchResult::new()),
        
        // Variable capture patterns
        (exp, Pattern::Capture(name)) => {
            let mut result = MatchResult::new();
            result.bind(name, Box::new(exp.clone()));
            Some(result)
        }
        
        // Type-checked capture
        (exp, Pattern::CaptureTyped(name, expected_type)) => {
            if expected_type.matches(exp) {
                let mut result = MatchResult::new();
                result.bind(name, Box::new(exp.clone()));
                Some(result)
            } else {
                None
            }
        }
        
        // Opcode matching
        (Exp::Op(op), Pattern::Op(op_pattern)) if op == op_pattern => {
            Some(MatchResult::new())
        }
        (Exp::Op1(op, a), Pattern::Op1(op_pattern, p_a)) => {
            if op == op_pattern {
                match_exp(a, p_a)
            } else {
                None
            }
        }
        (Exp::Op2(op, a, b), Pattern::Op2(op_pattern, p_a, p_b)) => {
            if op == op_pattern {
                match match_exp(a, p_a) {
                    Some(mut r1) => {
                        if let Some(r2) = match_exp(b, p_b) {
                            r1.bindings.extend(r2.bindings);
                            return Some(r1);
                        }
                    }
                    None => {}
                }
                match match_exp(a, p_a) {
                    Some(r) => Some(r),
                    None => match_exp(b, p_b),
                }
            } else {
                None
            }
        }
        (Exp::Op3(op, a, b, c), Pattern::Op3(op_pattern, p_a, p_b, p_c)) => {
            if op == op_pattern {
                match match_exp(a, p_a) {
                    Some(mut r1) => {
                        if let Some(r2) = match_exp(b, p_b) {
                            if let Some(r3) = match_exp(c, p_c) {
                                r1.bindings.extend(r2.bindings);
                                r1.bindings.extend(r3.bindings);
                                return Some(r1);
                            }
                        }
                    }
                    None => {}
                }
            }
            None
        }
        (Exp::Op4(op, a, b, c, d), Pattern::Op4(op_pattern, p_a, p_b, p_c, p_d)) => {
            if op == op_pattern {
                match match_exp(a, p_a) {
                    Some(mut r1) => {
                        if let Some(r2) = match_exp(b, p_b) {
                            if let Some(r3) = match_exp(c, p_c) {
                                if let Some(r4) = match_exp(d, p_d) {
                                    r1.bindings.extend(r2.bindings);
                                    r1.bindings.extend(r3.bindings);
                                    r1.bindings.extend(r4.bindings);
                                    return Some(r1);
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
            None
        }
        
        // List/tuple patterns
        (Exp::OpN(op, args), Pattern::OpN(op_pattern, p_args)) if op == op_pattern => {
            match_patterns(args, p_args)
        }
        
        _ => None,
    }
}

/// Match a list of expressions against a list of patterns
pub fn match_patterns(exprs: &[Exp], patterns: &[Pattern]) -> Option<MatchResult> {
    if patterns.contains(&Pattern::Ellipsis) {
        // Find the ellipsis and match before/after
        if let Some(ellipsis_idx) = patterns.iter().position(|p| *p == Pattern::Ellipsis) {
            // Match prefix
            let prefix_patterns = &patterns[..ellipsis_idx];
            let suffix_patterns = &patterns[ellipsis_idx + 1..];
            
            if exprs.len() < prefix_patterns.len() + suffix_patterns.len() {
                return None;
            }
            
            // Match prefix
            let prefix_exprs = &exprs[..prefix_patterns.len()];
            let prefix_result = match_patterns(prefix_exprs, prefix_patterns)?;
            
            // Match suffix
            let suffix_exprs = &exprs[exprs.len() - suffix_patterns.len()..];
            let suffix_result = match_patterns(suffix_exprs, suffix_patterns)?;
            
            // Merge results
            let mut result = prefix_result;
            result.bindings.extend(suffix_result.bindings);
            return Some(result);
        }
    }
    
    if exprs.len() != patterns.len() {
        return None;
    }
    
    let mut result = MatchResult::new();
    for (expr, pattern) in exprs.iter().zip(patterns.iter()) {
        match match_exp(expr, pattern) {
            Some(r) => {
                for (k, v) in r.bindings {
                    result.bindings.insert(k, v);
                }
            }
            None => return None,
        }
    }
    
    Some(result)
}

/// Pattern for matching expressions
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Match anything
    PatAny,
    
    /// Match nothing (shouldn't be used directly)
    None,
    
    /// Capture a subexpression with a name
    Capture(String),
    
    /// Capture with type check
    CaptureTyped(String, Box<TypePattern>),
    
    /// Match specific int value
    Int(u128),
    
    /// Match specific string value
    Str(String),
    
    /// Match opcode with no args
    Op(String),
    
    /// Match opcode with 1 arg
    Op1(String, Box<Pattern>),
    
    /// Match opcode with 2 args
    Op2(String, Box<Pattern>, Box<Pattern>),
    
    /// Match opcode with 3 args
    Op3(String, Box<Pattern>, Box<Pattern>, Box<Pattern>),
    
    /// Match opcode with 4 args
    Op4(String, Box<Pattern>, Box<Pattern>, Box<Pattern>, Box<Pattern>),
    
    /// Match opcode with N args
    OpN(String, Vec<Pattern>),
    
    /// Ellipsis - matches zero or more
    Ellipsis,
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Pattern::PatAny, Pattern::PatAny) => true,
            (Pattern::None, Pattern::None) => true,
            (Pattern::Ellipsis, Pattern::Ellipsis) => true,
            (Pattern::Int(a), Pattern::Int(b)) => a == b,
            (Pattern::Str(a), Pattern::Str(b)) => a == b,
            (Pattern::Op(a), Pattern::Op(b)) => a == b,
            (Pattern::Capture(a), Pattern::Capture(b)) => a == b,
            _ => false,
        }
    }
}

/// Type pattern for typed captures
#[derive(Debug, Clone)]
pub enum TypePattern {
    Int,
    Str,
    Var,
}

impl TypePattern {
    pub fn matches(&self, exp: &Exp) -> bool {
        match (self, exp) {
            (TypePattern::Int, Exp::Int(_)) => true,
            (TypePattern::Str, Exp::Str(_)) => true,
            (TypePattern::Var, Exp::Var(_)) => true,
            _ => false,
        }
    }
}

// Convenience patterns
pub fn var(name: &str) -> Pattern {
    Pattern::Capture(name.to_string())
}

pub fn op(op: &str) -> Pattern {
    Pattern::Op(op.to_string())
}

pub fn int(n: u128) -> Pattern {
    Pattern::Int(n)
}

pub fn wild() -> Pattern {
    Pattern::PatAny
}

pub fn ellipsis() -> Pattern {
    Pattern::Ellipsis
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_match() {
        let expr = Exp::int(42);
        let pattern = Pattern::Int(42);
        assert!(match_exp(&expr, &pattern).is_some());
    }
    
    #[test]
    fn test_capture() {
        let expr = Exp::int(42);
        let pattern = Pattern::Capture("x".to_string());
        let result = match_exp(&expr, &pattern).unwrap();
        assert!(matches!(result.get_exp("x"), Some(Exp::Int(42))));
    }
}

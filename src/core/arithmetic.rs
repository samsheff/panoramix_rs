//! Arithmetic operations for symbolic EVM execution

use crate::core::types::Exp;

const UINT_256_MAX: u128 = u128::MAX;

pub fn eval(exp: &Exp) -> Exp {
    // Simplified - just return as-is for symbolic expressions
    exp.clone()
}

pub fn simplify_bool(exp: &Exp) -> Exp {
    match exp {
        Exp::Op1(op, a) if op == "iszero" => {
            let simplified = simplify_bool(a);
            match simplified {
                Exp::Op1(op2, a2) if op2 == "iszero" => (*a2).clone(),
                _ => Exp::Op1("iszero".into(), Box::new(simplified)),
            }
        }
        _ => exp.clone(),
    }
}

pub fn is_zero(exp: &Exp) -> bool {
    match exp {
        Exp::Int(n) => *n == 0,
        Exp::Op1(op, a) if op == "iszero" => {
            match a.as_ref() {
                Exp::Int(n) => *n != 0,
                _ => false,
            }
        }
        _ => false,
    }
}

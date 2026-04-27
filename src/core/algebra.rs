//! Algebraic operations for symbolic execution

use crate::core::types::Exp;

/// Symbolic addition
pub fn add_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        return Exp::Int(na.wrapping_add(*nb));
    }
    if let Exp::Int(0) = &a { return b; }
    if let Exp::Int(0) = &b { return a; }
    Exp::Op2("add".into(), Box::new(a), Box::new(b))
}

/// Symbolic multiplication
pub fn mul_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        return Exp::Int(na.wrapping_mul(*nb));
    }
    if let Exp::Int(0) = &a { return Exp::Int(0); }
    if let Exp::Int(0) = &b { return Exp::Int(0); }
    if let Exp::Int(1) = &a { return b; }
    if let Exp::Int(1) = &b { return a; }
    Exp::Op2("mul".into(), Box::new(a), Box::new(b))
}

/// Symbolic subtraction
pub fn sub_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        return Exp::Int(na.wrapping_sub(*nb));
    }
    if let Exp::Int(0) = &b { return a; }
    Exp::Op2("sub".into(), Box::new(a), Box::new(b))
}

/// Symbolic division
pub fn div_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        if *nb == 0 { return Exp::Int(0); }
        return Exp::Int(na / nb);
    }
    Exp::Op2("div".into(), Box::new(a), Box::new(b))
}

/// Symbolic modulo
pub fn mod_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        if *nb == 0 { return Exp::Int(0); }
        return Exp::Int(na % nb);
    }
    Exp::Op2("mod".into(), Box::new(a), Box::new(b))
}

/// Symbolic AND
pub fn and_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        return Exp::Int(*na & *nb);
    }
    if let Exp::Int(0) = &a { return a; }
    if let Exp::Int(0) = &b { return b; }
    if let Exp::Int(n) = &a { if *n == u128::MAX { return b; } }
    if let Exp::Int(n) = &b { if *n == u128::MAX { return a; } }
    Exp::Op2("and".into(), Box::new(a), Box::new(b))
}

/// Symbolic OR
pub fn or_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        return Exp::Int(*na | *nb);
    }
    if let Exp::Int(0) = &a { return b; }
    if let Exp::Int(0) = &b { return a; }
    Exp::Op2("or".into(), Box::new(a), Box::new(b))
}

/// Symbolic XOR
pub fn xor_op(a: Exp, b: Exp) -> Exp {
    if let (Exp::Int(na), Exp::Int(nb)) = (&a, &b) {
        return Exp::Int(*na ^ *nb);
    }
    if let Exp::Int(0) = &a { return b; }
    if let Exp::Int(0) = &b { return a; }
    Exp::Op2("xor".into(), Box::new(a), Box::new(b))
}

/// Mask and shift operation
pub fn mask_op(val: Exp, size: u64, offset: u64, shl: u64, shr: u64) -> Exp {
    if size == 256 && offset == 0 && shl == 0 && shr == 0 {
        return val;
    }
    Exp::Op4(
        "mask_shl".into(),
        Box::new(Exp::Int(size as u128)),
        Box::new(Exp::Int(offset as u128)),
        Box::new(Exp::Int(shl as u128)),
        Box::new(val),
    )
}

/// Check if expression is power of 2
pub fn to_exp2(exp: &Exp) -> Option<u32> {
    if let Exp::Int(n) = exp {
        if *n > 0 && (*n & (*n - 1)) == 0 {
            return Some((*n as u32).trailing_zeros());
        }
    }
    None
}

/// Simplify expression algebraically
pub fn simplify(exp: &Exp) -> Exp {
    match exp {
        Exp::Op2(op, a, b) if op == "add" => {
            let a = simplify(a);
            let b = simplify(b);
            add_op(a, b)
        }
        Exp::Op2(op, a, b) if op == "mul" => {
            let a = simplify(a);
            let b = simplify(b);
            mul_op(a, b)
        }
        Exp::Op2(op, a, b) if op == "sub" => {
            let a = simplify(a);
            let b = simplify(b);
            sub_op(a, b)
        }
        _ => exp.clone(),
    }
}

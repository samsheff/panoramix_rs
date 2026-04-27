//! While loop conversion - converts gotos to structured while loops
//!
//! This is a port of Panoramix's whiles.py

use crate::core::types::{Exp, Trace};
use crate::prettify::explain;

/// Main entry point - convert gotos to whiles
pub fn make_whiles(trace: Trace, _timeout: u64) -> Trace {
    let trace = make(&trace);
    explain("Loops -> whiles", &trace);

    // Clean up jumpdests
    let trace = rewrite_trace(&trace, |line| {
        if line.opcode() == "jumpdest" {
            vec![]
        } else {
            vec![line.clone()]
        }
    });

    // TODO: apply simplify_trace with timeout

    trace
}

/*
/// Simplify trace with optional timeout
pub fn simplify_trace(trace: Trace, timeout: u64) -> Trace {
    // TODO: implement simplify_trace
    trace
}
*/

/// Main conversion function
fn make(trace: &[Exp]) -> Vec<Exp> {
    let mut res = Vec::new();

    let mut idx = 0;
    while idx < trace.len() {
        let line = &trace[idx];

        // Match if statement - Exp::If(Box<Exp>, Vec<Exp>, Vec<Exp>)
        if let Exp::If(cond, if_true, if_false) = line {
            res.push(Exp::If(
                cond.clone(),
                make(if_true),
                make(if_false),
            ));
        }
        // Match label - Exp::Label(Box<Exp>, Vec<(u64, u64, Exp)>)
        else if let Exp::Label(jd, vars) = line {
            // Try to convert to while loop
            match to_while(&trace[idx + 1..], jd) {
                Ok((path, inside, remaining, cond)) => {
                    let inside = make(&inside);
                    let remaining = make(&remaining);

                    // Replace variables in path
                    let mut before = path;
                    for (_, v_idx, v_val) in vars {
                        before = replace_var(&before, &Exp::Var(format!("var_{}", v_idx)), v_val);
                    }
                    let before = make(&before);

                    res.extend(before);
                    res.push(Exp::While(Box::new(cond), inside));
                    res.extend(remaining);
                    return res;
                }
                Err(_e) => {
                    eprintln!("couldn't make loop for line {:?}, omitting it", line);
                }
            }
        }
        // Match goto - Exp::Goto(Box<Exp>, Vec<(u64, u64, Exp)>)
        else if let Exp::Goto(target, setvars) = line {
            res.push(Exp::Goto(
                target.clone(),
                setvars.clone(),
            ));
        }
        // Default: keep the line
        else {
            res.push(line.clone());
        }

        idx += 1;
    }

    res
}

/// Convert a label's body to a while loop
fn to_while(trace: &[Exp], jd: &Exp) -> Result<(Vec<Exp>, Vec<Exp>, Vec<Exp>, Exp), String> {
    let mut path: Vec<Exp> = Vec::new();
    let mut remaining = trace.to_vec();

    while !remaining.is_empty() {
        let (line, rest) = remaining.split_first().unwrap();

        // Match if statement
        if let Exp::If(cond, if_true, if_false) = line {
            // Check for revert patterns
            if is_revert(if_true) {
                path.push(Exp::Op1(
                    "require".to_string(),
                    Box::new(is_zero(cond)),
                ));
                remaining = rest.to_vec();
                continue;
            }
            if is_revert(if_false) {
                path.push(Exp::Op1("require".to_string(), Box::new((**cond).clone())));
                remaining = if_true.clone();
                continue;
            }

            // Find goto targets in each branch
            let jds_true = get_jds(if_true);
            let jds_false = get_jds(if_false);

            // Assert that jd is in exactly one branch
            let in_true = jds_true.contains(jd);
            let in_false = jds_false.contains(jd);

            if in_true == in_false {
                return Err(format!(
                    "jd {:?} not in exactly one branch: {:?}, {:?}",
                    jd, jds_true, jds_false
                ));
            }

            // Add path information to goto statements
            if in_true {
                let if_true = rewrite_trace(if_true, |line| add_path(line, &path));
                return Ok((path, if_true, if_false.clone(), (**cond).clone()));
            } else {
                let if_false = rewrite_trace(if_false, |line| add_path(line, &path));
                return Ok((path, if_false, if_true.clone(), is_zero(cond)));
            }
        }

        path.push(line.clone());
        remaining = rest.to_vec();
    }

    Err("no if after label".to_string())
}

/// Check if a trace is a revert (single return 0 or invalid/revert)
fn is_revert(trace: &[Exp]) -> bool {
    if trace.len() > 1 {
        return false;
    }

    if trace.is_empty() {
        return false;
    }

    let line = &trace[0];

    // return 0
    if let Exp::Op2(op, a, _) = line {
        if op == "return" {
            if let Exp::Int(0) = **a {
                return true;
            }
        }
    }

    // invalid or revert
    let line_op = line.opcode();
    line_op == "revert" || line_op == "invalid"
}

/// Get all jump destinations from a trace
fn get_jds(trace: &[Exp]) -> Vec<Exp> {
    let mut result = Vec::new();

    for exp in trace {
        if let Exp::Goto(target, _) = exp {
            result.push((**target).clone());
        }
    }

    result
}

/// Add path information to a goto statement
fn add_path(line: &Exp, path: &[Exp]) -> Vec<Exp> {
    if let Exp::Goto(_target, setvars) = line {
        // Replace variables in path
        let mut new_path = path.to_vec();
        for (_, v_idx, v_val) in setvars {
            new_path = replace_var(&new_path, &Exp::Var(format!("var_{}", v_idx)), v_val);
        }
        let mut result = new_path;
        result.push(line.clone());
        result
    } else {
        vec![line.clone()]
    }
}

/// Rewrite trace with a function
fn rewrite_trace<F>(trace: &[Exp], f: F) -> Vec<Exp>
where
    F: Fn(&Exp) -> Vec<Exp>,
{
    let mut result = Vec::new();
    for exp in trace {
        result.extend(f(exp));
    }
    result
}

/// Replace a variable reference with a value
fn replace_var(trace: &[Exp], var: &Exp, val: &Exp) -> Vec<Exp> {
    trace
        .iter()
        .map(|exp| replace_var_exp(exp, var, val))
        .collect()
}

/// Replace variable in a single expression
fn replace_var_exp(exp: &Exp, var: &Exp, val: &Exp) -> Exp {
    if exp == var {
        return val.clone();
    }

    match exp {
        Exp::Op1(op, a) => Exp::Op1(op.clone(), Box::new(replace_var_exp(a, var, val))),
        Exp::Op2(op, a, b) => Exp::Op2(
            op.clone(),
            Box::new(replace_var_exp(a, var, val)),
            Box::new(replace_var_exp(b, var, val)),
        ),
        Exp::Op3(op, a, b, c) => Exp::Op3(
            op.clone(),
            Box::new(replace_var_exp(a, var, val)),
            Box::new(replace_var_exp(b, var, val)),
            Box::new(replace_var_exp(c, var, val)),
        ),
        Exp::Op4(op, a, b, c, d) => Exp::Op4(
            op.clone(),
            Box::new(replace_var_exp(a, var, val)),
            Box::new(replace_var_exp(b, var, val)),
            Box::new(replace_var_exp(c, var, val)),
            Box::new(replace_var_exp(d, var, val)),
        ),
        Exp::OpN(op, args) => Exp::OpN(
            op.clone(),
            args.iter().map(|a| replace_var_exp(a, var, val)).collect(),
        ),
        Exp::If(cond, then_br, else_br) => Exp::If(
            Box::new(replace_var_exp(cond, var, val)),
            replace_var(then_br, var, val),
            replace_var(else_br, var, val),
        ),
        Exp::While(cond, body) => Exp::While(
            Box::new(replace_var_exp(cond, var, val)),
            replace_var(body, var, val),
        ),
        Exp::Label(target, vars) => Exp::Label(Box::new(replace_var_exp(target, var, val)), vars.clone()),
        Exp::Goto(target, setvars) => Exp::Goto(Box::new(replace_var_exp(target, var, val)), setvars.clone()),
        _ => exp.clone(),
    }
}

/// Create an iszero expression
fn is_zero(exp: &Exp) -> Exp {
    Exp::Op1("iszero".to_string(), Box::new(exp.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_revert() {
        assert!(is_revert(&[Exp::Op2(
            "return".to_string(),
            Box::new(Exp::Int(0)),
            Box::new(Exp::None)
        )]));
    }

    #[test]
    fn test_is_not_revert() {
        assert!(!is_revert(&[Exp::Op2(
            "return".to_string(),
            Box::new(Exp::Int(1)),
            Box::new(Exp::None)
        )]));
        assert!(!is_revert(&[]));
    }
}

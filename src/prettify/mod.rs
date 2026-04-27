//! Pretty printing for decompiled output
//!
//! Converts IR to human-readable Solidity-like syntax.

use crate::core::types::Exp;

/// Color constants for terminal output
pub const COLOR_GREEN: &str = "\x1b[32m";
pub const COLOR_END: &str = "\x1b[0m";
pub const COLOR_BOLD: &str = "\x1b[1m";
pub const COLOR_GRAY: &str = "\x1b[90m";
pub const COLOR_RED: &str = "\x1b[31m";
pub const COLOR_BLUE: &str = "\x1b[34m";
pub const COLOR_YELLOW: &str = "\x1b[33m";
pub const COLOR_OKGREEN: &str = "\x1b[92m";
pub const COLOR_WARNING: &str = "\x1b[93m";
pub const COLOR_HEADER: &str = "\x1b[1m\x1b[34m";

/// Pretty print a trace
pub fn pprint_trace(trace: &[Exp]) {
    for exp in trace {
        println!("{}", pretty_exp(exp));
    }
}

/// Pretty print a single expression
pub fn pretty_exp(exp: &Exp) -> String {
    match exp {
        Exp::Var(name) => name.clone(),
        
        Exp::Int(n) => {
            // Format as hex if larger than typical numbers
            if *n > 10u128.pow(6) && *n % 10u128.pow(6) != 0 {
                format!("0x{:x}", n)
            } else {
                n.to_string()
            }
        }
        
        Exp::Str(s) => format!("\"{}\"", s),
        
        Exp::None => "none".to_string(),
        
        Exp::Jd(n) => format!("jd({})", n),
        
        Exp::Op(op) => format!("({})", op),
        
        Exp::Op1(op, a) => {
            let a_str = pretty_exp(a);
            match op.as_str() {
                "not" => format!("!{}", a_str),
                "iszero" => format!("iszero({})", a_str),
                "sha3" => format!("sha3({})", a_str),
                _ => format!("({} {})", op, a_str),
            }
        }
        
        Exp::Op2(op, a, b) => {
            let a_str = pretty_exp(a);
            let b_str = pretty_exp(b);
            match op.as_str() {
                "add" => format!("({} + {})", a_str, b_str),
                "sub" => format!("({} - {})", a_str, b_str),
                "mul" => format!("({} * {})", a_str, b_str),
                "div" => format!("({} / {})", a_str, b_str),
                "mod" => format!("({} % {})", a_str, b_str),
                "eq" => format!("({} == {})", a_str, b_str),
                "lt" => format!("({} < {})", a_str, b_str),
                "gt" => format!("({} > {})", a_str, b_str),
                "le" => format!("({} <= {})", a_str, b_str),
                "ge" => format!("({} >= {})", a_str, b_str),
                "and" => format!("({} and {})", a_str, b_str),
                "or" => format!("({} or {})", a_str, b_str),
                "xor" => format!("({} xor {})", a_str, b_str),
                "shl" => format!("({} << {})", b_str, a_str),
                "shr" => format!("({} >> {})", b_str, a_str),
                "exp" => format!("({}^{})", a_str, b_str),
                "sadd" => format!("({} +' {})", a_str, b_str),
                "smul" => format!("({} *' {})", a_str, b_str),
                "sdiv" => format!("({} /' {})", a_str, b_str),
                "sgt" => format!("({} >' {})", a_str, b_str),
                "slt" => format!("({} <' {})", a_str, b_str),
                "sge" => format!("({} >=' {})", a_str, b_str),
                "sle" => format!("({} <=' {})", a_str, b_str),
                _ => format!("({} {} {})", op, a_str, b_str),
            }
        }
        
        Exp::Op3(op, a, b, c) => {
            let a_str = pretty_exp(a);
            let b_str = pretty_exp(b);
            let c_str = pretty_exp(c);
            format!("({} {} {} {})", op, a_str, b_str, c_str)
        }
        
        Exp::Op4(op, a, b, c, d) => {
            let a_str = pretty_exp(a);
            let b_str = pretty_exp(b);
            let c_str = pretty_exp(c);
            let d_str = pretty_exp(d);
            format!("({} {} {} {} {})", op, a_str, b_str, c_str, d_str)
        }
        
        Exp::OpN(op, args) => {
            if args.is_empty() {
                format!("({})", op)
            } else {
                let args_str: Vec<String> = args.iter().map(|e| pretty_exp(e)).collect();
                format!("({} {})", op, args_str.join(" "))
            }
        }
        
        Exp::If(cond, then_br, else_br) => {
            let cond_str = pretty_exp(cond);
            let then_strs: Vec<String> = then_br.iter().map(|e| pretty_exp(e)).collect();
            let else_strs: Vec<String> = else_br.iter().map(|e| pretty_exp(e)).collect();
            format!(
                "(if {} [{}] [{}])",
                cond_str,
                then_strs.join(" "),
                else_strs.join(" ")
            )
        }
        
        Exp::While(cond, body) => {
            let cond_str = pretty_exp(cond);
            let body_strs: Vec<String> = body.iter().map(|e| pretty_exp(e)).collect();
            format!("(while {} [{}])", cond_str, body_strs.join(" "))
        }
        
        Exp::Goto(target, _) => {
            format!("(goto {})", pretty_exp(target))
        }
        
        Exp::Loop(label, body, _, _) => {
            let label_str = pretty_exp(label);
            let body_strs: Vec<String> = body.iter().map(|e| pretty_exp(e)).collect();
            format!("(loop {} [{}])", label_str, body_strs.join(" "))
        }
        
        Exp::Label(target, _) => {
            format!("(label {})", pretty_exp(target))
        }
        
        Exp::Continue => "(continue)".to_string(),
        
        Exp::Break => "(break)".to_string(),
    }
}

/// Pretty type representation
pub fn pretty_type(var: &str) -> String {
    var.to_string()
}

/// Print representation
pub fn pprint_repr(trace: &[Exp]) {
    for exp in trace {
        println!("{:?}", exp);
    }
}

/// Explain a step in decompilation
pub fn explain(label: &str, trace: &[Exp]) {
    println!("\n{}:\n", label);
    pprint_trace(trace);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_var() {
        assert_eq!(pretty_exp(&Exp::var("x")), "x");
        assert_eq!(pretty_exp(&Exp::var("myVar")), "myVar");
    }
    
    #[test]
    fn test_int() {
        assert_eq!(pretty_exp(&Exp::int(42)), "42");
        assert_eq!(pretty_exp(&Exp::int(0)), "0");
    }
    
    #[test]
    fn test_add() {
        let exp = Exp::add(Exp::var("a"), Exp::int(1));
        assert_eq!(pretty_exp(&exp), "(a + 1)");
    }
    
    #[test]
    fn test_mul() {
        let exp = Exp::mul(Exp::var("x"), Exp::int(2));
        assert_eq!(pretty_exp(&exp), "(x * 2)");
    }
    
    #[test]
    fn test_not() {
        let exp = Exp::Op1("not".into(), Box::new(Exp::var("x")));
        assert_eq!(pretty_exp(&exp), "!x");
    }
}

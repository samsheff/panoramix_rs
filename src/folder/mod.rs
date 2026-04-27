//! Algebraic simplification and folding
//!
//! Turns intermediate representation into concise human-readable form.

use crate::core::types::Exp;

/// Terminating opcodes that end execution flow
const TERMINATING: &[&str] = &[
    "return", "stop", "selfdestruct", "invalid", "assert_fail", "revert", "continue", "undefined",
];

/// Main entry point - fold a trace into simplified form
pub fn fold(trace: Vec<Exp>) -> Vec<Exp> {
    match as_paths(&trace) {
        Ok(paths) => {
            let log = meta_fold_paths(paths);
            fold_aux(&log)
        }
        Err(_) => trace,
    }
}

/// Unfold trace into branchless paths, converting ifs to conditions
pub fn as_paths(trace: &[Exp]) -> Result<Vec<Vec<Exp>>, ()> {
    let transformed = replace_f(trace, &make_fands);
    Ok(as_paths_inner(&transformed, &Vec::new()))
}

fn as_paths_inner(trace: &[Exp], path: &[Exp]) -> Vec<Vec<Exp>> {
    let mut current_path = path.to_vec();
    
    for line in trace {
        match line {
            Exp::If(cond, if_true, if_false) => {
                let mut true_path = current_path.clone();
                true_path.push((**cond).clone());
                true_path.extend_from_slice(if_true);
                
                let mut false_path = current_path.clone();
                false_path.push(Exp::iszero((**cond).clone()));
                false_path.extend_from_slice(if_false);
                
                return vec![true_path, false_path];
            }
            Exp::Loop(label, body, _, _) => {
                current_path.push(Exp::Op2(
                    "LOOP".into(),
                    Box::new((**label).clone()),
                    Box::new(Exp::None),
                ));
                return as_paths_inner(body, &current_path);
            }
            _ => {
                current_path.push(line.clone());
            }
        }
    }
    
    vec![current_path]
}

/// Apply a transformation function to all expressions in a trace
fn replace_f(trace: &[Exp], f: &dyn Fn(&Exp) -> Exp) -> Vec<Exp> {
    trace.iter().map(|e| replace_f_exp(e, f)).collect()
}

fn replace_f_exp(exp: &Exp, f: &dyn Fn(&Exp) -> Exp) -> Exp {
    let transformed = match exp {
        Exp::Op1(op, a) => Exp::Op1(op.clone(), Box::new(replace_f_exp(a, f))),
        Exp::Op2(op, a, b) => Exp::Op2(op.clone(), 
            Box::new(replace_f_exp(a, f)), 
            Box::new(replace_f_exp(b, f))),
        Exp::Op3(op, a, b, c) => Exp::Op3(op.clone(),
            Box::new(replace_f_exp(a, f)),
            Box::new(replace_f_exp(b, f)),
            Box::new(replace_f_exp(c, f))),
        Exp::Op4(op, a, b, c, d) => Exp::Op4(op.clone(),
            Box::new(replace_f_exp(a, f)),
            Box::new(replace_f_exp(b, f)),
            Box::new(replace_f_exp(c, f)),
            Box::new(replace_f_exp(d, f))),
        Exp::OpN(op, args) => Exp::OpN(op.clone(), replace_f(args, f)),
        Exp::If(cond, then_br, else_br) => Exp::If(
            Box::new(replace_f_exp(cond, f)),
            replace_f(then_br, f),
            replace_f(else_br, f),
        ),
        Exp::While(cond, body) => Exp::While(
            Box::new(replace_f_exp(cond, f)),
            replace_f(body, f),
        ),
        Exp::Loop(label, body, jds, vars) => Exp::Loop(
            Box::new(replace_f_exp(label, f)),
            replace_f(body, f),
            jds.clone(),
            vars.clone(),
        ),
        Exp::Label(target, jds) => Exp::Label(
            Box::new(replace_f_exp(target, f)),
            jds.clone(),
        ),
        Exp::Goto(target, jds) => Exp::Goto(
            Box::new(replace_f_exp(target, f)),
            jds.clone(),
        ),
        _ => exp.clone(),
    };
    f(&transformed)
}

/// Make fands - convert "or" to "for" and "and" to "fand"
fn make_fands(exp: &Exp) -> Exp {
    match exp {
        Exp::Op2(op, a, b) if op == "or" => {
            Exp::OpN("for".into(), vec![*a.clone(), *b.clone()])
        }
        Exp::Op2(op, a, b) if op == "and" => {
            Exp::OpN("fand".into(), vec![*a.clone(), *b.clone()])
        }
        _ => exp.clone(),
    }
}

/// Unmake fands - convert "for" back to "or" and "fand" back to "and"
fn unmake_fands(exp: &Exp) -> Exp {
    match exp {
        Exp::OpN(op, args) if op.as_str() == "for" && args.len() == 2 => {
            Exp::Op2("or".into(), Box::new(args[0].clone()), Box::new(args[1].clone()))
        }
        Exp::OpN(op, args) if op.as_str() == "fand" && args.len() == 2 => {
            Exp::Op2("and".into(), Box::new(args[0].clone()), Box::new(args[1].clone()))
        }
        _ => exp.clone(),
    }
}

/// Fold a single expression
pub fn fold_exp(exp: &Exp) -> Exp {
    exp.clone()
}

// ============================================================================
// Helper functions
// ============================================================================

fn opcode(exp: &Exp) -> &str {
    exp.opcode()
}

fn car(exp: &Exp) -> Exp {
    match exp {
        Exp::OpN(_, args) if !args.is_empty() => args[0].clone(),
        _ => exp.clone(),
    }
}

// ============================================================================
// Main folding algorithms
// ============================================================================

fn meta_fold_paths(paths: Vec<Vec<Exp>>) -> Vec<Exp> {
    let for_merge: Vec<Vec<Exp>> = paths.into_iter()
        .filter(|r| !r.is_empty())
        .collect();
    
    if for_merge.is_empty() {
        return vec![];
    }
    
    let output = fold_paths(for_merge);
    let output = flatten_or(&output);
    let output = cleanup_ors(&output);
    let output = make_ifs(&output);
    let output = merge_ifs(&output);
    
    replace_f(&output, &unmake_fands)
}

fn fold_paths(for_merge: Vec<Vec<Exp>>) -> Vec<Exp> {
    if for_merge.is_empty() {
        return vec![];
    }
    
    if for_merge.len() == 1 {
        return for_merge.into_iter().next().unwrap();
    }
    
    let mut sorted = for_merge;
    sorted.sort_by(|a, b| b.len().cmp(&a.len()));
    
    let begin_offset = compute_begin_offset(&sorted);
    let end_offset = compute_end_offset(&sorted);
    
    let s_with = starting_with_or(&sorted, begin_offset);
    
    let merged = if end_offset > 0 {
        let e_with = ending_with_or(&s_with, end_offset);
        let mut result = sorted[0][..begin_offset].to_vec();
        result.push(make_or(&e_with));
        result.extend_from_slice(&sorted[0][sorted[0].len() - end_offset..]);
        result
    } else {
        let mut result = sorted[0][..begin_offset].to_vec();
        result.push(make_or(&s_with));
        result
    };
    
    let mut output = vec![];
    for line in merged {
        if opcode(&line) != "or" {
            output.push(line);
        } else {
            let (ors, paths) = fold_or_line(&line);
            output.push(ors);
            output.extend(fold_paths(paths));
        }
    }
    
    output
}

fn compute_begin_offset(sorted: &[Vec<Exp>]) -> usize {
    let mut idx = 0;
    while starting_with_or(sorted, idx + 1).len() == sorted.len() {
        idx += 1;
    }
    idx
}

fn compute_end_offset(sorted: &[Vec<Exp>]) -> usize {
    let mut idx = 1;
    while ending_with_or(sorted, idx).len() == sorted.len() {
        idx += 1;
    }
    idx.saturating_sub(1)
}

fn starting_with_or(paths: &[Vec<Exp>], offset: usize) -> Vec<Vec<Exp>> {
    if offset == 0 || paths.is_empty() || offset > paths[0].len() {
        return vec![];
    }
    
    let prefix = &paths[0][..offset];
    let mut result = vec![];
    
    for path in paths {
        if path.len() >= offset && &path[..offset] == prefix {
            result.push(path[offset..].to_vec());
        }
    }
    
    result
}

fn ending_with_or(paths: &[Vec<Exp>], offset: usize) -> Vec<Vec<Exp>> {
    if offset == 0 || paths.is_empty() || offset > paths[0].len() {
        return vec![];
    }
    
    let suffix_start = paths[0].len() - offset;
    let suffix = &paths[0][suffix_start..];
    let mut result = vec![];
    
    for path in paths {
        if path.len() >= offset {
            let path_suffix_start = path.len() - offset;
            if &path[path_suffix_start..] == suffix {
                result.push(path[..path_suffix_start].to_vec());
            }
        }
    }
    
    result
}

fn make_or(paths: &[Vec<Exp>]) -> Exp {
    if paths.is_empty() {
        return Exp::None;
    }
    if paths.len() == 1 {
        if paths[0].is_empty() {
            return Exp::None;
        }
        return Exp::OpN("and".into(), paths[0].clone());
    }
    
    let mut same_first = true;
    let first_first = if !paths[0].is_empty() { &paths[0][0] } else { &Exp::None };
    for p in paths.iter().skip(1) {
        let p_first = if !p.is_empty() { &p[0] } else { &Exp::None };
        if p_first != first_first {
            same_first = false;
            break;
        }
    }
    
    if same_first {
        let mut results = vec![];
        for p in paths {
            if p.len() > 1 {
                results.push(Exp::OpN("and".into(), p[1..].to_vec()));
            } else {
                results.push(Exp::None);
            }
        }
        return Exp::OpN("or".into(), results);
    }
    
    Exp::OpN("or".into(), paths.iter()
        .map(|p| Exp::OpN("and".into(), p.clone()))
        .collect())
}

fn fold_or_line(line: &Exp) -> (Exp, Vec<Vec<Exp>>) {
    let variants = match line {
        Exp::OpN(op, args) if op.as_str() == "or" => args.clone(),
        _ => return (line.clone(), vec![]),
    };
    
    if variants.is_empty() {
        return (line.clone(), vec![]);
    }
    
    let mut longest_idx = 0;
    let mut shortest_idx = 0;
    let mut longest_len = 0;
    let mut shortest_len = usize::MAX;
    
    for (i, v) in variants.iter().enumerate() {
        let len = match v {
            Exp::OpN(_, args) => args.len(),
            _ => 1,
        };
        if len > longest_len {
            longest_len = len;
            longest_idx = i;
        }
        if len < shortest_len {
            shortest_len = len;
            shortest_idx = i;
        }
    }
    
    let longest = &variants[longest_idx];
    let shortest = &variants[shortest_idx];
    
    let longest_args = match longest {
        Exp::OpN(_, args) => args,
        _ => return (line.clone(), vec![]),
    };
    let shortest_args = match shortest {
        Exp::OpN(_, args) => args,
        _ => return (line.clone(), vec![]),
    };
    
    let mut best: Option<(usize, usize, Vec<Vec<Exp>>)> = None;
    
    for idx1 in 1..shortest_args.len().max(1) {
        let s1 = collect_starting_with(&variants, &shortest_args[..idx1]);
        for idx2 in 1..longest_args.len().max(1) {
            let s2 = collect_starting_with(&variants, &longest_args[..idx2]);
            if best.is_none() && s1 == s2 && s1.len() + s2.len() + 1 == variants.len() {
                best = Some((idx1, idx2, s1.clone()));
            }
        }
    }
    
    if let Some((b1, b2, best_s)) = best {
        return (
            make_or(&[shortest_args[..b1].to_vec(), longest_args[..b2].to_vec()]),
            best_s,
        );
    }
    
    let first_elems: Vec<Exp> = variants.iter().map(|v| 
        match v {
            Exp::OpN(_, args) if !args.is_empty() => args[0].clone(),
            _ => v.clone(),
        }
    ).collect();
    
    let s1 = collect_starting_with(&variants, &first_elems);
    let s2 = collect_starting_with(&variants, &first_elems);
    
    let s1_folded = fold_paths(s1.clone());
    let s2_folded = fold_paths(s2.clone());
    
    let first_elem = &first_elems[shortest_idx];
    let longer_first = &first_elems[longest_idx];
    
    let shorter_path = and_op_path(first_elem, &s1_folded);
    let longer_path = and_op_path(longer_first, &s2_folded);
    
    (Exp::OpN("or".into(), vec![shorter_path, longer_path]), vec![])
}

fn collect_starting_with(variants: &[Exp], prefix: &[Exp]) -> Vec<Vec<Exp>> {
    let mut result = vec![];
    
    for v in variants {
        let args = match v {
            Exp::OpN(_, a) => a,
            _ => return result,
        };
        if args.len() >= prefix.len() && &args[..prefix.len()] == prefix {
            result.push(args[prefix.len()..].to_vec());
        }
    }
    
    result
}

fn and_op_path(first: &Exp, path: &[Exp]) -> Exp {
    if path.is_empty() {
        return first.clone();
    }
    if path.len() == 1 {
        if let Exp::OpN(_, ref mut args) = &mut path[0].clone() {
            args.insert(0, first.clone());
            return Exp::OpN("and".into(), args.clone());
        }
    }
    let mut combined = vec![first.clone()];
    combined.extend_from_slice(path);
    Exp::OpN("and".into(), combined)
}

// ============================================================================
// Flatten, cleanup_ors, make_ifs, merge_ifs
// ============================================================================

fn flatten_or(path: &[Exp]) -> Vec<Exp> {
    let mut res = vec![];
    
    for line in path {
        if opcode(line) != "or" {
            res.push(line.clone());
            continue;
        }
        
        let (branch1, branch2) = match line {
            Exp::OpN(op, args) if op.as_str() == "or" && args.len() == 2 => {
                (flatten_single(&args[0]), flatten_single(&args[1]))
            }
            _ => {
                res.push(line.clone());
                continue;
            }
        };
        
        if branch1.len() == 1 && branch2.len() == 1 {
            continue;
        }
        
        if ends_exec_vec(&branch1) {
            res.extend(try_merge_flatten(&branch1, &branch2));
        } else {
            res.push(Exp::OpN("or".into(), vec![
                flatten_or_inner(&branch1),
                flatten_or_inner(&branch2),
            ]));
        }
    }
    
    res
}

fn flatten_single(exp: &Exp) -> Vec<Exp> {
    match exp {
        Exp::OpN(op, args) if op.as_str() == "and" => args.clone(),
        _ => vec![exp.clone()],
    }
}

fn flatten_or_inner(exp: &[Exp]) -> Exp {
    let flattened: Vec<Exp> = exp.iter()
        .flat_map(|a| flatten_or(&[a.clone()]))
        .collect();
    if flattened.len() == 1 {
        flattened[0].clone()
    } else {
        Exp::OpN("and".into(), flattened)
    }
}

fn ends_exec_vec(path: &[Exp]) -> bool {
    if path.is_empty() {
        return false;
    }
    
    let line = &path[path.len() - 1];
    
    if TERMINATING.contains(&opcode(line)) {
        true
    } else if opcode(line) == "or" {
        if let Exp::OpN(op, args) = line {
            if op.as_str() == "or" && args.len() == 2 {
                ends_exec_vec(flatten_single(&args[0]).as_slice()) && 
                ends_exec_vec(flatten_single(&args[1]).as_slice())
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

fn try_merge_flatten(one: &[Exp], two: &[Exp]) -> Vec<Exp> {
    let (shorter, longer) = if one.len() > two.len() {
        (two, one)
    } else {
        (one, two)
    };
    
    let mut idx = 1;
    while idx < shorter.len() && longer[longer.len() - idx] == shorter[shorter.len() - idx] {
        idx += 1;
    }
    idx -= 1;
    
    if idx > 0 {
        vec![Exp::OpN("or".into(), vec![
            Exp::OpN("and".into(), longer[..longer.len() - idx].to_vec()),
            Exp::OpN("and".into(), shorter[..shorter.len() - idx].to_vec()),
        ])]
    } else {
        vec![Exp::OpN("or".into(), vec![
            Exp::OpN("and".into(), one.to_vec()),
        ])]
    }
}

fn cleanup_ors(path: &[Exp]) -> Vec<Exp> {
    let mut ret = vec![];
    let mut idx = 0;
    
    while idx < path.len() {
        let line = &path[idx];
        
        if is_list_exp(line) {
            let mut new_path = path[..idx].to_vec();
            new_path.extend(list_to_vec(line));
            new_path.extend_from_slice(&path[idx + 1..]);
            return cleanup_ors(&new_path);
        }
        
        if opcode(line) != "or" {
            ret.push(line.clone());
        } else if let Exp::OpN(op, args) = line {
            if op.as_str() == "or" {
                if args.len() == 1 {
                    ret.push(cleanup_ors(&list_to_vec(&args[0]))[0].clone());
                    idx += 1;
                } else if args.len() == 2 {
                    let a1 = flatten_single(&args[0]);
                    let a2 = flatten_single(&args[1]);
                    if a1.len() == 1 {
                        ret.push(make_or(&[a2[1..].to_vec()]));
                    } else {
                        ret.push(make_or(&[a1[1..].to_vec(), a2[1..].to_vec()]));
                    }
                    idx += 1;
                } else {
                    ret.push(line.clone());
                }
            } else {
                ret.push(line.clone());
            }
        } else {
            ret.push(line.clone());
        }
        
        idx += 1;
    }
    
    ret
}

fn is_list_exp(exp: &Exp) -> bool {
    matches!(exp, Exp::OpN(op, _) if op.as_str() == "and" || op.as_str() == "or")
}

fn list_to_vec(exp: &Exp) -> Vec<Exp> {
    match exp {
        Exp::OpN(_, args) => args.clone(),
        _ => vec![exp.clone()],
    }
}

fn make_ifs(path: &[Exp]) -> Vec<Exp> {
    let mut ret = vec![];
    
    for line in path {
        if opcode(line) != "or" {
            ret.push(line.clone());
        } else if let Exp::OpN(op, args) = line {
            if op.as_str() == "or" && args.len() == 2 {
                let b1 = flatten_single(&args[0]);
                let b2 = flatten_single(&args[1]);
                if !b1.is_empty() && !b2.is_empty() {
                    let cond = b1[0].clone();
                    let then_branch = if b1.len() > 1 { b1[1..].to_vec() } else { vec![] };
                    let else_branch = if b2.len() > 1 { b2[1..].to_vec() } else { vec![] };
                    ret.push(Exp::If(
                        Box::new(cond),
                        then_branch,
                        else_branch,
                    ));
                } else {
                    ret.push(line.clone());
                }
            } else {
                ret.push(line.clone());
            }
        } else {
            ret.push(line.clone());
        }
    }
    
    ret
}

fn try_merge_ifs(cond: &Exp, if_true: &[Exp], if_false: &[Exp]) -> (Vec<Exp>, Exp) {
    let mut idx = 0;
    while idx < if_true.len() && idx < if_false.len() && if_true[idx] == if_false[idx] {
        idx += 1;
    }
    
    if idx > 0 {
        let lines = if_true[..idx].to_vec();
        let merged = Exp::If(
            Box::new(cond.clone()),
            if_true[idx..].to_vec(),
            if_false[idx..].to_vec(),
        );
        (lines, merged)
    } else {
        let merged = Exp::If(
            Box::new(cond.clone()),
            if_true.to_vec(),
            if_false.to_vec(),
        );
        (vec![], merged)
    }
}

fn merge_ifs(path: &[Exp]) -> Vec<Exp> {
    let mut ret = vec![];
    let mut idx = 0;
    
    while idx < path.len() {
        let line = &path[idx];
        
        if opcode(line) != "if" {
            ret.push(line.clone());
            idx += 1;
            continue;
        }
        
        let (cond, if_true, if_false) = match line {
            Exp::If(c, t, f) => ((**c).clone(), t.clone(), f.clone()),
            _ => {
                ret.push(line.clone());
                idx += 1;
                continue;
            }
        };
        
        if if_false.is_empty() {
            let merged_true = merge_ifs(&if_true);
            let merged_false = merge_ifs(&path[idx + 1..]);
            let (lines, merged) = try_merge_ifs(&cond, &merged_true, &merged_false);
            ret.extend(lines);
            ret.push(merged);
            return ret;
        } else {
            let merged_true = merge_ifs(&if_true);
            let merged_false = merge_ifs(&if_false);
            let (lines, merged) = try_merge_ifs(&cond, &merged_true, &merged_false);
            ret.extend(lines);
            ret.push(merged);
        }
        
        idx += 1;
    }
    
    ret
}

fn fold_aux(trace: &[Exp]) -> Vec<Exp> {
    let mut out = vec![];
    let mut idx = 0;
    
    while idx < trace.len() {
        let line = &trace[idx];
        
        match line {
            Exp::While(cond, body) => {
                let folded_body = fold(body.clone());
                out.push(Exp::While(cond.clone(), folded_body));
            }
            Exp::If(cond, if_true, if_false) => {
                let if_true_folded = fold_aux(if_true);
                
                let false_terminates = if_false.len() == 1 && (
                    (opcode(&if_false[0]) == "return" && if_false[0] == Exp::int(0)) ||
                    (opcode(&if_false[0]) == "revert" && if_false[0] == Exp::int(0)) ||
                    opcode(&if_false[0]) == "invalid"
                );
                
                if if_false.is_empty() || false_terminates {
                    if !if_true_folded.is_empty() && !TERMINATING.contains(&opcode(&if_true_folded[if_true_folded.len() - 1])) {
                        let mut new_true = if_true_folded.clone();
                        if !if_false.is_empty() {
                            new_true.push(if_false[0].clone());
                        }
                        out.push(Exp::If(cond.clone(), new_true, vec![]));
                    } else {
                        out.push(Exp::If(cond.clone(), if_true_folded, vec![]));
                    }
                    return out;
                } else {
                    let if_false_folded = fold_aux(if_false);
                    
                    let true_terminates = !if_true_folded.is_empty() && 
                        TERMINATING.contains(&opcode(&if_true_folded[if_true_folded.len() - 1]));
                    let false_invalid = !if_false_folded.is_empty() && 
                        opcode(&if_false_folded[0]) == "invalid";
                    
                    if true_terminates && !false_invalid && !if_false_folded.is_empty() {
                        out.push(Exp::If(cond.clone(), if_true_folded, vec![]));
                        out.extend(if_false_folded);
                    } else {
                        out.push(Exp::If(cond.clone(), if_true_folded, if_false_folded));
                    }
                }
            }
            _ => {
                out.push(line.clone());
            }
        }
        
        idx += 1;
    }
    
    out
}

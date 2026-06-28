use crate::parser::{Expr, Op};
use crate::units::{convert_unit, lookup_unit};
use crate::currency::convert_currency;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ResultValue {
    Number(f64),
    Unit(f64, String),
    Currency(f64, String),
    Percentage(f64),
    Error(String),
    Empty,
}

impl std::fmt::Display for ResultValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultValue::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{}", *n as i64)
                } else {
                    let s = format!("{:.4}", n);
                    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
                    write!(f, "{}", trimmed)
                }
            }
            ResultValue::Unit(n, u) => {
                let val_str = if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    let s = format!("{:.4}", n);
                    s.trim_end_matches('0').trim_end_matches('.').to_string()
                };
                write!(f, "{} {}", val_str, u)
            }
            ResultValue::Currency(n, c) => {
                let symbol = match c.as_str() {
                    "USD" => "$",
                    "EUR" => "€",
                    "GBP" => "£",
                    "JPY" => "¥",
                    _ => "",
                };
                if symbol.is_empty() {
                    write!(f, "{:.2} {}", n, c)
                } else {
                    write!(f, "{}{:.2}", symbol, n)
                }
            }
            ResultValue::Percentage(p) => write!(f, "{}%", p),
            ResultValue::Error(e) => write!(f, "Error: {}", e),
            ResultValue::Empty => write!(f, ""),
        }
    }
}

fn eval_expr(expr: &Expr, env: &[ResultValue], symbols: &mut HashMap<String, ResultValue>, cache_path: Option<&str>) -> ResultValue {
    match expr {
        Expr::Number(n) => ResultValue::Number(*n),
        Expr::Assign(name, val_expr) => {
            let val = eval_expr(val_expr, env, symbols, cache_path);
            if let ResultValue::Error(_) = &val {
                val
            } else {
                symbols.insert(name.clone(), val.clone());
                val
            }
        }
        Expr::Percentage(inner) => {
            match eval_expr(inner, env, symbols, cache_path) {
                ResultValue::Number(n) => ResultValue::Percentage(n),
                ResultValue::Error(e) => ResultValue::Error(e),
                _ => ResultValue::Error("Invalid percentage operand".to_string()),
            }
        }
        Expr::Unit(val, name) => ResultValue::Unit(*val, name.clone()),
        Expr::Currency(val, code) => ResultValue::Currency(*val, code.clone()),
        Expr::Variable(var) => {
            if var == "ans" {
                if env.is_empty() {
                    ResultValue::Error("No previous line results (ans)".to_string())
                } else {
                    let prev = env.last().unwrap();
                    if let ResultValue::Error(e) = prev {
                        ResultValue::Error(format!("Depends on line with error: {}", e))
                    } else {
                        prev.clone()
                    }
                }
            } else if var.starts_with("line") {
                let num_str = &var[4..];
                if let Ok(line_num) = num_str.parse::<usize>() {
                    if line_num == 0 {
                        ResultValue::Error("Lines are 1-indexed".to_string())
                    } else if line_num > env.len() {
                        ResultValue::Error(format!("Line {} has not been evaluated yet", line_num))
                    } else {
                        let res = &env[line_num - 1];
                        if let ResultValue::Error(e) = res {
                            ResultValue::Error(format!("Depends on line {} with error: {}", line_num, e))
                        } else {
                            res.clone()
                        }
                    }
                } else {
                    if let Some(val) = symbols.get(var) {
                        val.clone()
                    } else {
                        ResultValue::Empty
                    }
                }
            } else {
                if let Some(val) = symbols.get(var) {
                    val.clone()
                } else {
                    ResultValue::Empty
                }
            }
        }
        Expr::Convert(lhs_expr, target) => {
            let lhs = eval_expr(lhs_expr, env, symbols, cache_path);
            match lhs {
                ResultValue::Unit(val, from_unit) => {
                    match convert_unit(val, &from_unit, target) {
                        Ok(new_val) => ResultValue::Unit(new_val, target.clone()),
                        Err(e) => ResultValue::Error(e),
                    }
                }
                ResultValue::Currency(val, from_curr) => {
                    match convert_currency(val, &from_curr, target, cache_path) {
                        Ok(new_val) => ResultValue::Currency(new_val, target.to_uppercase()),
                        Err(e) => ResultValue::Error(e),
                    }
                }
                ResultValue::Number(val) => {
                    if lookup_unit(target).is_some() {
                        ResultValue::Unit(val, target.clone())
                    } else {
                        ResultValue::Currency(val, target.to_uppercase())
                    }
                }
                ResultValue::Error(e) => ResultValue::Error(e),
                ResultValue::Empty => ResultValue::Empty,
                _ => ResultValue::Error("Cannot convert this value type".to_string()),
            }
        }
        Expr::Binary(lhs_expr, op, rhs_expr) => {
            let lhs = eval_expr(lhs_expr, env, symbols, cache_path);
            let rhs = eval_expr(rhs_expr, env, symbols, cache_path);

            match (lhs, rhs) {
                (ResultValue::Error(e), _) => ResultValue::Error(e),
                (_, ResultValue::Error(e)) => ResultValue::Error(e),

                // Percentage arithmetic
                (ResultValue::Number(l), ResultValue::Percentage(p)) => {
                    match op {
                        Op::Add => ResultValue::Number(l + l * (p / 100.0)),
                        Op::Sub => ResultValue::Number(l - l * (p / 100.0)),
                        Op::Mul => ResultValue::Number(l * (p / 100.0)),
                        Op::Div => ResultValue::Number(l / (p / 100.0)),
                        _ => ResultValue::Error("Unsupported operation with percentage".to_string()),
                    }
                }
                (ResultValue::Unit(l_val, l_unit), ResultValue::Percentage(p)) => {
                    match op {
                        Op::Add => ResultValue::Unit(l_val + l_val * (p / 100.0), l_unit),
                        Op::Sub => ResultValue::Unit(l_val - l_val * (p / 100.0), l_unit),
                        Op::Mul => ResultValue::Unit(l_val * (p / 100.0), l_unit),
                        Op::Div => ResultValue::Unit(l_val / (p / 100.0), l_unit),
                        _ => ResultValue::Error("Unsupported operation with percentage".to_string()),
                    }
                }
                (ResultValue::Currency(l_val, l_curr), ResultValue::Percentage(p)) => {
                    match op {
                        Op::Add => ResultValue::Currency(l_val + l_val * (p / 100.0), l_curr),
                        Op::Sub => ResultValue::Currency(l_val - l_val * (p / 100.0), l_curr),
                        Op::Mul => ResultValue::Currency(l_val * (p / 100.0), l_curr),
                        Op::Div => ResultValue::Currency(l_val / (p / 100.0), l_curr),
                        _ => ResultValue::Error("Unsupported operation with percentage".to_string()),
                    }
                }
                (ResultValue::Percentage(p), ResultValue::Number(r)) => {
                    match op {
                        Op::Mul => ResultValue::Number(r * (p / 100.0)),
                        _ => ResultValue::Error("Unsupported operation with percentage".to_string()),
                    }
                }
                (ResultValue::Percentage(p), ResultValue::Unit(r_val, r_unit)) => {
                    match op {
                        Op::Mul => ResultValue::Unit(r_val * (p / 100.0), r_unit),
                        _ => ResultValue::Error("Unsupported operation with percentage".to_string()),
                    }
                }
                (ResultValue::Percentage(p), ResultValue::Currency(r_val, r_curr)) => {
                    match op {
                        Op::Mul => ResultValue::Currency(r_val * (p / 100.0), r_curr),
                        _ => ResultValue::Error("Unsupported operation with percentage".to_string()),
                    }
                }

                // Standard number arithmetic
                (ResultValue::Number(l), ResultValue::Number(r)) => {
                    match op {
                        Op::Add => ResultValue::Number(l + r),
                        Op::Sub => ResultValue::Number(l - r),
                        Op::Mul => ResultValue::Number(l * r),
                        Op::Div => {
                            if r == 0.0 {
                                ResultValue::Error("Division by zero".to_string())
                            } else {
                                ResultValue::Number(l / r)
                            }
                        }
                        Op::Pow => ResultValue::Number(l.powf(r)),
                    }
                }

                // Unit arithmetic
                (ResultValue::Unit(l_val, l_unit), ResultValue::Unit(r_val, r_unit)) => {
                    match op {
                        Op::Add => {
                            match convert_unit(r_val, &r_unit, &l_unit) {
                                Ok(conv_r) => ResultValue::Unit(l_val + conv_r, l_unit),
                                Err(e) => ResultValue::Error(e),
                            }
                        }
                        Op::Sub => {
                            match convert_unit(r_val, &r_unit, &l_unit) {
                                Ok(conv_r) => ResultValue::Unit(l_val - conv_r, l_unit),
                                Err(e) => ResultValue::Error(e),
                            }
                        }
                        Op::Mul => ResultValue::Error("Multiplication of two units is not supported".to_string()),
                        Op::Div => {
                            match convert_unit(r_val, &r_unit, &l_unit) {
                                Ok(conv_r) => {
                                    if conv_r == 0.0 {
                                        ResultValue::Error("Division by zero".to_string())
                                    } else {
                                        ResultValue::Number(l_val / conv_r)
                                    }
                                }
                                Err(e) => ResultValue::Error(e),
                            }
                        }
                        _ => ResultValue::Error("Operation not supported for units".to_string()),
                    }
                }
                (ResultValue::Unit(l_val, l_unit), ResultValue::Number(r)) => {
                    match op {
                        Op::Mul => ResultValue::Unit(l_val * r, l_unit),
                        Op::Div => {
                            if r == 0.0 {
                                ResultValue::Error("Division by zero".to_string())
                            } else {
                                ResultValue::Unit(l_val / r, l_unit)
                            }
                        }
                        _ => ResultValue::Error("Operation not supported".to_string()),
                    }
                }
                (ResultValue::Number(l), ResultValue::Unit(r_val, r_unit)) => {
                    match op {
                        Op::Mul => ResultValue::Unit(l * r_val, r_unit),
                        _ => ResultValue::Error("Operation not supported".to_string()),
                    }
                }

                // Currency arithmetic
                (ResultValue::Currency(l_val, l_curr), ResultValue::Currency(r_val, r_curr)) => {
                    match op {
                        Op::Add => {
                            match convert_currency(r_val, &r_curr, &l_curr, cache_path) {
                                Ok(conv_r) => ResultValue::Currency(l_val + conv_r, l_curr),
                                Err(e) => ResultValue::Error(e),
                            }
                        }
                        Op::Sub => {
                            match convert_currency(r_val, &r_curr, &l_curr, cache_path) {
                                Ok(conv_r) => ResultValue::Currency(l_val - conv_r, l_curr),
                                Err(e) => ResultValue::Error(e),
                            }
                        }
                        Op::Div => {
                            match convert_currency(r_val, &r_curr, &l_curr, cache_path) {
                                Ok(conv_r) => {
                                    if conv_r == 0.0 {
                                        ResultValue::Error("Division by zero".to_string())
                                    } else {
                                        ResultValue::Number(l_val / conv_r)
                                    }
                                }
                                Err(e) => ResultValue::Error(e),
                            }
                        }
                        _ => ResultValue::Error("Operation not supported for currencies".to_string()),
                    }
                }
                (ResultValue::Currency(l_val, l_curr), ResultValue::Number(r)) => {
                    match op {
                        Op::Mul => ResultValue::Currency(l_val * r, l_curr),
                        Op::Div => {
                            if r == 0.0 {
                                ResultValue::Error("Division by zero".to_string())
                            } else {
                                ResultValue::Currency(l_val / r, l_curr)
                            }
                        }
                        _ => ResultValue::Error("Operation not supported".to_string()),
                    }
                }
                (ResultValue::Number(l), ResultValue::Currency(r_val, r_curr)) => {
                    match op {
                        Op::Mul => ResultValue::Currency(l * r_val, r_curr),
                        _ => ResultValue::Error("Operation not supported".to_string()),
                    }
                }

                (l, r) => ResultValue::Error(format!(
                    "Mismatched types: cannot perform {:?} on {:?} and {:?}",
                    op, l, r
                )),
            }
        }
    }
}

fn sum_results(results: Vec<ResultValue>, cache_path: Option<&str>) -> ResultValue {
    if results.is_empty() {
        return ResultValue::Empty;
    }
    let mut iter = results.into_iter();
    let mut acc = iter.next().unwrap();

    for val in iter {
        acc = match (acc, val) {
            (ResultValue::Error(e), _) => ResultValue::Error(e),
            (_, ResultValue::Error(e)) => ResultValue::Error(e),
            (ResultValue::Empty, v) => v,
            (v, ResultValue::Empty) => v,

            (ResultValue::Number(l), ResultValue::Number(r)) => ResultValue::Number(l + r),
            (ResultValue::Unit(l_val, l_unit), ResultValue::Unit(r_val, r_unit)) => {
                match convert_unit(r_val, &r_unit, &l_unit) {
                    Ok(conv_r) => ResultValue::Unit(l_val + conv_r, l_unit),
                    Err(e) => ResultValue::Error(e),
                }
            }
            (ResultValue::Currency(l_val, l_curr), ResultValue::Currency(r_val, r_curr)) => {
                match convert_currency(r_val, &r_curr, &l_curr, cache_path) {
                    Ok(conv_r) => ResultValue::Currency(l_val + conv_r, l_curr),
                    Err(e) => ResultValue::Error(e),
                }
            }
            (l, r) => ResultValue::Error(format!("Cannot sum mismatched types: {:?} and {:?}", l, r)),
        };
    }
    acc
}

pub fn evaluate_document(lines: &[String], cache_path: Option<&str>) -> Vec<String> {
    let mut env: Vec<ResultValue> = Vec::new();
    let mut symbols: HashMap<String, ResultValue> = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            env.push(ResultValue::Empty);
            continue;
        }
        match crate::parser::parse_line(line) {
            Ok(exprs) => {
                let mut results = Vec::new();
                for expr in exprs {
                    results.push(eval_expr(&expr, &env, &mut symbols, cache_path));
                }
                let final_val = sum_results(results, cache_path);
                env.push(final_val);
            }
            Err(e) => {
                env.push(ResultValue::Error(e));
            }
        }
    }

    env.into_iter().map(|res| res.to_string()).collect()
}

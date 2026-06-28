use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct NumenParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Percentage(Box<Expr>),
    Unit(f64, String),
    Currency(f64, String),
    Variable(String), // "line1", "ans"
    Binary(Box<Expr>, Op, Box<Expr>),
    Convert(Box<Expr>, String), // target unit or currency code
    Assign(String, Box<Expr>),  // variable name, value expression
}

pub fn parse_line(input: &str) -> Result<Vec<Expr>, String> {
    let pairs = NumenParser::parse(Rule::line, input)
        .map_err(|e| e.to_string())?;

    let mut exprs = Vec::new();
    for pair in pairs {
        if pair.as_rule() == Rule::line {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::expr {
                    exprs.push(parse_expr(inner)?);
                }
            }
        }
    }
    Ok(exprs)
}

fn parse_expr(pair: Pair<Rule>) -> Result<Expr, String> {
    match pair.as_rule() {
        Rule::expr => {
            let inner = pair.into_inner().next().unwrap();
            parse_expr(inner)
        }
        Rule::assignment => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string().to_lowercase();
            let val = parse_expr(inner.next().unwrap())?;
            Ok(Expr::Assign(name, Box::new(val)))
        }
        Rule::conversion => {
            let mut inner = pair.into_inner();
            let lhs = parse_expr(inner.next().unwrap())?;
            let _convert_kw = inner.next().unwrap();
            let target = inner.next().unwrap().as_str().to_string();
            Ok(Expr::Convert(Box::new(lhs), target))
        }
        Rule::add_sub => {
            parse_binary_expr(pair)
        }
        Rule::mul_div => {
            parse_binary_expr(pair)
        }
        Rule::power => {
            let mut inner = pair.into_inner();
            let mut lhs = parse_expr(inner.next().unwrap())?;
            while let Some(rhs_pair) = inner.next() {
                let rhs = parse_expr(rhs_pair)?;
                lhs = Expr::Binary(Box::new(lhs), Op::Pow, Box::new(rhs));
            }
            Ok(lhs)
        }
        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::percentage => {
                    let mut inner_pairs = inner.into_inner();
                    let val_pair = inner_pairs.next().unwrap();
                    let val = match val_pair.as_rule() {
                        Rule::number => {
                            let num_str = val_pair.as_str().replace(",", "");
                            let num = num_str.parse::<f64>().map_err(|e| e.to_string())?;
                            Expr::Number(num)
                        }
                        Rule::variable => Expr::Variable(val_pair.as_str().to_string().to_lowercase()),
                        _ => return Err("Invalid percentage operand".to_string()),
                    };
                    Ok(Expr::Percentage(Box::new(val)))
                }
                Rule::currency_amount => {
                    let mut inner_pairs = inner.into_inner();
                    let first = inner_pairs.next().unwrap();
                    if first.as_rule() == Rule::currency_symbol {
                        let symbol = first.as_str().to_string();
                        let num_str = inner_pairs.next().unwrap().as_str().replace(",", "");
                        let num = num_str.parse::<f64>().map_err(|e| e.to_string())?;
                        let code = match symbol.as_str() {
                            "$" => "USD",
                            "€" => "EUR",
                            "£" => "GBP",
                            "¥" => "JPY",
                            _ => "USD",
                        }.to_string();
                        Ok(Expr::Currency(num, code))
                    } else {
                        let num_str = first.as_str().replace(",", "");
                        let num = num_str.parse::<f64>().map_err(|e| e.to_string())?;
                        let code = inner_pairs.next().unwrap().as_str().to_string();
                        Ok(Expr::Currency(num, code))
                    }
                }
                Rule::unit_amount => {
                    let mut inner_pairs = inner.into_inner();
                    let num_str = inner_pairs.next().unwrap().as_str().replace(",", "");
                    let num = num_str.parse::<f64>().map_err(|e| e.to_string())?;
                    let unit = inner_pairs.next().unwrap().as_str().to_string();
                    Ok(Expr::Unit(num, unit))
                }
                Rule::number => {
                    let num_str = inner.as_str().replace(",", "");
                    let num = num_str.parse::<f64>().map_err(|e| e.to_string())?;
                    Ok(Expr::Number(num))
                }
                Rule::variable => {
                    Ok(Expr::Variable(inner.as_str().to_string().to_lowercase()))
                }
                Rule::unary_minus => {
                    let inner_val = inner.into_inner().next().unwrap();
                    let operand = parse_expr(inner_val)?;
                    Ok(Expr::Binary(Box::new(Expr::Number(0.0)), Op::Sub, Box::new(operand)))
                }
                Rule::expr => {
                    parse_expr(inner)
                }
                _ => Err(format!("Unexpected primary rule: {:?}", inner.as_rule())),
            }
        }
        _ => Err(format!("Unexpected rule: {:?}", pair.as_rule())),
    }
}

fn parse_binary_expr(pair: Pair<Rule>) -> Result<Expr, String> {
    let mut inner = pair.into_inner();
    let mut lhs = parse_expr(inner.next().unwrap())?;

    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_rule() {
            Rule::add_sub_op | Rule::mul_div_op => {
                let op_str = op_pair.as_str().trim();
                match op_str {
                    "+" | "plus" => Op::Add,
                    "-" | "minus" => Op::Sub,
                    "*" | "times" => Op::Mul,
                    "/" | "divided by" | "over" => Op::Div,
                    "of" => Op::Mul,
                    _ => return Err(format!("Unknown operator: {}", op_str)),
                }
            }
            _ => return Err(format!("Expected operator, found: {:?}", op_pair.as_rule())),
        };
        let rhs = parse_expr(inner.next().unwrap())?;
        lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
    }

    Ok(lhs)
}

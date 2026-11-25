use chrono::{Duration, NaiveDate};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    /// `None` is FoxPro's empty date.
    Date(Option<NaiveDate>),
    Null,
    Array(Array),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    /// 0 for a one-dimensional array.
    pub cols: usize,
    pub data: Vec<Value>,
}

impl Array {
    pub fn new(dims: &[f64]) -> Result<Array, String> {
        let (rows, cols) = match dims {
            [r] => (*r, 0.0),
            [r, c] => (*r, *c),
            _ => return Err("Un arreglo admite una o dos dimensiones".into()),
        };
        let len = rows * cols.max(1.0);
        // shortcut: fixed 1,000,000 element cap (about 32 MB) so a typo cannot exhaust memory
        if rows < 1.0 || cols < 0.0 || len > 1e6 {
            return Err("Dimensiones de arreglo no válidas".into());
        }
        Ok(Array { cols: cols as usize, data: vec![Value::Bool(false); len as usize] })
    }

    pub fn rows(&self) -> usize {
        if self.cols == 0 { self.data.len() } else { self.data.len() / self.cols }
    }

    /// Position in `data` of a 1-based `[i]` or `[row, col]` subscript.
    pub fn offset(&self, idx: &[f64]) -> Result<usize, String> {
        let pos = match idx {
            [i] => *i - 1.0,
            [r, c] if self.cols > 0 && *c >= 1.0 && *c <= self.cols as f64 => {
                (*r - 1.0) * self.cols as f64 + *c - 1.0
            }
            _ => -1.0,
        };
        if pos >= 0.0 && pos < self.data.len() as f64 {
            Ok(pos as usize)
        } else {
            Err("Subíndice fuera de rango".into())
        }
    }
}

impl Value {
    pub fn display(&self) -> String {
        match self {
            Value::Num(n) => fmt_num(*n),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => if *b { ".T." } else { ".F." }.into(),
            // shortcut: always MM/DD/YYYY, upgrade when SET DATE / SET CENTURY are implemented
            Value::Date(Some(d)) => d.format("%m/%d/%Y").to_string(),
            Value::Date(None) => "  /  /    ".into(),
            Value::Null => ".NULL.".into(),
            Value::Array(a) => a.data[0].display(),
        }
    }

    /// The letter TYPE() and VARTYPE() report.
    pub fn type_char(&self) -> char {
        match self {
            Value::Num(_) => 'N',
            Value::Str(_) => 'C',
            Value::Bool(_) => 'L',
            Value::Date(_) => 'D',
            Value::Null => 'X',
            Value::Array(a) => a.data[0].type_char(),
        }
    }
}

// shortcut: up to 10 decimals, no FoxPro column padding; upgrade when SET DECIMALS is implemented
pub fn fmt_num(n: f64) -> String {
    let s = format!("{:.10}", n);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Rounds half away from zero, as FoxPro does.
pub fn round(x: f64, dec: i32) -> f64 {
    let p = 10f64.powi(dec);
    (x * p).round() / p + 0.0
}

pub fn modulo(x: f64, y: f64) -> Result<f64, String> {
    if y == 0.0 { Err("División entre cero".into()) } else { Ok(x - y * (x / y).floor()) }
}

/// Parses `m/d/y`, or `^y-m-d` in the strict literal form. Blank or invalid gives the empty date.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let strict = s.trim_start().starts_with('^');
    let parts: Option<Vec<i32>> =
        s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().ok()).collect();
    let [a, b, c] = parts?[..] else { return None };
    let (y, m, d) = if strict { (a, b, c) } else { (c, a, b) };
    // shortcut: fixed two-digit-year pivot at 50, upgrade when SET CENTURY ROLLOVER is implemented
    let y = if strict || y >= 100 { y } else if y < 50 { 2000 + y } else { 1900 + y };
    NaiveDate::from_ymd_opt(y, m as u32, d as u32)
}

pub fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Date(x), Value::Date(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

const MISMATCH: &str = "Los tipos de datos no coinciden con el operador";

fn add_days(d: Option<NaiveDate>, n: f64) -> Option<NaiveDate> {
    d?.checked_add_signed(Duration::try_days(n as i64)?)
}

pub fn unary(op: &str, v: Value) -> Result<Value, String> {
    Ok(match (op, v) {
        (_, Value::Null) => Value::Null,
        ("-", Value::Num(x)) => Value::Num(-x),
        ("+", Value::Num(x)) => Value::Num(x),
        ("NOT", Value::Bool(b)) => Value::Bool(!b),
        _ => return Err(MISMATCH.into()),
    })
}

pub fn binary(op: &str, a: Value, b: Value) -> Result<Value, String> {
    use Value::*;
    if a == Null || b == Null {
        return Ok(Null);
    }
    Ok(match (op, a, b) {
        ("+", Num(x), Num(y)) => Num(x + y),
        ("+", Str(x), Str(y)) => Str(x + &y),
        ("+", Date(d), Num(n)) | ("+", Num(n), Date(d)) => Date(add_days(d, n)),
        ("-", Num(x), Num(y)) => Num(x - y),
        // string minus: the left side's trailing spaces move to the end
        ("-", Str(x), Str(y)) => {
            let t = x.trim_end_matches(' ');
            Str(format!("{t}{y}{}", " ".repeat(x.len() - t.len())))
        }
        ("-", Date(d), Num(n)) => Date(add_days(d, -n)),
        ("-", Date(Some(x)), Date(Some(y))) => Num((x - y).num_days() as f64),
        ("*", Num(x), Num(y)) => Num(x * y),
        ("/", Num(_), Num(y)) if y == 0.0 => return Err("División entre cero".into()),
        ("/", Num(x), Num(y)) => Num(x / y),
        ("%", Num(x), Num(y)) => Num(modulo(x, y)?),
        ("^" | "**", Num(x), Num(y)) => Num(x.powf(y)),
        ("$", Str(x), Str(y)) => Bool(y.contains(&x)),
        // SET EXACT OFF semantics: `=` compares only up to the length of the right side
        ("=", Str(x), Str(y)) => Bool(x.starts_with(&y)),
        ("<>" | "!=" | "#", Str(x), Str(y)) => Bool(!x.starts_with(&y)),
        (op, a, b) => {
            let ord = compare(&a, &b).ok_or(MISMATCH)?;
            Bool(match op {
                "=" | "==" => ord == Ordering::Equal,
                "<>" | "!=" | "#" => ord != Ordering::Equal,
                "<" => ord == Ordering::Less,
                "<=" => ord != Ordering::Greater,
                ">" => ord == Ordering::Greater,
                ">=" => ord != Ordering::Less,
                _ => return Err(MISMATCH.into()),
            })
        }
    })
}

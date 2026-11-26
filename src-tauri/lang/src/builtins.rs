use crate::value::{Value, compare, modulo, parse_date, round};
use Value::*;
use chrono::{Datelike, Local, Months, NaiveDate, Timelike};
use std::cmp::Ordering;

/// Every built-in function. Like keywords, they can be abbreviated to four letters or more;
/// when an abbreviation fits several, the first one listed wins.
pub const NAMES: &[&str] = &[
    "LEN", "UPPER", "LOWER", "PROPER", "ALLTRIM", "LTRIM", "RTRIM", "TRIM", "LEFT", "RIGHT", "SUBSTR", "AT", "ATC",
    "RAT", "OCCURS", "SPACE", "REPLICATE", "STRTRAN", "STUFF", "PADL", "PADR", "PADC", "CHR", "ASC", "STR", "VAL",
    "TRANSFORM", "CHRTRAN", "ISALPHA", "ISDIGIT", "ABS", "INT", "ROUND", "MOD", "MAX", "MIN", "SQRT", "CEILING",
    "FLOOR", "SIGN", "EXP", "LOG", "LOG10", "PI", "DATE", "TIME", "SECONDS", "CTOD", "DTOC", "DTOS", "YEAR", "MONTH",
    "DAY", "DOW", "CDOW", "CMONTH", "GOMONTH", "EMPTY", "ISNULL", "NVL", "BETWEEN", "INLIST", "ALEN", "IIF", "TYPE",
    "VARTYPE", "EVALUATE", "RAND", "RGB", "CREATEOBJECT", "MESSAGEBOX", "DODEFAULT", "INPUTBOX", "RECNO", "RECCOUNT", "EOF", "BOF", "FOUND", "DELETED", "ALIAS",
    "SELECT", "USED", "FCOUNT", "FIELD", "SEEK", "MENU", "PAD", "POPUP", "BAR", "PROMPT",
    "TABLEUPDATE", "TABLEREVERT", "CURSORSETPROP", "MESSAGE", "LINENO", "ERROR",
];

/// Implemented by the interpreter, not by `call`.
#[cfg(test)]
const SPECIAL: &[&str] = &["IIF", "TYPE", "VARTYPE", "EVALUATE", "RAND", "CREATEOBJECT", "MESSAGEBOX", "DODEFAULT", "INPUTBOX", "RECNO", "RECCOUNT", "EOF",
    "BOF", "FOUND", "DELETED", "ALIAS", "SELECT", "USED", "FCOUNT", "FIELD", "SEEK", "MENU", "PAD", "POPUP", "BAR", "PROMPT",
    "TABLEUPDATE", "TABLEREVERT", "CURSORSETPROP", "MESSAGE", "LINENO", "ERROR",
];

pub fn resolve(name: &str) -> Option<&'static str> {
    let exact = NAMES.iter().copied().find(|n| *n == name);
    exact.or_else(|| NAMES.iter().copied().find(|n| name.len() >= 4 && n.starts_with(name)))
}

const FEW: &str = "Faltan argumentos";
const BAD: &str = "Tipo de argumento no válido";
const UNIMPLEMENTED: &str = "Función no implementada";

/// FoxPro strings top out at 16 MB; this also keeps a typo from exhausting memory.
fn rep(s: &str, n: usize) -> Result<String, String> {
    if s.len().saturating_mul(n) > 16 << 20 { Err("Cadena demasiado larga".into()) } else { Ok(s.repeat(n)) }
}

/// 1-based character position of the `occ`-th match, 0 when there is none.
fn at(needle: &str, hay: &str, occ: usize, from_end: bool) -> f64 {
    if needle.is_empty() || occ == 0 {
        return 0.0;
    }
    let hit = if from_end { hay.rmatch_indices(needle).nth(occ - 1) } else { hay.match_indices(needle).nth(occ - 1) };
    hit.map_or(0.0, |(i, _)| hay[..i].chars().count() as f64 + 1.0)
}

/// TRANSFORM(value, mask): the value drawn through a FoxPro picture.
/// Before a space, `@` functions: `!` upper case, `L` leading zeros, `Z` blank when zero, `$` currency,
/// `R` keep the literals of the mask without spending characters on them.
// shortcut: the pictures and functions old programs use most; the rest of the mask is copied as it is
fn picture(v: &Value, mask: &str) -> String {
    let (functions, pattern) = match mask.strip_prefix('@') {
        Some(rest) => rest.split_once(' ').unwrap_or((rest, "")),
        None => ("", mask),
    };
    let has = |f: char| functions.to_uppercase().contains(f);
    let width = pattern.chars().count();
    match v {
        Num(n) if pattern.is_empty() => if has('Z') && *n == 0.0 { String::new() } else { v.display() },
        Num(n) if has('Z') && *n == 0.0 => " ".repeat(width),
        Num(n) => {
            let decimals = pattern.split_once('.').map_or(0, |(_, d)| d.chars().filter(|c| "9#".contains(*c)).count());
            let digits = format!("{:.*}", decimals, n.abs());
            let (whole, fraction) = digits.split_once('.').unwrap_or((&digits, ""));
            let mut text = String::new();
            for (i, d) in whole.chars().enumerate() {
                if pattern.contains(',') && i > 0 && (whole.len() - i) % 3 == 0 {
                    text.push(',');
                }
                text.push(d);
            }
            if decimals > 0 {
                text = format!("{text}.{fraction}");
            }
            let sign = if *n < 0.0 { "-" } else { "" };
            let money = if pattern.contains('$') || has('$') { "$" } else { "" };
            let text = format!("{money}{sign}{text}");
            match width.checked_sub(text.chars().count()) {
                Some(room) => (if has('L') { "0" } else { " " }).repeat(room) + &text,
                // as in FoxPro, a number that does not fit shows as asterisks
                None => "*".repeat(width),
            }
        }
        Str(text) => {
            let text = if has('!') { text.to_uppercase() } else { text.clone() };
            if pattern.is_empty() {
                return text;
            }
            let (mut chars, mut out) = (text.chars(), String::new());
            for m in pattern.chars() {
                match m {
                    '!' => out.extend(chars.next().map(|c| c.to_uppercase().next().unwrap_or(c))),
                    'X' | 'A' | '9' | '#' | 'N' => out.extend(chars.next()),
                    literal => {
                        if !has('R') {
                            chars.next();
                        }
                        out.push(literal);
                    }
                }
            }
            out
        }
        other => other.display(),
    }
}

pub fn call(name: &str, a: &[Value]) -> Result<Value, String> {
    let v = |i: usize| a.get(i).ok_or(FEW.to_string());
    let n = |i: usize| match v(i)? {
        Num(x) => Ok(*x),
        _ => Err(BAD.to_string()),
    };
    let s = |i: usize| match v(i)? {
        Str(x) => Ok(x.as_str()),
        _ => Err(BAD.to_string()),
    };
    let d = |i: usize| match v(i)? {
        Date(x) => Ok(*x),
        _ => Err(BAD.to_string()),
    };
    // a count or position: negatives clamp to zero
    let cnt = |i: usize| n(i).map(|x| x.max(0.0) as usize);
    let cnt_or = |i: usize, default: usize| if a.len() > i { cnt(i) } else { Ok(default) };
    let chars = |i: usize| s(i).map(|x| x.chars().collect::<Vec<char>>());
    let text = |c: &[char]| c.iter().collect::<String>();

    Ok(match name {
        "LEN" => Num(s(0)?.chars().count() as f64),
        "UPPER" => Str(s(0)?.to_uppercase()),
        "LOWER" => Str(s(0)?.to_lowercase()),
        "PROPER" => {
            let mut start = true;
            let fix = |c: char| {
                let out = if start { c.to_uppercase().next() } else { c.to_lowercase().next() };
                start = !c.is_alphanumeric();
                out.unwrap_or(c)
            };
            Str(s(0)?.chars().map(fix).collect())
        }
        "ALLTRIM" => Str(s(0)?.trim_matches(' ').into()),
        "LTRIM" => Str(s(0)?.trim_start_matches(' ').into()),
        "RTRIM" | "TRIM" => Str(s(0)?.trim_end_matches(' ').into()),
        "LEFT" => Str(s(0)?.chars().take(cnt(1)?).collect()),
        "RIGHT" => {
            let c = chars(0)?;
            Str(text(&c[c.len() - cnt(1)?.min(c.len())..]))
        }
        "SUBSTR" => Str(s(0)?.chars().skip(cnt(1)?.max(1) - 1).take(cnt_or(2, usize::MAX)?).collect()),
        "AT" => Num(at(s(0)?, s(1)?, cnt_or(2, 1)?, false)),
        "ATC" => Num(at(&s(0)?.to_uppercase(), &s(1)?.to_uppercase(), cnt_or(2, 1)?, false)),
        "RAT" => Num(at(s(0)?, s(1)?, cnt_or(2, 1)?, true)),
        "OCCURS" => Num(if s(0)?.is_empty() { 0 } else { s(1)?.matches(s(0)?).count() } as f64),
        "SPACE" => Str(rep(" ", cnt(0)?)?),
        "REPLICATE" => Str(rep(s(0)?, cnt(1)?)?),
        "STRTRAN" => {
            let with = if a.len() > 2 { s(2)? } else { "" };
            Str(if s(1)?.is_empty() { s(0)?.into() } else { s(0)?.replace(s(1)?, with) })
        }
        "STUFF" => {
            let c = chars(0)?;
            let from = (cnt(1)?.max(1) - 1).min(c.len());
            let to = from.saturating_add(cnt(2)?).min(c.len());
            Str(format!("{}{}{}", text(&c[..from]), s(3)?, text(&c[to..])))
        }
        "PADL" | "PADR" | "PADC" => {
            let c: Vec<char> = v(0)?.display().chars().collect();
            let width = cnt(1)?;
            let fill: String = if a.len() > 2 { s(2)?.chars().take(1).collect() } else { " ".into() };
            let pad = width.saturating_sub(c.len());
            let left = match name {
                "PADL" => pad,
                "PADR" => 0,
                _ => pad / 2,
            };
            Str(format!("{}{}{}", rep(&fill, left)?, text(&c[..c.len().min(width)]), rep(&fill, pad - left)?))
        }
        "CHR" => Str(char::from_u32(n(0)? as u32).ok_or(BAD)?.into()),
        "ASC" => Num(s(0)?.chars().next().map_or(0, |c| c as u32) as f64),
        "STR" => {
            let (len, dec) = (cnt_or(1, 10)?.min(255), cnt_or(2, 0)?.min(18));
            let t = format!("{:.*}", dec, round(n(0)?, dec as i32));
            Str(if t.len() > len { "*".repeat(len) } else { format!("{t:>len$}") })
        }
        "VAL" => {
            let t = s(0)?.trim_start();
            let (mut end, mut dot) = (0, false);
            for (i, c) in t.char_indices() {
                let sign = i == 0 && (c == '-' || c == '+');
                if !(c.is_ascii_digit() || sign || (c == '.' && !dot)) {
                    break;
                }
                dot |= c == '.';
                end = i + 1;
            }
            Num(t[..end].parse().unwrap_or(0.0))
        }
        "TRANSFORM" if a.len() > 1 => Str(picture(v(0)?, s(1)?)),
        "TRANSFORM" => Str(v(0)?.display()),
        "CHRTRAN" => {
            let (from, to) = (chars(1)?, chars(2)?);
            // characters of `from` with no counterpart in `to` are removed
            let swap = |c: char| match from.iter().position(|f| *f == c) {
                Some(i) => to.get(i).copied(),
                None => Some(c),
            };
            Str(s(0)?.chars().filter_map(swap).collect())
        }
        "ISALPHA" => Bool(s(0)?.chars().next().is_some_and(char::is_alphabetic)),
        "ISDIGIT" => Bool(s(0)?.chars().next().is_some_and(|c| c.is_ascii_digit())),

        "ABS" => Num(n(0)?.abs()),
        "INT" => Num(n(0)?.trunc()),
        "ROUND" => Num(round(n(0)?, n(1)? as i32)),
        "MOD" => Num(modulo(n(0)?, n(1)?)?),
        "MAX" | "MIN" => {
            let mut best = v(0)?;
            for x in &a[1..] {
                let ord = compare(x, best).ok_or(BAD)?;
                if ord == if name == "MAX" { Ordering::Greater } else { Ordering::Less } {
                    best = x;
                }
            }
            best.clone()
        }
        "SQRT" if n(0)? < 0.0 => return Err("SQRT no admite números negativos".into()),
        "SQRT" => Num(n(0)?.sqrt()),
        "CEILING" => Num(n(0)?.ceil()),
        "FLOOR" => Num(n(0)?.floor()),
        "SIGN" => Num(if n(0)? == 0.0 { 0.0 } else { n(0)?.signum() }),
        "EXP" => Num(n(0)?.exp()),
        "LOG" | "LOG10" if n(0)? <= 0.0 => return Err("El logaritmo necesita un número positivo".into()),
        "LOG" => Num(n(0)?.ln()),
        "LOG10" => Num(n(0)?.log10()),
        "PI" => Num(std::f64::consts::PI),
        // a colour as FoxPro stores it: red in the low byte
        "RGB" => Num(n(0)?.clamp(0.0, 255.0).trunc() + n(1)?.clamp(0.0, 255.0).trunc() * 256.0 + n(2)?.clamp(0.0, 255.0).trunc() * 65536.0),

        "DATE" if a.is_empty() => Date(Some(Local::now().date_naive())),
        "DATE" => Date(NaiveDate::from_ymd_opt(n(0)? as i32, n(1)? as u32, n(2)? as u32)),
        "TIME" => Str(Local::now().format("%H:%M:%S").to_string()),
        "SECONDS" => {
            let t = Local::now().time();
            Num(t.num_seconds_from_midnight() as f64 + (t.nanosecond() / 1_000_000) as f64 / 1000.0)
        }
        "CTOD" => Date(parse_date(s(0)?)),
        "DTOC" if a.len() < 2 => {
            d(0)?;
            Str(a[0].display())
        }
        "DTOC" | "DTOS" => Str(d(0)?.map_or(" ".repeat(8), |x| x.format("%Y%m%d").to_string())),
        "YEAR" => Num(d(0)?.map_or(0, |x| x.year()) as f64),
        "MONTH" => Num(d(0)?.map_or(0, |x| x.month()) as f64),
        "DAY" => Num(d(0)?.map_or(0, |x| x.day()) as f64),
        "DOW" => Num(d(0)?.map_or(0, |x| x.weekday().num_days_from_sunday() + 1) as f64),
        "CDOW" => Str(d(0)?.map_or(String::new(), |x| x.format("%A").to_string())),
        "CMONTH" => Str(d(0)?.map_or(String::new(), |x| x.format("%B").to_string())),
        "GOMONTH" => {
            let months = n(1)? as i32;
            let shift = Months::new(months.unsigned_abs());
            Date(d(0)?.and_then(|x| if months < 0 { x.checked_sub_months(shift) } else { x.checked_add_months(shift) }))
        }

        "EMPTY" => Bool(match v(0)? {
            Num(x) => *x == 0.0,
            Str(x) => x.trim().is_empty(),
            Bool(b) => !b,
            Date(x) => x.is_none(),
            Null | Array(_) | Object(_) => false,
        }),
        "ISNULL" => Bool(*v(0)? == Null),
        "NVL" => if *v(0)? == Null { v(1)? } else { v(0)? }.clone(),
        "BETWEEN" => {
            let (low, high) = (compare(v(0)?, v(1)?).ok_or(BAD)?, compare(v(0)?, v(2)?).ok_or(BAD)?);
            Bool(low != Ordering::Less && high != Ordering::Greater)
        }
        "INLIST" => Bool(a.iter().skip(1).any(|x| compare(&a[0], x) == Some(Ordering::Equal)) && v(0).is_ok()),
        "ALEN" => {
            let Array(arr) = v(0)? else { return Err("ALEN necesita un arreglo".into()) };
            Num(match cnt_or(1, 0)? {
                0 => arr.data.len(),
                1 => arr.rows(),
                2 => arr.cols,
                _ => return Err(BAD.into()),
            } as f64)
        }
        _ => return Err(UNIMPLEMENTED.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // NAMES drives name resolution and `call` does the work: they must not drift apart
    // the help screen must list exactly what exists
    #[test]
    fn every_name_is_documented() {
        let mut documented: Vec<&str> = crate::help::FUNCTIONS.iter().map(|(_, syntax, _)| crate::help::function_name(syntax)).collect();
        let mut names = NAMES.to_vec();
        documented.sort();
        names.sort();
        assert_eq!(documented, names);
    }

    #[test]
    fn every_name_is_implemented() {
        for name in NAMES.iter().filter(|n| !SPECIAL.contains(n)) {
            assert_ne!(call(name, &[]), Err(UNIMPLEMENTED.into()), "{name}");
        }
    }
}

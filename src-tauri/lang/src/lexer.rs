use crate::value::parse_date;
use crate::{Error, R};
use chrono::NaiveDate;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Date(Option<NaiveDate>),
    /// Upper-cased: the language is case-insensitive.
    Ident(String),
    Op(&'static str),
}

/// One statement: a physical line plus any `;` continuations.
pub struct Line {
    pub no: usize,
    pub toks: Vec<Tok>,
}

// longest first, so "==" wins over "="
const OPS: &[&str] = &[
    "==", "<>", "!=", "<=", ">=", "**", "??", "+", "-", "*", "/", "^", "%", "=", "<", ">", "#", "$", "!", "(", ")",
    "[", "]", ",", "@", "?", ".",
];

pub fn lex(src: &str) -> R<Vec<Line>> {
    let c: Vec<char> = src.chars().collect();
    let at = |i: usize| c.get(i).copied().unwrap_or('\0');
    let eol = |mut i: usize| {
        while i < c.len() && c[i] != '\n' {
            i += 1;
        }
        i
    };
    let mut lines = Vec::new();
    let mut toks: Vec<Tok> = Vec::new();
    let (mut i, mut no, mut start) = (0, 1, 1);

    while i < c.len() {
        let ch = c[i];
        if toks.is_empty() {
            start = no;
        }
        let fail = |msg: &str| Err(Error::at(no, msg));
        match ch {
            '\n' => {
                if !toks.is_empty() {
                    lines.push(Line { no: start, toks: std::mem::take(&mut toks) });
                }
                no += 1;
                i += 1;
            }
            ' ' | '\t' | '\r' => i += 1,
            // a `*` comment line; a trailing `;` continues the comment
            '*' if toks.is_empty() => loop {
                let end = eol(i);
                let more = c[i..end].iter().rev().find(|x| !x.is_whitespace()) == Some(&';');
                i = end;
                if !more || i >= c.len() {
                    break;
                }
                i += 1;
                no += 1;
            },
            '&' if at(i + 1) == '&' => i = eol(i),
            ';' => {
                let mut j = i + 1;
                while matches!(at(j), ' ' | '\t' | '\r') {
                    j += 1;
                }
                if at(j) == '&' && at(j + 1) == '&' {
                    j = eol(j);
                }
                if j < c.len() && c[j] != '\n' {
                    return fail("';' solo puede ir al final de la línea");
                }
                no += 1;
                i = j + 1;
            }
            '\'' | '"' => {
                let end = (i + 1..eol(i)).find(|&j| c[j] == ch);
                let Some(end) = end else { return fail("Cadena sin cerrar") };
                toks.push(Tok::Str(c[i + 1..end].iter().collect()));
                i = end + 1;
            }
            // `[` opens a string unless it follows something that can be subscripted
            '[' if !matches!(toks.last(), Some(Tok::Ident(_) | Tok::Op(")" | "]"))) => {
                let end = (i + 1..eol(i)).find(|&j| c[j] == ']');
                let Some(end) = end else { return fail("Cadena sin cerrar") };
                toks.push(Tok::Str(c[i + 1..end].iter().collect()));
                i = end + 1;
            }
            '{' => {
                let end = (i + 1..eol(i)).find(|&j| c[j] == '}');
                let Some(end) = end else { return fail("Fecha sin cerrar") };
                let text: String = c[i + 1..end].iter().collect();
                let date = parse_date(&text);
                if date.is_none() && text.contains(|x: char| x.is_ascii_digit()) {
                    return fail("Fecha no válida");
                }
                toks.push(Tok::Date(date));
                i = end + 1;
            }
            _ if ch.is_ascii_digit() || (ch == '.' && at(i + 1).is_ascii_digit()) => {
                let from = i;
                while at(i).is_ascii_digit() {
                    i += 1;
                }
                if at(i) == '.' && at(i + 1).is_ascii_digit() {
                    i += 1;
                    while at(i).is_ascii_digit() {
                        i += 1;
                    }
                }
                let sign = matches!(at(i + 1), '+' | '-') as usize;
                if matches!(at(i), 'e' | 'E') && at(i + 1 + sign).is_ascii_digit() {
                    i += 1 + sign;
                    while at(i).is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = c[from..i].iter().collect();
                match text.parse() {
                    Ok(n) => toks.push(Tok::Num(n)),
                    Err(_) => return fail("Número no válido"),
                }
            }
            // .T. .F. .NULL. .AND. .OR. .NOT.
            '.' if at(i + 1).is_alphabetic() => {
                let mut j = i + 1;
                while at(j).is_alphabetic() {
                    j += 1;
                }
                let word: String = c[i + 1..j].iter().collect::<String>().to_uppercase();
                let tok = match word.as_str() {
                    _ if at(j) != '.' => None,
                    "T" | "Y" => Some(Tok::Bool(true)),
                    "F" | "N" => Some(Tok::Bool(false)),
                    "NULL" => Some(Tok::Null),
                    "AND" | "OR" | "NOT" => Some(Tok::Ident(word)),
                    _ => None,
                };
                match tok {
                    Some(t) => {
                        toks.push(t);
                        i = j + 1;
                    }
                    None => {
                        toks.push(Tok::Op("."));
                        i += 1;
                    }
                }
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                // `m.` only says "memory variable"; drop it
                if matches!(ch, 'm' | 'M') && at(i + 1) == '.' && (at(i + 2).is_alphabetic() || at(i + 2) == '_') {
                    i += 2;
                }
                let from = i;
                while at(i).is_alphanumeric() || at(i) == '_' {
                    i += 1;
                }
                toks.push(Tok::Ident(c[from..i].iter().collect::<String>().to_uppercase()));
            }
            _ => {
                let op = OPS.iter().find(|op| op.chars().enumerate().all(|(k, x)| at(i + k) == x));
                let Some(op) = op else { return fail(&format!("Carácter no válido: '{ch}'")) };
                toks.push(Tok::Op(op));
                i += op.len();
            }
        }
    }
    if !toks.is_empty() {
        lines.push(Line { no: start, toks });
    }

    // `#DEFINE NAME tokens` replaces NAME, token by token, on the lines that follow
    let mut defines: HashMap<String, Vec<Tok>> = HashMap::new();
    let mut out = Vec::new();
    for mut line in lines {
        let expand = |toks: &[Tok]| -> Vec<Tok> {
            let swap = |t: &Tok| match t {
                Tok::Ident(w) if defines.contains_key(w) => defines[w].clone(),
                _ => vec![t.clone()],
            };
            toks.iter().flat_map(swap).collect()
        };
        if line.toks[0] == Tok::Op("#") {
            match &line.toks[1..] {
                [Tok::Ident(d), Tok::Ident(name), body @ ..] if d == "DEFINE" => {
                    let body = expand(body);
                    defines.insert(name.clone(), body);
                }
                _ => return Err(Error::at(line.no, "Directiva de preprocesador aún no soportada")),
            }
            continue;
        }
        if !defines.is_empty() {
            line.toks = expand(&line.toks);
        }
        out.push(line);
    }
    Ok(out)
}

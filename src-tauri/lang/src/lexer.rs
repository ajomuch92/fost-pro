use crate::value::parse_date;
use crate::{Error, R, is_kw};
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
    /// A name written with the `m.` prefix: a variable, never a table field.
    MemVar(String),
    Op(&'static str),
    /// A whole statement that has `&name` in it, as written: it is read again when it runs,
    /// once the variable's text is known.
    Macro(String),
}

impl Tok {
    /// The token as source text that lexes back to it.
    fn text(&self) -> String {
        match self {
            Tok::Num(n) => n.to_string(),
            Tok::Str(s) => match ['"', '\''].into_iter().find(|q| !s.contains(*q)) {
                Some(q) => format!("{q}{s}{q}"),
                None => format!("[{s}]"),
            },
            Tok::Bool(b) => if *b { ".T." } else { ".F." }.into(),
            Tok::Null => ".NULL.".into(),
            Tok::Date(d) => d.map_or("{}".into(), |d| d.format("{^%Y-%m-%d}").to_string()),
            Tok::Ident(w) => w.clone(),
            Tok::MemVar(w) => format!("m.{w}"),
            Tok::Op(op) => op.trim_start_matches("WITH").into(),
            Tok::Macro(text) => text.clone(),
        }
    }
}

/// Tokens as source text, for an expression that has to be kept on disk.
pub fn source(toks: &[Tok]) -> String {
    let mut out = String::new();
    for (i, t) in toks.iter().enumerate() {
        // a member dot stays glued to its neighbours: with spaces it would read as something else
        let dot = |t: &Tok| *t == Tok::Op(".");
        if i > 0 && !dot(t) && !dot(&toks[i - 1]) {
            out.push(' ');
        }
        out.push_str(&t.text());
    }
    out
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
    // where the statement being read begins in `c`
    let mut from = 0;
    // a statement with a macro in it is kept as its text
    let whole = |toks: Vec<Tok>, from: usize, to: usize| {
        if toks.iter().any(|t| matches!(t, Tok::Macro(_))) { vec![Tok::Macro(c[from..to].iter().collect())] } else { toks }
    };

    while i < c.len() {
        let ch = c[i];
        if toks.is_empty() {
            start = no;
            from = i;
        }
        let fail = |msg: &str| Err(Error::at(no, msg));
        match ch {
            '\n' => {
                let mut done = whole(std::mem::take(&mut toks), from, i);
                // TEXT ... ENDTEXT: the lines in between are text, not code
                if matches!(done.as_slice(), [Tok::Ident(w), rest @ ..] if w == "TEXT" && !matches!(rest.first(), Some(Tok::Op(_)))) {
                    let (body, mut j) = (i + 1, i + 1);
                    loop {
                        if j >= c.len() {
                            return Err(Error::at(start, "TEXT sin cerrar"));
                        }
                        let end = eol(j);
                        no += 1;
                        let line: String = c[j..end].iter().collect();
                        if line.split_whitespace().next().is_some_and(|w| is_kw(&w.to_uppercase(), "ENDTEXT")) {
                            done.push(Tok::Str(c[body..j.max(body + 1) - 1].iter().collect()));
                            i = end;
                            break;
                        }
                        j = end + 1;
                    }
                    lines.push(Line { no: start, toks: done });
                    continue;
                }
                if !done.is_empty() {
                    lines.push(Line { no: start, toks: done });
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
            // .T. .F. .NULL. .AND. .OR. .NOT., or the dot of `object.member`
            '.' if at(i + 1).is_alphabetic() || at(i + 1) == '_' => {
                // glued to what comes before, it reads a member: `x.t.y` is not `x`, true, `y`
                let glued = i > 0 && (c[i - 1].is_alphanumeric() || matches!(c[i - 1], '_' | ')' | ']'));
                let mut j = i + 1;
                while at(j).is_alphabetic() {
                    j += 1;
                }
                let word: String = c[i + 1..j].iter().collect::<String>().to_uppercase();
                let tok = match word.as_str() {
                    _ if at(j) != '.' => None,
                    "AND" | "OR" | "NOT" => Some(Tok::Ident(word)),
                    _ if glued => None,
                    "T" | "Y" => Some(Tok::Bool(true)),
                    "F" | "N" => Some(Tok::Bool(false)),
                    "NULL" => Some(Tok::Null),
                    _ => None,
                };
                match tok {
                    Some(t) => {
                        toks.push(t);
                        i = j + 1;
                    }
                    None => {
                        // "WITH." is the leading dot of `.Caption` inside a WITH block
                        toks.push(Tok::Op(if glued { "." } else { "WITH." }));
                        i += 1;
                    }
                }
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                // `m.name` is the variable `name`, even when the open table has a field called that
                let memvar = matches!(ch, 'm' | 'M') && at(i + 1) == '.' && (at(i + 2).is_alphabetic() || at(i + 2) == '_');
                if memvar {
                    i += 2;
                }
                let from = i;
                while at(i).is_alphanumeric() || at(i) == '_' {
                    i += 1;
                }
                let name = c[from..i].iter().collect::<String>().to_uppercase();
                toks.push(if memvar { Tok::MemVar(name) } else { Tok::Ident(name) });
            }
            '&' if at(i + 1).is_alphabetic() || at(i + 1) == '_' => {
                toks.push(Tok::Macro(String::new()));
                i += 1;
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
        lines.push(Line { no: start, toks: whole(toks, from, c.len()) });
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

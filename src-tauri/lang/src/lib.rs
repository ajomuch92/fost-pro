//! Fost language core: a FoxPro-compatible lexer, parser and tree-walking interpreter.

mod builtins;
mod interp;
mod lexer;
mod parser;
mod value;

pub use interp::Interp;

#[derive(Debug, PartialEq)]
pub struct Error {
    /// 1-based source line; 0 until the statement being executed is known.
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Línea {}: {}", self.line, self.msg)
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error { line: 0, msg }
    }
}

fn err(msg: impl Into<String>) -> Error {
    Error { line: 0, msg: msg.into() }
}

type R<T> = Result<T, Error>;

/// FoxPro keywords may be abbreviated to their first four letters or more.
fn is_kw(word: &str, full: &str) -> bool {
    word == full || (word.len() >= 4 && full.starts_with(word))
}

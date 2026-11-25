//! Fost language core: a FoxPro-compatible lexer, parser and tree-walking interpreter.

mod builtins;
mod interp;
mod help;
mod lexer;
mod object;
mod objects;
mod parser;
mod value;

pub use interp::{Host, Interp, Open};
pub use help::reference;
pub use object::class_info;

/// Stack size for the thread that owns an `Interp`. The interpreter recurses on the native
/// stack, and 128 nested calls (the limit) overflow a default 2 MB thread in debug builds.
pub const STACK_SIZE: usize = 64 << 20;

#[derive(Debug, PartialEq)]
pub struct Error {
    /// 1-based source line; 0 until the statement being executed is known.
    pub line: usize,
    pub msg: String,
    /// Where the line is, when it is not in the source handed to `Interp::run`:
    /// a program loaded by DO, or a routine defined by an earlier run.
    pub file: Option<String>,
}

impl Error {
    fn at(line: usize, msg: impl Into<String>) -> Error {
        Error { line, msg: msg.into(), file: None }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{file}, línea {}: {}", self.line, self.msg),
            None => write!(f, "Línea {}: {}", self.line, self.msg),
        }
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::at(0, msg)
    }
}

fn err(msg: impl Into<String>) -> Error {
    Error::at(0, msg)
}

type R<T> = Result<T, Error>;

/// FoxPro keywords may be abbreviated to their first four letters or more.
fn is_kw(word: &str, full: &str) -> bool {
    word == full || (word.len() >= 4 && full.starts_with(word))
}

//! `fost program.fost` runs a file; `fost` alone reads commands from stdin, one per line.

use fost_lang::Interp;
use std::io::{BufRead, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut interp = Interp::default();
    let mut run = |src: &str| {
        let res = interp.run(src);
        // `?` prints its newline first, so close the last line here
        let out = interp.take_output();
        if !out.is_empty() {
            println!("{}", out.strip_prefix('\n').unwrap_or(&out));
        }
        if let Err(e) = &res {
            eprintln!("{e}");
        }
        res.is_ok()
    };
    match std::env::args().nth(1) {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(src) if run(&src) => ExitCode::SUCCESS,
            Ok(_) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("{path}: {e}");
                ExitCode::FAILURE
            }
        },
        None => {
            for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                run(&line);
                let _ = std::io::stdout().flush();
            }
            ExitCode::SUCCESS
        }
    }
}

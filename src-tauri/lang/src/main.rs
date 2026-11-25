//! `fost program.fost` runs a file; `fost` alone reads commands from stdin, one per line.

use fost_lang::Interp;
use std::io::{BufRead, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let thread = std::thread::Builder::new().stack_size(fost_lang::STACK_SIZE).spawn(run);
    thread.expect("failed to start the interpreter thread").join().unwrap_or(ExitCode::FAILURE)
}

fn run() -> ExitCode {
    let mut interp = Interp::default();
    let path = std::env::args().nth(1);
    // DO looks for other programs next to the one being run
    if let Some(dir) = path.as_deref().and_then(|p| std::path::Path::new(p).parent()) {
        if !dir.as_os_str().is_empty() {
            interp.dir = dir.into();
        }
    }
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
    match path {
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

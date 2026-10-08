//! `quire-cli`: a sheet in, its answers column out. The engine does
//! the work; this binary only routes a file (or stdin) and picks the
//! output shape.

use std::io::Read;

const USAGE: &str = "usage: quire-cli [--json] [--stats] [file]\n \
reads a .quire sheet (or any text file; `-` or no file means stdin)\n \
and prints it with its answers column - plain text, or JSON with\n \
--json. --stats reports the fixed-point pass count on stderr";

fn main() {
    let mut json = false;
    let mut stats = false;
    let mut file: Option<String> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => json = true,
            "--stats" => stats = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                eprintln!("quire-cli: unknown argument {arg}\n{USAGE}");
                std::process::exit(2);
            }
            _ if file.is_none() => file = Some(arg),
            _ => {
                eprintln!("quire-cli: unexpected argument {arg}");
                std::process::exit(2);
            }
        }
    }
    let text = match file.as_deref() {
        None | Some("-") => {
            let mut buf = String::new();
            if std::io::stdin().read_to_string(&mut buf).is_err() {
                eprintln!("quire-cli: stdin is not valid UTF-8");
                std::process::exit(2);
            }
            buf
        }
        Some(path) => std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("quire-cli: {path}: {e}");
            std::process::exit(2);
        }),
    };
    let out = if json {
        quire_cli::render_json(&text)
    } else {
        quire_cli::render_column(&text)
    };
    print!("{out}");
    if stats {
        let (lines, s) = quire_eval::evaluate_sheet_stats(&text);
        eprintln!(
            "quire-cli: {} lines, {} passes{}",
            s.lines,
            s.passes,
            if s.capped { " (CAP HIT)" } else { "" }
        );
        let _ = lines;
    }
}

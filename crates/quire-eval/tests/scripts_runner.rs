//! File-driven engine tests: every `tests/scripts/*.quire` is a sheet
//! carrying its own expectations, kalker's integration pattern.
//!
//! - `//= N` trailing a line pins that line's answer (golden sheets).
//! - `# expect: N` after an expression line asserts its result.
//! - `# err: text` after an expression line asserts it failed with
//!   that substring in the message.
//!
//! Expectation lines are stripped before evaluation; everything else
//! (headings included) reaches the engine untouched. One runner walks
//! the directory, so dropping a new `.quire` file in is enough.

use std::fs;
use std::path::Path;

use quire_eval::{LineKind, Outcome, evaluate_sheet, parse_sheet};

#[test]
fn script_files_hold_their_expectations() {
    // currency corpus lines convert against the engine's test rates
    // (every currency at 1.0)
    quire_eval::use_test_rates();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .expect("scripts directory exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "quire"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no .quire scripts found");
    for path in paths {
        run_script(&path);
    }
}

enum Want {
    Value(String),
    Err(String),
}

fn run_script(path: &Path) {
    let raw = fs::read_to_string(path).expect("script readable");
    let name = path
        .file_name()
        .expect("has a name")
        .to_string_lossy()
        .to_string();

    let mut cleaned: Vec<String> = Vec::new();
    // one expectation slot per cleaned line index; None most of the time
    let mut wants: Vec<Option<Want>> = Vec::new();
    let mut last_expr: Option<usize> = None;

    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("# expect:") {
            let i = attach(&mut last_expr, &name, raw_line_no(&raw, line));
            wants[i] = Some(Want::Value(rest.trim().to_string()));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# err:") {
            let i = attach(&mut last_expr, &name, raw_line_no(&raw, line));
            wants[i] = Some(Want::Err(rest.trim().to_string()));
            continue;
        }
        // comment lines are prose, even when they mention `//=`
        if trimmed.starts_with("//") {
            cleaned.push(line.to_string());
            wants.push(None);
            continue;
        }
        let (body, inline) = match line.find("//=") {
            Some(i) => (&line[..i], Some(line[i + 3..].trim().to_string())),
            None => (line, None),
        };
        let idx = cleaned.len();
        cleaned.push(body.to_string());
        let kind = parse_sheet(body).first().map(|l| l.kind);
        let is_expr = matches!(kind, Some(LineKind::Expression) | Some(LineKind::Reference));
        if is_expr {
            last_expr = Some(idx);
        }
        if let Some(v) = inline {
            assert!(
                is_expr,
                "{name}: `//=` on a non-expression line ({}): {body:?}",
                raw_line_no(&raw, line)
            );
            wants.push(Some(Want::Value(v)));
        } else {
            wants.push(None);
        }
    }

    let sheet = evaluate_sheet(&cleaned.join("\n"));
    assert_eq!(
        sheet.len(),
        cleaned.len(),
        "{name}: line count shifted during evaluation"
    );
    for (idx, want) in wants.iter().enumerate() {
        let Some(want) = want else { continue };
        let outcome = sheet[idx]
            .outcome
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: line {} has no result", idx + 1));
        match (want, outcome) {
            (Want::Value(want), Outcome::Value(v)) => {
                let got = quire_eval::format_number(*v);
                assert_eq!(
                    got,
                    *want,
                    "{name}: line {} wanted {want:?}, got {got:?}",
                    idx + 1
                );
            }
            // unit-engine results compare against the crate's own
            // rendering (`# expect: 5.3 kg`, bare rates as
            // `1200 /month`), which is also its parseable source
            (Want::Value(want), Outcome::Quantity(v)) => {
                let got = quire_eval::render(v);
                assert_eq!(
                    got,
                    *want,
                    "{name}: line {} wanted {want:?}, got {got:?}",
                    idx + 1
                );
            }
            (Want::Err(sub), Outcome::Failed(e)) => {
                let msg = e.kind.to_string();
                assert!(
                    msg.contains(sub.as_str()),
                    "{name}: line {} wanted error containing {sub:?}, got {msg:?}",
                    idx + 1
                );
            }
            (Want::Value(want), Outcome::Failed(e)) => panic!(
                "{name}: line {} wanted {want:?}, failed with {}",
                idx + 1,
                e.kind
            ),
            (Want::Err(sub), Outcome::Value(v)) => panic!(
                "{name}: line {} wanted error containing {sub:?}, got {}",
                idx + 1,
                quire_eval::format_number(*v)
            ),
            (Want::Err(sub), Outcome::Quantity(v)) => panic!(
                "{name}: line {} wanted error containing {sub:?}, got {}",
                idx + 1,
                v
            ),
        }
    }
}

/// Index of the most recent expression line; `# expect:` / `# err:`
/// always attach to one, and one must exist above.
fn attach(last_expr: &mut Option<usize>, name: &str, line_no: usize) -> usize {
    last_expr.unwrap_or_else(|| {
        panic!("{name}: expectation on line {line_no} has no expression line above it")
    })
}

fn raw_line_no(raw: &str, line: &str) -> usize {
    raw.lines().position(|l| l == line).unwrap_or(0) + 1
}

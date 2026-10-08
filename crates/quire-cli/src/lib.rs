//! The `quire-cli` binary's rendering: a sheet plus its answers
//! column as plain text or JSON, for terminals and agents. Pure
//! functions over the engine; the binary is the thin part.

/// The whole sheet with its answers column as text: raw lines padded
/// to the widest line, the rendered answer two spaces right of it.
/// Lines without answers (prose, headings, blanks) print alone, so
/// the output round-trips the sheet's shape.
pub fn render_column(sheet: &str) -> String {
    let outcomes = quire_eval::evaluate_sheet(sheet);
    let raws: Vec<&str> = sheet.lines().collect();
    let width = raws.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let mut out = String::new();
    for (line, raw) in outcomes.iter().zip(&raws) {
        match line.outcome.as_ref().map(|o| o.render()) {
            Some(answer) => {
                out.push_str(&format!("{:<width$}  {}\n", raw, answer, width = width));
            }
            None => {
                out.push_str(raw);
                out.push('\n');
            }
        }
    }
    out
}

/// The same evaluation as JSON: one object per line with `n`, `raw`,
/// and `answer` (omitted when the line has no result). Hand-rolled
/// escaping keeps the crate stdlib-only.
pub fn render_json(sheet: &str) -> String {
    let outcomes = quire_eval::evaluate_sheet(sheet);
    let raws: Vec<&str> = sheet.lines().collect();
    let mut out = String::from("{\"lines\":[");
    for (i, line) in outcomes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let raw = raws.get(i).copied().unwrap_or("");
        out.push_str(&format!(
            "{{\"n\":{},\"raw\":\"{}\"",
            line.number,
            escaped(raw)
        ));
        if let Some(o) = &line.outcome {
            out.push_str(&format!(",\"answer\":\"{}\"", escaped(&o.render())));
        }
        out.push('}');
    }
    out.push_str("]}\n");
    out
}

fn escaped(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "milk = 3.50\nmilk * 2\n\njust words\n";

    #[test]
    fn column_preserves_the_sheet_shape() {
        let out = render_column(SHEET);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4);
        // prose lines print alone; math lines get the answer column
        assert!(lines[0].starts_with("milk = 3.50"));
        assert!(lines[0].ends_with("3.5"));
        assert!(lines[2].is_empty());
        assert!(lines[3].starts_with("just words"));
        assert!(!lines[3].contains("=>"));
    }

    #[test]
    fn json_rounds_the_same_cells() {
        let out = render_json(SHEET);
        assert!(
            out.contains("\"raw\":\"milk = 3.50\",\"answer\":\"3.5\""),
            "{out}"
        );
        assert!(out.contains("\"raw\":\"just words\"}"), "{out}");
        assert!(!out.contains("\\u"), "{out}");
    }

    #[test]
    fn json_escapes_quotes_and_control_characters() {
        let out = render_json("note: said \"hello\"\n");
        assert!(out.contains("said \\\"hello\\\""), "{out}");
    }
}

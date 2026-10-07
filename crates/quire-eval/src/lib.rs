//! quire-eval: the calculation engine behind Quire.
//!
//! Phase 0 delivered the sheet line model (what each line of a sheet
//! *is*). Phase 1 adds the pipeline over it: [`tokenize`] to tokens,
//! [`parser::parse`] to a statement AST, and [`evaluate_sheet`] to
//! one outcome per line. The contract for all of it is the Semantics
//! section of spec.md.

mod error;
mod eval;
mod format;
mod parser;
mod tokens;
mod units;

pub use error::{ErrKind, QuireError, Span};
pub use eval::{LineOutcome, Outcome, evaluate_line, evaluate_sheet};
pub use format::{format_number, format_number_with};
pub use parser::{Expr, Stmt};
pub use tokens::{Tok, tokenize};
pub use units::{set_exchange_rates, use_test_rates};

use std::fmt;

/// What a line of a sheet is, decided by its leading shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Empty or whitespace only.
    Blank,
    /// First non-space characters are `//`.
    Comment,
    /// ATX-style heading: 1-6 `#` then a space, tab, or end of line.
    Heading,
    /// Starts something the engine may evaluate (see `classify`).
    Expression,
    /// Exactly one identifier: the sheet asking what a name is.
    /// Evaluates to its value when bound above, plain text when not.
    Reference,
    /// Anything else: prose the engine leaves alone.
    Text,
}

impl fmt::Display for LineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LineKind::Blank => "blank",
            LineKind::Comment => "comment",
            LineKind::Heading => "heading",
            LineKind::Expression => "expression",
            LineKind::Reference => "reference",
            LineKind::Text => "text",
        };
        f.write_str(s)
    }
}

/// One physical line of a sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// One-based position in the sheet; blanks count, numbers never skip.
    pub number: usize,
    pub raw: String,
    pub kind: LineKind,
}

impl Line {
    pub fn new(number: usize, raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let kind = classify(&raw);
        Line { number, raw, kind }
    }
}

/// Split a sheet into classified lines.
pub fn parse_sheet(text: &str) -> Vec<Line> {
    text.lines()
        .enumerate()
        .map(|(i, raw)| Line::new(i + 1, raw))
        .collect()
}

/// The structural index of a sheet: assignments and headings, in
/// document order with their one-based line numbers. Feeds the UI's
/// completion, go-to-definition, and outline features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetIndex {
    /// `(line, name)` for every assignment line, top to bottom.
    pub assignments: Vec<(usize, String)>,
    /// `(line, title)` for every heading, top to bottom (the `#`s
    /// stripped from the title).
    pub headings: Vec<(usize, String)>,
}

pub fn index_sheet(text: &str) -> SheetIndex {
    let mut index = SheetIndex::default();
    for line in parse_sheet(text) {
        match line.kind {
            LineKind::Expression => {
                let t = line.raw.trim();
                if let Some(eq) = t.find('=')
                    && !t[eq..].starts_with("==")
                {
                    let name = t[..eq].trim_end();
                    let valid = {
                        let mut chars = name.chars();
                        let first = chars.next();
                        first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
                    };
                    if valid {
                        index.assignments.push((line.number, name.to_string()));
                    }
                }
            }
            LineKind::Heading => {
                let title = line
                    .raw
                    .trim_start()
                    .trim_start_matches('#')
                    .trim()
                    .to_string();
                index.headings.push((line.number, title));
            }
            _ => {}
        }
    }
    index
}

/// Classification rules (spec.md, "Line kinds"):
///
/// - blank and comment shapes come first;
/// - 1-6 `#` then space/tab/end is a heading (`#tag` is not);
/// - a line is an Expression if it starts with a digit, `.digit`,
///   `$digit`, or `(`; or with `+`/`-` *glued* to more content
///   (`-5` is signed, `- 5` is a markdown bullet); or leads with the
///   keyword `total`; or is a variable statement (`name = ...`,
///   single `=`, not `==`); or leads with an identifier immediately
///   followed by `*`, `/`, `^`, or `(` (a reference line, `milk * 2`);
/// - a line that is exactly one identifier is a Reference: the sheet
///   asking what that name is worth (evaluated leniently, see
///   eval.rs);
/// - everything else is Text.
///
/// Leading whitespace is insignificant. Classification is shape-only:
/// an Expression line the parser rejects errors on its own line and
/// never affects its neighbours. Identifier-led lines keep `+`/`-`
/// for prose ("War and Peace - part 1" is a sentence): a missed
/// calculation beats a false error.
fn classify(raw: &str) -> LineKind {
    // both ends: a trailing space must not un-reference a bare name
    let t = raw.trim();
    if t.is_empty() {
        return LineKind::Blank;
    }
    if t.starts_with("//") {
        return LineKind::Comment;
    }
    if is_heading(t) {
        return LineKind::Heading;
    }
    if is_expression(t) {
        return LineKind::Expression;
    }
    if is_reference(t) {
        return LineKind::Reference;
    }
    LineKind::Text
}

fn is_heading(t: &str) -> bool {
    let hashes = t.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return false;
    }
    matches!(t[hashes..].chars().next(), None | Some(' ') | Some('\t'))
}

fn is_expression(t: &str) -> bool {
    let b = t.as_bytes();
    match b[0] {
        c if c.is_ascii_digit() => true,
        b'(' => true,
        b'.' | b'$' => b.get(1).is_some_and(|&c| c.is_ascii_digit()),
        b'+' | b'-' => b.get(1).is_some_and(|&c| !c.is_ascii_whitespace()),
        _ => is_word_expression(t),
    }
}

/// A line that is exactly one identifier (`groceries`, `answer`):
/// classified Reference, evaluated leniently (eval.rs). The keyword
/// `total` never reaches here; it is an Expression first.
fn is_reference(t: &str) -> bool {
    let mut chars = t.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Word-led lines: the keyword `total`, assignments, and reference
/// lines. A reference is an identifier immediately followed (spaces
/// allowed) by a multiplication-family operator or a group; `+`/`-`
/// and bare identifiers stay prose.
fn is_word_expression(t: &str) -> bool {
    if t == "total" || t.starts_with("total ") || t.starts_with("total\t") {
        return true;
    }
    if is_assignment(t) {
        return true;
    }
    let ident_end = t
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(t.len());
    if ident_end == 0 {
        return false;
    }
    matches!(
        t[ident_end..].trim_start().chars().next(),
        Some('*') | Some('/') | Some('^') | Some('(')
    )
}

fn is_assignment(t: &str) -> bool {
    let Some(eq) = t.find('=') else {
        return false;
    };
    if t[eq..].starts_with("==") {
        return false;
    }
    let name = t[..eq].trim_end();
    let mut chars = name.chars();
    let first = chars.next();
    first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use LineKind as K;

    #[test]
    fn classifies_lines() {
        let cases: &[(&str, LineKind)] = &[
            ("", K::Blank),
            ("   ", K::Blank),
            ("\t\t", K::Blank),
            ("// a note", K::Comment),
            ("  // indented note", K::Comment),
            ("# Title", K::Heading),
            ("###### deep", K::Heading),
            ("#", K::Heading),
            ("  ## indented heading", K::Heading),
            ("#\tTabbed", K::Heading),
            ("#tag", K::Text),
            ("####### seven hashes", K::Text),
            ("3 + 4", K::Expression),
            ("  12 * (2 + 1)", K::Expression),
            (".5 + 1", K::Expression),
            ("$5.60 * 3", K::Expression),
            ("(1 + 2) * 3", K::Expression),
            ("-5", K::Expression),
            ("+7", K::Expression),
            ("total", K::Expression),
            ("total * 2", K::Expression),
            ("rate = 12", K::Expression),
            ("  rate  =  12", K::Expression),
            ("_private = 1", K::Expression),
            ("milk * 2", K::Expression),
            ("milk / price", K::Expression),
            ("milk(2)", K::Expression),
            // `- ` and `+ ` with a space read as markdown bullets.
            ("- 5", K::Text),
            ("+ 7", K::Text),
            ("- buy milk", K::Text),
            // `==` is not assignment; prose stays prose.
            ("rate == 12", K::Text),
            ("a sentence = not an assignment", K::Text),
            ("$ rate", K::Text),
            // `+`/`-` after a name stays prose: a missed calculation
            // beats a false error cell.
            ("milk + 2", K::Text),
            ("War and Peace - part 1", K::Text),
            ("A note (important)", K::Text),
            ("Notes on the renovation", K::Text),
            // a bare identifier is the sheet asking what a name is
            ("groceries", K::Reference),
            ("answer", K::Reference),
            ("_x", K::Reference),
        ];

        let index = index_sheet("# Plan\nmilk = 3.50\n\n## Later\nx_1 = milk * 2\n");
        assert_eq!(
            index.assignments,
            vec![(2, "milk".to_string()), (5, "x_1".to_string())]
        );
        assert_eq!(
            index.headings,
            vec![(1, "Plan".to_string()), (4, "Later".to_string())]
        );
        for (raw, want) in cases {
            assert_eq!(Line::new(1, *raw).kind, *want, "line {raw:?}");
        }
    }

    #[test]
    fn sheet_lines_are_numbered_without_skipping_blanks() {
        let sheet = parse_sheet("milk = 3.50\n\nmilk * 2\n");
        let numbers: Vec<_> = sheet.iter().map(|l| l.number).collect();
        assert_eq!(numbers, vec![1, 2, 3]);
        let kinds: Vec<_> = sheet.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![K::Expression, K::Blank, K::Expression]);
    }
}

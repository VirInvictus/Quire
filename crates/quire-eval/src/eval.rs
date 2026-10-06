//! Evaluator: top-down sheet evaluation with per-line outcomes.
//!
//! State flows downward only: an assignment is visible to lines below,
//! never above. A failed line is contained: it shows its own error and
//! leaves variables, `answer`, and the running total untouched.

use std::collections::HashMap;

use crate::error::{ErrKind, QuireError};
use crate::format::format_number;
use crate::parser::{BinOp, Expr, Stmt, parse};
use crate::tokens::{Tok, tokenize};
use crate::{Line, LineKind};

/// What one line produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Value(f64),
    Failed(QuireError),
}

impl Outcome {
    /// The answer-cell text: the formatted value or the error message.
    pub fn render(&self) -> String {
        match self {
            Outcome::Value(v) => format_number(*v),
            Outcome::Failed(e) => e.kind.to_string(),
        }
    }
}

/// One classified line plus, for expression lines, what it produced.
#[derive(Debug, Clone, PartialEq)]
pub struct LineOutcome {
    pub number: usize,
    pub kind: LineKind,
    pub outcome: Option<Outcome>,
}

#[derive(Default)]
struct Ctx {
    vars: HashMap<String, f64>,
    answer: Option<f64>,
    subtotal: f64,
}

/// Evaluate a whole sheet top-down. Headings and `total` lines reset
/// the running subtotal (the Phase 1 gate decision: both act as
/// boundaries).
pub fn evaluate_sheet(text: &str) -> Vec<LineOutcome> {
    let mut ctx = Ctx::default();
    let mut out = Vec::new();
    for line in parse_sheet_lines(text) {
        let outcome = match line.kind {
            LineKind::Expression => Some(eval_line(&line.raw, &mut ctx)),
            LineKind::Reference => eval_reference(&line.raw, &mut ctx),
            LineKind::Heading => {
                ctx.subtotal = 0.0;
                None
            }
            _ => None,
        };
        out.push(LineOutcome {
            number: line.number,
            kind: line.kind,
            outcome,
        });
    }
    out
}

/// Evaluate a single line against a fresh context. Table-test and
/// app convenience; sheets go through `evaluate_sheet`. An unbound
/// reference has nothing to say context-free, so it surfaces here as
/// a `NoAnswer`-shaped error.
pub fn evaluate_line(raw: &str) -> Outcome {
    let mut ctx = Ctx::default();
    match crate::parse_sheet(raw).first().map(|l| l.kind) {
        Some(LineKind::Reference) => {
            // context-free, an unbound reference needs a named error
            let miss = match tokenize(raw).ok().as_deref().and_then(|t| t.first()) {
                Some(t) if matches!(t.tok, Tok::Answer) => ErrKind::NoAnswer,
                Some(t) => match &t.tok {
                    Tok::Ident(n) => ErrKind::UnknownName(n.clone()),
                    _ => ErrKind::UnexpectedEol {
                        expected: "a value",
                    },
                },
                None => ErrKind::UnexpectedEol {
                    expected: "a value",
                },
            };
            eval_reference(raw, &mut ctx)
                .unwrap_or_else(|| Outcome::Failed(QuireError::new((0, 0), miss)))
        }
        _ => eval_line(raw, &mut ctx),
    }
}

/// A bare identifier line: the sheet asking what a name is worth.
/// Bound above -> its value, counted like any expression result
/// (toward `answer`, the totals, and later lines). Unbound -> no cell
/// at all; prose stays prose and bare names never error.
fn eval_reference(raw: &str, ctx: &mut Ctx) -> Option<Outcome> {
    let toks = tokenize(raw).ok()?;
    if toks.len() != 1 {
        return None;
    }
    let v = match &toks[0].tok {
        Tok::Ident(name) => *ctx.vars.get(name)?,
        Tok::Answer => ctx.answer?,
        _ => return None,
    };
    ctx.answer = Some(v);
    ctx.subtotal += v;
    Some(Outcome::Value(v))
}

fn eval_line(raw: &str, ctx: &mut Ctx) -> Outcome {
    let toks = match tokenize(raw) {
        Ok(t) => t,
        Err(e) => return Outcome::Failed(e),
    };
    if toks.is_empty() {
        // classification guarantees expression-shaped lines carry
        // tokens; this is the defensive net, not a real path
        return Outcome::Failed(QuireError::new(
            (0, 0),
            ErrKind::UnexpectedEol {
                expected: "an expression",
            },
        ));
    }
    let stmt = match parse(&toks) {
        Ok(s) => s,
        Err(e) => return Outcome::Failed(e),
    };
    match eval_stmt(&stmt, ctx) {
        Ok(v) => Outcome::Value(v),
        Err(e) => Outcome::Failed(e),
    }
}

fn eval_stmt(stmt: &Stmt, ctx: &mut Ctx) -> Result<f64, QuireError> {
    match stmt {
        Stmt::Assign(name, _, e) => {
            let v = eval_expr(e, ctx)?;
            ctx.vars.insert(name.clone(), v);
            ctx.answer = Some(v);
            ctx.subtotal += v;
            Ok(v)
        }
        // a total reports the subtotal and becomes the answer, but
        // never adds itself to the sum it reports, and it starts a
        // new sum (spec: results "since the previous `total` line")
        Stmt::Expr(Expr::Total(_)) => {
            let v = ctx.subtotal;
            ctx.answer = Some(v);
            ctx.subtotal = 0.0;
            Ok(v)
        }
        Stmt::Expr(e) => {
            let v = eval_expr(e, ctx)?;
            ctx.answer = Some(v);
            ctx.subtotal += v;
            Ok(v)
        }
    }
}

fn eval_expr(e: &Expr, ctx: &Ctx) -> Result<f64, QuireError> {
    let v = match e {
        Expr::Num(n) => *n,
        Expr::Name(n, span) => ctx
            .vars
            .get(n)
            .copied()
            .ok_or_else(|| QuireError::new(*span, ErrKind::UnknownName(n.clone())))?,
        Expr::Total(_) => ctx.subtotal,
        Expr::Answer(span) => ctx
            .answer
            .ok_or_else(|| QuireError::new(*span, ErrKind::NoAnswer))?,
        Expr::Neg(span, inner) => {
            let v = -eval_expr(inner, ctx)?;
            finite(v, *span)?
        }
        // standalone percent is x / 100
        Expr::Pct(span, inner) => {
            let v = eval_expr(inner, ctx)? / 100.0;
            finite(v, *span)?
        }
        Expr::Bin(op, l, r, span) => {
            let a = eval_expr(l, ctx)?;
            let b = eval_expr(r, ctx)?;
            let v = match op {
                // relative percent: `a + b%` is `a + a*b/100`; since a
                // percent already evaluates to b/100, the relative
                // delta is simply a * b
                BinOp::Add if is_pct(r) => a + a * b,
                BinOp::Sub if is_pct(r) => a - a * b,
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => {
                    if b == 0.0 {
                        return Err(QuireError::new(*span, ErrKind::DivideByZero));
                    }
                    a / b
                }
                BinOp::Pow => a.powf(b),
            };
            finite(v, *span)?
        }
    };
    Ok(v)
}

fn is_pct(e: &Expr) -> bool {
    matches!(e, Expr::Pct(..))
}

fn finite(v: f64, span: crate::error::Span) -> Result<f64, QuireError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(QuireError::new(span, ErrKind::OutOfRange))
    }
}

fn parse_sheet_lines(text: &str) -> Vec<Line> {
    crate::parse_sheet(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrKind;

    fn show(src: &str) -> String {
        evaluate_line(src).render()
    }

    fn err_of(src: &str) -> ErrKind {
        match evaluate_line(src) {
            Outcome::Failed(e) => e.kind,
            Outcome::Value(v) => panic!("expected error, got {v}"),
        }
    }

    #[test]
    fn arithmetic_table() {
        assert_eq!(show("2 + 2"), "4");
        assert_eq!(show("1 + 2 * 3"), "7");
        assert_eq!(show("(1 + 2) * 3"), "9");
        assert_eq!(show("-2^2"), "-4");
        assert_eq!(show("2^3^2"), "512");
        assert_eq!(show("2^-3"), "0.125");
        assert_eq!(show("7 / 2"), "3.5");
        assert_eq!(show("--5"), "5");
        assert_eq!(show("10 - 4 - 3"), "3");
        assert_eq!(show(".5 + 1"), "1.5");
    }

    #[test]
    fn percent_table() {
        assert_eq!(show("15%"), "0.15");
        assert_eq!(show("200 + 15%"), "230");
        assert_eq!(show("200 - 10%"), "180");
        assert_eq!(show("15% of 200"), "30");
        assert_eq!(show("200 * 15%"), "30");
        // percent on the left of + stays a plain value (NoteCalc rule)
        assert_eq!(show("15% + 200"), "200.15");
        // relative only when the RHS is directly a percent
        assert_eq!(show("200 + (10% of 50)"), "205");
    }

    #[test]
    fn error_table() {
        assert_eq!(err_of("10 / 0"), ErrKind::DivideByZero);
        assert_eq!(err_of("milk"), ErrKind::UnknownName("milk".into()));
        assert_eq!(err_of("answer"), ErrKind::NoAnswer);
        assert_eq!(
            err_of("2 +"),
            ErrKind::UnexpectedEol {
                expected: "a value"
            }
        );
        assert_eq!(err_of("2 3"), ErrKind::TrailingTokens);
        assert_eq!(err_of("()"), ErrKind::EmptyParens);
        assert_eq!(err_of("$5"), ErrKind::BadChar('$'));
        assert_eq!(err_of("1.2.3"), ErrKind::BadNumber);
        assert_eq!(err_of("2^10000"), ErrKind::OutOfRange);
        assert_eq!(err_of("total = 5"), ErrKind::AssignToKeyword("total"));
    }

    #[test]
    fn trailing_comments_change_nothing() {
        assert_eq!(show("2 + 2 // quick sum"), "4");
        assert_eq!(show("3.5 * 2 //= 7 must not leak"), "7");
    }

    #[test]
    fn sheet_state_flows_downward_only() {
        let sheet = "\
milk = 3.50
milk * 2
(answer) + 1
total
# Section
total
";
        let lines = evaluate_sheet(sheet);
        let values: Vec<_> = lines
            .iter()
            .map(|l| l.outcome.as_ref().map(|o| o.render()))
            .collect();
        assert_eq!(
            values,
            vec![
                Some("3.5".to_string()),  // milk = 3.50
                Some("7".to_string()),    // milk * 2
                Some("8".to_string()),    // (answer) + 1, parenthesized per spec
                Some("18.5".to_string()), // total: 3.5 + 7 + 8
                None,                     // # Section resets
                Some("0".to_string()),    // total
            ]
        );
    }

    #[test]
    fn reassignment_reads_nearest_binding_above() {
        let sheet = "x = 2\nx * 10\nx = 3\nx * 10\n";
        let lines = evaluate_sheet(sheet);
        let rendered: Vec<_> = lines
            .iter()
            .filter_map(|l| l.outcome.as_ref().map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["2", "20", "3", "30"]);
    }

    #[test]
    fn bare_references_are_lenient() {
        // bound: the value, counted like any expression result
        let sheet = "milk = 3.50\nmilk\nanswer\ntotal\n";
        let lines = evaluate_sheet(sheet);
        let rendered: Vec<_> = lines
            .iter()
            .map(|l| l.outcome.as_ref().map(|o| o.render()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                Some("3.5".to_string()),  // milk = 3.50
                Some("3.5".to_string()),  // milk (reference)
                Some("3.5".to_string()),  // answer (reference)
                Some("10.5".to_string()), // total: 3.5 + 3.5 + 3.5
            ]
        );

        // unbound: no cell at all, and nothing after it is poisoned
        let sheet = "2 + 2\nghost\nanswer\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[1].outcome, None);
        assert_eq!(
            lines[2].outcome.as_ref().map(|o| o.render()),
            Some("4".to_string())
        );
    }

    #[test]
    fn failed_lines_are_contained() {
        let sheet = "ok = 5\nbad = 1 / 0\nok * 2\ntotal\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(
            lines[1].outcome,
            Some(Outcome::Failed(QuireError::new(
                (8, 9),
                ErrKind::DivideByZero
            )))
        );
        // the failure did not touch variables, answer, or subtotal
        assert_eq!(
            lines[2].outcome.as_ref().map(|o| o.render()),
            Some("10".to_string())
        );
        assert_eq!(
            lines[3].outcome.as_ref().map(|o| o.render()),
            Some("15".to_string())
        );
    }
}

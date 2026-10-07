//! Evaluator: top-down sheet evaluation with per-line outcomes.
//!
//! State flows downward only: an assignment is visible to lines below,
//! never above. A failed line is contained: it shows its own error and
//! leaves variables, `answer`, and the running total untouched.
//!
//! Two engines cooperate (spec.md "Unit expressions"): the scalar
//! engine owns everything that has always evaluated, and the unit
//! bridge (units.rs, an embedded numbat) takes only the lines the
//! scalar path declines. Quantity results bind like any result.

use std::collections::HashMap;

use numbat::value::Value as UnitValue;

use crate::error::{ErrKind, QuireError};
use crate::format::format_number;
use crate::parser::{BinOp, Expr, Stmt, parse};
use crate::tokens::{Tok, tokenize};
use crate::units::{self, Bridge};
use crate::{Line, LineKind};

/// What one line produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Value(f64),
    /// A unit-engine value (a quantity, `5.3 kg`). Rendered in the
    /// unit engine's own notation, which is also parseable source.
    Quantity(UnitValue),
    Failed(QuireError),
}

impl Outcome {
    /// The answer-cell text: the formatted value or the error message.
    pub fn render(&self) -> String {
        match self {
            Outcome::Value(v) => format_number(*v),
            Outcome::Quantity(v) => units::render(v),
            Outcome::Failed(e) => e.kind.to_string(),
        }
    }
}

/// A stored result: a scalar on the sheet's own engine, or a unit
/// value from the bridge.
#[derive(Debug, Clone, PartialEq)]
enum Num {
    S(f64),
    Q(UnitValue),
}

impl Num {
    /// The parseable source this binding seeds into the unit engine.
    fn source(&self) -> String {
        match self {
            Num::S(v) => units::scalar_source(*v),
            Num::Q(v) => units::render(v),
        }
    }

    fn outcome(&self) -> Outcome {
        match self {
            Num::S(v) => Outcome::Value(*v),
            Num::Q(v) => Outcome::Quantity(v.clone()),
        }
    }
}

/// The running subtotal behind `total`. Scalar sums stay on the fast
/// f64 path (the historical behavior, byte for byte); the first
/// quantity promotes the sum to a list the unit engine adds at the
/// `total` line, dimension-safely.
#[derive(Debug, Clone, PartialEq)]
enum Sum {
    S(f64),
    Q(Vec<Num>),
}

impl Default for Sum {
    fn default() -> Self {
        Sum::S(0.0)
    }
}

impl Sum {
    fn push(&mut self, num: Num) {
        match (self, num) {
            (Sum::S(s), Num::S(v)) => *s += v,
            (sum, num) => {
                let mut items = match std::mem::replace(sum, Sum::S(0.0)) {
                    Sum::S(s) => vec![Num::S(s)],
                    Sum::Q(items) => items,
                };
                items.push(num);
                *sum = Sum::Q(items);
            }
        }
    }
}

/// Why a scalar evaluation handed the line over to the unit bridge.
enum Fail {
    Err(QuireError),
    Reroute,
}

impl From<QuireError> for Fail {
    fn from(e: QuireError) -> Self {
        Fail::Err(e)
    }
}

/// The message for a line that needs the unit path but names a part
/// of the grammar it does not support yet (`total`, `answer`,
/// percent).
const NO_UNIT_SUPPORT: &str =
    "units cannot mix with `answer`, `total`, or percent in this line yet";

/// One classified line plus, for expression lines, what it produced.
#[derive(Debug, Clone, PartialEq)]
pub struct LineOutcome {
    pub number: usize,
    pub kind: LineKind,
    pub outcome: Option<Outcome>,
}

#[derive(Default)]
struct Ctx {
    vars: HashMap<String, Num>,
    fns: HashMap<String, (Vec<String>, Expr)>,
    answer: Option<Num>,
    subtotal: Sum,
    /// Per-tag sums behind `total @tag` (spec.md "Tags"): sheet-wide,
    /// reset only by a plain `total`; `total @tag` is a pure view.
    tag_sums: HashMap<String, Sum>,
    /// Live function-call depth, for the recursion cap (spec.md
    /// "Functions": a cycle fails the calling line).
    depth: u8,
}

/// Split trailing tags off an Expression line: `lunch = 12.50 @food
/// @london` becomes the body `lunch = 12.50` plus the tag list. A
/// tag is whitespace-prefixed `@identifier` glued to the line's end;
/// anything else keeps the line whole (and `@` mid-line tokenizes to
/// its own error).
fn split_tags(raw: &str) -> (String, Vec<String>, Option<String>) {
    let mut body = raw.trim_end().to_string();
    let mut tags = Vec::new();
    let mut stamp = None;
    loop {
        let trimmed = body.trim_end();
        let Some(at) = trimmed.rfind('@') else { break };
        // the annotation must start the line or follow whitespace
        if !(at == 0 || trimmed[..at].ends_with(char::is_whitespace)) {
            break;
        }
        let after = &trimmed[at + 1..];
        if let Some(name) = tag_name(after) {
            // a trailing tag must run to the end of the line; stragglers
            // after the name leave the line whole for the tokenizer
            if after.len() == name.len() {
                tags.push(name.to_string());
                body = trimmed[..at].trim_end().to_string();
                continue;
            }
            break;
        }
        // a date stamp: optional single space, then YYYY-MM-DD-ish
        let date = after.strip_prefix(' ').unwrap_or(after);
        if stamp.is_none() && is_date_shape(date) {
            stamp = Some(date.to_string());
            body = trimmed[..at].trim_end().to_string();
            continue;
        }
        break;
    }
    tags.reverse();
    (body, tags, stamp)
}

/// A glued tag name: identifier-shaped, letter or underscore first
/// (digit-first `@2026` belongs to the date stamp form).
fn tag_name(after: &str) -> Option<&str> {
    let mut chars = after.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let end = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    Some(&after[..end])
}

/// `YYYY-MM-DD` with one- or two-digit month and day.
fn is_date_shape(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 3
        && !parts[0].is_empty()
        && parts[0].len() <= 4
        && parts[1].len() <= 2
        && parts[2].len() <= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
}

/// Evaluate a whole sheet top-down. Headings and `total` lines reset
/// the running subtotal (the Phase 1 gate decision: both act as
/// boundaries).
pub fn evaluate_sheet(text: &str) -> Vec<LineOutcome> {
    let mut ctx = Ctx::default();
    let mut bridge = Bridge::new();
    let mut out = Vec::new();
    for line in parse_sheet_lines(text) {
        let outcome = match line.kind {
            LineKind::Expression => eval_line(&line.raw, &mut ctx, &mut bridge),
            LineKind::Reference => eval_reference(&line.raw, &mut ctx, &mut bridge),
            LineKind::Heading => {
                // headings section the plain math; tag sums are
                // sheet-wide views (spec.md "Tags")
                ctx.subtotal = Sum::S(0.0);
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
pub fn evaluate_line(raw: &str) -> Option<Outcome> {
    let mut ctx = Ctx::default();
    let mut bridge = Bridge::new();
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
            Some(
                eval_reference(raw, &mut ctx, &mut bridge)
                    .unwrap_or_else(|| Outcome::Failed(QuireError::new((0, 0), miss))),
            )
        }
        _ => eval_line(raw, &mut ctx, &mut bridge),
    }
}

/// A bare identifier line: the sheet asking what a name is worth.
/// Bound above -> its value, counted like any expression result
/// (toward `answer`, the totals, and later lines). Unbound -> no cell
/// at all; prose stays prose and bare names never error.
fn eval_reference(raw: &str, ctx: &mut Ctx, bridge: &mut Option<Bridge>) -> Option<Outcome> {
    let toks = tokenize(raw).ok()?;
    if toks.len() != 1 {
        return None;
    }
    let num = match &toks[0].tok {
        Tok::Ident(name) => ctx.vars.get(name)?.clone(),
        Tok::Answer => ctx.answer.clone()?,
        _ => return None,
    };
    ctx.answer = Some(num.clone());
    ctx.subtotal.push(num.clone());
    Some(match num {
        Num::S(v) => Outcome::Value(v),
        Num::Q(_) => {
            let _ = bridge;
            num.outcome()
        }
    })
}

fn eval_line(raw: &str, ctx: &mut Ctx, bridge: &mut Option<Bridge>) -> Option<Outcome> {
    let (body, tags, _stamp) = split_tags(raw);
    let raw = body.as_str();
    let toks = match tokenize(raw) {
        Ok(t) => t,
        Err(e) => return Some(Outcome::Failed(e)),
    };
    if toks.is_empty() {
        // classification guarantees expression-shaped lines carry
        // tokens; this is the defensive net, not a real path
        return Some(Outcome::Failed(QuireError::new(
            (0, 0),
            ErrKind::UnexpectedEol {
                expected: "an expression",
            },
        )));
    }
    let stmt = match parse(&toks) {
        Ok(s) => s,
        Err(e) => {
            return Some(declined_line(raw, &toks, &tags, ctx, bridge, Fail::Err(e)));
        }
    };
    Some(match eval_stmt(&stmt, &tags, ctx, bridge) {
        Ok(Some(num)) => num.outcome(),
        Ok(None) => return None, // a function definition: no cell
        Err(Fail::Err(e)) => Outcome::Failed(e),
        Err(Fail::Reroute) => declined_line(
            raw,
            &toks,
            &tags,
            ctx,
            bridge,
            Fail::Err(QuireError::new(
                (0, 0),
                ErrKind::Unit(NO_UNIT_SUPPORT.into()),
            )),
        ),
    })
}

/// A line the scalar path declined: the unit bridge takes it when the
/// grammar permits (no `total`/`answer`/percent tokens and at least
/// one known unit name), and otherwise the original failure stands
/// (spec: a missed calculation beats a false error).
fn declined_line(
    raw: &str,
    toks: &[crate::tokens::Token],
    tags: &[String],
    ctx: &mut Ctx,
    bridge: &mut Option<Bridge>,
    fail: Fail,
) -> Outcome {
    let routable = !toks
        .iter()
        .any(|t| matches!(t.tok, Tok::Total | Tok::Answer | Tok::Of | Tok::Percent))
        && bridge.as_mut().is_some_and(|b| {
            toks.iter()
                .any(|t| matches!(&t.tok, Tok::Ident(n) if b.knows_unit(n)))
        });
    if !routable {
        return match fail {
            Fail::Err(e) => Outcome::Failed(e),
            Fail::Reroute => Outcome::Failed(QuireError::new(
                (0, 0),
                ErrKind::Unit(NO_UNIT_SUPPORT.into()),
            )),
        };
    }
    match bridge_eval(raw, toks, ctx, bridge) {
        Outcome::Quantity(v) => {
            record_tags(ctx, tags, Num::Q(v.clone()));
            Outcome::Quantity(v)
        }
        other => other,
    }
}

/// Compile the line for the unit engine (seed `let`s for the bound
/// names it references, assignment as a numbat `let`), evaluate, and
/// bind the result like any expression result.
fn bridge_eval(
    raw: &str,
    toks: &[crate::tokens::Token],
    ctx: &mut Ctx,
    bridge: &mut Option<Bridge>,
) -> Outcome {
    let Some(bridge) = bridge else {
        return Outcome::Failed(QuireError::new(
            (0, 0),
            ErrKind::Unit(NO_UNIT_SUPPORT.into()),
        ));
    };

    // The engine's comment character is `#`, the sheet's is `//`:
    // the scalar tokenizer strips comments before parsing, so the
    // compiled line must too, or numbat reads `// 46.5` as division.
    let raw = raw.split("//").next().unwrap_or(raw);
    if raw.trim().is_empty() {
        // a pure-comment body reaching the bridge means the scalar
        // path never saw tokens for it; nothing to evaluate
        return Outcome::Failed(QuireError::new(
            (0, 0),
            ErrKind::UnexpectedEol {
                expected: "an expression",
            },
        ));
    }
    let mut source = String::new();
    let mut referenced = std::collections::BTreeSet::new();
    for t in toks {
        if let Tok::Ident(n) = &t.tok
            && ctx.vars.contains_key(n)
        {
            referenced.insert(n.clone());
        }
    }
    for name in &referenced {
        let Some(num) = ctx.vars.get(name) else {
            continue;
        };
        source.push_str(&format!("let {name} = {}\n", num.source()));
    }

    // An assignment line becomes a numbat `let` so the sheet can
    // store the quantity it produces (`=` in numbat is comparison).
    // A trailing reference to the name turns the let back into a
    // value: numbat answers `let` statements with Continue, and the
    // sheet wants the stored quantity as the line's result.
    let assignment = split_assignment(raw);
    match &assignment {
        Some((name, rest)) => source.push_str(&format!("let {name} = {rest}\n{name}")),
        None => source.push_str(raw),
    }

    match bridge.eval(&source) {
        Ok(v) => {
            let num = Num::Q(v.clone());
            if let Some((name, _)) = &assignment {
                ctx.vars.insert(name.clone(), num.clone());
            }
            ctx.answer = Some(num.clone());
            ctx.subtotal.push(num);
            Outcome::Quantity(v)
        }
        Err(message) => Outcome::Failed(QuireError::new((0, 0), ErrKind::Unit(message))),
    }
}

/// The `name = rest` split of an assignment line. Classification
/// already guarantees the shape (single `=`, identifier head), so
/// this only re-finds the split on the raw text.
fn split_assignment(raw: &str) -> Option<(String, &str)> {
    let trimmed = raw.trim_start();
    let ident_end = trimmed
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(trimmed.len());
    if ident_end == 0 {
        return None;
    }
    let name = &trimmed[..ident_end];
    let rest = trimmed[ident_end..].trim_start();
    let rest = rest.strip_prefix('=')?;
    if rest.starts_with('=') {
        return None;
    }
    Some((name.to_string(), rest))
}

fn eval_stmt(
    stmt: &Stmt,
    tags: &[String],
    ctx: &mut Ctx,
    bridge: &mut Option<Bridge>,
) -> Result<Option<Num>, Fail> {
    match stmt {
        Stmt::FnDef(name, params, body, _) => {
            // definitions are visible below, produce no cell, and
            // never touch answer, subtotal, or tags (spec.md
            // "Functions")
            ctx.fns.insert(name.clone(), (params.clone(), body.clone()));
            Ok(None)
        }
        Stmt::Assign(name, _, e) => {
            let num = eval_expr(e, ctx)?;
            ctx.vars.insert(name.clone(), num.clone());
            ctx.answer = Some(num.clone());
            ctx.subtotal.push(num.clone());
            record_tags(ctx, tags, num.clone());
            Ok(Some(num))
        }
        // a total reports the subtotal and becomes the answer, but
        // never adds itself to the sum it reports, and it starts a
        // new sum (spec: results "since the previous `total` line").
        // A tagged total is a pure view over the tag sums: it
        // reports and resets nothing.
        Stmt::Expr(Expr::Total(_)) => {
            let num = if tags.is_empty() {
                let num = finish_sum(&ctx.subtotal, bridge)?;
                ctx.subtotal = Sum::S(0.0);
                ctx.tag_sums.clear();
                num
            } else {
                let mut merged = Sum::S(0.0);
                for tag in tags {
                    if let Some(sum) = ctx.tag_sums.get(tag) {
                        merge_sum(&mut merged, sum.clone());
                    }
                }
                finish_sum(&merged, bridge)?
            };
            ctx.answer = Some(num.clone());
            Ok(Some(num))
        }
        Stmt::Expr(e) => {
            let num = eval_expr(e, ctx)?;
            ctx.answer = Some(num.clone());
            ctx.subtotal.push(num.clone());
            record_tags(ctx, tags, num.clone());
            Ok(Some(num))
        }
    }
}

fn record_tags(ctx: &mut Ctx, tags: &[String], num: Num) {
    for tag in tags {
        ctx.tag_sums
            .entry(tag.clone())
            .or_default()
            .push(num.clone());
    }
}

fn merge_sum(base: &mut Sum, other: Sum) {
    match other {
        Sum::S(v) => base.push(Num::S(v)),
        Sum::Q(items) => {
            for item in items {
                base.push(item);
            }
        }
    }
}

fn finish_sum(sum: &Sum, bridge: &mut Option<Bridge>) -> Result<Num, Fail> {
    match sum {
        Sum::S(s) => Ok(Num::S(*s)),
        Sum::Q(items) => sum_by_bridge(items, bridge),
    }
}

/// A subtotal that includes quantities sums through the unit engine:
/// like dimensions add, anything else fails the total line (spec:
/// scalar and quantity results never silently mix).
fn sum_by_bridge(items: &[Num], bridge: &mut Option<Bridge>) -> Result<Num, Fail> {
    let Some(bridge) = bridge else {
        return Err(Fail::Reroute);
    };
    let mut source = String::new();
    for (i, item) in items.iter().enumerate() {
        source.push_str(&format!("let __q{i} = {}\n", item.source()));
    }
    source.push_str(
        &(0..items.len())
            .map(|i| format!("__q{i}"))
            .collect::<Vec<_>>()
            .join(" + "),
    );
    bridge
        .eval(&source)
        .map(Num::Q)
        .map_err(|message| Fail::Err(QuireError::new((0, 0), ErrKind::Unit(message))))
}

fn eval_expr(e: &Expr, ctx: &mut Ctx) -> Result<Num, Fail> {
    let num = match e {
        Expr::Num(n) => Num::S(*n),
        Expr::Call(name, args, span) => {
            let Some((params, body)) = ctx.fns.get(name).cloned() else {
                return Err(Fail::Err(QuireError::new(
                    *span,
                    ErrKind::UnknownName(name.clone()),
                )));
            };
            if args.len() != params.len() {
                return Err(Fail::Err(QuireError::new(
                    *span,
                    ErrKind::CallArity {
                        name: name.clone(),
                        expected: params.len(),
                        got: args.len(),
                    },
                )));
            }
            ctx.depth += 1;
            if ctx.depth > 64 {
                ctx.depth -= 1;
                return Err(Fail::Err(QuireError::new(*span, ErrKind::NestTooDeep)));
            }
            // the call frame: params shadow sheet variables; the body
            // sees the sheet's variables, functions, and answer, but
            // a zero subtotal (`total` in a body reads zero, spec)
            let mut frame = Ctx {
                vars: ctx.vars.clone(),
                fns: ctx.fns.clone(),
                answer: ctx.answer.clone(),
                subtotal: Sum::S(0.0),
                tag_sums: Default::default(),
                depth: ctx.depth,
            };
            for (param, arg) in params.iter().zip(args) {
                let value = eval_expr(arg, ctx)?;
                frame.vars.insert(param.clone(), value);
            }
            let num = eval_expr(&body, &mut frame)?;
            ctx.depth -= 1;
            num
        }
        Expr::Name(n, span) => ctx
            .vars
            .get(n)
            .cloned()
            .ok_or_else(|| QuireError::new(*span, ErrKind::UnknownName(n.clone())))?,
        Expr::Total(_) => match &ctx.subtotal {
            Sum::S(s) => Num::S(*s),
            Sum::Q(_) => return Err(Fail::Reroute),
        },
        Expr::Answer(span) => ctx
            .answer
            .clone()
            .ok_or_else(|| QuireError::new(*span, ErrKind::NoAnswer))?,
        Expr::Neg(span, inner) => {
            let Num::S(v) = eval_expr(inner, ctx)? else {
                return Err(Fail::Reroute);
            };
            Num::S(finite(-v, *span)?)
        }
        // standalone percent is x / 100
        Expr::Pct(span, inner) => {
            let Num::S(v) = eval_expr(inner, ctx)? else {
                return Err(Fail::Reroute);
            };
            Num::S(finite(v / 100.0, *span)?)
        }
        Expr::Bin(op, l, r, span) => {
            let (Num::S(a), Num::S(b)) = (eval_expr(l, ctx)?, eval_expr(r, ctx)?) else {
                return Err(Fail::Reroute);
            };
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
                        return Err(Fail::Err(QuireError::new(*span, ErrKind::DivideByZero)));
                    }
                    a / b
                }
                BinOp::Pow => a.powf(b),
            };
            Num::S(finite(v, *span)?)
        }
    };
    Ok(num)
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
    use crate::use_test_rates;

    fn show(src: &str) -> String {
        evaluate_line(src).expect("a result").render()
    }

    fn err_of(src: &str) -> ErrKind {
        match evaluate_line(src) {
            Some(Outcome::Failed(e)) => e.kind,
            Some(Outcome::Value(v)) => panic!("expected error, got {v}"),
            Some(Outcome::Quantity(v)) => panic!("expected error, got {v}"),
            None => panic!("expected error, got no cell"),
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

    #[test]
    fn unit_lines_route_to_the_unit_engine() {
        // the scalar parse declines `5 kg` (trailing identifier) and
        // the line names a known unit: the unit engine takes it and
        // the rendering is the engine's own notation
        assert_eq!(show("5 kg + 300 g"), "5300 g");
    }

    #[test]
    fn unit_bindings_flow_like_any_result() {
        // `bag + 300 g` stays prose (the identifier-led rule covers
        // quantities too); parenthesizing asks for the arithmetic
        let sheet = "bag = 5 kg\n(bag) + 300 g\nbag\ntotal\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                "5 kg".to_string(),    // the binding stores the quantity
                "5300 g".to_string(),  // the parenthesized unit line
                "5 kg".to_string(),    // bare reference
                "15300 g".to_string(), // total: the assign, the sum, and the reference
            ]
        );
    }

    #[test]
    fn mixed_totals_fail_and_stay_contained() {
        let sheet = "bag = 5 kg\nmilk = 3.5\ntotal\ntotal\n";
        let lines = evaluate_sheet(sheet);
        // scalar and quantity never silently mix (spec.md)
        assert!(matches!(
            &lines[2].outcome,
            Some(Outcome::Failed(e)) if matches!(e.kind, ErrKind::Unit(_))
        ));
        // the failed total left the sum untouched, so it fails again
        assert!(matches!(
            &lines[3].outcome,
            Some(Outcome::Failed(e)) if matches!(e.kind, ErrKind::Unit(_))
        ));
    }

    #[test]
    fn scalar_engine_stays_alone_without_units() {
        // no unit names anywhere: byte-identical legacy behavior
        assert_eq!(show("2 + 2"), "4");
        assert_eq!(err_of("2 bloognorch"), ErrKind::TrailingTokens);
    }

    #[test]
    fn functions_define_call_and_shadow() {
        let sheet = "double(x) = x * 2
double(21)
area(w, h) = w * h
area(3, 4)
n = 10
double(n)
";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        // the definitions produce no cells; the calls do
        assert_eq!(rendered, vec!["42", "12", "10", "20"]);
    }

    #[test]
    fn function_scoping_and_limits() {
        // params shadow sheet variables; redefinition wins below;
        // arity mismatches fail the calling line; recursion is capped
        let sheet = "x = 100
double(x) = x * 2
double(5)
double(x) = x + 1
double(5)
double(1, 2)
loop(x) = loop(x)
loop(1)
";
        let lines = evaluate_sheet(sheet);
        let rendered: Vec<Option<String>> = lines
            .into_iter()
            .map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                Some("100".to_string()), // x = 100
                None,                    // the definition: no cell
                Some("10".to_string()),  // param shadows the sheet variable
                None,                    // redefinition: no cell
                Some("6".to_string()),   // the redefined body wins below
                Some("`double` expects 1 argument, got 2".to_string()),
                None, // the recursive definition: no cell
                Some("expression nested too deeply".to_string()),
            ]
        );
    }

    #[test]
    fn unit_lines_strip_sheet_comments_before_the_bridge() {
        // the engine's comment char is `#`; a trailing `//` comment
        // must never reach numbat as division (Brandon hit this by
        // uncommenting the tour's currency line)
        use_test_rates();
        let sheet = "50 USD -> EUR // 46.5 \u{20ac}\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["50 \u{20ac}".to_string()]);
    }

    #[test]
    fn tag_totals_are_pure_views() {
        let sheet = "\
rent = 1200 @housing
groceries = 250 @food
total @housing
total @food
total @housing @food
total
total @housing
";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                "1,200".to_string(), // the view, before any reset
                "250".to_string(),   // the other tag
                "1,200".to_string(), // the first tagged total again
                "250".to_string(),
                "1,450".to_string(), // the union of two tags
                "1,450".to_string(), // the plain total includes tagged lines
                "0".to_string(),     // the plain total reset the tag sums
            ]
        );
    }

    #[test]
    fn tags_are_sheet_wide_across_headings() {
        // headings section the plain math, but tag sums accumulate
        // across them: the budget pattern is categories in sections
        // with the views at the end
        let sheet = "\
5 kg @bulk
# Aisle
3 kg @bulk
total @bulk
total
";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                "5 kg".to_string(), // the first tagged unit line
                "3 kg".to_string(), // the second, after the heading
                "8 kg".to_string(), // the view spans the headings
                "3 kg".to_string(), // the plain total is section-bounded
            ]
        );
    }
}

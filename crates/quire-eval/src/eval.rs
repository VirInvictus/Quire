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
use crate::parser::{BinOp, Expr, Stmt, parse, parse_strict};
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
    /// A `&N` reference with no result yet (spec.md "Line
    /// references"): the whole line answers blank until the target
    /// produces a value.
    Null,
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
    fns: HashMap<String, Vec<(Vec<String>, Expr)>>,
    answer: Option<Num>,
    subtotal: Sum,
    /// Per-tag sums behind `total @tag` (spec.md "Tags"): sheet-wide,
    /// reset only by a plain `total`; `total @tag` is a pure view.
    tag_sums: HashMap<String, Sum>,
    /// Live function-call depth, for the recursion cap (spec.md
    /// "Functions": a cycle fails the calling line).
    depth: u8,
    /// Evaluation steps consumed so far; capped to stop runaway
    /// computation (a deep recursion passes the depth cap but burns
    /// real time without it).
    steps: u64,
    /// Line outcomes so far, for `&N` resolution (spec.md "Line
    /// references"): populated as the sheet evaluates, so a ref can
    /// only ever see lines above it.
    outcomes: HashMap<usize, Outcome>,
    /// The one-based line being evaluated.
    line: usize,
    /// True inside a function-call frame: `&N` is sheet-positional
    /// and bodies refuse it (spec.md "Line references"), exactly as
    /// they refuse `total`'s sheet position.
    in_fn_body: bool,
    /// When watching a line (the breakdown popover), its operations
    /// record human-readable steps here, bottom-up.
    watch: Option<usize>,
    explain_steps: Vec<String>,
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
    evaluate_sheet_explaining(text, None).0
}

/// The step-by-step breakdown of one sheet line (the Ctrl+click
/// popover, spec.md "The breakdown"): the line's scalar operations,
/// bottom-up, as `operands = value` strings, evaluated in the full
/// sheet context so variables and references resolve. None when the
/// scalar engine does not reduce the line into operations
/// (quantities, definitions, prose) or the line is out of range.
pub fn explain_sheet_line(text: &str, line_no: usize) -> Option<Vec<String>> {
    let (outcomes, steps) = evaluate_sheet_explaining(text, Some(line_no));
    if steps.is_empty() {
        return None;
    }
    let _ = outcomes;
    Some(steps)
}

fn evaluate_sheet_explaining(text: &str, watch: Option<usize>) -> (Vec<LineOutcome>, Vec<String>) {
    let mut ctx = Ctx {
        watch,
        ..Ctx::default()
    };
    let mut bridge = Bridge::new();
    let mut out = Vec::new();
    for line in parse_sheet_lines(text) {
        ctx.line = line.number;
        let outcome = match line.kind {
            LineKind::Expression => eval_line(&line.raw, &mut ctx, &mut bridge),
            LineKind::Text => mixed_line(&line.raw, &mut ctx, &mut bridge),
            LineKind::Reference => eval_reference(&line.raw, &mut ctx, &mut bridge),
            LineKind::Heading => {
                // headings section the plain math; tag sums are
                // sheet-wide views (spec.md "Tags")
                ctx.subtotal = Sum::S(0.0);
                None
            }
            _ => None,
        };
        if let Some(outcome) = &outcome {
            ctx.outcomes.insert(line.number, outcome.clone());
        }
        out.push(LineOutcome {
            number: line.number,
            kind: line.kind,
            outcome,
        });
    }
    (out, ctx.explain_steps)
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
        Tok::Ident(name) => match ctx.vars.get(name) {
            Some(num) => num.clone(),
            None if bridge.is_some() && is_date_word(name) => {
                // bare date vocabulary answers through the engine
                let source = translate_date_phrases(name);
                return match bridge.as_mut().unwrap().eval(&source) {
                    Ok(v) => {
                        let num = Num::Q(v);
                        ctx.answer = Some(num.clone());
                        ctx.subtotal.push(num.clone());
                        Some(num.outcome())
                    }
                    Err(message) => Some(Outcome::Failed(QuireError::new(
                        (0, 0),
                        ErrKind::Unit(message),
                    ))),
                };
            }
            None => return None,
        },
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
    let mut raw = body.as_str();
    let toks = match tokenize(raw) {
        Ok(t) => t,
        Err(e) => {
            // sentence-terminal punctuation is prose glue (spec.md
            // "Mixed lines"): a lab-notebook line may end in a
            // period. One retry with the glue stripped; a genuine
            // bad character stands.
            let trimmed = raw.trim_end_matches(['.', ',', ';', ':', '!', '?']);
            if trimmed.len() == raw.len() {
                return Some(Outcome::Failed(e));
            }
            match tokenize(trimmed) {
                Ok(t) => {
                    raw = trimmed;
                    t
                }
                Err(_) => return Some(Outcome::Failed(e)),
            }
        }
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
        Err(e) => return declined_line(raw, &toks, &tags, ctx, bridge, Fail::Err(e)),
    };
    Some(match eval_stmt(&stmt, &tags, ctx, bridge) {
        Ok(Some(num)) => num.outcome(),
        Ok(None) => return None, // a function definition: no cell
        // a blank &N ref: the line answers nothing until the target
        // produces a value (spec.md "Line references")
        Err(Fail::Null) => return None,
        // classify-by-failure: an unknown name may be prose, and the
        // prose-stripped skeleton may still answer
        Err(Fail::Err(e)) if matches!(e.kind, ErrKind::UnknownName(_)) => {
            // a scalar failure on a name the unit engine knows - a
            // unit, a period, a date word - hands the line to the
            // bridge (implicit multiplication lets value-unit shapes
            // parse now, so the failure surfaces here instead of at
            // the parse). Not bridge-known, or the bridge refused:
            // the mixed skeleton keeps its shot.
            unit_line(raw, &toks, &e, &tags, ctx, bridge)
                .or_else(|| mixed_line(raw, ctx, bridge))
                .unwrap_or(Outcome::Failed(e))
        }
        Err(Fail::Err(e)) => Outcome::Failed(e),
        Err(Fail::Reroute) => {
            // the unit bridge refused; the mixed skeleton gets one
            // more shot before the honest error stands
            declined_line(
                raw,
                &toks,
                &tags,
                ctx,
                bridge,
                Fail::Err(QuireError::new(
                    (0, 0),
                    ErrKind::Unit(NO_UNIT_SUPPORT.into()),
                )),
            )?
        }
    })
}

/// Mixed-line evaluation (spec.md "Mixed lines"): a Text line whose
/// prose words strip out to a complete expression answers with that
/// expression's value - `50 apples at 3 each` is `150`. Word
/// operators map (`at`/`of`/`times` multiply, `plus` adds, `minus`
/// subtracts); unknown names drop as prose; anything that does not
/// fully evaluate leaves the line as prose (a missed calculation
/// beats a false error, and mixed failures are silent by design).
fn mixed_line(raw: &str, ctx: &mut Ctx, bridge: &mut Option<Bridge>) -> Option<Outcome> {
    // sentence-terminal punctuation is prose glue: a lab-notebook
    // line ends with a period without losing its answer (spec.md
    // "Mixed lines")
    let raw = raw.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    let toks = tokenize(raw).ok()?;
    if toks.is_empty() {
        return None;
    }
    // reserved words keep their errors: `total = 5` must never
    // become a mixed `5`. Tags and line refs carry structured
    // meaning too - dropping them would misread malformed stamps
    // and refs as arithmetic.
    if toks.iter().any(|t| {
        matches!(
            t.tok,
            Tok::Total | Tok::Answer | Tok::Tag(_) | Tok::LineRef(_)
        )
    }) {
        return None;
    }
    // reverse percent phrases preempt the skeleton (spec.md
    // "Reverse percents"): their `of` must never become a multiply
    if let Some(outcome) = reverse_percent(&toks, ctx, bridge) {
        return Some(outcome);
    }
    // the skeleton: drop prose idents, map word operators. A line
    // with no numbers cannot become math.
    let mut kept: Vec<crate::tokens::Token> = Vec::new();
    let mut saw_number = false;
    for t in &toks {
        match &t.tok {
            Tok::Num(_) => saw_number = true,
            // commas are prose glue too (functions in sentences are
            // rare enough to lose their argument separators)
            Tok::Comma => continue,
            Tok::Plus
            | Tok::Minus
            | Tok::Star
            | Tok::Slash
            | Tok::Caret
            | Tok::LParen
            | Tok::RParen => {}
            Tok::Ident(n) => match n.to_lowercase().as_str() {
                "at" | "of" | "times" => {
                    kept.push(crate::tokens::Token {
                        tok: Tok::Star,
                        span: t.span,
                    });
                    continue;
                }
                "plus" => {
                    kept.push(crate::tokens::Token {
                        tok: Tok::Plus,
                        span: t.span,
                    });
                    continue;
                }
                "minus" => {
                    kept.push(crate::tokens::Token {
                        tok: Tok::Minus,
                        span: t.span,
                    });
                    continue;
                }
                // known names survive; unknown names are the prose
                _ => {
                    // functions are NOT prose candidates: numbat's
                    // own fn names (`show`, `count`) are ordinary
                    // English words, and keeping them would poison
                    // sentences
                    let known = ctx.vars.contains_key(n)
                        || ctx.fns.contains_key(n)
                        || bridge.as_ref().is_some_and(|b| b.knows_unit(n));
                    if !known {
                        continue;
                    }
                }
            },
            // anything the scalar tokenizer rejects (prose punctuation)
            _ => continue,
        }
        kept.push(t.clone());
    }
    if !saw_number {
        return None;
    }
    // a skeleton carrying a unit or an engine function evaluates
    // through the bridge - sentences with quantities answer (spec.md
    // "Mixed lines"). Refusals fall back to the strict scalar path.
    if kept.iter().any(|t| {
        matches!(&t.tok, Tok::Ident(n) if !ctx.vars.contains_key(n) && !ctx.fns.contains_key(n))
    }) {
        let kinds: Vec<&str> = kept
            .iter()
            .map(|t| match &t.tok {
                Tok::Num(_) => "num",
                // a bound sheet name is a "var" (it never multiplies
                // by accident); everything else kept is a unit or
                // an engine function
                Tok::Ident(n) => {
                    if ctx.vars.contains_key(n) || ctx.fns.contains_key(n) {
                        "var"
                    } else {
                        "unit"
                    }
                }
                _ => "op",
            })
            .collect();
        // the adjacency rule: a number may lean on the unit it
        // measures, units may stand together, operators separate
        // anything - but two value-ish tokens never multiply by
        // accident, and a bound name next to anything stays prose
        let well_formed = !matches!(kinds.first(), Some(&"op"))
            && !matches!(kinds.last(), Some(&"op"))
            && kinds.windows(2).all(|w| match (w[0], w[1]) {
                ("op", _) | (_, "op") => true,
                ("num", "unit") | ("unit", "unit") => true,
                _ => false,
            });
        if well_formed {
            let skeleton = kept.iter().map(token_text).collect::<Vec<_>>().join(" ");
            match bridge_eval(&skeleton, &kept, ctx, bridge) {
                Outcome::Failed(e) if !bridge_error_is_unknown(&e) => {
                    return Some(Outcome::Failed(e))
                }
                outcome @ (Outcome::Quantity(_) | Outcome::Value(_)) => return Some(outcome),
                _ => {}
            }
        }
    }
    // the skeleton must parse and evaluate; anything else is prose.
    // Strict grammar: implicit multiplication lives on the
    // expression path, so a prose remnant like `5 milk` with milk
    // bound stays prose (spec.md "Operators")
    let stmt = parse_strict(&kept).ok()?;
    match eval_stmt(&stmt, &[], ctx, bridge) {
        Ok(Some(num)) => Some(num.outcome()),
        _ => None,
    }
}

/// The source form of a kept skeleton token (spec.md "Mixed lines").
fn token_text(t: &crate::tokens::Token) -> String {
    match &t.tok {
        Tok::Num(v) => format!("{v}"),
        Tok::Ident(n) => n.clone(),
        Tok::Plus => "+".to_string(),
        Tok::Minus => "-".to_string(),
        Tok::Star => "*".to_string(),
        Tok::Slash => "/".to_string(),
        Tok::Caret => "^".to_string(),
        Tok::Percent => "%".to_string(),
        Tok::LParen => "(".to_string(),
        Tok::RParen => ")".to_string(),
        _ => String::new(),
    }
}

/// Sheet date vocabulary (spec.md "Dates"): words that route to the
/// unit engine even though no unit shares their name.
fn is_date_word(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "today" | "now" | "tomorrow" | "yesterday"
    )
}

/// An identifier token matching a phrase word, case-insensitively
/// (the date vocabulary's rule). The keyword `of` has its own
/// token, and the phrases must see it as the word.
fn is_word(t: &crate::tokens::Token, word: &str) -> bool {
    match &t.tok {
        Tok::Ident(n) => n.eq_ignore_ascii_case(word),
        Tok::Of => word == "of",
        _ => false,
    }
}

/// Reverse percent phrases (spec.md "Reverse percents"): the line
/// rewrites to its arithmetic and answers through the scalar
/// engine. `N is P% of what` divides out the percent; `off` and
/// `on` ride the relative-percent rules (`(1 - P%)`, `(1 + P%)`);
/// `N is what percent of M` answers the fraction N/M. Strict
/// shapes: anything else returns None and the caller's path -
/// usually the mixed skeleton's silence - stands.
fn reverse_percent(
    toks: &[crate::tokens::Token],
    ctx: &mut Ctx,
    bridge: &mut Option<Bridge>,
) -> Option<Outcome> {
    let is_at = toks.iter().position(|t| is_word(t, "is"))?;
    if is_at == 0 {
        return None;
    }
    let head = &toks[..is_at];
    let rest = &toks[is_at + 1..];
    // synthesized tokens borrow the `is` span: a failure in the
    // rewritten arithmetic points into the phrase
    let at = toks[is_at].span;
    let plain = |tok: Tok| crate::tokens::Token { tok, span: at };
    let group = |inner: &[crate::tokens::Token]| -> Vec<crate::tokens::Token> {
        let mut out = vec![plain(Tok::LParen)];
        out.extend(inner.iter().cloned());
        out.push(plain(Tok::RParen));
        out
    };
    // `(head)` + body, evaluated as one statement
    let mut eval_wrap = |body: Vec<crate::tokens::Token>| -> Option<Outcome> {
        let mut rewritten = vec![plain(Tok::LParen)];
        rewritten.extend(head.iter().cloned());
        rewritten.push(plain(Tok::RParen));
        rewritten.extend(body);
        let stmt = parse(&rewritten).ok()?;
        match eval_stmt(&stmt, &[], ctx, bridge) {
            Ok(Some(num)) => Some(num.outcome()),
            _ => None,
        }
    };

    // find the base: `N is P% of|off|on what`
    if rest.len() >= 3 {
        let n = rest.len();
        let op = &rest[n - 2];
        let what = &rest[n - 1];
        let pct = &rest[..n - 2];
        let of_family = is_word(what, "what")
            && !pct.is_empty()
            && matches!(pct.last().map(|t| &t.tok), Some(Tok::Percent));
        if of_family {
            // each shape owns its whole, balanced body: `of` divides
            // by the percent; `off`/`on` divide by `(1 - P%)` /
            // `(1 + P%)`, riding the relative-percent rules
            let mut body: Vec<crate::tokens::Token> = vec![plain(Tok::Slash)];
            let matched = if is_word(op, "of") {
                true
            } else if is_word(op, "off") || is_word(op, "on") {
                body.push(plain(Tok::LParen));
                body.push(plain(Tok::Num(1.0)));
                body.push(plain(if is_word(op, "off") {
                    Tok::Minus
                } else {
                    Tok::Plus
                }));
                true
            } else {
                false
            };
            if matched {
                body.extend(group(pct));
                if !is_word(op, "of") {
                    body.push(plain(Tok::RParen));
                }
                return eval_wrap(body);
            }
        }
    }

    // find the percent: `N is what percent of M` (`what % of`)
    if rest.len() >= 4
        && is_word(&rest[0], "what")
        && (is_word(&rest[1], "percent") || matches!(rest[1].tok, Tok::Percent))
        && is_word(&rest[2], "of")
    {
        let mut body = vec![plain(Tok::Slash)];
        body.extend(group(&rest[3..]));
        return eval_wrap(body);
    }

    None
}

/// The unit fallback (spec.md "Unit expressions" routing): a scalar
/// failure on an unbound name the bridge knows - a unit, a period,
/// or a date word - hands the raw line to the bridge, which reads it
/// natively and answers with a quantity. Implicit multiplication
/// lets value-unit shapes parse on the scalar path now, so this
/// catches what the parse-failed route used to. Keyword- and
/// percent-carrying lines stay scalar (the routing rule). A bridge
/// refusal returns None so the caller's scalar funnel - mixed-line
/// silence included - stands untouched.
fn unit_line(
    raw: &str,
    toks: &[crate::tokens::Token],
    e: &QuireError,
    tags: &[String],
    ctx: &mut Ctx,
    bridge: &mut Option<Bridge>,
) -> Option<Outcome> {
    let ErrKind::UnknownName(name) = &e.kind else {
        return None;
    };
    let known = bridge
        .as_ref()
        .is_some_and(|b| b.knows_unit(name) || b.knows_function(name) || is_date_word(name));
    if !known
        || toks
            .iter()
            .any(|t| matches!(t.tok, Tok::Total | Tok::Answer | Tok::Of | Tok::Percent))
    {
        return None;
    }
    match bridge_eval(raw, toks, ctx, bridge) {
        Outcome::Quantity(v) => {
            record_tags(ctx, tags, Num::Q(v.clone()));
            Some(Outcome::Quantity(v))
        }
        // the bridge read the line and found a real unit error (a
        // dimension clash, a bad conversion): that message beats the
        // scalar path's "unknown name". Only an unknown-identifier
        // refusal falls back - the line carries a name the bridge
        // cannot seed, and the mixed skeleton keeps its shot
        Outcome::Failed(e) if !bridge_error_is_unknown(&e) => Some(Outcome::Failed(e)),
        _ => None,
    }
}

/// Whether a bridge failure means "a name the engine does not know"
/// (fall back to the scalar funnel) rather than a real unit error
/// (surface it). numbat words the former "Unknown identifier".
fn bridge_error_is_unknown(e: &QuireError) -> bool {
    matches!(&e.kind, ErrKind::Unit(message) if message.contains("nknown"))
}

/// Soulver-style date phrases translate to numbat datetime calls:
/// `3 weeks from today` is `today() + 3 weeks`. Case-insensitive
/// tails; anything unmatched returns the line unchanged.
fn translate_date_phrases(body: &str) -> String {
    let trimmed = body.trim();
    let lower = trimmed.to_lowercase();
    for (tail, template) in [
        (" from today", "today() + {expr}"),
        (" from now", "now() + {expr}"),
        (" ago", "today() - {expr}"),
    ] {
        if lower.strip_suffix(tail).is_some() {
            let head = &trimmed[..trimmed.len() - tail.len()];
            return template.replace("{expr}", head.trim());
        }
    }
    match lower.as_str() {
        "tomorrow" => "today() + 1 day".to_string(),
        "yesterday" => "today() - 1 day".to_string(),
        "today" => "today()".to_string(),
        "now" => "now()".to_string(),
        _ => trimmed.to_string(),
    }
}

/// Embedded date vocabulary becomes its function call: `today + 30
/// days` is `today() + 30 days` on the bridge. Word-boundary scan;
/// a word already followed by `(` is left alone, and this runs only
/// on the line body, never on the `let` seeds.
fn translate_date_words(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < source.len() {
        let c = source[i..].chars().next().unwrap();
        if c.is_ascii_alphabetic() || c == '_' {
            let end = source[i..]
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .map(|l| i + l)
                .unwrap_or(source.len());
            let word = &source[i..end];
            let called = source[end..].starts_with('(');
            let repl = match word.to_lowercase().as_str() {
                "today" if !called => Some("today()"),
                "now" if !called => Some("now()"),
                _ => None,
            };
            out.push_str(repl.unwrap_or(word));
            i = end;
        } else {
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// A line the scalar path declined: routing happens in order - a
/// blank &N ref answers nothing yet; the unit bridge takes unit and
/// date lines; the mixed-line skeleton takes prose with embedded
/// math; everything else keeps the original failure (spec: a missed
/// calculation beats a false error).
fn declined_line(
    raw: &str,
    toks: &[crate::tokens::Token],
    tags: &[String],
    ctx: &mut Ctx,
    bridge: &mut Option<Bridge>,
    fail: Fail,
) -> Option<Outcome> {
    if matches!(fail, Fail::Null) {
        // a blank &N ref: the line answers nothing yet
        return None;
    }
    let routable = !toks
        .iter()
        .any(|t| matches!(t.tok, Tok::Total | Tok::Answer | Tok::Of | Tok::Percent))
        && (toks.iter().any(
            |t| matches!(&t.tok, Tok::LineRef(n) if ctx.outcomes.contains_key(&(*n as usize))),
        ) || bridge.as_ref().is_some_and(|b| {
            toks.iter().any(|t| {
                matches!(&t.tok, Tok::Ident(n) if b.knows_unit(n) || b.knows_function(n) || is_date_word(n))
            })
        })
        // arithmetic over quantity-valued names (spec.md "Unit
        // expressions" routing): the names are bound, the bridge
        // seeds them - `leftover = paycheck - rent` over rate
        // variables routes on the touch alone
        || toks
            .iter()
            .any(|t| matches!(&t.tok, Tok::Ident(n) if matches!(ctx.vars.get(n), Some(Num::Q(_))))));
    if routable {
        return Some(match bridge_eval(raw, toks, ctx, bridge) {
            Outcome::Quantity(v) => {
                record_tags(ctx, tags, Num::Q(v.clone()));
                Outcome::Quantity(v)
            }
            other => other,
        });
    }
    match fail {
        // last resort: the mixed-line skeleton. Pure prose stays
        // silent; digit-led prose ("2 tickets to the show") can now
        // answer with its leading math
        Fail::Err(e) => Some(mixed_line(raw, ctx, bridge).unwrap_or(Outcome::Failed(e))),
        Fail::Reroute => Some(Outcome::Failed(QuireError::new(
            (0, 0),
            ErrKind::Unit(NO_UNIT_SUPPORT.into()),
        ))),
        Fail::Null => None,
    }
}

/// `$` glued before a digit is decoration (spec.md "Recurring
/// amounts"); the bridge reads raw source, so the compiled text
/// drops the byte. A `$` anywhere else stays: it is a genuine bad
/// character there, and the engine's own message is honest.
fn strip_dollar_decoration(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek().is_some_and(|n| n.is_ascii_digit()) {
            continue;
        }
        out.push(c);
    }
    out
}

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

    // `&N` refs translate to the referenced line's rendered value.
    // The `$` decoration strips first, and textually: a rendered
    // money rate (`1200 $/month`) may legitimately re-introduce a
    // `$` the engine reads natively, so the order is not cosmetic.
    let mut translated = strip_dollar_decoration(raw);
    let mut scan = 0usize;
    while let Some(at) = translated[scan..].find('&') {
        let at = scan + at;
        let digits: String = translated[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            scan = at + 1;
            continue;
        }
        let end = at + 1 + digits.len();
        let n: u32 = digits.parse().unwrap_or(u32::MAX);
        let replacement = match ctx.outcomes.get(&(n as usize)) {
            Some(Outcome::Value(v)) => Some(format!("{v}")),
            Some(Outcome::Quantity(v)) => Some(crate::units::render(v)),
            _ => None,
        };
        let Some(replacement) = replacement else {
            return Outcome::Failed(QuireError::new(
                (at, end),
                ErrKind::BadLineRef(format!("line {n} has no result")),
            ));
        };
        translated.replace_range(at..end, &replacement);
        scan = at + replacement.len();
    }
    let raw = &translated;

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
    // function definitions seed as numbat `fn`s, but ONLY the ones
    // the line calls (transitively): seeding everything would let a
    // single fn named after a numbat builtin poison every routed
    // line. Bodies must be pure arithmetic (no `total`/`answer`),
    // and numbat's dimension checking makes them work on quantities
    // for free (spec.md "Functions": engine scope)
    let mut needed: std::collections::BTreeSet<String> = toks
        .iter()
        .filter_map(|t| match &t.tok {
            Tok::Ident(n) if ctx.fns.contains_key(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    let mut expanded: std::collections::BTreeSet<String> = Default::default();
    while let Some(name) = needed.iter().next().cloned() {
        needed.remove(&name);
        if !expanded.insert(name.clone()) {
            continue;
        }
        if let Some(clauses) = ctx.fns.get(&name) {
            for (_, body) in clauses {
                collect_callees(body, &mut needed);
            }
        }
    }
    for name in &expanded {
        if let Some(clauses) = ctx.fns.get(name) {
            for (params, body) in clauses {
                if let Some(body_source) = numbat_source(body) {
                    source.push_str(&format!(
                        "fn {name}({}) = {body_source}\n",
                        params.join(", ")
                    ));
                }
            }
        }
    }
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
        Some((name, rest)) => source.push_str(&format!(
            "let {name} = {}\n{name}",
            translate_date_words(&translate_date_phrases(rest))
        )),
        None => source.push_str(&translate_date_words(&translate_date_phrases(raw))),
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

/// The names a function body calls, for transitive fn seeding.
fn collect_callees(expr: &Expr, out: &mut std::collections::BTreeSet<String>) {
    match expr {
        Expr::Call(name, args, _) => {
            out.insert(name.clone());
            for arg in args {
                collect_callees(arg, out);
            }
        }
        Expr::Neg(_, inner) | Expr::Pct(_, inner) => collect_callees(inner, out),
        Expr::Bin(_, l, r, _) => {
            collect_callees(l, out);
            collect_callees(r, out);
        }
        _ => {}
    }
}

/// Render a function body as numbat source, when it translates:
/// arithmetic, names, and calls do; `total` and `answer` do not
/// (their meaning is sheet-positional, numbat has none), and percent
/// does not either - a relative percent cannot ride the bridge, so
/// percent bodies stay scalar-only (spec.md "Functions").
fn numbat_source(expr: &Expr) -> Option<String> {
    Some(match expr {
        Expr::Num(n) => format!("{n}"),
        Expr::Name(n, _) => n.clone(),
        Expr::Total(_) | Expr::Answer(_) | Expr::Pct(..) | Expr::LineRef(..) => return None,
        Expr::Neg(_, inner) => format!("(-{})", numbat_source(inner)?),
        Expr::Bin(op, l, r, _) => format!(
            "({} {} {})",
            numbat_source(l)?,
            match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Pow => "^",
            },
            numbat_source(r)?
        ),
        Expr::Call(name, args, _) => format!(
            "{name}({})",
            args.iter()
                .map(numbat_source)
                .collect::<Option<Vec<_>>>()?
                .join(", ")
        ),
    })
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
            let entry = ctx.fns.entry(name.clone()).or_default();
            // a clause with the same param signature replaces its
            // predecessor (redefinition wins below, like variables)
            if let Some(slot) = entry.iter_mut().find(|(existing, _)| existing == params) {
                *slot = (params.clone(), body.clone());
            } else {
                entry.push((params.clone(), body.clone()));
            }
            Ok(None)
        }
        Stmt::Assign(name, _, e) => {
            let num = eval_expr(e, ctx, bridge)?;
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
            if ctx.watch == Some(ctx.line) {
                ctx.explain_steps
                    .push(format!("total = {}", num.outcome().render()));
            }
            Ok(Some(num))
        }
        Stmt::Expr(e) => {
            let num = eval_expr(e, ctx, bridge)?;
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

fn eval_expr(e: &Expr, ctx: &mut Ctx, bridge: &mut Option<Bridge>) -> Result<Num, Fail> {
    let num = match e {
        Expr::Num(n) => Num::S(*n),
        Expr::Call(name, args, span) => {
            let Some(clauses) = ctx.fns.get(name).cloned() else {
                return Err(Fail::Err(QuireError::new(
                    *span,
                    ErrKind::UnknownName(name.clone()),
                )));
            };
            ctx.depth += 1;
            if ctx.depth > 200 {
                ctx.depth -= 1;
                return Err(Fail::Err(QuireError::new(*span, ErrKind::NestTooDeep)));
            }
            // evaluate the arguments once
            let arg_values: Vec<Num> = args
                .iter()
                .map(|a| eval_expr(a, ctx, bridge))
                .collect::<Result<_, _>>()?;
            // clause selection: literal-param clauses (the param string
            // parses as a number and the arg matches it) before
            // variable-param clauses, in definition order
            let mut result = None;
            for (params, body) in &clauses {
                if params.len() != args.len() {
                    continue;
                }
                let literal_match =
                    params
                        .iter()
                        .zip(&arg_values)
                        .all(|(p, v)| match p.parse::<f64>() {
                            Ok(literal) => matches!(v, Num::S(x) if *x == literal),
                            Err(_) => true,
                        });
                if !literal_match {
                    continue;
                }
                // the call frame: params shadow sheet variables; the
                // body sees the sheet's variables, functions, and
                // answer, but a zero subtotal (`total` in a body reads
                // zero, spec)
                let mut frame = Ctx {
                    vars: ctx.vars.clone(),
                    fns: ctx.fns.clone(),
                    answer: ctx.answer.clone(),
                    subtotal: Sum::S(0.0),
                    tag_sums: Default::default(),
                    depth: ctx.depth,
                    steps: ctx.steps,
                    outcomes: ctx.outcomes.clone(),
                    line: ctx.line,
                    in_fn_body: true,
                    watch: ctx.watch,
                    explain_steps: std::mem::take(&mut ctx.explain_steps),
                };
                for (param, value) in params.iter().zip(&arg_values) {
                    if param.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        continue; // literal pattern: nothing to bind
                    }
                    frame.vars.insert(param.clone(), value.clone());
                }
                result = Some(eval_expr(body, &mut frame, bridge)?);
                // the body's own steps belong to the watched line too
                ctx.explain_steps.append(&mut frame.explain_steps);
                break;
            }
            ctx.depth -= 1;
            match result {
                Some(n) => n,
                None => {
                    // no clause matched on arity: report the mismatch
                    let expected = clauses.first().map(|(p, _)| p.len());
                    let kind = expected.map_or(ErrKind::UnknownName(name.clone()), |arity| {
                        ErrKind::CallArity {
                            name: name.clone(),
                            expected: arity,
                            got: args.len(),
                        }
                    });
                    return Err(Fail::Err(QuireError::new(*span, kind)));
                }
            }
        }
        Expr::Name(n, span) => ctx
            .vars
            .get(n)
            .cloned()
            .ok_or_else(|| QuireError::new(*span, ErrKind::UnknownName(n.clone())))?,
        Expr::LineRef(n, span) => {
            if ctx.in_fn_body {
                // sheet-positional like `total` (spec.md "Line
                // references"): a body cannot hold a ref, or a line
                // shift would silently re-aim it
                return Err(Fail::Err(QuireError::new(
                    *span,
                    ErrKind::BadLineRef("function bodies cannot use line references".into()),
                )));
            }
            if (*n as usize) == ctx.line {
                return Err(Fail::Err(QuireError::new(
                    *span,
                    ErrKind::BadLineRef(format!(
                        "line {n} is this line; a reference must point elsewhere"
                    )),
                )));
            }
            match ctx.outcomes.get(&(*n as usize)) {
                Some(Outcome::Value(v)) => Num::S(*v),
                Some(Outcome::Quantity(v)) => Num::Q(v.clone()),
                Some(Outcome::Failed(e)) => return Err(Fail::Err(e.clone())),
                None => return Err(Fail::Null),
            }
        }
        // a quantity subtotal finishes through the bridge wherever
        // `total` appears - in an expression as much as on its own
        // line (spec: a total that includes quantity results sums
        // them dimension-safely)
        Expr::Total(_) => match &ctx.subtotal {
            Sum::S(s) => Num::S(*s),
            Sum::Q(_) => finish_sum(&ctx.subtotal, bridge)?,
        },
        Expr::Answer(span) => ctx
            .answer
            .clone()
            .ok_or_else(|| QuireError::new(*span, ErrKind::NoAnswer))?,
        Expr::Neg(span, inner) => {
            let Num::S(v) = eval_expr(inner, ctx, bridge)? else {
                return Err(Fail::Reroute);
            };
            Num::S(finite(-v, *span)?)
        }
        // standalone percent is x / 100
        Expr::Pct(span, inner) => {
            let Num::S(v) = eval_expr(inner, ctx, bridge)? else {
                return Err(Fail::Reroute);
            };
            Num::S(finite(v / 100.0, *span)?)
        }
        Expr::Bin(op, l, r, span) => {
            let (Num::S(a), Num::S(b)) = (eval_expr(l, ctx, bridge)?, eval_expr(r, ctx, bridge)?)
            else {
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
            if ctx.watch == Some(ctx.line) {
                // the breakdown popover: one readable step per
                // operation, bottom-up
                let fmt = |v: f64| format_number(v);
                let step = match (op, is_pct(r)) {
                    (BinOp::Add, true) => format!(
                        "{} + {}% (of {}) = {}",
                        fmt(a),
                        fmt(b * 100.0),
                        fmt(a),
                        fmt(v)
                    ),
                    (BinOp::Sub, true) => format!(
                        "{} - {}% (of {}) = {}",
                        fmt(a),
                        fmt(b * 100.0),
                        fmt(a),
                        fmt(v)
                    ),
                    _ => {
                        let sym = match op {
                            BinOp::Add => "+",
                            BinOp::Sub => "-",
                            BinOp::Mul => "*",
                            BinOp::Div => "/",
                            BinOp::Pow => "^",
                        };
                        format!("{} {sym} {} = {}", fmt(a), fmt(b), fmt(v))
                    }
                };
                ctx.explain_steps.push(step);
            }
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
        // `2 3` multiplies now (implicit); a stray closer trails
        assert_eq!(err_of("2 )"), ErrKind::TrailingTokens);
        assert_eq!(err_of("()"), ErrKind::EmptyParens);
        // `$5` is decoration now (see dollar_prefix_is_decoration);
        // the error table keeps a stray-`$` case
        assert_eq!(err_of("$ rate"), ErrKind::BadChar('$'));
        assert_eq!(err_of("1.2.3"), ErrKind::BadNumber);
        assert_eq!(err_of("2^10000"), ErrKind::OutOfRange);
        assert_eq!(err_of("total = 5"), ErrKind::AssignToKeyword("total"));
    }

    #[test]
    fn dollar_prefix_is_decoration() {
        assert_eq!(show("$5.60 * 3"), "16.8");
        assert_eq!(show("$1200"), "1,200");
        // a `$` anywhere but before a digit stays a bad character
        assert_eq!(err_of("$ rate"), ErrKind::BadChar('$'));
        assert_eq!(err_of("$"), ErrKind::BadChar('$'));
    }

    #[test]
    fn recurrence_phrases_answer_as_rates() {
        // the written period stays in the answer (spec.md "Recurring
        // amounts"); the engine's year prints as its short alias
        assert_eq!(show("$1200/month"), "1200 /month");
        assert_eq!(show("950 / month"), "950 /month");
        assert_eq!(show("1200/months"), "1200 /month");
        assert_eq!(show("45/week"), "45 /week");
        assert_eq!(show("12/day"), "12 /day");
        assert_eq!(show("60/quarter"), "60 /quarter");
        assert_eq!(show("5000/year"), "5000 /yr");
    }

    #[test]
    fn rates_add_across_periods() {
        assert_eq!(show("950/month + 50/month"), "1000 /month");
        // the sum displays in the largest period involved
        assert_eq!(show("950/month + 365/year"), "11765 /yr");
    }

    #[test]
    fn per_day_is_an_explicit_conversion() {
        assert_eq!(show("1200/month -> 1/day"), "39.4259 /day");
    }

    #[test]
    fn rate_times_duration_is_a_plain_amount() {
        assert_eq!(show("950/month * 12 months"), "11400");
        // a bare multiplier leaves the rate a rate
        assert_eq!(show("950/month * 12"), "11400 /month");
    }

    #[test]
    fn rates_bind_tag_total_and_reference() {
        let sheet = "\
rent = 950/month @fixed
internet = 45/month @fixed
total @fixed
&3 * 2
";
        let lines = evaluate_sheet(sheet);
        let values: Vec<_> = lines
            .iter()
            .map(|l| l.outcome.as_ref().map(|o| o.render()))
            .collect();
        assert_eq!(
            values,
            vec![
                Some("950 /month".to_string()),
                Some("45 /month".to_string()),
                Some("995 /month".to_string()),
                Some("1990 /month".to_string()),
            ]
        );
    }

    #[test]
    fn rates_compose_with_stamps() {
        let sheet = "rent = $1200/month @ 2026-10-01 @rent\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[0].outcome.as_ref().unwrap().render(), "1200 /month");
    }

    #[test]
    fn scalar_in_a_rate_total_fails_loudly() {
        let sheet = "\
rent = 950/month
milk = 3.5
total
";
        let lines = evaluate_sheet(sheet);
        let text = lines[2].outcome.as_ref().unwrap().render();
        // the engine words bare-rate mismatches as a failed
        // constraint (money rates word them left/right hand side);
        // both name the dimension and fail this line only
        assert!(text.contains("Time"), "{text}");
    }

    #[test]
    fn bound_period_name_wins_over_the_phrase() {
        let sheet = "month = 12\n950/month\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[1].outcome.as_ref().unwrap().render(), "79.1666666667");
    }

    #[test]
    fn phrase_silence_survives_a_bridge_refusal() {
        // the bridge cannot seed the unknown, so today's scalar
        // funnel stands: the line keeps its unknown-month error,
        // exactly as before the hook existed
        assert_eq!(
            err_of("950/month + shipping"),
            ErrKind::UnknownName("month".into())
        );
    }

    #[test]
    fn keyword_and_nonslash_lines_never_route_to_rates() {
        // Quire keywords keep the scalar funnel (the routing rule)
        assert_eq!(
            err_of("total / month"),
            ErrKind::UnknownName("month".into())
        );
        // no slash, no phrase: a bare period word reads as the unit
        // it is, and the bridge's dimension clash names the problem
        // (with a suggested fix) instead of "unknown name"
        match err_of("(month) + 1") {
            ErrKind::Unit(message) => assert!(message.contains("Time"), "{message}"),
            other => panic!("expected a unit error, got {other:?}"),
        };
    }

    #[test]
    fn sentences_carry_units_through_the_engine() {
        // one number leaning on its unit, inside a sentence
        assert_eq!(show("the stock measures 2 mol/L"), "2 molar");
        assert_eq!(show("the sample weighs 0.101 g"), "0.101 g");
        // sentence punctuation is prose glue
        assert_eq!(show("the sample weighs 0.101 g."), "0.101 g");
        // two numbers never multiply by accident
        let lines = evaluate_sheet("we ran it at 300 K, then 310 K\n");
        assert!(lines[0].outcome.is_none());
        // a bound name next to anything stays prose (the 5-milk rule)
        let sheet = "milk = 3.50\nwe bought 5 milk\n";
        assert!(evaluate_sheet(sheet).last().unwrap().outcome.is_none());
        // failures stay silent
        assert!(
            evaluate_sheet("the sqrt of missing_thing\n")[0]
                .outcome
                .is_none()
        );
    }

    #[test]
    fn dollar_amounts_demote_in_prose() {
        // the mixed-line rule, now seeing `$` amounts too
        assert_eq!(show("I paid $5 for milk"), "5");
    }

    #[test]
    fn arithmetic_over_quantity_variables_routes() {
        // rate variables carry no unit word, and the arithmetic must
        // route anyway (the budget template's leftover line)
        let sheet = "\
paycheck = 2400/month
rent = 950/month
leftover = paycheck - rent
";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[2].outcome.as_ref().unwrap().render(), "1450 /month");
    }

    #[test]
    fn unit_variables_compute_without_unit_words() {
        // the same routing gap, pre-recurrence: `bag * 2` used to
        // die on "units cannot mix with..." because no unit word
        // appeared in the line
        let sheet = "bag = 5 kg\nbag * 2\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[1].outcome.as_ref().unwrap().render(), "10 kg");
    }

    #[test]
    fn reverse_percent_phrases() {
        // find the base: the divisor rides the relative-percent
        // rules for off and on
        assert_eq!(show("20 is 10% of what"), "200");
        assert_eq!(show("180 is 10% off what"), "200");
        assert_eq!(show("220 is 10% on what"), "200");
        assert_eq!(show("41 is 17% on what"), "35.0427350427");
        // find the percent: the fraction (a percent is its fraction)
        assert_eq!(show("30 is what percent of 200"), "0.15");
        assert_eq!(show("20 is what % of 200"), "0.1");
        assert_eq!(show("20 IS WHAT Percent OF 80"), "0.25");
    }

    #[test]
    fn reverse_percents_admit_expressions_and_compose() {
        // the value side may be an expression
        assert_eq!(show("2 * 20 is 10% of what"), "400");
        // the answer composes like any value
        let sheet = "\
30 is what percent of 200
answer * 200
";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[1].outcome.as_ref().unwrap().render(), "30");
        // a variable may carry the question's value
        let sheet = "budget = 90\nbudget is 10% off what\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[1].outcome.as_ref().unwrap().render(), "100");
    }

    #[test]
    fn phrase_misses_keep_the_old_paths() {
        // a near-miss on an expression line keeps its honest error
        // (the first unknown name, now that the line parses)
        assert_eq!(
            err_of("20 is 10% of somewhere"),
            ErrKind::UnknownName("is".into())
        );
        // prose that merely contains `is` stays prose
        let lines = evaluate_sheet("rent is due soon\n");
        assert!(lines[0].outcome.is_none());
    }

    #[test]
    fn implicit_multiplication_evaluates() {
        assert_eq!(show("3(4 + 5)"), "27");
        assert_eq!(show("2 3"), "6");
        assert_eq!(show("(1 + 2)(3 + 4)"), "21");
        assert_eq!(show("2 -3"), "-1");
        let sheet = "x = 7\n2x\n1/2x\n2^3x\n";
        let lines = evaluate_sheet(sheet);
        let values: Vec<_> = lines[1..]
            .iter()
            .map(|l| l.outcome.as_ref().unwrap().render())
            .collect();
        assert_eq!(values, vec!["14", "3.5", "56"]);
        // value-unit shapes route through the engine as before
        assert_eq!(show("5 kg + 300 g"), "5300 g");
    }

    #[test]
    fn implicit_mult_keeps_calls_and_prose() {
        let sheet = "double(x) = x * 2\ndouble(21)\ndouble(21)(2)\n";
        let lines = evaluate_sheet(sheet);
        // the definition line: no cell
        assert!(lines[0].outcome.is_none());
        assert_eq!(lines[1].outcome.as_ref().unwrap().render(), "42");
        assert_eq!(lines[2].outcome.as_ref().unwrap().render(), "84");
        // a prose remnant with a bound name stays silent (the mixed
        // skeleton parses strict)
        let sheet = "milk = 3.50\nI paid $5 for milk\n";
        let lines = evaluate_sheet(sheet);
        assert!(lines[1].outcome.is_none());
    }

    #[test]
    fn percent_bodies_stay_scalar_only() {
        // the scalar path reads the relative percent as always
        let sheet = "p(x) = x + 10%\np(2)\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[1].outcome.as_ref().unwrap().render(), "2.2");
        // the unit path refuses the body (spec.md "Functions"): a
        // relative percent cannot ride the bridge, so seeding one
        // would silently change its meaning
        let sheet = "p(x) = x + 10%\np(2 kg)\n";
        let lines = evaluate_sheet(sheet);
        assert!(matches!(
            &lines[1].outcome,
            Some(Outcome::Failed(e)) if matches!(e.kind, ErrKind::Unit(_) | ErrKind::UnknownName(_))
        ));
    }

    #[test]
    fn line_refs_are_refused_in_function_bodies() {
        // sheet-positional like `total` (spec.md "Line references"):
        // a body ref would silently re-aim when lines shift
        let sheet = "base = 10\nf(x) = &1 + x\nf(5)\n";
        let lines = evaluate_sheet(sheet);
        assert!(matches!(
            &lines[2].outcome,
            Some(Outcome::Failed(e))
                if e.kind.to_string().contains("function bodies cannot use line references")
        ));
    }

    #[test]
    fn math_functions_route_to_the_engine() {
        assert_eq!(show("sqrt(144)"), "12");
        assert_eq!(show("log10(1000)"), "3");
        assert_eq!(show("2 * abs(-3)"), "6");
        assert_eq!(show("sin(30 deg)"), "0.5");
        // unknown functions still demote in mixed lines
        assert_eq!(show("foo(2)"), "2");
    }

    #[test]
    fn embedded_date_words_call_the_functions() {
        // `today + 30 days` composes like the phrase forms
        let lines = evaluate_sheet("leave = today + 30 days\n");
        assert!(matches!(&lines[0].outcome, Some(Outcome::Quantity(_))));
        let lines = evaluate_sheet("gap = now() - today\n");
        assert!(matches!(&lines[0].outcome, Some(Outcome::Quantity(_))));
    }

    #[test]
    fn quantity_totals_finish_inside_expressions() {
        // a quantity subtotal finishes through the bridge wherever
        // `total` appears - in an assignment as much as on its own
        // line (spec: totals sum dimension-safely)
        let sheet = "rent = 950/month\nfood = 320/month\nwhole = total\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[2].outcome.as_ref().unwrap().render(), "1270 /month");
        // the trip shape: money amounts, then the per-day split
        // (a rate divided by a duration is honestly month⁻², not a
        // per-day figure - only amounts split per day)
        let sheet = "rent = 950 EUR\nfood = 320 EUR\nwhole = total\nper_day = whole / 30 days\n";
        let lines = evaluate_sheet(sheet);
        assert_eq!(lines[2].outcome.as_ref().unwrap().render(), "1270 €");
        assert_eq!(lines[3].outcome.as_ref().unwrap().render(), "42.3333 €/day");
    }

    #[test]
    fn explain_sheet_line_walks_bottom_up() {
        let text = "rent = 950\n3 * 4 + rent / 2\n";
        // the operations record in evaluation order, bottom-up
        assert_eq!(
            explain_sheet_line(text, 2),
            Some(vec![
                "3 * 4 = 12".to_string(),
                "950 / 2 = 475".to_string(),
                "12 + 475 = 487".to_string(),
            ])
        );
        // relative percent reads in the sheet's words
        let text = "200 + 15%\n";
        assert_eq!(
            explain_sheet_line(text, 1),
            Some(vec!["200 + 15% (of 200) = 230".to_string()])
        );
        // a total line explains as one step
        let text = "5\n10\ntotal\n";
        assert_eq!(
            explain_sheet_line(text, 3),
            Some(vec!["total = 15".to_string()])
        );
        // quantities, prose, and out-of-range lines explain as None
        assert_eq!(explain_sheet_line("5 kg + 300 g\n", 1), None);
        assert_eq!(explain_sheet_line("just words\n", 1), None);
        assert_eq!(explain_sheet_line("2 + 2\n", 9), None);
    }

    #[test]
    fn unknown_names_in_expressions_demote_to_prose() {
        // classify-by-failure: the math answers, the words demote
        assert_eq!(show("2 bloognorch"), "2");
        assert_eq!(show("2 tickets to the show"), "2");
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
        // (unknown-name lines now fall to the mixed skeleton, so the
        // pure-error case is a bare unbound reference)
        assert_eq!(show("2 + 2"), "4");
        assert_eq!(
            err_of("bloognorch"),
            ErrKind::UnknownName("bloognorch".into())
        );
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
    fn date_phrases_translate_and_answer() {
        let sheet = "tomorrow\n3 weeks from today\n90 minutes from now\n6 months ago\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        // every phrase answers a datetime render (dates vary with
        // the clock; pin the SHAPE, not the value)
        assert_eq!(rendered.len(), 4, "{rendered:?}");
        for text in &rendered {
            assert!(
                text.starts_with("20") && text.len() >= 16,
                "not a datetime render: {text:?} in {rendered:?}"
            );
        }
    }

    #[test]
    fn unit_lines_call_user_functions() {
        // `twice`, not `double`: numbat's prelude claims `double` as
        // a constant, and the spec's sharp edge applies to function
        // names too
        let sheet = "twice(x) = x * 2\ntwice(2 kg)\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["4 kg".to_string()]);
    }

    #[test]
    fn unit_lines_see_mid_sheet_redefinitions() {
        // the redefinition is dimension-sound (x + 1 would refuse a
        // Mass argument - numbat checks bridged functions)
        let sheet = "twice(x) = x * 2\ntwice(2 kg)\ntwice(x) = x * 3\ntwice(2 kg)\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["4 kg".to_string(), "6 kg".to_string()]);
    }

    #[test]
    fn line_refs_resolve_above_only() {
        let sheet = "5 + 5\n&1 * 3\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["10".to_string(), "30".to_string()]);
    }

    #[test]
    fn mixed_lines_evaluate_the_math_and_stay_silent_on_failure() {
        // word operators map; prose drops; the answer joins the sheet
        let sheet = "50 apples at 3 each\n2 coffees plus 1 tea\n6 apples minus 1 apple\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(
            rendered,
            vec!["150".to_string(), "3".to_string(), "5".to_string()]
        );
        // pure prose stays prose: no numbers, no cell
        let sheet = "this line is only words\n";
        assert!(
            evaluate_sheet(sheet)
                .into_iter()
                .all(|l| l.outcome.is_none())
        );
        // pure prose stays silent, never an error cell
        let sheet = "apples and pears\n";
        assert!(
            evaluate_sheet(sheet)
                .into_iter()
                .all(|l| l.outcome.is_none())
        );
        // a dangling word operator fails the line: never a silent
        // guess (the honest error surfaces from the first unknown
        // name now that implicit multiplication parses the line)
        let lines = evaluate_sheet("3 pears plus\n");
        assert!(matches!(
            &lines[0].outcome,
            Some(Outcome::Failed(e)) if matches!(e.kind, ErrKind::UnknownName(_))
        ));
    }

    #[test]
    fn forward_refs_answer_blank_until_the_target_exists() {
        // a ref to a line below is null: the referencing line shows
        // no cell until the target answers
        let sheet = "&2 + 1\n10\n";
        let lines = evaluate_sheet(sheet);
        // line 1: blank (target not yet answered)
        assert_eq!(lines[0].outcome, None);
        // line 2: 10, and now line 1 can see it
        assert_eq!(
            lines[1].outcome.as_ref().map(|o| o.render()),
            Some("10".to_string())
        );
    }

    #[test]
    fn line_refs_poison_and_bridge() {
        // a failed referenced line poisons the referencing line
        let sheet = "1 / 0\n&1 + 1\n";
        let lines = evaluate_sheet(sheet);
        assert!(matches!(
            &lines[1].outcome,
            Some(Outcome::Failed(e)) if matches!(e.kind, ErrKind::DivideByZero)
        ));
        // quantities resolve through the bridge
        let sheet = "2 kg\n&1 * 3\n";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["2 kg".to_string(), "6 kg".to_string()]);
    }

    #[test]
    fn recursive_functions_with_literal_clauses() {
        // literal clauses match before general ones; recursion is
        // depth-capped so a missing base case fails instead of hanging
        let sheet = "\
fact(0) = 1
fact(n) = n * fact(n - 1)
fact(5)
";
        let rendered: Vec<_> = evaluate_sheet(sheet)
            .into_iter()
            .filter_map(|l| l.outcome.map(|o| o.render()))
            .collect();
        assert_eq!(rendered, vec!["120".to_string()]);
    }

    #[test]
    fn runaway_computation_hits_the_budget() {
        // exponential recursion: without memoisation this would burn
        // CPU indefinitely; the budget stops it
        let sheet = "\
fib(0) = 0
fib(1) = 1
fib(n) = fib(n - 1) + fib(n - 2)
fib(30)
";
        // the point is proving evaluate_sheet terminates (the budget
        // consumed ~1M steps) rather than hanging forever
        let _lines = evaluate_sheet(sheet);
    }

    #[test]
    fn normal_sheets_stay_well_under_the_budget() {
        let sheet = "\
a = 10
b = a * 2
c = b + a
total @nothing
f(x) = x * 2
f(c)
5 kg + 300 g
26.2 miles -> km
";
        let lines = evaluate_sheet(sheet);
        assert!(lines.iter().all(|l| {
            l.outcome
                .as_ref()
                .is_none_or(|o| !matches!(o, Outcome::Failed(_)))
        }));
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

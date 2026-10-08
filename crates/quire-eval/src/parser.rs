//! Parser: tokens into a statement AST, recursive descent with one
//! function per precedence level (kalker's shape).
//!
//! Ladder, loosest to tightest: `+ -` over `* / of` over
//! right-associative `^` over unary minus over postfix `%` over
//! primary. `^` binding tighter than unary minus is what makes
//! `-2^2` equal `-(2^2)` per spec.

use crate::error::{ErrKind, QuireError, Span, describe};
use crate::tokens::{Tok, Token};

/// Parentheses and unary chains may nest this deep, no deeper; deep
/// enough for any honest sheet, shallow enough to never overflow the
/// stack on adversarial input.
pub const MAX_DEPTH: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    /// A variable reference (`answer` and `total` have their own
    /// variants; the tokenizer keywords them out of `Ident`).
    Name(String, Span),
    /// The subtotal: sums results since the last `total` or heading.
    Total(Span),
    /// The most recent expression result above this line.
    Answer(Span),
    Neg(Span, Box<Expr>),
    /// Postfix percent. Standalone it is `x / 100`; under `+`/`-` the
    /// evaluator reads it relatively (spec.md, Semantics).
    Pct(Span, Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>, Span),
    /// `name(arg, ...)`. Functions are defined by statement form
    /// `FnDef` (spec.md "Functions").
    Call(String, Vec<Expr>, Span),
    /// `&N`: the result of sheet line N (spec.md "Line references").
    LineRef(u32, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assign(String, Span, Expr),
    /// `name(a, b) = expression`: the inline function definition.
    FnDef(String, Vec<String>, Expr, Span),
    Expr(Expr),
}

pub fn parse(tokens: &[Token]) -> Result<Stmt, QuireError> {
    parse_inner(tokens, true)
}

/// Parse with implicit multiplication OFF: the mixed-line
/// skeleton's grammar (spec.md "Operators"), so a prose remnant
/// like `5 milk` with `milk` bound stays prose. Everything else is
/// identical.
pub fn parse_strict(tokens: &[Token]) -> Result<Stmt, QuireError> {
    parse_inner(tokens, false)
}

fn parse_inner(tokens: &[Token], implicit: bool) -> Result<Stmt, QuireError> {
    if tokens.is_empty() {
        return Err(QuireError::new(
            (0, 0),
            ErrKind::UnexpectedEol {
                expected: "an expression",
            },
        ));
    }
    // `name(params) = body`: scan ahead for the parameter list's
    // closing paren followed by the single `=`. A call statement
    // (`double(21)`) has no `=` and falls through to the expression.
    if let (Tok::Ident(name), Some(Tok::LParen)) = (&tokens[0].tok, tokens.get(1).map(|t| &t.tok)) {
        let mut depth = 0usize;
        let mut close = None;
        for (i, t) in tokens.iter().enumerate().skip(1) {
            match t.tok {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(close) = close
            && tokens.get(close + 1).map(|t| &t.tok) == Some(&Tok::Equals)
            && tokens.get(close + 2).is_some()
        {
            if name == "total" || name == "answer" {
                let keyword: &'static str = if name == "total" { "total" } else { "answer" };
                return Err(QuireError::new(
                    tokens[0].span,
                    ErrKind::AssignToKeyword(keyword),
                ));
            }
            let mut params = Vec::new();
            for t in &tokens[2..close] {
                let p = match &t.tok {
                    Tok::Ident(p) => p.clone(),
                    // numeric literals are pattern params: `fact(0) = 1`
                    Tok::Num(v) => {
                        let text = format!("{v}");
                        if params.contains(&text) {
                            return Err(QuireError::new(t.span, ErrKind::DuplicateParam(text)));
                        }
                        params.push(text.clone());
                        continue;
                    }
                    _ => continue,
                };
                if params.contains(&p) {
                    return Err(QuireError::new(t.span, ErrKind::DuplicateParam(p)));
                }
                params.push(p);
            }
            let mut p = P {
                toks: tokens,
                pos: close + 2,
                depth: 0,
                implicit,
            };
            let body = p.expr()?;
            p.expect_end()?;
            return Ok(Stmt::FnDef(name.clone(), params, body, tokens[0].span));
        }
    }
    match (&tokens[0].tok, tokens.get(1).map(|t| &t.tok)) {
        (Tok::Ident(name), Some(Tok::Equals)) => {
            let (name, span) = (name.clone(), tokens[0].span);
            let mut p = P {
                toks: tokens,
                pos: 2,
                depth: 0,
                implicit,
            };
            let e = p.expr()?;
            p.expect_end()?;
            Ok(Stmt::Assign(name, span, e))
        }
        (Tok::Total, Some(Tok::Equals)) => Err(QuireError::new(
            tokens[0].span,
            ErrKind::AssignToKeyword("total"),
        )),
        (Tok::Answer, Some(Tok::Equals)) => Err(QuireError::new(
            tokens[0].span,
            ErrKind::AssignToKeyword("answer"),
        )),
        _ => {
            let mut p = P {
                toks: tokens,
                pos: 0,
                depth: 0,
                implicit,
            };
            let e = p.expr()?;
            p.expect_end()?;
            Ok(Stmt::Expr(e))
        }
    }
}

struct P<'t> {
    toks: &'t [Token],
    pos: usize,
    depth: u32,
    /// implicit multiplication lives on the expression path only:
    /// the mixed-line skeleton parses with it off so prose
    /// remnants keep today's grammar (spec.md "Operators")
    implicit: bool,
}

type R = Result<Expr, QuireError>;

impl<'t> P<'t> {
    fn peek(&self) -> Option<&'t Token> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<&'t Token> {
        let t = self.toks.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn enter(&mut self, span: Span) -> Result<(), QuireError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(QuireError::new(span, ErrKind::NestTooDeep));
        }
        Ok(())
    }

    fn expr(&mut self) -> R {
        let mut lhs = self.term()?;
        while let Some(t) = self.peek() {
            let op = match t.tok {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            let span = t.span;
            self.pos += 1;
            let rhs = self.term()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    fn term(&mut self) -> R {
        let mut lhs = self.unary()?;
        while let Some(t) = self.peek() {
            let (op, consume) = match t.tok {
                Tok::Star => (BinOp::Mul, true),
                Tok::Slash => (BinOp::Div, true),
                Tok::Of => (BinOp::Mul, true),
                // implicit multiplication (kalker's rule): a number,
                // bare name, or `(` directly after a value multiplies
                // at `*` precedence, left-associative (`1/2pi` is
                // `(1/2)*pi`). The token is not consumed; `name(`
                // still reaches primary's call branch first.
                Tok::Num(_) | Tok::Ident(_) | Tok::LParen if self.implicit => (BinOp::Mul, false),
                _ => break,
            };
            let span = t.span;
            if consume {
                self.pos += 1;
            }
            let rhs = self.unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs), span);
        }
        Ok(lhs)
    }

    /// `^` binds tighter than unary minus (spec: `-2^2` is `-(2^2)`),
    /// so the power level sits *below* unary and takes a `postfix`
    /// base. Its right side re-enters at `unary`, which makes `^`
    /// right-associative and lets `2^-3` parse.
    fn power(&mut self) -> R {
        let base = self.postfix()?;
        if let Some(t) = self.peek()
            && t.tok == Tok::Caret
        {
            let span = t.span;
            self.pos += 1;
            self.enter(span)?;
            let exp = self.unary();
            self.depth -= 1;
            let exp = exp?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exp), span));
        }
        Ok(base)
    }

    fn unary(&mut self) -> R {
        if let Some(t) = self.peek()
            && t.tok == Tok::Minus
        {
            let span = t.span;
            self.pos += 1;
            self.enter(span)?;
            let inner = self.unary();
            self.depth -= 1;
            return inner.map(|e| Expr::Neg(span, Box::new(e)));
        }
        self.power()
    }

    fn postfix(&mut self) -> R {
        let mut e = self.primary()?;
        while let Some(t) = self.peek() {
            if t.tok == Tok::Percent {
                let span = t.span;
                self.pos += 1;
                e = Expr::Pct(span, Box::new(e));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn primary(&mut self) -> R {
        let Some(t) = self.bump() else {
            return Err(QuireError::new(
                self.line_end(),
                ErrKind::UnexpectedEol {
                    expected: "a value",
                },
            ));
        };
        match &t.tok {
            Tok::Num(v) => Ok(Expr::Num(*v)),
            Tok::LineRef(n) => Ok(Expr::LineRef(*n, t.span)),
            Tok::Ident(n) if self.peek().map(|p| p.tok.clone()) == Some(Tok::LParen) => {
                // a call: consume the paren and the argument list
                let name = n.clone();
                let open = self.bump().unwrap().span;
                self.enter(open)?;
                let mut args = Vec::new();
                if let Some(closing) = self.peek()
                    && closing.tok == Tok::RParen
                {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Expr::Call(name, args, (t.span.0, closing.span.1)));
                }
                loop {
                    args.push(self.expr()?);
                    match self.peek().map(|p| p.tok.clone()) {
                        Some(Tok::Comma) => {
                            self.pos += 1;
                        }
                        Some(Tok::RParen) => {
                            let closing = self.bump().unwrap();
                            self.depth -= 1;
                            return Ok(Expr::Call(name, args, (t.span.0, closing.span.1)));
                        }
                        Some(other) => {
                            return Err(QuireError::new(
                                self.peek().unwrap().span,
                                ErrKind::UnexpectedTok {
                                    expected: "`,` or `)`",
                                    got: describe(&other),
                                },
                            ));
                        }
                        None => {
                            return Err(QuireError::new(
                                self.line_end(),
                                ErrKind::UnexpectedEol { expected: "`)`" },
                            ));
                        }
                    }
                }
            }
            Tok::Ident(n) => Ok(Expr::Name(n.clone(), t.span)),
            Tok::Total => Ok(Expr::Total(t.span)),
            Tok::Answer => Ok(Expr::Answer(t.span)),
            Tok::LParen => {
                if let Some(closing) = self.peek()
                    && closing.tok == Tok::RParen
                {
                    return Err(QuireError::new(
                        (t.span.0, closing.span.1),
                        ErrKind::EmptyParens,
                    ));
                }
                self.enter(t.span)?;
                let inner = self.expr();
                self.depth -= 1;
                let e = inner?;
                match self.peek() {
                    Some(closing) if closing.tok == Tok::RParen => {
                        self.pos += 1;
                        Ok(e)
                    }
                    Some(_) => Err(QuireError::new(
                        self.peek().unwrap().span,
                        ErrKind::UnexpectedTok {
                            expected: "`)`",
                            got: describe(&self.peek().unwrap().tok),
                        },
                    )),
                    None => Err(QuireError::new(
                        self.line_end(),
                        ErrKind::UnexpectedEol { expected: "`)`" },
                    )),
                }
            }
            other => Err(QuireError::new(
                t.span,
                ErrKind::UnexpectedTok {
                    expected: "a value",
                    got: describe(other),
                },
            )),
        }
    }

    fn expect_end(&self) -> Result<(), QuireError> {
        match self.peek() {
            None => Ok(()),
            Some(t) => Err(QuireError::new(t.span, ErrKind::TrailingTokens)),
        }
    }

    fn line_end(&self) -> Span {
        self.toks
            .last()
            .map(|t| (t.span.1, t.span.1))
            .unwrap_or((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::tokenize;

    fn expr(src: &str) -> Expr {
        let toks = tokenize(src).expect("tokenizes");
        match parse(&toks).expect("parses") {
            Stmt::Expr(e) => e,
            Stmt::Assign(..) | Stmt::FnDef(..) => panic!("unexpected statement"),
        }
    }

    #[test]
    fn precedence_and_associativity() {
        // -2^2 is -(2^2) per spec
        assert!(matches!(expr("-2^2"), Expr::Neg(_, e)
            if matches!(*e, Expr::Bin(BinOp::Pow, ..))));
        // 2^3^2 is right-associative
        assert!(matches!(expr("2^3^2"), Expr::Bin(BinOp::Pow, _, e, _)
            if matches!(*e, Expr::Bin(BinOp::Pow, ..))));
        // * binds tighter than +
        assert!(matches!(expr("1 + 2 * 3"), Expr::Bin(BinOp::Add, _, e, _)
            if matches!(*e, Expr::Bin(BinOp::Mul, ..))));
        // of is multiplication
        assert!(matches!(expr("15% of 200"), Expr::Bin(BinOp::Mul, l, _, _)
            if matches!(*l, Expr::Pct(..))));
        // postfix percent chains
        assert!(matches!(expr("5%%"), Expr::Pct(_, e)
            if matches!(*e, Expr::Pct(..))));
    }

    #[test]
    fn assignment_shape() {
        let toks = tokenize("milk = 3.50").expect("tokenizes");
        assert!(matches!(parse(&toks), Ok(Stmt::Assign(name, _, _)) if name == "milk"));
        // keywords refuse assignment
        let toks = tokenize("total = 5").expect("tokenizes");
        assert!(matches!(parse(&toks), Err(e) if e.kind == ErrKind::AssignToKeyword("total")));
        let toks = tokenize("answer = 5").expect("tokenizes");
        assert!(matches!(parse(&toks), Err(e) if e.kind == ErrKind::AssignToKeyword("answer")));
    }

    #[test]
    fn error_positions() {
        // trailing junk carries its own span (`2 3` multiplies now;
        // a stray closer still trails)
        let toks = tokenize("2 )").expect("tokenizes");
        assert!(
            matches!(parse(&toks), Err(e) if e.kind == ErrKind::TrailingTokens && e.span == (2, 3))
        );
        // unclosed paren points at end of line
        let toks = tokenize("(1 + 2").expect("tokenizes");
        assert!(
            matches!(parse(&toks), Err(e) if e.kind == ErrKind::UnexpectedEol { expected: "`)`" })
        );
        // deep nesting is refused, not crashed into
        let deep: String = "(".repeat(5000);
        let toks = tokenize(&deep).expect("tokenizes");
        assert!(matches!(parse(&toks), Err(e) if e.kind == ErrKind::NestTooDeep));
    }

    #[test]
    fn implicit_multiplication() {
        // the headline shapes
        assert!(matches!(expr("2pi"), Expr::Bin(BinOp::Mul, l, _, _)
            if matches!(*l, Expr::Num(2.0))));
        assert!(matches!(expr("3(4 + 5)"), Expr::Bin(BinOp::Mul, l, _, _)
            if matches!(*l, Expr::Num(3.0))));
        // paren-paren chains
        assert!(matches!(expr("(1 + 2)(3 + 4)"), Expr::Bin(BinOp::Mul, ..)));
        // adjacent numbers multiply
        assert!(matches!(expr("2 3"), Expr::Bin(BinOp::Mul, ..)));
        // same precedence as *, left-associative: 1/2pi is (1/2)*pi
        assert!(matches!(expr("1/2pi"), Expr::Bin(BinOp::Mul, l, _, _)
            if matches!(*l, Expr::Bin(BinOp::Div, ..))));
        // power binds tighter: 2^3pi is (2^3)*pi
        assert!(matches!(expr("2^3pi"), Expr::Bin(BinOp::Mul, l, _, _)
            if matches!(*l, Expr::Bin(BinOp::Pow, ..))));
        // subtraction is not implicit: 2 -3 stays binary
        assert!(matches!(expr("2 -3"), Expr::Bin(BinOp::Sub, ..)));
        // a name followed by ( is a call, not a product
        assert!(matches!(expr("milk(2)"), Expr::Call(..)));
        // a call followed by a value multiplies
        assert!(matches!(expr("milk(2)x"), Expr::Bin(BinOp::Mul, l, _, _)
            if matches!(*l, Expr::Call(..))));
    }

    #[test]
    fn strict_mode_keeps_the_old_grammar() {
        // the mixed-line skeleton parses with implicit mult off
        let toks = tokenize("2 3").expect("tokenizes");
        assert!(matches!(
            parse_strict(&toks),
            Err(e) if e.kind == ErrKind::TrailingTokens
        ));
        // explicit operators are unaffected
        let toks = tokenize("2 * 3").expect("tokenizes");
        assert!(matches!(parse_strict(&toks), Ok(Stmt::Expr(..))));
    }
}

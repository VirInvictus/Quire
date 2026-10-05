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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assign(String, Span, Expr),
    Expr(Expr),
}

pub fn parse(tokens: &[Token]) -> Result<Stmt, QuireError> {
    if tokens.is_empty() {
        return Err(QuireError::new(
            (0, 0),
            ErrKind::UnexpectedEol {
                expected: "an expression",
            },
        ));
    }
    match (&tokens[0].tok, tokens.get(1).map(|t| &t.tok)) {
        (Tok::Ident(name), Some(Tok::Equals)) => {
            let (name, span) = (name.clone(), tokens[0].span);
            let mut p = P {
                toks: tokens,
                pos: 2,
                depth: 0,
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
            let op = match t.tok {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Of => BinOp::Mul,
                _ => break,
            };
            let span = t.span;
            self.pos += 1;
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
            Stmt::Assign(..) => panic!("unexpected assignment"),
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
        // trailing junk carries its own span
        let toks = tokenize("2 3").expect("tokenizes");
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
}

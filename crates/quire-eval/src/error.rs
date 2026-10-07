//! Errors for the Quire engine. Every error carries a byte span into
//! its line, because an answer cell must be able to point at exactly
//! what went wrong (kalker's missing spans are the cautionary tale).

use std::fmt;

/// Byte range into the line the error came from.
pub type Span = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrKind {
    /// A character the grammar does not know (`$`, `,`, ...).
    BadChar(char),
    /// A number literal that could not be read (`1.2.3`, overflow).
    BadNumber,
    /// Parentheses or unary chains nested past the engine's limit.
    NestTooDeep,
    /// The next token was not what the grammar wanted.
    UnexpectedTok { expected: &'static str, got: String },
    /// The line ended where the grammar wanted a token.
    UnexpectedEol { expected: &'static str },
    /// A complete expression was parsed but tokens remain (`2 3`).
    TrailingTokens,
    /// `()` with nothing inside.
    EmptyParens,
    /// Assignment to a reserved word (`total = 5`).
    AssignToKeyword(&'static str),
    /// A name read before it is assigned.
    UnknownName(String),
    /// `answer` read with no expression line above.
    NoAnswer,
    /// Division by zero.
    DivideByZero,
    /// A result that is not finite (`1e308 * 10`).
    OutOfRange,
    /// A unit-engine message for a line the unit path took (the
    /// engine's own first message line; see spec.md "Unit
    /// expressions").
    Unit(String),
    /// A function definition repeating a parameter name.
    DuplicateParam(String),
    /// `&N` pointing at a line with no result (prose, heading) or
    /// at/below the referencing line.
    BadLineRef(String),
    /// A call passing the wrong number of arguments.
    CallArity {
        name: String,
        expected: usize,
        got: usize,
    },
}

impl fmt::Display for ErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrKind::BadChar(c) => write!(f, "unexpected `{c}`"),
            ErrKind::BadNumber => f.write_str("malformed number"),
            ErrKind::NestTooDeep => f.write_str("expression nested too deeply"),
            ErrKind::UnexpectedTok { expected, got } => {
                write!(f, "expected {expected}, found {got}")
            }
            ErrKind::UnexpectedEol { expected } => {
                write!(f, "expected {expected} before end of line")
            }
            ErrKind::TrailingTokens => f.write_str("unexpected extra input after the expression"),
            ErrKind::EmptyParens => f.write_str("empty parentheses"),
            ErrKind::AssignToKeyword(k) => write!(f, "cannot assign to `{k}`"),
            ErrKind::UnknownName(n) => write!(f, "unknown name `{n}`"),
            ErrKind::NoAnswer => write!(f, "`answer` has no previous line"),
            ErrKind::DivideByZero => f.write_str("division by zero"),
            ErrKind::OutOfRange => f.write_str("result out of range"),
            ErrKind::Unit(message) => f.write_str(message),
            ErrKind::DuplicateParam(p) => {
                write!(f, "duplicate parameter `{p}`")
            }
            ErrKind::BadLineRef(message) => f.write_str(message),
            ErrKind::CallArity {
                name,
                expected,
                got,
            } => write!(
                f,
                "`{name}` expects {expected} argument{}, got {got}",
                if *expected == 1 { "" } else { "s" }
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuireError {
    pub span: Span,
    pub kind: ErrKind,
}

impl QuireError {
    pub fn new(span: Span, kind: ErrKind) -> Self {
        QuireError { span, kind }
    }
}

impl fmt::Display for QuireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (bytes {}..{})", self.kind, self.span.0, self.span.1)
    }
}

/// Human description of a token for `expected ..., found ...` messages.
pub fn describe(tok: &crate::tokens::Tok) -> String {
    use crate::tokens::Tok;
    match tok {
        Tok::Num(_) => "a number".to_string(),
        Tok::Ident(n) => format!("`{n}`"),
        Tok::Total => "`total`".to_string(),
        Tok::Answer => "`answer`".to_string(),
        Tok::Of => "`of`".to_string(),
        Tok::Plus => "`+`".to_string(),
        Tok::Minus => "`-`".to_string(),
        Tok::Star => "`*`".to_string(),
        Tok::Slash => "`/`".to_string(),
        Tok::Caret => "`^`".to_string(),
        Tok::Percent => "`%`".to_string(),
        Tok::LParen => "`(`".to_string(),
        Tok::RParen => "`)`".to_string(),
        Tok::Tag(n) => format!("`@{n}`"),
        Tok::DateStamp(d) => format!("`@{d}`"),
        Tok::Equals => "`=`".to_string(),
        Tok::Comma => "`,`".to_string(),
        Tok::LineRef(n) => format!("`&{n}`"),
        Tok::Arrow => "`->`".to_string(),
    }
}

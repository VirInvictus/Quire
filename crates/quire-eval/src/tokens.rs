//! Tokenizer: one line of a sheet into spans-carrying tokens.
//!
//! A `//` starts a comment that runs to the end of the line, so the
//! tokenizer simply stops there; line-leading comments were already
//! classified as Comment lines before this ever runs.

use crate::error::{ErrKind, QuireError, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Ident(String),
    Total,
    Answer,
    Of,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Percent,
    LParen,
    RParen,
    Equals,
    /// `@name` trailing an expression line (spec.md "Tags").
    Tag(String),
    /// `@YYYY-MM-DD` trailing an assignment (spec.md "Dated
    /// snapshots"); documentation metadata, never an expression part.
    DateStamp(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, QuireError> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\r' => i += 1,
            b'/' if b.get(i + 1) == Some(&b'/') => break,
            b'/' => {
                out.push(Token {
                    tok: Tok::Slash,
                    span: (i, i + 1),
                });
                i += 1;
            }
            c if c.is_ascii_digit()
                || (c == b'.' && b.get(i + 1).is_some_and(|&d| d.is_ascii_digit())) =>
            {
                let start = i;
                let mut dots = 0;
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                    if b[i] == b'.' {
                        dots += 1;
                    }
                    i += 1;
                }
                let text = &src[start..i];
                let bad = || QuireError::new((start, i), ErrKind::BadNumber);
                if dots > 1 || text.ends_with('.') {
                    return Err(bad());
                }
                let v: f64 = text.parse().map_err(|_| bad())?;
                if !v.is_finite() {
                    return Err(bad());
                }
                out.push(Token {
                    tok: Tok::Num(v),
                    span: (start, i),
                });
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let tok = match &src[start..i] {
                    "total" => Tok::Total,
                    "answer" => Tok::Answer,
                    "of" => Tok::Of,
                    w => Tok::Ident(w.to_string()),
                };
                out.push(Token {
                    tok,
                    span: (start, i),
                });
            }
            b'+' => push(&mut out, &mut i, Tok::Plus),
            b'-' => push(&mut out, &mut i, Tok::Minus),
            b'*' => push(&mut out, &mut i, Tok::Star),
            b'^' => push(&mut out, &mut i, Tok::Caret),
            b'%' => push(&mut out, &mut i, Tok::Percent),
            b'(' => push(&mut out, &mut i, Tok::LParen),
            b')' => push(&mut out, &mut i, Tok::RParen),
            b'=' => push(&mut out, &mut i, Tok::Equals),
            b'@' => {
                let start = i;
                i += 1;
                // a digit after the @ begins a date stamp (spec.md
                // "Dated snapshots"); an identifier begins a tag.
                if b.get(i).is_some_and(|c| c.is_ascii_digit()) {
                    let ds = i;
                    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'-') {
                        i += 1;
                    }
                    let text = &src[ds..i];
                    let parts: Vec<&str> = text.split('-').collect();
                    let shaped = parts.len() == 3
                        && (1..=4).contains(&parts[0].len())
                        && parts[1].len() <= 2
                        && parts[2].len() <= 2
                        && parts
                            .iter()
                            .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()));
                    if !shaped {
                        return Err(QuireError::new((start, start + 1), ErrKind::BadChar('@')));
                    }
                    out.push(Token {
                        tok: Tok::DateStamp(text.to_string()),
                        span: (start, i),
                    });
                    continue;
                }
                let name_start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                if i == name_start
                    || !src[name_start..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                {
                    let ch = src[start..].chars().next().unwrap();
                    return Err(QuireError::new(
                        (start, start + ch.len_utf8()),
                        ErrKind::BadChar(ch),
                    ));
                }
                out.push(Token {
                    tok: Tok::Tag(src[name_start..i].to_string()),
                    span: (start, i),
                });
            }
            _ => {
                let ch = src[i..].chars().next().unwrap();
                let kind = ErrKind::BadChar(ch);
                return Err(QuireError::new((i, i + ch.len_utf8()), kind));
            }
        }
    }
    Ok(out)
}

fn push(out: &mut Vec<Token>, i: &mut usize, tok: Tok) {
    out.push(Token {
        tok,
        span: (*i, *i + 1),
    });
    *i += 1;
}

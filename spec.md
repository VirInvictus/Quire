# Quire spec

Quire is a Soulver-style notepad calculator for Linux: a plain-text
sheet where prose and calculations share the page, and every expression
answers on its own line in a results column at the right edge of the
editor.

This document is the contract. Behavior described here is normative;
the roadmap sequences it, and patchnotes record what shipped.

## Purpose

Soulver showed that a calculator can read like a page of scratch
notes. It is macOS-only, and Linux has no equivalent with the same
feel. Quire is that equivalent for the Wayland desktop: local-first,
plain files, and a calculation model built for how people actually
write sums (`200 + 15%`, `milk = 3.50`, `total`).

## Non-goals

- No cloud, accounts, sync, or telemetry. Local-first, plain files.
- No WYSIWYG markdown rendering in v1 (highlighting only; a rendered
  preview is a separate later decision).
- No plugin or extension system.
- No mobile, Windows, or macOS builds.

## The sheet

A document (a *sheet*) is plain UTF-8 text. Lines are the unit of
evaluation and display. Every physical line counts, blanks included;
line numbers never skip.

Leading whitespace is insignificant for classification.

### Line kinds

| Kind | Rule | Rendered as |
|---|---|---|
| Blank | whitespace only | empty |
| Comment | first non-space is `//` | dimmed |
| Heading | 1-6 `#` then space/tab/end of line | styled (CommonMark's 3-space indent limit does not apply; any leading whitespace is allowed) |
| Expression | shape rule below | text + result |
| Reference | exactly one identifier (`[A-Za-z_][A-Za-z0-9_]*`, not a keyword), optional surrounding whitespace | text + result when the name is bound above; plain text when it is not |
| Text | everything else | text |

`//` starts a comment that runs to the end of the line: on its own
line the line is a Comment; trailing an expression, the tokenizer
strips it and nothing changes.

### Expression shape

A line is an Expression if any of these holds:

- it starts with a digit, a `.` followed by a digit, a `$` followed by
  a digit, or an opening parenthesis;
- it starts with `+` or `-` **glued** to more content (`-5`, `+7`). A
  `-` or `+` followed by whitespace starts a markdown list item (Text):
  the sheet reads as markdown, so `- buy milk` is a bullet, not a
  signed number;
- it is the bare keyword `total`;
- it is a variable statement: an identifier
  (`[A-Za-z_][A-Za-z0-9_]*`), optional spaces, a single `=` (not
  `==`), then the value expression;
- it leads with the keyword `total` (bare, or `total` opening an
  expression such as `total * 2`);
- it leads with an identifier immediately followed (optional spaces)
  by `*`, `/`, `^`, or `(`: a reference line like `milk * 2`.

That last rule is deliberately narrow. Identifier-led lines whose next
token is `+` or `-` are Text: prose like "War and Peace - part 1" must
not grow an error cell, and a missed calculation beats a false error.
A line that is *nothing but* an identifier is a Reference and evaluates
leniently (see Evaluation semantics).

Classification is shape-only. An Expression line the parser rejects
still evaluates, to an error on its own line; it never poisons
neighbouring lines. Digit-led prose (`2 tickets to the show`) is a
known cost of the deterministic rule: it classifies as Expression and
errors on its own line.

## Evaluation semantics (Phase 1)

- **Numbers.** Decimal literals (`12`, `3.50`). No scientific
  notation and no digit separators in v1.
- **Operators.** `+`, `-`, `*`, `/`; parentheses; unary minus; `^`
  for power, right-associative, binding tighter than unary minus, so
  `-2^2` is `-(2^2)`.
- **Percent.** The four forms:
  - `x%` alone is `x / 100`;
  - `a + b%` is `a + a*b/100`, and `a - b%` is `a - a*b/100`
    (relative add and subtract);
  - `b% of a` is `b/100 * a` (the `of` keyword);
  - `a * b%` and `a / b%` treat `b%` as plain `b/100`.
- **Variables.** `name = expression` evaluates immediately and is
  visible to lines below, never to lines above. Reassignment is
  allowed; a reading line sees the nearest binding above it.
- **References.** A Reference line (a bare identifier) asks what that
  name is worth. Bound above, it shows the value and counts like any
  expression result — toward `answer`, toward totals, visible to
  lines below. Unbound, it renders as plain text with no cell: bare
  names never error, and a bare `answer` behaves the same way for the
  most recent result.
- **`answer`** refers to the most recent Expression result above the
  reading line.
- **`total`** on a line by itself sums every Expression result since
  the previous `total` line or the most recent heading, whichever is
  later (or from the top of the sheet). Consecutive totals therefore
  act as subtotals, and a heading sections the math as well as the
  page. A total reports the sum and starts a new one; it does not add
  itself. A line that merely uses `total` (such as `total * 2`) is an
  ordinary expression line: its result joins the sum like any other.
- **Errors.** Division by zero, unknown names, malformed expressions.
  An error belongs to its line only.

### Results

The results column shows one value or error per Expression line,
aligned to that line and scrolling with the sheet. It is part of the
editor's right edge, not a separately scrolled pane.

Formatting: integer parts grouped in threes with `,`; up to 12
significant digits; trailing zeros trimmed; whole results print
without a decimal part.

## Architecture

- **Two-crate workspace.** `quire-eval` is the engine: sheet model,
  tokenizer, parser, evaluator. No GTK, no I/O dependencies; it is a
  standalone Rust library by design. `quire` is the GTK4 application.
- **Toolchain.** Rust 2024 (floor 1.85); `gtk4` 0.11 and `sourceview5`
  0.11 from Phase 2; plain GTK4, no libadwaita, styled through
  `vir-gtk` with the house Kanagawa Dragon palette.
- **Editing surface.** GtkSourceView. Markdown highlighting for
  structure; math lines are ordinary text the engine re-reads.
- **Live evaluation.** The whole sheet re-evaluates on every buffer
  change, debounced. Sheets are small and evaluation is O(n); no
  incremental machinery in v1.
- **Documents.** Plain UTF-8, no sidecar metadata. The app registers
  the `.quire` extension but opens and saves any text file.
- **Fonts.** Bundled (OFL-licensed) for the numeric surface, with
  generic fallbacks. Nothing may assume an installed font. Exact faces
  are the Phase 3 typography decision.
- **Currency, units, dates.** Out of v1. Own implementation vs
  embedding `numbat` is the recorded Phase 6 decision.

## Testing

- Table-driven unit tests in-module: classification, tokenizer,
  parser, the four percent forms, variables, `answer`, `total`,
  errors, formatting.
- File-driven script tests: every
  `crates/quire-eval/tests/scripts/*.quire` file is a sheet carrying
  its own expectations. `//= N` trailing a line pins that line's
  answer (the golden-sheet layer, including every example in this
  spec); `# expect: N` after an expression line asserts its result;
  `# err: text` asserts a failure whose message contains `text`. One
  runner walks the directory; dropping in a new `.quire` file is
  enough.
- An adversarial corpus: malformed, hostile, and pathological sheets
  must evaluate without panicking, errors contained to their lines.
- `quire`: the model layer is tested headlessly. Visual acceptance is
  a human display pass, not CI.

## VERSION

`VERSION` at the repo root and the workspace version in `Cargo.toml`
are one version carried twice: bump both in the same commit.

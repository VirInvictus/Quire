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
- No live prices, ever. Currency converts against daily
  reference-rate snapshots the app fetches from the ECB on demand
  (Phase 6): the fetch is optional and never required - the stale
  cache keeps every conversion working offline, and rates can always
  be typed into a sheet by hand (Phase 9 budgets and portfolios are
  built on manual snapshots). No other feature touches the network,
  and nothing requires an account, sync, or telemetry.
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
  by `*`, `/`, `^`, or `(`: a reference line like `milk * 2`;
- it starts with `&` followed by a digit: a line carrying a
  reference (`&1 * 3`, see Line references).

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

## Unit expressions (Phase 6)

Expression lines may carry physical units, powered by an embedded
[numbat](https://numbat.dev) context (MIT OR Apache-2.0, built with
its network-fetching and plotting features off). Currency and dates
have also landed on the same engine (see the Currency and Dates
sections).

- **Routing.** A line evaluates on the scalar engine exactly as
  before, byte for byte. The numbat path takes an Expression line
  only when the scalar path declines it, in one of these ways: the
  line's parse failed and at least one identifier numbat knows as a
  unit; its evaluation failed on an unbound name the unit engine
  knows (a unit, a period, or a date word - implicit multiplication
  now lets value-unit shapes parse on the scalar path, so the
  failure surfaces at evaluation); its evaluation touched a
  quantity-valued name (arithmetic over quantity results and
  variables - the names are bound, the bridge seeds them, so no
  unit word is needed in the line); or the line carries a
  recurrence phrase (see Recurring amounts). In every case the
  line's tokens include no Quire keyword (`of`, `total`, `answer`)
  and no percent. Everything else keeps today's behavior, including
  its errors.
- **Quantities.** A unit line's result is a quantity: a number and a
  dimension (`5 kg + 300 g` is `5.3 kg`). Quantity results bind like
  any result: toward `answer`, toward totals, visible to lines below,
  and usable in later unit lines by name. Bare references to a
  quantity show the quantity. Quire's scalar variables seed the unit
  path as plain numbers, so `milk * 2` with `milk = 3.50` works in a
  unit line too.
- **Totals.** A `total` over scalar results is unchanged. A total
  that includes quantity results sums them dimension-safely through
  the unit engine: like dimensions add (`3 kg + 5 kg`); a mismatch
  (`3 kg` against `5 m`) fails the total line and nothing else.
  Scalar and quantity results never silently mix: a total mixing them
  fails the same way.
- **Formatting.** Scalar results keep the Results formatting rules
  (comma grouping, 12 significant digits, trimmed). Quantity results
  render in the unit engine's own notation.
- **Errors.** A unit line the engine rejects fails on its own line
  with the unit engine's message; a failed unit line is contained
  like any other. Known sharp edge: a sheet variable named after a
  unit-engine builtin (say `pi`) fails on the unit path with a clash
  error.
- **Toolchain.** The numbat dependency raises the workspace floor
  from Rust 1.85 to numbat's 1.88.

## Dates (Phase 6)

Date arithmetic rides the unit engine's jiff-backed datetime module.
Date words are sheet vocabulary, case-insensitive:

- **Bare words.** `today`, `now`, `tomorrow`, and `yesterday` on
  their own line answer as datetimes (`tomorrow` is midnight of the
  next day). `today` anchors to midnight; `now` carries the time.
- **Phrases.** `<amount> <time-unit> from today` (or `from now`)
  adds; `<amount> <time-unit> ago` subtracts: `3 weeks from today`,
  `90 minutes from now`, `6 months ago`. Any time unit the engine
  knows works.
- **Results.** A datetime renders as `YYYY-MM-DD HH:MM UTC-offset`.
  Datetime arithmetic composes with everything else: `tomorrow -
  today` answers a duration; tagging and totals work as anywhere.
- **Engine scope.** Datetimes are unit-engine values: scalar
  arithmetic on them routes to the bridge like quantities.

## Currency (Phase 6)

`50 USD -> EUR` converts through the unit engine against a daily
reference-rate snapshot the app fetches from the ECB on demand. The
snapshot caches under `~/.cache/quire/ecb.xml` and seeds the engine
at startup: once fetched, conversions work offline forever after -
the stale cache keeps every rate working, and rates can always be
typed into a sheet by hand. The refresh interval is a setting
(`currency-refresh-hours`, default 24) with a menu item to force a
fetch; rates seed once per process, so a refresh applies from the
next launch. A sheet without rates sees unknown names, never a
hang. No live prices, ever (see Non-goals): the snapshot is a
reference rate, and money composes with the unit engine's dimension
rules - totals over money results sum them dimension-safely.

## Recurring amounts (Phase 8)

An Expression line may state a recurring amount: a value divided by
a period, glued or spaced - `$1200/month`, `950 / month`,
`60/quarter`. The phrase evaluates through the unit engine as a rate
quantity, and the answer keeps the written period: `$1200/month`
answers `1200 /month`. A rate written with an ISO currency is a
money rate (`1200 USD/month` answers `1200 $/month`); any other
period is a conversion away: `1200/month -> 1/day` answers
`39.4259 /day`.

- **Periods.** `day`, `week`, `month`, `quarter`, `year`, singular
  or plural (`/months`). A quarter is three months (91.31 days),
  registered into the engine at startup beside the prelude. Mixed
  periods add dimension-safely; the sum displays in the largest
  period involved (`950/month + 45/week` is `1145.67 /month`, and
  adding a `/year` term turns the whole sum's display to `/yr`) -
  display only, the value is one dimension. (Soulver displays the
  last-written period; the value is the same.)
- **Dollar decoration.** A `$` immediately before a number is
  decoration everywhere: `$1200` is the number 1200. It never names
  a currency - money rates and conversion are ISO-code syntax
  (`50 USD -> EUR`).
- **Quantity rails.** A rate result binds like any quantity result:
  toward `answer`, toward totals and tag sums, visible to lines
  below, usable by name and through `&N`. A rate total is
  dimension-checked like any quantity total: a bare scalar in a rate
  sum fails the total line, so a forgotten `/month` is caught rather
  than silently under-counted. `rent = 950/month @fixed` composes
  with tags and dated stamps. A rate times a duration is a plain
  amount (`950/month * 12 months` is `11400`).
- **Boundaries.** The phrase needs the unit engine, so it does not
  compose with `total`, `answer`, or percent inside a line (the same
  rule as unit expressions; a `total` line itself sums rate results
  fine). A variable bound to a period name wins over the phrase
  (`month = 12` makes `950/month` divide by 12): nearest binding
  above, like any name.
- **Constants.** The engine's own: week = 7 days, month = year/12
  (30.4368 days), quarter = 91.3105 days, year = 365.2422 days.

## Completion (Phase 4/6)

Tab completes the word before the cursor when the match is unique -
NoteCalc's rule: act only on a unique match, never on ambiguity.
Candidates are the sheet's own names bound above the cursor line
plus the engine's names: units, their aliases, and prelude
variables. The word must be identifier-shaped, and a completion
inserts only its remainder. Prefixed unit symbols (`kg`), typechecker
constants (`pi`), and currency codes evaluate when typed but are not
candidates: the first are parse-time combinations, the second have
no public accessor, and the currencies module may only load on
demand (its unit values snapshot the exchange rates at load).

## Line references (Phase 7)

`&N` in an expression references the result of sheet line N - any
line, above or below.

- **Blank default.** A reference whose target has no result yet
  (prose, heading, an empty line, or simply a line below that has
  not produced a value) is null: the referencing line shows no cell
  at all until the target answers. Whole-sheet re-evaluation fills
  these in naturally as the sheet grows.
- **Poisoning.** A reference to a FAILED line propagates that error
  to the referencing line.
- **Sheet vocabulary.** References work in scalar and unit
  expressions alike, and their results join `answer`, totals, and
  tags like any expression result. Function bodies cannot use them
  (sheet-positional, like `total`).

**Self-updating.** The app keeps `&N` tokens pointing at their line:
when lines are inserted, deleted, or pasted so that a referenced line
moves, the app rewrites the affected tokens (in place, as one edit)
so each still names the line it targeted. The rewritten number is
always the referenced line's true position; a ref whose target line
is deleted entirely follows the line that takes its place. Loading a
sheet never rewrites anything - refs read exactly as authored until a
line actually moves. Only refs the engine resolves participate: the
`&N` text inside comments, prose, and function bodies is the user's
text and is never touched. The behavior is a setting (`follow-refs`,
default on); with it off, refs are purely positional (`&11` means the
11th physical line, period), and the user adjusts them manually.

## Mixed lines (Phase 7)

A prose line whose words strip out to a complete expression answers
with that expression - `50 apples at 3 each` is `150`, `2 coffees
plus 1 tea` is `3`. Word operators map (`at`, `of`, and `times`
multiply; `plus` adds; `minus` subtracts); unknown names drop as
prose; everything else composes.

- **Silent by design.** A mixed attempt that does not fully
  evaluate leaves the line as prose - a missed calculation beats a
  false error, and mixed lines never grow error cells. A dangling
  word operator on an otherwise-parseable line still fails honestly
  (the scalar parse caught it first).
- **Boundaries.** Reserved words (`total`, `answer`), tags, and
  line references never enter a mixed skeleton: their structured
  meaning must not be misread as arithmetic.
- **Results.** A mixed line's value joins `answer`, totals, and
  tags like any expression result.

## Reverse percents (Phase 7)

A line may ask a percent question in words. The phrases match the
line whole and rewrite to their arithmetic before the mixed-line
skeleton runs, so they answer instead of erroring; a line that does
not fully match a phrase keeps the mixed lines' silence.

- **Find the base.** `N is P% of what` divides out the percent
  (`20 is 10% of what` is `200`); `off` and `on` undo a discount or
  a markup through the relative-percent rules (`180 is 10% off what`
  is `200`, `220 is 10% on what` is `200`).
- **Find the percent.** `N is what percent of M` answers N/M: the
  percent itself, in Quire's convention that a percent is its
  fraction (`30 is what percent of 200` is `0.15`, because `15%` is
  0.15). It composes like any value (`answer * 200` is 30). `what %
  of` reads the same.
- **Shape.** The words match case-insensitively; the value and the
  percent may be any expressions (`rent is 10% off what` works).
  Inside a phrase `is` and `what` are question words, never names.
  These phrases preempt the word-operator skeleton, so `of` in
  `is P% of what` never multiplies.

## Tags (Phase 8)

An Expression line may end with one or more tags: `@` immediately
followed by an identifier, each preceded by whitespace (`lunch =
12.50 @food @london`). Tags are sheet-local labels, not variables.

- **Summing.** `total @tag` reports the sheet-wide sum of every
  tagged result - through the unit engine when any item is a
  quantity, exactly like the plain total's dimension rules. A
  `total @a @b` sums lines carrying either tag.
- **Boundaries.** Tag sums are sheet-wide: a heading sections the
  plain math but never touches them, so categories can live in
  their own sections with the views gathered at the end. A plain
  `total` reports and resets the plain sum and the tag sums.
  `total @tag` is a pure view: it reports and resets nothing.
- **Plain totals include tagged lines.** A tagged result counts
  toward the section's plain total exactly as an untagged one does;
  tags are an additional view, not a partition.
- **Scope.** Tags attach to Expression lines only (a Text line with
  `@` stays prose). `@` followed by anything that is not an
  identifier is an error, as today; the dated-snapshot form of
  Phase 9 will claim `@` followed by a date.
- **Errors.** A `total @tag` with no tagged results sums to zero,
  like an empty section.

## Dated snapshots (Phase 9)

An assignment may carry a date stamp documenting when its value was
true: `AAPL = 190 @2026-10-06` (glued) or `AAPL = 190 @ 2026-10-06`
(single space after the `@`). The date is sheet documentation: it
changes nothing about evaluation, and the value binds exactly as an
undated assignment. A stamp may combine with trailing tags
(`aapl = 10 * 190 @ 2026-10-06 @stocks`). The date shape is
`YYYY-MM-DD` (one- or two-digit month and day accepted); `@`
followed by anything else that is not an identifier tag remains an
error. A `@` mid-expression is still an error, and stamps appear
only at a line's end, like tags.

## Functions (Phase 7/8)

`name(a, b) = expression` defines a function; lines below call it
with `name(...)`. The body is inline - the only shape that fits a
line-model sheet, where every line evaluates on its own.

- **Visibility.** A definition is visible to lines below it, never
  above; a redefinition wins below, like variables. Definitions
  produce no answer cell.
- **Scoping.** Parameters shadow sheet variables for the body; the
  body also sees sheet variables and functions defined above.
  `total` inside a body reads zero: bodies do not section the sheet.
  `answer` inside a body reads the sheet's most recent result, as
  anywhere. Names may not be `total` or `answer`.
- **Arity.** Calling with the wrong number of arguments fails the
  calling line.
- **Recursion and clauses.** A function may be defined in multiple
  clauses, and clauses with LITERAL arguments match before general
  ones: `fact(0) = 1` then `fact(n) = n * fact(n - 1)` is a working
  factorial - recursive calls are allowed and depth-capped (a
  runaway recursion fails the calling line, never a hang). Clause
  selection is nearest-definition-first, literals before variables.
- **Engine scope.** Functions work on both engines: the scalar
  path evaluates them directly, and unit lines seed their
  definitions into the unit engine, where dimension checking
  applies (`twice(2 kg)` is `4 kg`; a body like `x + 1` refuses a
  Mass argument). Definitions whose bodies use `total`, `answer`,
  or percent stay scalar-only. A function named after a unit-engine
  builtin (say `double`, which numbat claims as a constant) fails
  on the unit path with a clash error - only for lines that call
  it.
- **Zero-parameter functions** are allowed and behave like named
  constants.

## Evaluation semantics (Phase 1)

- **Numbers.** Decimal literals (`12`, `3.50`). No scientific
  notation and no digit separators in v1. A `$` immediately before a
  number is decoration: `$1200` is the number 1200 (currency is
  ISO-code syntax, `50 USD -> EUR`; see Recurring amounts).
- **Math functions.** The engine's functions - `sqrt`, `log10`,
  `ln`, `sin`, `cos`, `exp`, `abs`, and the rest of the prelude -
  evaluate through the unit engine: `sqrt(144)` is `12`,
  `sin(30 deg)` is `0.5`. Unknown names still demote in mixed lines,
  so a typo'd function is prose, not a wrong answer.
- **Operators.** `+`, `-`, `*`, `/`; parentheses; unary minus; `^`
  for power, right-associative, binding tighter than unary minus, so
  `-2^2` is `-(2^2)`. Implicit multiplication: a number, bare name,
  or `(` directly after a value multiplies - `2pi`, `3(4+5)`,
  `(1+2)(3+4)`, `2 3` - at the same precedence as `*`,
  left-associative, so `1/2pi` is `(1/2)*pi`. A name followed by `(`
  is still a function call, `2 -3` is still subtraction, and
  unknown names keep the mixed lines' demotion (`2 tickets` is
  still `2`). The mixed-line skeleton itself keeps today's grammar:
  implicit multiplication lives on the expression path, not in
  prose remnants.
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
  expression result: toward `answer`, toward totals, visible to
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
without a decimal part. The app's answer-decimals setting fixes
how many decimal places a scalar answer shows - `14.5` at two
places reads `14.50`, trailing zeros kept (a setting that would
zero a value out keeps the full rendering, so `1/3` never reads as
zero). A
line's answer can cycle alternative
formats with Alt+Up/Down - standard, fixed two decimals, hex, bin -
a per-line view choice that never changes the sheet (unit values
keep the engine's notation; hex and bin apply to non-negative
integers). The choice rides the line number and shifts when the
sheet's structure shifts.

Within each heading region, scalar answers share a decimal-point
column (the region model): integer answers pad on the right so
their implied dot sits where the region's fractional dots are, and
the column's right alignment lines every dot in the region up.
Quantities, errors, hex and bin keep their own shapes.

## The breakdown (Phase 7)

Ctrl+clicking a math line opens a popover with its step-by-step
breakdown: the line's scalar operations, bottom-up, each as
`operands = value`, evaluated in the full sheet context so
variables, references, and function bodies resolve
(`3 * 4 + rent / 2` shows `3 * 4 = 12`, then `950 / 2 = 475`, then
the sum; a relative percent reads `200 + 15% (of 200) = 230`).
Lines the scalar engine does not reduce into operations - unit
lines, definitions, prose - have no breakdown and Ctrl+click does
nothing on them. Ctrl+clicking a task-list checkbox toggles it
instead (see The sheet).

## Task lists (Phase 7)

A list line may open with a GFM checkbox - `- [ ] plan the trip` -
and stays prose for the engine. Ctrl+clicking the box toggles it
(`x` for done, space for open), as a plain user edit the undo
history reverses. Checkboxes never enter the math: a dangling or
malformed box is just text.

## Definition sheets (Phase 8)

Sheets often open with a block of assignments - `name = value`, one
per line, a `//` note trailing - and the app treats that block as
the definition sheet: the outline popover lists every bound name
beneath the headings (pick one to jump to its line), Ctrl+B jumps
from any use to its definition, and Tab completes names. The
answers column is the value column; the comments are the notes.

## Architecture

- **Two-crate workspace.** `quire-eval` is the engine: sheet model,
  tokenizer, parser, evaluator. No GTK, no I/O dependencies; it is a
  standalone Rust library by design. `quire` is the GTK4 application.
- **Toolchain.** Rust 2024 (floor 1.88); `gtk4` 0.11 and `sourceview5`
  0.11 from Phase 2; plain GTK4, no libadwaita, styled through
  `vir-gtk` with the house Kanagawa Dragon palette. `numbat`
  (default features off) is the unit engine; `attohttpc` (rustls,
  the same client numbat's own fetch uses) backs the ECB currency
  fetch.
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
- **Currency, units, dates.** The numbat embed powers units and
  dates; currency converts against ECB daily snapshots (offline-first).
  These are IN, not out: the original "out of v1" note is
  superseded by Phase 6 completion.

## Testing

- Table-driven unit tests in-module: classification, tokenizer,
  parser, the four percent forms, variables, `answer`, `total`,
  errors, formatting.
- File-driven script tests: every `.quire` file under
  `crates/quire-eval/tests/scripts/` (subfolders included) is a
  sheet carrying its own expectations. `//= N` trailing a line pins
  that line's answer; `# expect: N` after an expression line asserts
  its result; `# err: text` asserts a failure whose message contains
  `text`. One runner walks the tree; dropping in a new `.quire` file
  is enough. Each feature's corpus carries this spec's examples for
  that feature (units, recurrence, reverse percents, and so on);
  the ambiguities subfolder pins the implicit-multiplication edge
  cases.
- An adversarial corpus: malformed, hostile, and pathological sheets
  must evaluate without panicking, errors contained to their lines.
- `quire`: the model layer is tested headlessly. Visual acceptance is
  a human display pass, not CI.

## VERSION

`VERSION` at the repo root and the workspace version in `Cargo.toml`
are one version carried twice: bump both in the same commit.

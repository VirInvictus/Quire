# AGENTS.md

Guidance for coding agents working in Quire. Overrides the global
`~/.zcode/AGENTS.md` where they conflict.

## What this is

Quire: a Soulver-style notepad calculator for Linux. Plain-text
sheets, per-line live results in a right-hand column. Portfolio piece,
public, MIT.

## Where this stands (updated 2026-10-08, v1.5.2)

v1.5.2 shipped and tagged: Phases 6-9 are complete, and the full
Soulver-depth surface is live on the starter page - mixed lines now
carry units through the engine, so a lab-notebook sentence with one
number and one unit answers on its own line: arithmetic with implicit multiplication
(`2pi`, `3(4+5)`; the mixed skeleton parses strict), percents
including the reverse questions in words, variables, totals, tags,
mixed lines, functions with recursion and multi-clause matching,
line references that follow their target lines (the follow-refs
toggle, default on; GtkTextMark identity plus the pure refs.rs
convergence module: the fourth attempt, after three cascade-stripped
ones), dates (jiff-backed), recurring amounts (rate quantities
through the numbat bridge; `$` is decoration), currency (ECB,
offline-first; its own spec section since the amendment pass),
units (numbat embed), Tab completion over sheet and engine names,
dated snapshots, templates, answer-decimals setting, format cycling,
hover tooltips, and token-level error highlighting. The app renders
sheets at 20px JetBrains Mono in Kanagawa Dragon/Lotus through
vir-gtk, with the custom `quire` language spec, scheme pair,
renderer-drawn answers column, token-level error underlining, and
hover tooltips. Packaging: Meson wrapper, desktop file, AppStream
metainfo, hicolor icons, mime package, tag-gated release CI with
server-side verbatim notes.

Post-1.0 queue: dates depth, budget/portfolio depth, the Phase 10
preview gate (recorded default: never), the agent-friendly CLI, and
Brandon's display passes - the tour's new sections, the rate cells,
and the region alignment deserve eyes; they gate polish, not the
release.

## Stack

- Rust 2024, two-crate workspace: `crates/quire-eval` (engine; no
  GTK, no I/O dependencies) and `crates/quire` (the GTK4 app).
- The engine embeds `numbat` (default-features off: no network code,
  no plotly) as the unit layer behind spec.md "Unit expressions".
  This raises the workspace Rust floor to numbat's 1.88. Bridge
  facts that bite (crates/quire-eval/src/units.rs):
  - The prelude loads once per process into a thread-local master
    `Context`; every evaluation pass clones it. Never interpret the
    prelude per pass (30-170 ms).
  - numbat answers `let` statements with `Continue`, not a value:
    assignment lines compile to `let name = rest` plus a trailing
    `name` reference.
  - Statement seeds separate with newlines; numbat has no `;`.
  - `knows_unit` probes `1 <name>`: prefixed forms (`kg`) are not
    registry names.
  - numbat's error Display is short multi-line detail; the bridge
    collapses it to one compact line for the answers column.
- Plain GTK4, NO libadwaita. Styling goes through `vir-gtk` (a git
  dependency tracked on `main`, pinned by Cargo.lock; 1.4.3 is the
  floor - see the Alert gotcha below).
  `portal::init` + `connect_dark_changed` + resplice on every
  dark/light flip: `base_css` at the crate tier, the app sheet at the
  app tier via `palette.replace_tokens(APP_CSS)`.
- Editor surface: `sourceview5` 0.11 (needs the
  `gtksourceview5-devel` system package; CI's Ubuntu ships
  GtkSourceView 5.12, so NO version-gated features: the v5_16
  feature once red-ran CI).
- Packaging is Meson wrapping cargo (the house shape): `meson.build`
  at the root installs the binary, `data/`'s desktop file and
  metainfo, the `data/icons/hicolor` set, `data/mime/quire.xml`, and
  the gschema (compiled at install). `gnome.post_install` skips
  itself under DESTDIR, so the release job compiles the staged
  schemas by hand. The app's runtime extraction (fonts, lang,
  schemes, schema) is independent of the install; both alone are a
  working Quire. Tag-gated `release.yml` (pinned fedora:44) cuts the
  GitHub release and attaches the tarball.
- No third-party dependencies beyond what spec.md Architecture lists
  without asking first.

## Commands

From the repo root:

- `cargo build` / `cargo test` (workspace-wide)
- `cargo test -p quire-eval` for engine-only
- `cargo fmt` before committing
- `meson setup builddir --prefix=/usr` + `meson install -C builddir`
  for the full desktop install (never commits; builddir/ and stage/
  are gitignored, and the release job is the rehearsed version of
  this flow)

## VERSION

`VERSION` at the repo root and the workspace `version` in `Cargo.toml`
are one version carried twice: bump both in the same commit. Releases
tag `vX.Y.Z` with the matching patchnotes entry as the tag message,
verbatim, via `--cleanup=verbatim`. **Tag as you go** (Brandon,
2026-10-07, after 0.1.0 sat frozen through ten shipped features):
landed features bump minor, fixes bump patch - never leave value
accumulating under Unreleased.

## Docs flow (keep this in every landed chunk)

- `patchnotes.md`: every landed chunk appends a bullet under the
  unreleased version.
- `roadmap.md`: tick boxes the moment work ships; decision gates
  marked [D] get an explicit prompt before the work lands.
- `spec.md`: semantics changes update the spec FIRST or in the same
  commit; the spec is the contract.
- `README.md`: user-facing reality (features that exist, build
  requirements) updates when it changes, not "eventually".
- This file: agent-facing reality (stack facts, gotchas, status prose)
  updates in the same commit when it changes.

## Hard-won rendering and language gotchas

- A GtkSourceView `.lang` needs a ROOT context whose id matches the
  language id, or the file loads, `language("quire")` returns Some,
  and the highlight engine silently does nothing.
- `extended="true"` regexes treat a bare `#` as a comment start: a
  header rule written `#{1,6}` degenerates to match-every-line. Keep
  such rules plain single-line matches.
- A `<match>` context styles only the matched span. Whole-line
  styling needs a `<start>`/`<end>` pair (the semi-highlighted-token
  bug).
- The Gutter widget ignores CSS backgrounds, and the view's
  below-text layer is clipped to the text window: the answers column
  tone and hairline are painted by the renderer widget's own
  `snapshot` before the line loop.
- GtkSourceGutterLines are 0-based buffer indices; engine sheet line
  numbers are 1-based.
- TextView CSS `line-height` clips glyph ascenders at any value: do
  not set it. Set the caret with the scheme's `cursor` style, not
  CSS. Prefer whole-pixel font sizes (20px): fractional font metric
  heights put every baseline off-pixel and GSK shaves glyph tops.
- Pixel-snap custom text draws (`align_cell` returns f32; round it).
- The renderer reads cell colors from the scheme (`quire:result` /
  `quire:error`); palette hexes are the fallback path.
- Sheet token design (Brandon's spec): variables render blue
  (`quire:variable`), numbers and operators render in the default
  foreground. The math-line context is a zero-width-lookahead
  start/end region: a consuming start would eat the identifiers
  before the identifier sub-context ever runs.
- A `vir_gtk::widgets::Alert` must be pinned at 1.4.3 or newer: 1.4.2
  held the response state only through the caller's Alert value, so
  the fire-and-forget build-wire-present-drop shape (what the close
  guard does) shipped dead buttons. 1.4.3 anchors the state to the
  dialog window; dropping the Alert value after `present` is safe.
- Programmatic sheet-text rewrites have exactly one sanctioned
  shape (the self-updating refs in refs.rs + page.rs): identity in
  GtkTextMarks, never text diffs; splices applied with the
  evaluation handler blocked inside begin_irreversible_action. A
  rewrite that fires `changed` re-enters this pipeline and cascades
  (three stripped attempts prove it). set_text collapses every mark
  to offset 0: every load path must rebuild the mark set (load()
  does).
- Recurrence phrases ride the unit bridge (spec.md "Recurring
  amounts"): `quarter` registers at startup beside the prelude
  (`@aliases(quarters)` + `unit quarter: Time = 3 months`), and the
  unit fallback (`unit_line`) hands ANY scalar failure on a
  bridge-known name to the bridge - units, periods, date words - no
  whitelist, no slash gate (implicit multiplication made
  value-unit shapes parse, so the failures surface at evaluation).
  A bridge refusal falls back to the scalar funnel so the mixed
  lines' silence stands. Quantity-valued NAMES route on the touch
  alone (no unit word needed in the line; `bag * 2` works) -
  arithmetic over rate variables is how the budget template's
  leftover line lives.
- `$` before a digit is decoration twice over: the tokenizer skips
  the byte, and bridge source prep strips it textually BEFORE the
  &N translation (a rendered money rate may legitimately
  re-introduce a `$` the engine reads natively).
- units::render polishes the engine's numerator-less rate display
  (`1200 month⁻¹`) into the sheet's own form (`1200 /month`). The
  displayed form must stay parseable source: totals, tag sums, and
  &N refs re-feed rendered values to the engine. Sums over mixed
  periods display in the LARGEST period involved (numbat's choice).
- The keyword `of` tokenizes to its own `Tok::Of`, never an
  Ident - any word-level matcher (the reverse-percent phrases, the
  mixed skeleton) must accept both spellings. The reverse-percent
  recognizer runs BEFORE the mixed skeleton (its word-operator
  `of` would otherwise multiply the phrase into nonsense).
- apply_ref_plan applies relabels through refs::apply_relabels - a
  SIMULTANEOUS rename (pull every relabeled entry before
  reinserting). The mortgage template's consecutive derivation refs
  chain the relabel plan; a sequential keyed apply collapses the
  mark table onto one orphaned mark (Brandon's &26/&27/&28/&27 bug,
  v1.5.2).
- The debug surface: evaluate_sheet_stats (lines, passes, capped)
  and quire-cli --stats on the CLI side; QUIRE_DEBUG=1 on the app
  side eprints per-burst lines/passes/cap/elapsed/column-px/over-
  cap/errors and refit_column counts over-cap answers. The full run
  over all 21 sheets: everything settles, zero cap hits.
- evaluate_sheet is a FIXED-POINT loop now (MAX_EVAL_PASSES = 8):
  each pass seeds the next with its outcomes so forward `&N`
  resolves. Settling compares RENDERED strings (Outcome::render) -
  never derived PartialEq, because datetime instants flap per pass
  while their render is minute-stable. The one-pass fast path is
  Ctx.saw_unresolved_ref, set exactly where a `&N` misses its
  target (the scalar LineRef arm and the bridge's BadLineRef site);
  every other piece of sheet state restarts per pass. Genuinely
  cyclic sheets settle blank with no extra machinery (no cycle
  member is ever seeded).
- The recursion cap (200 quire-call levels) costs roughly 10 KB of
  stack PER LEVEL in debug builds. The app's 8 MB main thread is
  fine; Rust's default 2 MB test threads are NOT - heavy template
  or engine computes in tests run under with_big_stack
  (templates.rs). If evaluation ever moves off the main thread,
  give that thread a fat stack or lower the cap.
- Implicit multiplication (kalker's rule) lives on the expression
  path ONLY: `parser::parse_strict` is the mixed skeleton's
  grammar, so prose remnants never gain products. Because
  value-unit shapes (`5 kg + 300 g`) parse on the scalar path now,
  `unit_line` in eval.rs catches UnknownName failures on
  bridge-known names - the parse-failed route alone no longer
  reaches every unit line. Honest near-miss errors therefore
  surface from the first unknown name, not the parser.
- Every landed chunk updates the docs (see "Docs flow"): patchnotes
  bullet, roadmap ticks, spec on semantics, README on user-facing
  reality, this file on agent-facing reality. All five were current
  as of 2026-10-06.

## House rules that bite here

- spec.md is the contract: semantics changes update spec.md first.
- Never assume an installed font: the app bundles JetBrains Mono
  (SIL OFL 1.1, `crates/quire/resources/fonts/` with the license
  text) and installs it under `~/.local/share/fonts/Quire/` at
  startup, before GTK builds its font map. The language spec and
  schemes extract to `~/.local/share/quire/` the same way.
- Documents are plain UTF-8 files; no sidecars, no lock-in.
- Sheets are user data: the app never writes a sheet the user did not
  ask to save.

## Reference shelf (read-only, not ours)

Kept in `~/.gitrepos/` for Quire's benefit; never edit them, never
commit anything into them. A clone is removed once the last open
roadmap box citing it closes (2026-10-08 audit: gnome-text-editor,
gnome-calculator, and Marker were removed as absorbed; re-clone from
upstream if a closed citation ever reopens):

- `notecalc3` (AGPL-3.0): the closest Soulver-like. Learn semantics
  and behavior ONLY; AGPL code must never be copied or translated
  into MIT-licensed Quire. Cited by the open Phase 7 region-
  alignment box.
- `numbat` (MIT OR Apache-2.0): the embedded engine
  (`Context::new(BuiltinModuleImporter::default())`,
  `set_exchange_rates`). Kept as the semantics check for the
  dependency; its citations closed with Phases 6 and 8, but it is
  Cargo-linked, not just a shelf clone.
- `gtksourceview` (LGPL-2.1+, linked): the right-gutter renderer API
  behind the answers column; the markdown.lang fork source.
- `Apostrophe` (GPL-3.0): markdown editor; preview architecture,
  scroll sync, bundled-font chain, focus modes. Cited by the open
  Phase 7 popover box and the gated Phase 10.

(2026-10-08 second sweep: `kalker` removed - its last citation, the
Phase 7 implicit-multiplication box, shipped and ticked; re-clone
from upstream if an ambiguities question ever reopens it.)

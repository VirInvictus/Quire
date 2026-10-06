# AGENTS.md

Guidance for coding agents working in Quire. Overrides the global
`~/.zcode/AGENTS.md` where they conflict.

## What this is

Quire: a Soulver-style notepad calculator for Linux. Plain-text
sheets, per-line live results in a right-hand column. Portfolio piece,
public, MIT.

## Where this stands (2026-10-06)

Phases 0-3 shipped and pushed. The engine (Phase 1) covers the whole
Semantics section of spec.md: arithmetic, the four percent forms,
variables, `answer`, heading-or-total bounded `total`, bare-identifier
Reference lines (lenient: bound = value, unbound = plain text),
span-carrying errors. The app (Phases 2-3) renders sheets at 20px
JetBrains Mono in Kanagawa Dragon/Lotus through vir-gtk, with a
custom `quire` GtkSourceView language, a quire-dark/quire-light
scheme pair, and a right-gutter AnswersRenderer drawing the live
results. Next: Phase 4, documents and editing UX (see roadmap.md).

## Stack

- Rust 2024, two-crate workspace: `crates/quire-eval` (engine; no
  GTK, no I/O dependencies) and `crates/quire` (the GTK4 app).
- Plain GTK4, NO libadwaita. Styling goes through `vir-gtk` (a git
  dependency tracked on `main`, pinned by Cargo.lock) —
  `portal::init` + `connect_dark_changed` + resplice on every
  dark/light flip: `base_css` at the crate tier, the app sheet at the
  app tier via `palette.replace_tokens(APP_CSS)`.
- Editor surface: `sourceview5` 0.11 (needs the
  `gtksourceview5-devel` system package; CI's Ubuntu ships
  GtkSourceView 5.12, so NO version-gated features — the v5_16
  feature once red-ran CI).
- No third-party dependencies beyond what spec.md Architecture lists
  without asking first.

## Commands

From the repo root:

- `cargo build` / `cargo test` (workspace-wide)
- `cargo test -p quire-eval` for engine-only
- `cargo fmt` before committing

## VERSION

`VERSION` at the repo root and the workspace `version` in `Cargo.toml`
are one version carried twice: bump both in the same commit. Releases
tag `vX.Y.Z` with the matching patchnotes entry as the tag message,
verbatim, via `--cleanup=verbatim`.

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
  CSS. Prefer whole-pixel font sizes (20px) — fractional font metric
  heights put every baseline off-pixel and GSK shaves glyph tops.
- Pixel-snap custom text draws (`align_cell` returns f32; round it).
- The renderer reads cell colors from the scheme (`quire:result` /
  `quire:error`); palette hexes are the fallback path.
- Sheet token design (Brandon's spec): variables render blue
  (`quire:variable`), numbers and operators render in the default
  foreground. The math-line context is a zero-width-lookahead
  start/end region: a consuming start would eat the identifiers
  before the identifier sub-context ever runs.
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

Cloned into `~/.gitrepos/` for Quire's benefit; never edit them, never
commit anything into them:

- `notecalc3` (AGPL-3.0): the closest Soulver-like. Learn semantics
  and behavior ONLY; AGPL code must never be copied or translated
  into MIT-licensed Quire.
- `kalker` (MIT): parser ladder, span discipline, file-driven engine
  tests; embeddable alternative engine.
- `numbat` (MIT OR Apache-2.0): the Phase 6 embed candidate
  (`Context::new_without_importer()`, `set_exchange_rates`).
- `gnome-text-editor` (GPL-3.0): the GTK4 + sourceview5 app shape.
- `gtksourceview` (LGPL-2.1+, linked): the right-gutter renderer API
  behind the answers column; the markdown.lang fork source.
- `gnome-calculator` (GPL-3.0): currency-provider caching pattern
  (ECB XML under `~/.cache`, stale works offline).
- `Apostrophe` (GPL-3.0): markdown editor; preview architecture,
  scroll sync, bundled-font chain, focus modes.
- `Marker` (GPL-3.0): markdown editor; preview themes, export paths.

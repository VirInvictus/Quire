# AGENTS.md

Guidance for coding agents working in Quire. Overrides the global
`~/.zcode/AGENTS.md` where they conflict.

## What this is

Quire: a Soulver-style notepad calculator for Linux. Plain-text
sheets, per-line live results in a right-hand column. Portfolio piece,
public, MIT.

## Where this stands (updated 2026-10-07)

v0.5.1 shipped and tagged. All core features are live and on the
starter page: arithmetic, percents, variables, totals, tags, mixed
lines, functions with recursion and multi-clause matching, line
references (positional &N — auto-renumbering was stripped after
three failed attempts), dates (jiff-backed), currency (ECB,
offline-first), units (numbat embed), dated snapshots, templates,
answer-decimals setting, format cycling, hover tooltips, token-level
error highlighting, and the line-ref renumbering (stripped — refs
stay positional). The app renders sheets at 20px JetBrains Mono in
Kanagawa Dragon/Lotus through vir-gtk, with the custom `quire`
language spec, scheme pair, renderer-drawn answers column, token-
level error underlining, and hover tooltips. Packaging: Meson
wrapper, desktop file, AppStream metainfo, hicolor icons, mime
package, tag-gated release CI with server-side verbatim notes.

Next: dates depth, budget/portfolio depth, the &N self-updating
redesign (see the research-sprint brief in session memory), and
Brandon's display passes.

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
  floor - see the Alert gotcha below) —
  `portal::init` + `connect_dark_changed` + resplice on every
  dark/light flip: `base_css` at the crate tier, the app sheet at the
  app tier via `palette.replace_tokens(APP_CSS)`.
- Editor surface: `sourceview5` 0.11 (needs the
  `gtksourceview5-devel` system package; CI's Ubuntu ships
  GtkSourceView 5.12, so NO version-gated features — the v5_16
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
- A `vir_gtk::widgets::Alert` must be pinned at 1.4.3 or newer: 1.4.2
  held the response state only through the caller's Alert value, so
  the fire-and-forget build-wire-present-drop shape (what the close
  guard does) shipped dead buttons. 1.4.3 anchors the state to the
  dialog window; dropping the Alert value after `present` is safe.
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

# AGENTS.md

Guidance for coding agents working in Quire. Overrides the global
`~/.zcode/AGENTS.md` where they conflict.

## What this is

Quire: a Soulver-style notepad calculator for Linux. Plain-text
sheets, per-line live results in a right-hand column. Portfolio piece,
public, MIT.

## Stack

- Rust 2024, two-crate workspace: `crates/quire-eval` (engine; no
  GTK, no I/O dependencies) and `crates/quire` (the GTK4 app).
- Plain GTK4, NO libadwaita. Styling goes through `vir-gtk` (a git
  dependency, added in Phase 2) with the house Kanagawa Dragon
  palette. Never reach for an `Adw.` widget.
- Editor surface: `sourceview5` 0.11, which needs the
  `gtksourceview5-devel` system package from Phase 2 on.
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

## House rules that bite here

- spec.md is the contract: semantics changes update spec.md first.
- Never assume an installed font: the app bundles OFL fonts with
  generic fallbacks (Phase 3).
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
  behind the answers column; style scheme XML format.
- `gnome-calculator` (GPL-3.0): currency-provider caching pattern
  (ECB XML under `~/.cache`, stale works offline).

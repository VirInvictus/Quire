# Patchnotes

Newest first. The v0.1.0 entry accumulates until the first tagged
release.

## v0.1.0 (unreleased)

- Initial skeleton: two-crate cargo workspace (`quire-eval` engine +
  `quire` app stub), the sheet line model with classification tests,
  and the standard doc set (spec, roadmap, this file).
- Roadmap expanded to task level from reference-repo research
  (notecalc3, kalker, numbat, gnome-text-editor, gtksourceview,
  gnome-calculator); the Phase 2 results column is pinned to a
  right-edge GtkSourceGutterRenderer.
- Phase 1 engine: tokenizer, recursive-descent parser, and evaluator
  for the full spec Semantics set (arithmetic, the four percent
  forms, variables, `answer`, heading-or-total bounded `total`,
  span-carrying errors, comma-grouped 12-significant-digit
  formatting). Table tests, file-driven `.quire` script sheets with
  golden answers and error assertions, an adversarial corpus, and
  house-shaped CI.
- Markdown-surface research applied to the roadmap: a custom `quire`
  GtkSourceView language spec and scheme pair for Phase 3, contextual
  list continuation and a heading-jump popover for Phase 4, a gated
  Phase 8 preview decision with researched options, and a recorded
  deliberately-skipped list. Apostrophe and Marker cloned as
  references.
- Phase 2 window and live results: GTK4 app on vir-gtk Kanagawa
  theming (dark/light portal flips), GtkSourceView editor with
  bundled JetBrains Mono, and the answers column as a right-gutter
  GutterRenderer: per-line values in the palette heading tone, errors
  in red, idle-coalesced live re-evaluation. Verified on-desktop with
  screenshots; the debugging pass fixed 0-based gutter numbering and
  a Pango width/translate clip bug.

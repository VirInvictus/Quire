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
- Phase 3 typography and the markdown surface: a custom `quire`
  GtkSourceView language spec (fork of the in-tree markdown lang plus
  `//` comments and whole math-line styling) and the quire-dark /
  quire-light scheme pair in Kanagawa Dragon and Lotus tones,
  selected by the portal dark/light state. The sheet now renders
  three voices: dimmed italic comments, blue math lines, and bold
  yellow answers; the answers column reads its colors from the
  scheme, and the column tone plus hairline are drawn by the
  renderer across the full column height. Verified on-desktop with
  screenshots at 1x.
- The semi-highlighted math tokens fixed: the quire.lang math-line
  context is a start/end context now (a plain match styled only the
  matched span, which lit `groceries` but left `= 42.50` gray).
  Toggle-able line numbers added to the Phase 4 roadmap on request.
- Typography fixes from Brandon's first live session, round two:
  the 1px glyph-top shave turned out to be pixel snapping at 15px
  (JetBrains Mono's fractional 19.8px line height makes every
  baseline land off-pixel) - the sheet now renders at 20px, which
  lands clean and reads better anyway, and the answers renderer
  pixel-snaps its draw position. Bare identifier lines are also
  highlighted as references now (the quire.variable style), matching
  the evaluation added above.
- Token-level sheet coloring per Brandon's design: variables render
  blue, numbers and operators in the default foreground (the brief
  whole-line blue overshoot lasted one commit). The math-line context
  is a zero-width-lookahead start/end region so the identifier
  sub-context can see past the line's opening token. README.md and
  AGENTS.md brought current with the shipped reality, and the docs
  flow is now written into AGENTS.md as a checklist.
- Phase 4 document backbone: single-window model with new / open /
  save / save-as (FileDialog), dirty tracking with an
  unsaved-changes guard, recent-sheets menu, window-size and
  last-folder persistence in GSettings (schema compiled at startup
  into the user data dir), file monitoring that reloads clean sheets,
  and a toggle-able line-numbers gutter (menu + Ctrl+L). Live
  session caught the silent GSettings bind no-op (the BindingBuilder
  must be .build()-ed).
- Save round-trip verified live (Ctrl+S through the FileDialog,
  file lands with the sheet contents, recents and title update) and
  the first-press crash fixed: the `last-folder` key is a maybe-string
  (`ms`), and reading a nothing-variant through the plain string
  getter panicked - it now goes through the typed variant API as
  Option<String>.
- Three user-facing fixes from Brandon's first live session: TextView
  `line-height` clipped glyph ascenders on every line (the property is
  dropped; GTK clips whenever it is set), the caret was invisible on
  the dark canvas (the scheme now carries a `cursor` style), and bare
  identifier lines now evaluate as references - `groceries` on its own
  line shows its value when bound above and stays plain text when not
  (spec.md amended; unbound bare names never error).
- Phase 4 completion: drag-and-drop opening, a heading-jump outline
  button rebuilt from the engine's sheet index, Tab completion of
  variable names (unique-match rule), Ctrl+C copying the current
  line's answer, and Ctrl+B jumping to a variable's definition.
  Verified on-desktop: Tab completion live-typed and confirmed.
- The unsaved-changes Discard button works. The dead button was a
  vir-gtk bug: an Alert's response state lived only as long as the
  caller's Alert value, so the fire-and-forget present in the close
  guard dropped it before any click could answer (the dialog closed
  and nothing else happened; only saving made the window closable).
  Fixed upstream in vir-gtk 1.4.3, which anchors the response state to
  the dialog window; Quire's lockfile pins the new revision. Verified
  on-desktop: dirty sheet, close, Discard ends the window; the
  regression test lives in vir-gtk.

# Patchnotes

Newest first.

## Unreleased

- Functions reach the unit engine: unit lines call user functions
  and numbat dimension-checks them - `twice(x) = x * 2` makes
  `twice(2 kg)` answer `4 kg`. Only the functions a line calls are
  seeded, definitions whose bodies use `total`/`answer`/percent
  stay scalar-only, and function names colliding with numbat
  builtins (`double` is a prelude constant) fail just the calling
  lines. The bridge also went stateless: every unit evaluation
  clones the pristine master context, so mid-sheet redefinitions of
  functions and variables apply immediately.

## v0.2.0 (2026-10-07)

- Hovering a line shows its answer in full: the answers column clips
  long text, and a clipped error is a useless error - numbat's
  messages never fit. The tooltip is the whole cell text (errors and
  long answers alike), the whole line is the hover target, and the
  column keeps its size.
- Currency, offline-first: `50 USD -> EUR` converts through the
  unit engine, backed by an ECB daily reference-rate snapshot.
  The app fetches the ECB XML on demand (attohttpc, rustls - the
  same client numbat's own fetch uses), caches it under
  `~/.cache/quire/ecb.xml`, and seeds the engine from the cache at
  startup: once fetched, currency works offline forever after. The
  refresh interval is a GSettings key (24h default) with a menu
  item to force a fetch; the engine's rates are set-once per
  process, so a refresh applies from the next launch. A sheet
  without rates sees unknown names, never a hang - and the
  starter sheet carries the syntax as an uncomment line so an
  offline first run stays error-free.
- An answer-decimals setting: the menu's "Answer decimals" radio
  (Full, 0-4) fixes how many decimal places scalar answers show,
  persisted in GSettings and applied live - `14.5` at two places
  reads `14.50`, trailing zeros kept. A setting that would zero a
  value out keeps the full rendering instead, so `1/3` with a
  zero-decimal setting still reads `0.333333333333`. spec.md's
  Results section notes the rule.
- Result-format cycling: Alt+Up/Down on a line cycles its answer
  between the standard rendering, fixed two decimals (the
  allocation-lines fix - `41.4519906323` becomes `41.45`), hex, and
  binary. View-only, per line; the sheet stays clean and the choice
  shifts with edits until the stable-ids gate lands. spec.md's
  Results section notes the feature.
- Tags land (Phase 8 pulled forward): an expression line may end
  with `@tag` labels, and `total @tag` sums that group - the
  keystone for budgets and portfolios. Tagged results count toward
  the plain total as well; `total @a @b` sums the union of two
  tags; tagged totals are pure views that reset nothing, while
  headings and plain totals clear the tag sums. Tags ride the unit
  engine (`5 kg @bulk` totals as `5 kg`), and `@` followed by
  anything that is not an identifier stays an error, saving `@` +
  date for Phase 9's dated snapshots. spec.md gained the Tags
  section; corpus in `tests/scripts/tags.quire`.
- The plugin question was asked and ruled: budgets, portfolios, and
  future extensions are sheet primitives, not plugins - the spec's
  no-plugin non-goal stands (months of platform tax before one
  feature ships, and numbat's module system already serves as the
  unit-layer extension seam); revisit only if a real extension need
  appears post-1.0.
- The currency ruling is in: the no-network non-goal is now "no
  live prices, ever" with an optional offline-first daily
  reference-rate fetch (ECB XML cached under ~/.cache/quire, stale
  cache fully functional). The ECB layer through numbat's
  `set_exchange_rates` seam is greenlit for Phase 6; conversion
  support lands with it.
- Phase 6 opens: the unit layer. quire-eval embeds numbat (MIT OR
  Apache-2.0, built with its network-fetch and plotting features
  off) as the engine for expression lines the scalar path declines:
  `5 kg + 300 g` answers `5300 g`, `2 hours + 30 minutes` answers
  `150 min`. Quantity results bind like any result - toward
  `answer`, toward totals, visible to lines below - and `total`
  sums like-dimensioned quantities through the engine, refusing a
  mixed sum on the total line only. `bag + 300 g` stays prose (the
  identifier-led rule covers quantities too); `(bag) + 300 g` asks
  for the arithmetic. The scalar engine is untouched and the whole
  existing corpus passes byte-identical; the unit corpus lands as
  `tests/scripts/units.quire`. spec.md gained the "Unit
  expressions" section (amended first, per the contract). The
  embedded engine loads its prelude once per process and clones it
  per keystroke pass (~1 ms); currency waits on the no-network
  non-goal ruling, dates ride a later chunk.

## v0.1.0 (2026-10-06)

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
- Auto list continuation on Enter closes Phase 4. Enter on a list
  line continues it: bullets repeat as-is, `1.` / `1)` numbering
  increments, and indentation carries over. Enter on an empty item
  (just a marker) exits the list, and Enter on a math line, heading,
  comment, or reference stays a plain newline so the sheet keeps
  evaluating. The decision is a pure function with table tests
  (`lists.rs`); the Enter hook refuses modified keys and selections.
- Phase 5 packaging: a real hicolor icon set (the answers-column
  tile, Brandon's pick from three rendered candidates, replacing the
  placeholder logo), a desktop file, AppStream metainfo
  (appstreamcli-validated on every push and in the release job), a
  shared-mime-info package registering `.quire` sheets, and a Meson
  wrapper (the house shape) that installs all of it: the binary into
  bindir, the desktop entry, icons, mime package, and the GSettings
  schema compiled at install. The app stays self-contained: fonts,
  the language spec, and the schemes still extract to the user dirs
  at startup, so a bare `cargo build` and an installed Quire are
  equally whole. The tag-gated release job builds in a pinned
  fedora:44 container, runs the full checks, stages the install
  tree, and attaches `quire-vX.Y.Z-x86_64.tar.zst` to the GitHub
  release it creates from the tag's verbatim message.

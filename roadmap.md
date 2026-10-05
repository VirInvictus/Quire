# Roadmap

Phases are sequential; sub-tasks inside a phase are not. The
task-level detail below (2026-10-05) comes from studying six reference
clones: notecalc3 (AGPL: semantics only, never code), kalker (MIT),
numbat (MIT OR Apache-2.0), gnome-text-editor (GPL), gtksourceview
(LGPL, the library we link), gnome-calculator (GPL). Tasks marked
**[D]** are decision gates: they get an explicit prompt before the
work lands, with the current recommendation recorded here.

- [x] **Phase 0: Project skeleton.** Two-crate workspace, standard doc
  set, MIT, placeholder logo, `quire-eval` sheet line model with
  classification tests. (2026-10-05)
  - [x] Reference shelf cloned and cataloged: notecalc3, kalker,
    numbat, gnome-text-editor, gtksourceview, gnome-calculator.
    (2026-10-05)

- [ ] **Phase 1: `quire-eval` core.** Goal: the whole Semantics section
  of spec.md, green.
  - Tokenizer
    - [ ] Tokens with byte spans on every token. Kalker's missing
      error spans are the cautionary tale; we never ship an error
      without a position.
    - [ ] Number literals, identifiers, keywords (`total`, `answer`,
      `of`), operators, parentheses; `//` comment stripping.
    - [ ] `in`-style unit conversion stays out until Phase 6; reserve
      nothing in the grammar that would collide with prose.
  - Parser
    - [ ] Recursive descent, one function per precedence level
      (kalker's shape): `+ -` over `* /` over right-associative `^`
      over unary minus over postfix `%` over primary.
    - [ ] The four percent forms land in the term/primary positions
      per spec; `%` on the left of `+`/`-` is an error (NoteCalc
      behavior).
    - [ ] Assignment as a statement form (`name = expr`), not
      smuggled through the expression parser (kalker's `WasStmt`
      sentinel is the anti-pattern).
    - [ ] AST nodes carry spans.
  - Evaluator
    - [ ] `Context` struct: ordered bindings, nearest-binding-above
      wins; `answer`; totals state.
    - [ ] **[D]** `total` reset boundary. NoteCalc resets its sum at
      each heading; our spec says previous `total`. Recommendation:
      both act as boundaries (a heading sections a sheet).
    - [ ] Per-line `LineResult`: value or error with span and message;
      referencing a failed line poisons the referencing line with a
      clear message (NoteCalc behavior).
    - [ ] Error enum with Display strings, span-carrying throughout.
    - [ ] Result formatting per spec: thousands grouping, 12
      significant digits, trimmed zeros.
  - Tests
    - [ ] Table unit tests per feature: classification, parser,
      percent forms, variables, `answer`, `total`, errors.
    - [ ] File-driven engine tests: `tests/*.quire` scripts whose last
      line must hold true, auto-registered as cases (kalker's
      integration-testing pattern).
    - [ ] Golden sheet tests: whole sheet text against its expected
      results column, including every example in spec.md.
    - [ ] Evaluator totality: no panic on arbitrary input; a
      hand-built adversarial corpus first (property testing via
      `proptest` is a dependency ask, deferred and gated).
    - [ ] Sign-off gate: test plan and the **[D]** total boundary go
      to Brandon before implementation.
  - CI (tail of the phase)
    - [ ] GitHub Actions to the house shape: SHA-pinned actions,
      least-privilege permissions, concurrency cancellation, timeouts,
      pinned toolchain; `cargo fmt --check`, `clippy -D warnings`,
      `cargo test`.

- [ ] **Phase 2: Window and live results.** Goal: type `2 + 2`, see
  `4` aligned to that line.
  - Setup
    - [ ] Install `gtksourceview5-devel` (the one system package this
      phase needs).
    - [ ] Add `gtk4` 0.11 + `sourceview5` 0.11 deps; `vir-gtk` git dep
      for the house Kanagawa stylesheet.
    - [ ] App skeleton mirroring the gnome-text-editor shape:
      `QuireApplication` -> `QuireWindow` -> `QuirePage` (a
      `GtkSourceBuffer` subclass plus a `sourceview::View` subclass).
  - Results column (the recipe, verified against GtkSourceView 5.22
    source)
    - [ ] `AnswersRenderer`: Rust subclass of `gtk_source::GutterRenderer`;
      override `WidgetImpl::measure` (fixed column width), `begin` /
      `end` (per-frame caches), `query_data` (line to answer lookup),
      `snapshot_line` (draw via `get_line_extent` + `align_cell`).
    - [ ] Attach at `view.gutter(TextWindowType::Right)` with position
      0; scroll sync is free (the gutter tracks the vadjustment and
      repaints the visible rect).
    - [ ] Wrapped lines: `alignment_mode = CELL`; never hardcode line
      height (gnome-text-editor ships a `line-height` GSettings key;
      always read `get_line_extent`).
    - [ ] Answers model: a line-number-keyed map rebuilt on buffer
      change; only visible lines get painted, so per-frame cost stays
      bounded.
    - [ ] Error cells: short message in the Kanagawa red; token-level
      red highlighting is Phase 7 polish.
    - [ ] Collapse: renderer `visible = false` when the sheet has no
      expression lines (an empty gutter draws nothing).
  - Evaluation wiring
    - [ ] Re-evaluate on buffer change and measure whether debouncing
      matters: NoteCalc runs synchronous per keystroke at notepad
      scale and is fine. **[D]** if measurement contradicts the spec's
      "debounced", amend the spec.
    - [ ] Reserve `snapshot_layer(BELOW_TEXT)` row painting for a
      cursor-line expression highlight.

- [ ] **Phase 3: Typography.**
  - [ ] A Quire GtkSourceView style scheme (Kanagawa Dragon) as XML,
    shipped via the app gresource and loaded with
    `StyleSchemeManager::prepend_search_path` on a `resource://` URI
    (the gnome-text-editor pattern).
  - [ ] **[D]** Bundled font choice: JetBrains Mono, Iosevka, Martian
    Mono, or IBM Plex Mono (all OFL). NoteCalc bundles JetBrains Mono.
    Recommendation: JetBrains Mono for the numeric surface at v1.
  - [ ] Font loaded from the app's resources with a generic fallback
    chain; nothing assumes an installed font. Optional override via
    the `use-system-font` / `custom-font` GSettings pattern.
  - [ ] Refresh cached glyphs and colors in `css_changed` so font or
    scale changes never desync the answers column (the line-numbers
    renderer's gotcha).
  - [ ] Renderer colors read from the scheme (`style("line-numbers")`,
    `style("text")`), not hardcoded CSS.
  - [ ] **[D]** Light variant (Kanagawa Lotus): the scheme metadata
    supports a dark-variant pairing; recommend deferring light mode
    past 1.0.
  - [ ] Display pass with Brandon at 1x and 2x scale.

- [ ] **Phase 4: Documents and editing UX.**
  - [ ] GSettings schema: window size, last folder, wrap,
    show-line-numbers, font override, style variant.
  - [ ] Open/save via FileDialog; plain UTF-8, LF, no BOM.
  - [ ] Unsaved-changes guard (the gnome-text-editor
    save-changes-dialog shape).
  - [ ] File-changed-on-disk monitoring: at minimum detect and warn
    (gnome-text-editor's buffer-monitor pattern).
  - [ ] Recent sheets list (ordered, in GSettings).
  - [ ] Drag-and-drop a text file onto the window opens it.
  - [ ] Variable-name completion via GtkSourceCompletion; NoteCalc's
    rule is enough for v1: Tab completes only on a unique match.
  - [ ] Ctrl+C with no selection copies the current line's answer;
    Ctrl+B jumps to a variable's definition (cheap NoteCalc wins).
  - [ ] **[D]** One window with one sheet for v1 (recommend) vs tabs.

- [ ] **Phase 5: Packaging.**
  - [ ] Real icon replacing the placeholder logo; hicolor sizes.
  - [ ] Desktop file and AppStream metainfo at
    `io.github.virinvictus.Quire`, validated with appstreamcli.
  - [ ] **[D]** Build system: adopt Meson (house GTK-app precedent:
    resources, desktop file, metainfo, VERSION stamping) vs stay
    cargo-only with an install script. Recommendation: Meson, at this
    phase.
  - [ ] Release CI: tag-gated job that builds the release binary and
    attaches it to the GitHub release (house rule: releases carry the
    real artifact).
  - [ ] **[D]** Flatpak manifest (`org.gnome.Platform` 50, VirInvictus
    app-id): recommend deferring past 1.0; it is its own decision in
    the house precedent.

- [ ] **Phase 6: Currency, units, dates.**
  - [ ] **[D]** The gate: own implementation vs embedding numbat.
    Research points hard at embed: `numbat::Context::new_without_importer()`
    plus `interpret()` gives per-evaluation snapshot-and-rollback (a
    failing line leaves no state), spans on all errors, and MIT.
    Recommendation: embed.
  - [ ] If embed: sheet lines compile to numbat source; quantities
    carry their units; our formatter renders the answers column.
  - [ ] Currency with an owned fetching layer through numbat's
    `set_exchange_rates` seam: ECB daily XML, cached under
    `~/.cache/quire` like gnome-calculator's providers (stale cache
    works offline; refresh interval in GSettings).
  - [ ] Dates ride numbat's jiff-backed module (`datetime()`, `now()`,
    `calendar_add`, `-> tz(...)`).
  - [ ] Completion source: numbat's `variable_names()` /
    `unit_names()` / `get_completions_for()`.
  - [ ] Spec amendment pass: grow spec.md Semantics with whatever
    lands.

- [ ] **Phase 7 (post-0.1.0): Soulver-depth semantics.**
  - [ ] **[D]** Stable line references: NoteCalc's `&[line-id]` model
    with a picker UI vs `answer` only; line numbers shift on insert,
    so true references need stable ids. Recommendation: stable ids
    with the interactive chooser.
  - [ ] Mixed-line evaluation (NoteCalc's classify-by-failure):
    `50 apples at 3 EUR` evaluates the math and demotes the words.
    Gated on golden prose tests.
  - [ ] Reverse percent forms: `41 is 17% on what`, `20 is what
    percent of 60`.
  - [ ] Implicit multiplication (`2pi`, `3(4+5)`) with a dedicated
    ambiguities test folder (kalker pattern).
  - [ ] User functions: `name(params) = ...`.
  - [ ] Token-level error highlighting in the editor.
  - [ ] Alt+Up/Down cycles a line's result format (dec/hex/bin).
  - [ ] Region model: decimal-point-aligned answers per heading region
    (NoteCalc's renderer alignment).

- [ ] **v0.1.0 release** after Phase 4: bump VERSION, patchnotes,
  annotated tag with the patchnotes entry verbatim, GitHub release
  with the built artifact. A Flatpak is not required for the first
  tag.
- [ ] **1.0** after Phase 6: Brandon's display passes, docs truth pass
  (doc-drift-auditor), release-auditor pre-flight before the tag.

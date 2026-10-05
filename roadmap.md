# Roadmap

Phases are sequential; sub-tasks inside a phase are not. The
task-level detail (2026-10-05) comes from studying eight reference
clones: notecalc3 (AGPL: semantics only, never code), kalker (MIT),
numbat (MIT OR Apache-2.0), gnome-text-editor (GPL), gtksourceview
(LGPL, the library we link), gnome-calculator (GPL), Apostrophe
(GPL-3.0; the v2.4 clone is GTK3, upstream 2.6+ moved to GTK4) and
Marker (GPL-3.0, GTK3), plus a survey of the wider markdown-app
landscape (Zettlr, MarkText, ghostwriter, ReText, QOwnNotes). Tasks
marked **[D]** are decision gates: they get an explicit prompt before
the work lands, with the current recommendation recorded here.

- [x] **Phase 0: Project skeleton.** Two-crate workspace, standard doc
  set, MIT, placeholder logo, `quire-eval` sheet line model with
  classification tests. (2026-10-05)
  - [x] Reference shelf cloned and cataloged: notecalc3, kalker,
    numbat, gnome-text-editor, gtksourceview, gnome-calculator.
    (2026-10-05)
  - [x] Markdown-app research shelf: Apostrophe and Marker cloned;
    both sweeps applied to Phases 3, 4, 7, and 8. (2026-10-05)

- [x] **Phase 1: `quire-eval` core.** Goal: the whole Semantics section
  of spec.md, green. (2026-10-05)
  - Tokenizer
    - [x] Tokens with byte spans on every token. Kalker's missing
      error spans are the cautionary tale; we never ship an error
      without a position.
    - [x] Number literals, identifiers, keywords (`total`, `answer`,
      `of`), operators, parentheses; `//` comment stripping.
    - [x] `in`-style unit conversion stays out until Phase 6; reserve
      nothing in the grammar that would collide with prose.
  - Parser
    - [x] Recursive descent, one function per precedence level
      (kalker's shape): `+ -` over `* /` over right-associative `^`
      over unary minus over postfix `%` over primary.
    - [x] The four percent forms land in the term/primary positions
      per spec; `%` on the left of `+`/`-` is an error (NoteCalc
      behavior).
    - [x] Assignment as a statement form (`name = expr`), not
      smuggled through the expression parser (kalker's `WasStmt`
      sentinel is the anti-pattern).
    - [x] AST nodes carry spans.
  - Evaluator
    - [x] `Context` struct: ordered bindings, nearest-binding-above
      wins; `answer`; totals state.
    - [x] **[D]** `total` reset boundary: both a heading and a
      previous `total` act as boundaries (Brandon's pick, 2026-10-05).
    - [x] Per-line `LineResult`: value or error with span and message;
      referencing a failed line poisons the referencing line with a
      clear message (NoteCalc behavior).
    - [x] Error enum with Display strings, span-carrying throughout.
    - [x] Result formatting per spec: thousands grouping, 12
      significant digits, trimmed zeros.
  - Tests
    - [x] Table unit tests per feature: classification, parser,
      percent forms, variables, `answer`, `total`, errors.
    - [x] File-driven engine tests: `tests/*.quire` scripts with
      `//=` golden answers, `# expect:` result assertions, and
      `# err:` failure assertions, auto-run by a directory walker
      (kalker's integration-testing pattern).
    - [x] Golden sheet tests: whole sheet text against its expected
      results column, including every example in spec.md
      (`scripts/spec-examples.quire`).
    - [x] Evaluator totality: no panic on arbitrary input; an
      adversarial corpus covers depth bombs, hostile unicode, and
      pathological literals (property testing via `proptest` stays a
      gated future ask).
    - [x] Sign-off gate: test plan and the **[D]** total boundary
      approved by Brandon before implementation (2026-10-05).
  - CI (tail of the phase)
    - [x] GitHub Actions to the house shape: SHA-pinned actions,
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

- [ ] **Phase 3: Typography and the markdown surface.**
  - Language spec (the markdown research, 2026-10-05)
    - [ ] Custom GtkSourceView language spec under a new `quire` id:
      a fork of the in-tree `markdown.lang` keeping its header,
      list-marker, emphasis, and code contexts, plus Quire additions:
      `//` mapped to `def:comment` (the stock markdown lang maps
      nothing there) and a math-expression-line context (the
      `latex.lang` inline-math precedent). Shipped via the app
      gresource with `LanguageManager::set_search_path` before first
      load. Do NOT override the stock markdown lang id: sheets are
      not markdown, markdown.lang exposes no hook context, and it
      misclassifies a lone `---` as a setext heading.
    - [ ] Every custom style keeps a `map-to="def:*"` fallback so
      stock schemes still render sensibly if one is ever loaded.
  - Style scheme
    - [ ] A Quire scheme pair (`quire.xml` + `quire-dark`) with
      `parent-scheme` + `dark-variant` metadata, styling
      `quire:header`, `quire:list-marker`, comments, and a
      `quire:math` context, in the Kanagawa Dragon palette.
    - [ ] Scheme shipped via gresource +
      `StyleSchemeManager::prepend_search_path` on a `resource://`
      URI (the gnome-text-editor pattern).
    - [ ] The answers column reads `quire:result` from the active
      scheme via `get_style` so computed values match the theme from
      one source of truth.
  - Fonts
    - [ ] **[D]** Bundled font choice: JetBrains Mono, Iosevka,
      Martian Mono, or IBM Plex Mono (all OFL). NoteCalc bundles
      JetBrains Mono and Apostrophe bundles Fira; recommendation:
      JetBrains Mono for the numeric surface at v1.
    - [ ] Font loaded from the app's resources with a generic fallback
      chain; nothing assumes an installed font (the Apostrophe
      `@font-face` + `local()` + remote-fallback chain is the
      pattern). Optional override via the `use-system-font` /
      `custom-font` GSettings keys.
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
  - [ ] Auto list continuation on Enter, contextual: continue `- ` or
    numbering only when the line Enter was pressed on is a list or
    prose line, never on a math line (the one near-universal markdown
    editing behavior; Enter on an expression must stay a plain
    newline so the sheet keeps evaluating).
  - [ ] Heading-jump outline popover: headings are already first-class
    in the line model, and the markdown-app survey names the outline
    as the one structure feature that pays for itself in a
    math-first app.
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
    Gated on golden prose tests. (First candidate to pull forward:
    writing natural sheets keeps hitting the strict-shape rule, three
    times during Phase 1 alone.)
  - [ ] Reverse percent forms: `41 is 17% on what`, `20 is what
    percent of 60`.
  - [ ] Implicit multiplication (`2pi`, `3(4+5)`) with a dedicated
    ambiguities test folder (kalker pattern).
  - [ ] User functions: `name(params) = ...`.
  - [ ] Token-level error highlighting in the editor.
  - [ ] Ctrl+click a math line opens a popover with its step-by-step
    breakdown (Apostrophe's inline-preview popover pattern, minus the
    latex subprocesses).
  - [ ] Task-list checkbox toggling: the one GFM extra worth
    revisiting, since a notepad is a natural checklist.
  - [ ] Alt+Up/Down cycles a line's result format (dec/hex/bin).
  - [ ] Region model: decimal-point-aligned answers per heading region
    (NoteCalc's renderer alignment).

- [ ] **Phase 8 (post-1.0, gated): rendered preview, if ever.**
  - [ ] **[D]** The gate itself. Research read (2026-10-05): Quire's
    answers column IS the preview; the default is to never embed
    WebKit (webkit2gtk-4.1 is ~134 MB installed plus helper
    processes, disproportionate for a calculator). If a rendered view
    is ever justified, two shapes are on the table: (a) webkitgtk-6.0
    (the GTK4 API, stable since 2.40, Skia-based since 2.46) fed by
    pulldown-cmark (MIT, near-zero deps) generating HTML with math
    lines pre-masked (sheets are not valid CommonMark), theme-paired
    CSS via custom properties, and Apostrophe's scroll-scale sync;
    or (b) a fully native Pango-rendered formatted view (the
    html2pango path Fractal uses). Pandoc stays an export path,
    never a bundled engine (it is the heaviest module in Apostrophe's
    flatpak).

## Deliberately skipped (markdown-app research, 2026-10-05)

Recorded so future sessions do not re-litigate: smart punctuation and
typographic quotes (hostile to literal math input), typewriter and
focus/Hemingway modes (prose-drafting aids; revisit only by request),
image paste and asset management, footnote and grid-table editor UIs,
spellcheck in v1 (the path if ever wanted: libspelling with math lines
excluded), pandoc as a bundled engine, and WebKitGTK as a preview pane
(see Phase 8).

- [ ] **v0.1.0 release** after Phase 4: bump VERSION, patchnotes,
  annotated tag with the patchnotes entry verbatim, GitHub release
  with the built artifact. A Flatpak is not required for the first
  tag.
- [ ] **1.0** after Phase 6: Brandon's display passes, docs truth pass
  (doc-drift-auditor), release-auditor pre-flight before the tag.

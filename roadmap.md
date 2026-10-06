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

- [x] **Phase 2: Window and live results.** Goal: type `2 + 2`, see
  `4` aligned to that line. (2026-10-05)
  - Setup
    - [x] Install `gtksourceview5-devel` (5.20.0; Brandon ran the dnf
      install by hand after run0/polkit proved dead from tool shells).
    - [x] Add `gtk4` 0.11 + `sourceview5` 0.11 deps; `vir-gtk` git dep
      for the house Kanagawa stylesheet.
    - [x] App skeleton mirroring the gnome-text-editor shape:
      `QuireApplication` -> `QuireWindow` -> `QuirePage` (a
      `GtkSourceBuffer` subclass plus a `sourceview::View` subclass).
  - Results column (the recipe, verified against GtkSourceView 5.22
    source)
    - [x] `AnswersRenderer`: Rust subclass of `gtk_source::GutterRenderer`;
      landed simpler than the recipe: a fresh Pango layout per cell
      per frame (nothing to invalidate), `measure` gives the fixed
      column width, and `align_cell` gets the full inner column width
      so the right-aligned layout ends flush at the pad edge.
    - [x] Attach at `view.gutter(TextWindowType::Right)` with position
      0; scroll sync is free (the gutter tracks the vadjustment and
      repaints the visible rect).
    - [x] Wrapped lines: `alignment_mode = CELL`; never hardcode line
      height (gnome-text-editor ships a `line-height` GSettings key;
      always read `get_line_extent`).
    - [x] Answers model: a line-number-keyed map rebuilt on buffer
      change; only visible lines get painted, so per-frame cost stays
      bounded.
    - [x] Error cells: short message in the Kanagawa red; token-level
      red highlighting is Phase 7 polish.
    - [x] Collapse: renderer `visible = false` when the sheet has no
      expression lines (an empty gutter draws nothing).
  - Evaluation wiring
    - [x] Re-evaluate on buffer change, coalesced to the next idle
      turn (the spec's "debounced"; whole-sheet evaluation is O(n) on
      tiny sheets and one deferred pass per burst is plenty). No
      **[D]** needed: the spec text is satisfied as written.
    - [x] Cold-start repaint safety net: a one-shot queue_draw 400ms
      after page build, because a first-ever run can paint its
      earliest frames against a just-installed font cache.
  - Display pass
    - [x] Launched on the Hyprland desktop and screenshot-verified:
      values in the Kanagawa carpYellow right-aligned at the pad
      edge, errors in the Kanagawa red, JetBrains Mono throughout.
      Two bugs found and fixed in the pass (0-based gutter line
      numbers vs 1-based sheet numbers; a width-set right-aligned
      Pango layout translating its glyphs past the renderer clip).
    - [ ] Gutter chrome finding, carried to Phase 3: CSS background
      and border-left on the Gutter widget's class do not paint; the
      hairline separator and column tone need a different mechanism
      (renderer-drawn line or CSS node investigation).

- [x] **Phase 3: Typography and the markdown surface.** (2026-10-05)
  - Language spec (the markdown research, 2026-10-05)
    - [x] Custom GtkSourceView language spec under the new `quire` id
      (`resources/styles/quire.lang`): a fork of the in-tree
      `markdown.lang` keeping its header, list-marker, emphasis, and
      code-span contexts, plus Quire additions: `//` mapped to
      `def:comment` and a whole math-expression-line context. Shipped
      by runtime extraction to `~/.local/share/quire/lang/` with
      `LanguageManager::set_search_path` set before the first buffer
      (the gresource variant waits for the Phase 5 Meson decision).
      Do NOT override the stock markdown lang id: sheets are not
      markdown, and markdown.lang exposes no hook context.
    - [x] Every custom style keeps a `map-to="def:*"` fallback so
      stock schemes still render sensibly if one is ever loaded.
    - [x] Engine gotcha worth remembering: a `.lang` needs a ROOT
      context whose id matches the language id (`<context
      id="quire">`); without it the file loads, `language("quire")`
      returns Some, and the highlight engine silently does nothing.
    - [x] Extended-mode gotcha, same pass: `extended="true"` regexes
      treat a bare `#` as a comment start, so the header rule written
      as `#{1,6}` degenerated to match-every-line (whole sheet went
      header-yellow). The header context is a plain single-line match.
  - Style scheme
    - [x] Scheme pair (`quire-dark.xml` + `quire-light.xml`) styling
      `quire:header`, `quire:list-marker`, comments, `quire:math`,
      `quire:result`, and `quire:error` in the Kanagawa Dragon /
      Lotus palettes; selection follows the portal dark/light state.
    - [x] Schemes shipped by the same runtime extraction +
      `StyleSchemeManager::prepend_search_path` (gnome-text-editor
      pattern; gresource at Phase 5).
    - [x] The answers column reads `quire:result` / `quire:error`
      from the buffer's scheme via `style()` so computed values match
      the theme from one source of truth (palette hexes remain as the
      fallback path).
  - Fonts and chrome
    - [x] **[D]** Bundled font: JetBrains Mono (Brandon's pick,
      2026-10-05); OFL files live in `crates/quire/resources/fonts/`
      with the license text.
    - [x] Font installed at startup under `~/.local/share/fonts/Quire/`
      before GTK builds its font map, with a generic fallback chain in
      the stylesheet; nothing assumes an installed font. gresource
      loading revisits this at the Phase 5 Meson decision.
    - [x] Layout cache made safe by construction (fresh per frame);
      the renderer-lines `css_changed` invalidation dance only
      returns if a cache is ever reintroduced.
    - [x] Renderer colors read from the scheme (see `quire:result`
      above); palette hexes are the documented fallback.
    - [x] Gutter chrome settled as renderer-drawn: the renderer
      widget's own snapshot paints the column tone and the 1px
      hairline across the FULL widget box before the line loop draws
      text. Verified facts behind the choice: the Gutter widget
      ignores CSS backgrounds, and the view's below-text layer is
      clipped to the text window (under the gutter child), so neither
      surface can carry the chrome. The `snapshot_layer(BELOW_TEXT)`
      cursor-line highlight keeps its QuireView hook for later.
    - [x] **[D]** Light variant: BOTH schemes ship now (quire-light
      tones picked from the Lotus palette); deep Lotus polish stays a
      post-1.0 item. (Brandon's pick, 2026-10-05.)
    - [x] Rendering size 20px with pixel-snapped renderer positions
      (Brandon's live-session report: glyph tops shaved ~1px on every
      line at 15px - JetBrains Mono's fractional 19.8px line height
      lands every baseline off-pixel and GSK shaves the raster; 20px
      renders clean and reads better). Bare identifier lines also
      highlighted via a `bare-reference` context (quire.math style),
      matching the Reference evaluation.
    - [ ] Display pass with Brandon at 1x and 2x scale: 1x done and
      screenshot-verified during the phase; the 2x check (GDK_SCALE
      does not apply on Wayland) rides Brandon's display pass.

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
  - [ ] Toggle-able line numbers (Brandon's request): the
      GtkSourceView left gutter with its line-number renderer,
      behind a GSettings key and a menu/check action; default off
      (the answers column is the sheet's numbering).
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
    Gated on golden prose tests. (Partially pulled forward 2026-10-06:
    bare identifier references now evaluate when bound — Brandon hit
    the strict-shape rule live on his first session and it became a
    spec amendment; what remains here is mixed prose+math on one
    line and the `+`/`-` prose exception.)
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

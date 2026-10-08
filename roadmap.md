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
    - [x] Gutter chrome finding, carried to Phase 3 and resolved
      there: CSS background and border-left on the Gutter widget's
      class do not paint; the renderer itself draws the column tone
      and hairline (the ticked Phase 3 box).
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

- [x] **Phase 4: Documents and editing UX.** Document backbone
  shipped 2026-10-06 (GSettings schema compiled at startup into the
  user data dir; open/save/save-as via FileDialog; dirty tracking
  with the unsaved-changes guard on vir-gtk's Alert; recents in
  GSettings driving the header menu; file monitor reloading clean
  sheets; single-window model per Brandon's [D] pick; line-numbers
  toggle behind GSettings + Ctrl+L - the GSettings bind needed its
  BindingBuilder .build(), the silent no-op cost a debug session).
  The interactive save-dialog round-trip was verified live the same
  day (patchnotes). The guard's dead Discard button was a vir-gtk
  1.4.2 bug (Alert response state died with the dropped Alert value),
  fixed in vir-gtk 1.4.3 and adopted here the same day.
  - [x] GSettings schema: window size, last folder,
    show-line-numbers, recent files.
  - [x] Line-numbers toggle (menu + Ctrl+L), default off.
  - [x] Open/save/save-as via FileDialog; plain UTF-8.
  - [x] Unsaved-changes guard (vir-gtk Alert shape).
  - [x] File-changed-on-disk monitoring: reload while clean, stand
    down while dirty.
  - [x] Recent sheets list (ordered, in GSettings).
  - [x] Drag-and-drop a text file onto the window opens it.
  - [x] Auto list continuation on Enter, contextual: continue `- ` or
    numbering only when the line Enter was pressed on is a list or
    prose line, never on a math line (the one near-universal markdown
    editing behavior; Enter on an expression must stay a plain
    newline so the sheet keeps evaluating). Bullets repeat, numbering
    increments, and an empty item exits the list; the decision is
    pure functions in `lists.rs` with table tests.
  - [x] Heading-jump outline popover: headings are already first-class
    in the line model, and the markdown-app survey names the outline
    as the one structure feature that pays for itself in a
    math-first app.
  - [x] Toggle-able line numbers (Brandon's request): the
      GtkSourceView left gutter with its line-number renderer,
      behind a GSettings key and a menu/check action; default off
      (the answers column is the sheet's numbering).
  - [x] Variable-name completion via GtkSourceCompletion; NoteCalc's
    rule is enough for v1: Tab completes only on a unique match.
  - [x] Ctrl+C with no selection copies the current line's answer;
    Ctrl+B jumps to a variable's definition (cheap NoteCalc wins).
  - [x] **[D]** One window with one sheet for v1 (Brandon's pick,
    2026-10-06) vs tabs.

- [x] **Phase 5: Packaging.** (2026-10-06)
  - [x] Real icon replacing the placeholder logo; hicolor sizes. The
    answers-column tile (Brandon's pick from three rendered
    candidates): scalable SVG plus 128/256 PNGs in `data/icons/`,
    and `logo.svg` replaced with the same design.
  - [x] Desktop file and AppStream metainfo at
    `io.github.virinvictus.Quire`, validated with appstreamcli
    (every push via a CI job, and in the release job). Ships with a
    shared-mime-info package registering `application/x-quire` for
    `*.quire` sheets, and a real screenshot for the software-center
    entry.
  - [x] **[D]** Build system: Meson adopted (Brandon's pick,
    2026-10-06; the house shape: a thin cargo wrapper like Atrium's
    that owns the GNOME install layout). The app stays
    self-contained: fonts, language spec, and schemes still extract
    to the user dirs at startup, with or without the install.
  - [x] Release CI: a tag-gated job (pinned `fedora:44` container)
    runs the checks, validates the desktop file and metainfo, stages
    a Meson install tree (schemas compiled by hand under DESTDIR,
    where gnome.post_install deliberately skips), and attaches
    `quire-vX.Y.Z-x86_64.tar.zst` to the GitHub release it creates
    from the tag's verbatim message. (2026-10-06)
  - [x] **[D]** Flatpak manifest: deferred past 1.0 (Brandon's pick,
    2026-10-06; the roadmap recommendation accepted). Its own
    decision when the gate opens.

- [x] **Phase 6: Currency, units, dates.** (Complete 2026-10-08:
  both completion boxes landed.)
  - [x] **[D]** The gate: own implementation vs embedding numbat.
    Decided 2026-10-06, embed in the additive shape: the live gate
    prompt went unanswered, so the roadmap's recorded recommendation
    (embed) governs, refined to additive for userspace safety - the
    existing corpus stays byte-identical. Numbat joins with
    default-features off (no network code, no plotly). Full-compile
    (one engine for every line) remains the possible destination if
    the seams prove out.
  - [x] Units land additively (2026-10-06): spec.md "Unit
    expressions" amended first; quantity results bind like any
    result (`answer`, totals, visible below); totals sum
    like-dimensioned quantities through the engine and refuse mixed
    sums on the total line only; `bag + 300 g` stays prose and
    `(bag) + 300 g` asks for the arithmetic; quantity rendering is
    the engine's own notation. Corpus: `tests/scripts/units.quire`.
  - [x] Currency with an owned fetching layer through numbat's
    `set_exchange_rates` seam (2026-10-06): ECB daily XML fetched
    with attohttpc (numbat's own HTTP stack, rustls), cached under
    `~/.cache/quire/ecb.xml`, seeding the engine at startup - stale
    cache fully works offline, refresh interval in GSettings
    (`currency-refresh-hours`, default 24) with a menu item to force
    a fetch. The engine's rates are set-once per process, so a
    refresh applies from the next launch. Currency lines:
    `50 USD -> EUR`. Tour carries the syntax as an uncomment line
    (offline first-runs must stay error-free).
  - [x] Dates ride numbat's jiff-backed module (2026-10-07): bare
    date vocabulary (`today`, `now`, `tomorrow`, `yesterday`) and
    Soulver-style phrases (`3 weeks from today`, `90 minutes from
    now`, `6 months ago`) translate to datetime calls and answer as
    datetimes rendered `YYYY-MM-DD HH:MM UTC-offset`. Case-
    insensitive; composes with tags, totals, and functions. spec
    "Dates" section.
  - [x] Completion source (2026-10-08): the roadmap trio feeds the
    Tab completion - numbat's get_completions_for, the unit
    registry flattened (short aliases like `hours`), and the
    variable namespace - merged with sheet names bound above the
    cursor through `quire_eval::completion_names`; the app keeps
    NoteCalc's unique-match rule. Documented gaps: prefixed
    symbols (`kg`) are parse-time combinations, and typechecker
    constants (`pi`) have no public accessor - both evaluate when
    typed, they are just not candidates. spec "Completion".
  - [x] Spec amendment pass (2026-10-08): the doc-drift audit found
    seventeen drifts; the contract now tells the truth everywhere.
    Currency gained its own section (the cross-reference dangled
    since Phase 6); the Tags Summing and Functions Recursion bullets
    no longer contradict their own later amendments; percent bodies
    and `&N` in function bodies now MATCH the spec's scalar-only
    refusal in code (both silently did the opposite); the Testing
    section describes the per-feature corpora instead of a golden
    sheet that never held every example; the Expression shape lists
    the `&N` rule; Completion documents its three unreachable
    candidates.

- [x] **Phase 7 (post-0.1.0): Soulver-depth semantics.** (Complete
  2026-10-08.)
  - [x] Mixed-line evaluation (NoteCalc's classify-by-failure):
    `50 apples at 3 each` evaluates the math and demotes the words
    (shipped v0.4.0-v0.5.0; the word operators map, failures stay
    silent, and the skeleton parses strict so prose remnants keep
    their grammar). Bare identifier references pulled forward
    2026-10-06 on a live-session spec amendment.
  - [x] Reverse percent forms (2026-10-08): **[D]** gate, Brandon's
    picks: the find-the-percent form answers the fraction (Quire's
    percent-is-its-fraction convention; `30 is what percent of 200`
    is 0.15, no percent-typed display - the machinery was rejected
    at the rate gate), scope is the roadmap pair: the find-the-base
    triple `N is P% of|off|on what` (rewriting to `N / P%`,
    `N / (1 - P%)`, `N / (1 + P%)` through the existing
    relative-percent rules) plus `N is what percent of M` (`what %`
    reads the same). Soulver's remaining reverse shapes (as-a-%,
    change-between, proportion, odds) are deliberately out. The
    recognizer runs before the mixed skeleton (its `of` would
    otherwise multiply; the keyword tokenizes to its own `Of`
    token, which the matcher must accept). No-match keeps the
    silence. spec "Reverse percents"; corpus
    `tests/scripts/reverse-percent.quire`.
  - [x] Implicit multiplication (2026-10-08): `2pi`, `3(4+5)`,
    `(1+2)(3+4)`, `2 3` - kalker's rule verbatim: a number, bare
    name, or `(` directly after a value multiplies at `*`
    precedence, left-associative (`1/2pi` is `(1/2)*pi`), power
    binds tighter, and `name(` stays a call. Two riders the parser
    change forced: the mixed-line skeleton now parses strict
    (implicit lives on the expression path only, so prose remnants
    keep their grammar), and the unit fallback generalized - a
    scalar failure on a name the bridge knows hands the line over,
    because value-unit shapes (`5 kg + 300 g`) parse on the scalar
    path now. Honest errors surface from the first unknown name
    instead of the parser; three pins moved with that. Dedicated
    ambiguities folder: `tests/scripts/ambiguities/` (the runner
    walks subfolders now). spec "Operators" + the routing bullet.
  - [x] User functions: `name(params) = ...` (2026-10-06; inline
    bodies, params shadow sheet variables, redefinition wins below,
    depth-capped, arity-checked, no answer cell on the def line.
    Superseded since this box was written: recursion with literal
    clauses landed in v0.5.0 and functions reach the unit engine
    since v0.3.0 - spec "Functions" is the truth.) Corpus:
    `tests/scripts/functions.quire`.)
  - [x] **[D]** Stable line references (2026-10-07): `&N` visible
    references, any line above or below. Poisoning follows the
    failed-line rule; function bodies cannot use refs. spec "Line
    references".
  - [x] Self-updating refs (2026-10-07): tokens rewritten in place
    when lines shift, behind the follow-refs GSettings toggle
    (default on). The research sprint (five agents: spreadsheets,
    NoteCalc source, LSP/rope position mapping, Rust/GTK
    primitives, synthesis) converged on GtkTextMark identity with
    one batched blocked+irreversible splice pass in the debounced
    evaluation; the convergence logic is the GTK-free refs.rs
    module (20 table tests). Deleted targets follow the successor
    line; forward refs wait inert until their line is born; loaded
    sheets never rewrite. The three cascade-stripped attempts died
    because rewrites re-entered the changed pipeline and text diffs
    cannot distinguish undo from shift; marks have neither problem.
  - [x] Token-level error highlighting (2026-10-07): each failed
    line underlines the exact failing token in red in the sheet
    (engine spans, cleared and reapplied per pass), pairing with
    the hover tooltip - full message on hover, location in the
    sheet.
  - [x] Ctrl+click a math line opens a popover with its step-by-step
    breakdown (2026-10-08): the engine's explain_sheet_line records
    each scalar operation bottom-up in full sheet context (`3 * 4 =
    12`, then the sum; a relative percent reads `200 + 15% (of 200)
    = 230`), and the popover anchors at the click. Unit lines,
    definitions, and prose do not break down. spec "The breakdown".
  - [x] Task-list checkbox toggling (2026-10-08): GFM `- [ ]` /
    `- [x]` on list lines stays prose for the engine; Ctrl+clicking
    the box toggles one character as a user action undo reverses.
    spec "Task lists"; decisions in lists.rs with table tests.
  - [x] Answer-decimals setting (2026-10-06): a menu radio (Full,
    0-4 decimals) fixing the standard rendering's decimal places -
    exact places, padded (14.5 at two reads 14.50), persisted in
    GSettings; a setting that would zero a value out keeps the full
    rendering. Companion to format cycling.
  - [x] Alt+Up/Down cycles a line's result format (2026-10-06:
    standard, fixed two decimals, hex, bin - the fixed-decimals stop
    is what makes allocation lines readable. View-only state keyed
    by line number, shifting with edits until the stable-ids gate;
    unit values and errors keep their rendering. The portfolio
    template invites trying it on the pct lines.)
  - [x] Region model (2026-10-08): decimal-point-aligned answers
    per heading region - integer answers pad on the right so their
    implied dot sits where the region's fractional dots are, and the
    column's right alignment lines every dot up. Quantities, errors,
    hex and bin keep their shapes. spec "Results".

- [x] **Phase 8 (post-0.1.0): functions and tags.** (Complete
  2026-10-08; pulled to the front of the queue 2026-10-06 - Brandon:
  "I want the cool shit".)
  - [x] **[D]** Tag syntax gate: `@tag` glued to the end of an
    expression line, summed by `total @tag` (2026-10-06). Tagged
    results count toward the plain total too; `total @a @b` sums the
    union; tagged totals are pure views; headings and plain totals
    reset the tag sums; tags ride the unit engine (`5 kg @bulk`).
    Corpus: `tests/scripts/tags.quire`.
  - [x] Recurrence phrases (2026-10-08): **[D]** gate, Brandon's
    picks: rate quantities through the numbat bridge (Soulver's own
    model - the written period stays in the answer, `1200 /month`;
    the old "normalize to per-day" sketch is superseded, per-day is
    `1200/month -> 1/day`), periods day/week/month/quarter/year plus
    plurals (quarter = 3 months, registered into the engine beside
    the prelude), `$` decorative everywhere (the tokenizer skips
    `$` before a digit; ISO codes remain the only currency syntax),
    budget template migrated to `/month` lines (one `/year` income
    line demos mixed-period addition; such sums display in the
    largest period involved). The feature exposed and closed a
    Phase 6 routing gap: arithmetic over quantity-valued variables
    now routes on the touch alone (`bag * 2`, the budget leftover
    line), and the app's quantity cells render through the engine's
    display path (fixing datetime cells, which showed the engine's
    debug form since 0.6.0). spec "Recurring amounts"; corpus
    `tests/scripts/recurrence.quire`.
  - [x] Definition-sheet ergonomics (2026-10-08): the outline
    popover lists every bound name beneath the headings (pick one,
    land on its line) while the answers column carries the values
    and trailing comments the notes - the definition block reads as
    a table. Ctrl+B and Tab completion already served it; spec
    "Definition sheets".
- [x] **Phase 9: budgets and portfolios (all offline).** (Complete
  2026-10-08; sheet primitives first per Brandon's pick. The plugin
  question was asked and ruled 2026-10-06: engine primitives, NOT a
  plugin system.)
  - [x] Manual price snapshots: `AAPL = 190` with an optional dated
    form (`AAPL = 190 @ 2026-10-06`); portfolio sheets are variables
    (shares) times snapshot prices, summed and allocated by percent.
    (2026-10-06: the dated syntax, and the portfolio + budget sheet
    templates shipped behind a "New from template" menu section -
    the templates are the worked examples, pinned by tests.)
  - [x] Budget sheets (2026-10-08): category sections with
    heading-bounded subtotals, recurrence phrases (v0.7.0), and
    budget-vs-actual as paired variables - every category carries a
    `*_actual` twin and the variance section is plain subtraction
    (positive = under budget). NO live prices, ever (spec
    Non-goals): prices and rates enter sheets as manually typed
    snapshots. The migrated budget template is the worked example.
  - [x] Sheet templates shipped with the app (2026-10-08): budget
    (with actuals), portfolio, trip (dates + rates + currency, the
    Soulver 4 recipe), and a Complex Sample - a scientist's
    worksheet: ideal gas law, Arrhenius, triplicate statistics with
    sqrt, dilution, Henderson-Hasselbalch, projectile range, and
    combinatorics through recursive functions. The sample forced one
    primitive: the engine's math functions (sqrt, log10, ln, sin,
    abs, and the prelude) route to the unit engine - spec "Math
    functions" - and quantity totals finish inside expressions
    (`whole = total` over money).
- [ ] **Phase 10 (post-1.0, gated): rendered preview, if ever.**
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

## Soulver 4 parity register (Brandon's bar: 1:1 parity, then some)

Soulver 4's shipped surface mapped to Quire phases. Items land as
their phase gates open; the register is the checklist, not a promise
of order.

| Soulver 4 | Quire status |
|---|---|
| Variables / definition sheets | Shipped (Phase 1); definition-sheet ergonomics in Phase 8 |
| Percent forms | Shipped (Phase 1) |
| Totals / subtotals | Shipped (Phase 1, heading-or-total bounded) |
| Comments / headings / markdown structure | Shipped (Phases 0, 3) |
| Line highlighting | Shipped: reference lines + answers + the cursor-line band (v1.4.0) |
| Tags workflow | Phase 8 |
| Functions | Phase 7 |
| Units | Phase 6 (numbat embed gate) |
| Currency (manual snapshots) | Phase 6 + Phase 9 (never live) |
| Calendar / date math | Phase 6 (numbat jiff module) |
| Trip planning | Phase 9 (a sheet template recipe) |
| Autocomplete | Phase 4 |
| Line numbers | Phase 4 (toggle) |
| Agent-friendly CLI | Shipped post-1.0 (v1.4.0): `quire-cli`, a sheet in and its answers column out as text or JSON |
| Budgets / portfolios | Phase 9 (offline, sheet primitives first) |
| Rendered preview / sharing | Phase 10 gate (deliberately skipped for now) |

## Deliberately skipped (markdown-app research, 2026-10-05)

Recorded so future sessions do not re-litigate: smart punctuation and
typographic quotes (hostile to literal math input), typewriter and
focus/Hemingway modes (prose-drafting aids; revisit only by request),
image paste and asset management, footnote and grid-table editor UIs,
spellcheck in v1 (the path if ever wanted: libspelling with math lines
excluded), pandoc as a bundled engine, and WebKitGTK as a preview pane
(see Phase 8).

- [x] **v0.1.0 release** (2026-10-06): VERSION 0.1.0, patchnotes
  finalized, annotated tag with the patchnotes entry verbatim,
  GitHub release with the built tarball attached by the tag-gated
  release job. A Flatpak was not required for the first tag.
- [x] **1.0** (2026-10-08): Phases 6-9 complete, the whole suite
  green, release-auditor pre-flight run, and Brandon's instruction
  to cut on a successful build. The docs truth pass landed with
  v0.10.0's amendment pass. Brandon's display passes (1x and 2x)
  remain recommended eyeball time after the tag - the tour's new
  sections, the rate cells, and the region alignment deserve eyes -
  but they gate polish, not the release.
- [x] **Post-1.0: the rest of the register (v1.4.0, 2026-10-08):**
    cursor-line highlight (a full-height background TextTag riding
    the caret's line in the palette's raised card tone - the
    below-text snapshot hook the Phase 3 notes reserved turned out
    to be unexposed in sourceview5 0.11, so the native TextTag
    mechanism does it with zero custom paint), the agent-facing
    `quire-cli` binary (a sheet in, its answers column out as text
    or JSON; stdlib-only, a new workspace crate), and the Error
    Zoo's units section now teaches what it exhibits (addition
    fights, multiplication composes).
- [x] **Post-1.0: forward references (v1.3.0, 2026-10-08):** the
    spec promised `&N` on "any line, above or below" but the single
    top-down pass made forward refs permanently blank (Brandon hit
    it: &67 on line 65). evaluate_sheet is now a fixed-point loop -
    whole-sheet passes seeded with the last pass's outcomes,
    stopping when the rendered answers stabilize (cap 8, hardcoded;
    cycles settle blank because no cycle member is ever seeded;
    one-pass fast path via Ctx.saw_unresolved_ref). Three-agent
    research wave designed it; notecalc3 comparison: they ban
    forward refs in the UI, Quire resolves them.
- [x] **Post-1.0: debug mode + real templates (v1.5.0, 2026-10-08):**
    SheetStats (lines, passes, capped) exposed through
    evaluate_sheet_stats and quire-cli --stats; the app sprouts a
    QUIRE_DEBUG=1 diagnostics tap (lines, passes, cap, elapsed,
    column width, over-cap cells, error count per burst); the
    template menu grows a Mortgage walkthrough (headline cites its
    derivation through forward references - the v1.3.0 feature
    demoed as a real document) and a Freelance invoice (relative
    percent discount + VAT); the welcome sheet gains a compound-
    interest line and a Where-to-go-next pointer to the templates.
    The full debug run: 21 sheets, zero cap hits, all settling.
    Template-authoring lessons now pinned by tests: two-word names
    never bind, `total` cannot be an assignment name, and template
    forward refs must be counted against the final layout.
- [x] **Post-1.0: stress templates (v1.1.0, 2026-10-08):** a Stress
    Test (layout edges: wrapped lines, wide and 45-character
    answers, region-alignment pads, tab-led lines, blank forward
    refs, unicode prose) and an Error Zoo (every contained failure
    family plus hostile text), loadable from the template menu.
    Finding: the recursion cap costs ~10 KB/level in debug builds
    and overflows Rust's default 2 MB test threads - heavy computes
    run under a big-stack test harness; the app's 8 MB main thread
    is unaffected. Also fell out: bridge dimension clashes surface
    instead of "unknown name" (3 kg + 5 m names Mass vs Length),
    and the zoo's unknown-function cage tightened.

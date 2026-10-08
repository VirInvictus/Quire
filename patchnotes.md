# Patchnotes

Newest first.

## v1.2.0 (2026-10-08)

- Sentences carry units through the engine: the mixed-line skeleton
  evaluates through the unit engine when it holds one - `the stock
  solution measures 2 mol/L` answers `2 molar`, `the sample weighs
  0.101 g` answers `0.101 g`, trailing periods and commas are glue.
  The surviving words must sit the way numbers and units sit in
  math: a number may lean on its unit, units may stand together,
  and a bound name next to anything stays prose. This is the
  feature that makes a lab notebook read like a lab notebook.
- The Complex Sample is rewritten as that lab notebook: sentences
  carry the numbers, derivations keep their own lines when they
  deserve the room, and everything referenced later is bound with
  `=` in the paper's notation. Building it surfaced two name
  collisions (numbat claims `mean`, `std`, and the word `second`
  is the seconds unit) and the notebook words around them.

## v1.1.0 (2026-10-08)

- Two dev torture sheets join the template menu, built to stress and
  confuse the renderer on purpose. The Stress Test loads layout
  edges: wrapped long lines, a 45-character answer next to short
  ones, region-alignment padding, tab-led lines, blank forward
  references, very long names and tag runs, and unicode prose. The
  Error Zoo loads every contained failure family at once - division
  by zero, out-of-range powers, malformed numbers, unbalanced
  parens, unknown functions and names, dead references, recursion
  depth, arity clashes, fighting units, keyword misuse, and hostile
  text - each error caged to its own line while the sheet keeps
  computing past the last cage. If a cell overlaps, an error leaks,
  or anything misbehaves, the sheet names the finding.
- Two better error messages fell out of building the zoo: unit
  dimension clashes now surface the engine's own wording (`3 kg +
  5 m` names Mass versus Length instead of "unknown name kg"), and
  a bare period word in dead algebra reads as the unit it is, with
  numbat's suggested fix attached.
- Finding, documented: the recursion cap (200 levels) costs about
  10 KB of stack per level in debug builds - fine on the app's
  8 MB main thread, over the budget of Rust's default 2 MB test
  threads, so heavy template computes in the suite run under a
  big-stack harness.

## v1.0.0 (2026-10-08)

- 1.0: Phases 6 through 9 are complete and the Soulver-depth surface
  is whole - arithmetic with implicit multiplication, every
  percentage form including the reverse questions in words,
  functions with recursion and dimension-checked unit paths,
  self-updating line references, dates, recurring amounts, currency,
  units, math functions, tags, mixed lines, completion, and the
  template menu.
- Ctrl+click a math line and it explains itself: a popover shows the
  line's operations bottom-up, each as `operands = value`, evaluated
  in full sheet context so variables, references, and function
  bodies resolve. A relative percent reads in the sheet's words
  (`200 + 15% (of 200) = 230`). Unit lines and prose do not break
  down.
- Task lists: a list line may open with a GFM checkbox
  (`- [ ] plan the trip`); Ctrl+clicking the box toggles it as a
  plain user edit undo reverses. Checkboxes never enter the math.
- The region model: within each heading region, scalar answers
  align on the decimal point - integer answers pad so their implied
  dot sits where the region's fractional dots are. Quantities,
  errors, hex and bin keep their shapes.
- Definition sheets read as a table: the Outline popover lists every
  bound name beneath the headings; picking one lands on its line,
  with the answers column as the values and trailing comments as
  the notes.
- The template menu grows to four: budget (now with budget-vs-
  actual paired `*_actual` variables and a variance section where
  positive means under budget), portfolio, trip (dates for the
  countdown, rates for per-day costs, currency for the spend), and
  a Complex Sample - a scientist's worksheet running the ideal gas
  law, Arrhenius activation energy, triplicate statistics with a
  sample standard deviation, a dilution, Henderson-Hasselbalch pH,
  projectile range, and combinatorics through recursive functions.
- Two engine primitives the new sheets demanded: the prelude's math
  functions route to the unit engine (`sqrt(144)` is 12,
  `sin(30 deg)` is 0.5 - before, `log(100)` silently answered 100
  through the mixed skeleton), and quantity totals finish inside
  expressions (`whole = total` over money answers `990 EUR`). Date
  words call their functions mid-line too: `today + 30 days`.

## v0.10.0 (2026-10-08)

- Tab completion reaches the engine: completing a word now offers
  the sheet's names above the cursor plus units and their aliases
  (`hours`, `kilometer`) and prelude variables - the unique-match
  rule is unchanged, so Tab still inserts only when one candidate
  remains. Three numbat edges are documented in the engine:
  prefixed symbols (`kg`) and typechecker constants (`pi`) evaluate
  fine when typed but are not candidates, and currency codes can
  never safely be ones (the currencies module bakes the exchange
  rates into its unit values at load time, so it may only ever load
  on demand).
- The Phase 6 spec amendment pass: a doc-drift audit found seventeen
  drifts and the contract now tells the truth everywhere. Three were
  code fixes - percent-bodied functions refuse the unit path (the
  spec always said so; the code silently seeded them with plain
  division, changing their meaning), line references in function
  bodies are refused (sheet-positional like `total`; a body ref
  would silently re-aim when lines shift), and a currencies pre-load
  that briefly NaN'd every conversion was reverted for the load-time
  reason above. The rest were words: Currency gained its own spec
  section (the cross-reference had dangled since Phase 6), the Tags
  Summing and Functions Recursion bullets no longer contradict their
  own later amendments, the Testing section describes the per-
  feature corpora, and the Expression shape lists the `&N` rule.

## v0.9.0 (2026-10-08)

- Implicit multiplication, kalker's rule: a number, bare name, or
  `(` directly after a value multiplies - `2pi`, `3(4 + 5)`,
  `(1 + 2)(3 + 4)`, `2 3` - at the same precedence as `*`,
  left-associative (`1/2pi` is `(1/2)*pi`), with power binding
  tighter and `name(` staying a function call. The starter sheet
  gained a "Math, the way you jot it" section, and the ambiguities
  corpus ships as its own folder (`tests/scripts/ambiguities/`).
- Two riders the parser change forced, both userspace-preserving:
  the mixed-line skeleton now parses with implicit multiplication
  off, so a prose remnant like `I paid $5 for milk` with milk bound
  stays prose exactly as before; and the unit fallback generalized -
  a scalar failure on any name the unit engine knows hands the line
  to the bridge, because value-unit shapes parse on the scalar path
  now. Near-miss errors surface from the first unknown name instead
  of the parser (an honest error either way).

## v0.8.0 (2026-10-08)

- Reverse percent questions answer in words: `220 is 10% on what` is
  200, `180 is 10% off what` is 200, `20 is 10% of what` is 200, and
  `30 is what percent of 200` is 0.15 (`what %` reads the same). The
  find-the-percent answer is the percent itself in Quire's
  percent-is-its-fraction convention, so it composes like any value
  (`answer * 200` is 30). The value and the percent sides may be any
  expressions (`2 * 20 is 10% of what` is 400), the words match
  case-insensitively, and a near-miss keeps the old behavior (an
  honest error on expression lines, silence on prose). These phrases
  previously grew red error cells; they were the roadmap's last
  missing percent forms.

## v0.7.0 (2026-10-08)

- Recurring amounts: `$1200/month`, `950 / month`, `60/quarter`
  answer as rate quantities - the answer keeps the written period
  (`1200 /month`), rates add across periods, join totals and tag
  sums dimension-checked, convert explicitly (`1200/month -> 1/day`
  is `39.4259 /day`), and multiply by durations into plain amounts
  (`950/month * 12 months` is `11400`). Periods: day, week, month,
  quarter (three months, registered into the engine at startup),
  year, singular or plural; a quarter is 91.31 days, a month
  30.4368. The gate decided Soulver's own model over the old
  "normalize to per-day" sketch: a forgotten `/month` in a budget
  now fails its total loudly instead of under-counting silently.
- `$` before a number is decoration everywhere: `$5.60 * 3` is 16.8
  and `$1200/month` is a plain rate. Classification had accepted
  `$`-led lines since Phase 0; the tokenizer now backs it (a stray
  `$` still errors). Currency remains ISO-code syntax
  (`50 USD -> EUR`); `$` never names one.
- Two riders the feature surfaced: arithmetic over quantity-valued
  variables routes to the unit engine on the touch alone (`bag * 2`
  works; the budget template's leftover line needs it), and the
  answers column renders through the engine's display path, which it
  had bypassed - datetime cells had shown the engine's debug form
  since 0.6.0, and bare rates now read `950 /month` rather than
  `950 month⁻¹`.
- The budget template migrated to rates (its income mixes `/month`
  and `/year`; sums over mixed periods display in the largest
  period involved), and the starter sheet gained a Recurring amounts
  section.

## v0.6.0 (2026-10-07)

- Self-updating line references: `&N` follows its target when lines
  shift. Insert, delete, or paste above a reference and the token is
  rewritten in place so it still names its line; a ref whose target
  line is deleted follows the line that takes its place; undo and
  redo revert cleanly. Loading a sheet never rewrites anything (refs
  read as authored until a line actually moves), and the `&N` text
  inside comments, prose, and function bodies is never touched: only
  refs the engine resolves participate. Menu toggle: "Follow line
  references", on by default; off restores the purely positional
  reading.
- How it works (the fourth attempt; the first three cascaded into
  `&11 -> &111 -> &1112` and were stripped in 0.5.1): every
  referenced line carries an invisible right-gravity GtkTextMark
  that rides every edit inside the buffer's btree, so the sheet's
  structure is tracked by the editor itself, never by diffing text.
  The debounced pass reads the marks and renumbers only what
  drifted, in one batched splice with the evaluation handler
  blocked and the edit marked irreversible: the pipeline cannot
  re-read its own output, and undo or redo of the user's edit
  converges the digits back instead of fighting the rewrite. The
  convergence logic is a GTK-free pure module (refs.rs, 20 table
  tests: inserts, deletes, pastes, chained refs, colliding marks,
  forward refs, hand-retyped digits, the undo round-trip).
- The welcome sheet's reference tour line reflects the new
  behavior.

## v0.5.1 (2026-10-07)

- The &N renumber cascade fix: pressing Enter or Backspace near a
  `&N` reference caused the refs to cascade into garbage (`&11`
  becoming `&111`, then `&1112`, then merging with the next line).
  Two bugs compounded: the renumber pass used `buffer.set_text()`
  (replacing the entire buffer, resetting scroll and cursor), and
  the re-entry guard was missing (each targeted edit fired
  `changed`, which queued another renumber pass on the intermediate
  text). The fix: targeted per-token edits (only the shifted refs
  are rewritten, in place, back to front) and a re-entry guard that
  blocks the evaluation pass during application. Verified live:
  inserting a header above `&1 * 3` renumbers to `&2 * 3` with the
  viewport stable.

## v0.5.0 (2026-10-07)

- Mixed-line evaluation: prose lines whose words strip to a
  complete expression answer with it - `50 apples at 3 each` is
  `150`, `2 coffees plus 1 tea` is `3`. Word operators map
  (`at`/`of`/`times` multiply, `plus` adds, `minus` subtracts),
  unknown names drop as prose, and failures stay silent: mixed
  lines never grow error cells. spec.md gained the Mixed lines
  section; the tour demos the shape.
- Multi-clause functions with literal-pattern matching: `fact(0) = 1`
  and `fact(n) = n * fact(n - 1)` make recursive functions work -
  clauses with literal parameters match before variable ones, and
  redefining a clause replaces its predecessor (like variables).
  Forward line references: `&N` pointing below or at a not-yet-
  answered line is blank (no cell) instead of erroring - the line
  fills in when the target produces a value. spec.md gained the
  Mixed lines section and the Functions recursion-and-clauses
  clause; the tour gained a Recursion section.

## v0.4.1 (2026-10-07)

- The &N renumbering wiring lands: 0.4.0 shipped the renumber
  primitive and claimed references follow their targets, but the
  primitive had no caller - the claim was premature. The buffer now
  diffs every change against the previous text and rewrites moved
  refs in place, verified live (a header inserted above `&1 * 3`
  renumbers to `&2 * 3`, answer unchanged). The rewrite also keeps
  the `&` prefix (an early live test produced bare `3 * 3`).

## v0.4.0 (2026-10-07)

- Line references: `&N` in an expression answers with sheet line
  N's result - lines above only, a failed referenced line poisons
  the referencing line, and a no-result line fails with "has no
  result". References work in scalar and unit expressions alike
  (they translate to the referenced value), and their results join
  `answer`, totals, and tags. The app renumbers `&N` tokens in
  memory when insertions and deletions shift lines, as part of the
  same edit; the sheet on disk only ever shows what you typed or
  approved. spec.md gained the Line references section.
- Token-level error highlighting: each failed line underlines the
  exact token that failed in red, in the sheet itself - hover the
  cell for the full message, see the underline for the location.
  Cleared and reapplied on every evaluation pass.
- Mixed-line evaluation: prose lines whose words strip to a
  complete expression answer with it - `50 apples at 3 each` is
  `150`, `2 coffees plus 1 tea` is `3`. Word operators map
  (`at`/`of`/`times` multiply, `plus` adds, `minus` subtracts),
  unknown names drop as prose, and failures stay silent: mixed
  lines never grow error cells. `total`/`answer`, tags, and line
  references never enter a mixed skeleton. spec.md gained the
  Mixed lines section; the tour demos the shape.

## v0.3.0 (2026-10-07)

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

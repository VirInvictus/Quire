# Roadmap

- [x] **Phase 0: Project skeleton.** Two-crate workspace, standard doc
  set, MIT, placeholder logo, `quire-eval` sheet line model with
  classification tests. (2026-10-05)
- [ ] **Phase 1: `quire-eval` core.** Tokenizer, Pratt parser,
  evaluator: arithmetic, unary minus, `^`, the four percent forms,
  variables, `answer`, `total`, per-line errors. Table tests plus
  golden sheet tests per the spec's Semantics section.
- [ ] **Phase 2: Window and live results.** `gtk4` + `sourceview5`
  (installs the `gtksourceview5-devel` system package), vir-gtk
  Kanagawa styling, editor on the left, per-line results in the
  editor's right edge, debounced live re-evaluation. The first thing
  that looks like Quire.
- [ ] **Phase 3: Typography.** Bundled OFL fonts for the math surface,
  markdown highlighting tuned, spacing and colour pass.
- [ ] **Phase 4: Documents.** Open/save, `.quire` association, recent
  sheets, unsaved-changes guard.
- [ ] **Phase 5: Packaging.** Desktop file, a real icon (replaces the
  placeholder logo), AppStream metadata. Flatpak is its own decision
  (house precedent: `io.github.virinvictus.*` on GNOME 50).
- [ ] **Phase 6: Currency, units, dates.** Decide own implementation
  vs embedding `numbat` (MIT, units and date arithmetic on day one),
  then build it.
- [ ] **v0.1.0 release** after Phase 4: bump VERSION, patchnotes, tag
  with the patchnotes entry verbatim, GitHub release. A Flatpak is not
  required for the first tag.

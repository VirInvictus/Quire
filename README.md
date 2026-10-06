# Quire

A Soulver-style notepad calculator for Linux. Notes and math share one
plain-text sheet: you write, and every expression answers on its own
line in a results column down the right edge.

The design contract lives in [spec.md](spec.md); the work plan in
[roadmap.md](roadmap.md).

## How a sheet reads

Prose, markdown structure, and math share the page, each with its own
voice (Kanagawa Dragon by default, Lotus when the desktop asks for
light):

```text
# Kitchen remodel

// notes are dimmed italics
cabinets = 2400
counters = 950
total
```

`cabinets` and `counters` render blue (they are variables), the
numbers render plain, and the answers column shows `cabinets = 2400`,
`counters = 950`, `total = 3350` live as you type.

- Per-line results as you type; an error belongs to its line and never
  breaks the others
- Variables, bare references (`groceries` on its own line answers with
  its value), `answer`, and `total` for running subtotals
- Percentage forms that match how they are spoken: `200 + 15%` is 230,
  `15% of 200` is 30
- Markdown structure (headings, lists, emphasis, `//` comments) as
  styled text; documents are plain UTF-8, no lock-in
- Wayland-native GTK4, no libadwaita, Kanagawa-themed through
  [vir-gtk](https://github.com/VirInvictus/vir-gtk), bundled JetBrains
  Mono

## Status

Early development, moving fast. Phases 0-3 of the roadmap are
shipped: the evaluation engine (arithmetic, percents, variables,
totals, span-carrying errors), the GTK4 window with the live answers
column, and the sheet typography and highlighting. Documents and
editing UX are next; the first tagged release follows Phase 4.

## Building

Rust 1.85+ and the GTK 4 + GtkSourceView 5 development packages
(`gtk4-devel` and `gtksourceview5-devel` on Fedora,
`libgtk-4-dev` and `libgtksourceview-5-dev` on Debian/Ubuntu).

    cargo build
    cargo test

First run installs JetBrains Mono (SIL OFL 1.1, bundled) under
`~/.local/share/fonts/Quire/` and the sheet language and color
schemes under `~/.local/share/quire/`; nothing else is assumed of the
system.

## License

MIT. See [LICENSE](LICENSE).

# Quire

A Soulver-style notepad calculator for Linux. Notes and math share one
plain-text sheet: you write, and every expression answers on its own
line in a results column down the right edge.

Quire is early software, built in the open as a portfolio piece. The
design contract lives in [spec.md](spec.md); the work plan in
[roadmap.md](roadmap.md).

## Why

Soulver is macOS-only, and its author has said a Linux version is not
planned. Quire is the native-Linux take on the idea: a sheet of plain
text where prose, headings, and comments sit next to live
calculations, with variables, totals, and percentage arithmetic that
reads the way people actually write.

## Features (planned, per the roadmap)

- Per-line results as you type; an error belongs to its line and never
  breaks the others
- Variables, `answer` references, and `total` for column sums
- Percentage forms that match how they are spoken: `200 + 15%`,
  `15% of 200`
- Markdown-flavored structure (headings, lists, `//` comments) as
  styled text
- Plain UTF-8 documents; no lock-in
- Wayland-native GTK4, Kanagawa Dragon themed

## Building

Rust 1.85+ (edition 2024). The engine builds standalone today:

    cargo test -p quire-eval

The GTK4 application lands in Phase 2 of the roadmap; from that phase
on, a build needs the GTK4 and GtkSourceView 5 development packages.

## License

MIT. See [LICENSE](LICENSE).

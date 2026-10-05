//! Quire: a Soulver-style notepad calculator for Linux.
//!
//! Phase 0 skeleton: the GTK4 window and editor arrive in Phase 2
//! (see roadmap.md). The engine lives in the quire-eval crate.

fn main() {
    println!("quire {}", env!("CARGO_PKG_VERSION"));
    let sheet = quire_eval::parse_sheet("# Groceries\nmilk = 3.50\nmilk * 2\n");
    for line in &sheet {
        println!(
            "{:>3}  {:<10}  {}",
            line.number,
            line.kind.to_string(),
            line.raw
        );
    }
}

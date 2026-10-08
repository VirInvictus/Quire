use quire_eval::{evaluate_sheet, parse_sheet};

fn main() {
    for name in ["mortgage", "invoice"] {
        let sheet = std::fs::read_to_string(
            format!("/home/bdkl/.gitrepos/Quire/crates/quire/resources/templates/{name}.quire"),
        )
        .unwrap();
        println!("=== {name} ===");
        for line in evaluate_sheet(&sheet) {
            let raw = parse_sheet(&sheet)
                .into_iter()
                .find(|l| l.number == line.number)
                .map(|l| l.raw.clone())
                .unwrap_or_default();
            match &line.outcome {
                Some(o) => println!("{:>2} {:45} => {}", line.number, raw, o.render()),
                None => println!("{:>2} {:60} => (prose)", line.number, raw),
            }
        }
    }
}

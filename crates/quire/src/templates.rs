//! Shipped sheet templates (spec.md "Dated snapshots" era; the
//! portfolio and budget recipes). Headless on purpose: the tests run
//! the templates through the engine and fail on any error cell.

pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "Portfolio",
        include_str!("../resources/templates/portfolio.quire"),
    ),
    (
        "Budget",
        include_str!("../resources/templates/budget.quire"),
    ),
    ("Trip", include_str!("../resources/templates/trip.quire")),
    (
        "Complex Sample",
        include_str!("../resources/templates/complex.quire"),
    ),
    (
        "Stress Test",
        include_str!("../resources/templates/stress.quire"),
    ),
    (
        "Error Zoo",
        include_str!("../resources/templates/zoo.quire"),
    ),
    (
        "Mortgage",
        include_str!("../resources/templates/mortgage.quire"),
    ),
    (
        "Invoice",
        include_str!("../resources/templates/invoice.quire"),
    ),
];

#[cfg(test)]
mod tests {
    use super::TEMPLATES;

    /// The engine's recursion cap (200 quire-call levels) costs
    /// roughly 10 KB of stack per level in debug builds - fine on the
    /// app's 8 MB main thread, over the budget of Rust's default 2 MB
    /// test threads. Template computes run on a fat thread.
    fn with_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(f)
            .expect("spawn the big-stack thread")
            .join()
            .expect("the big-stack closure panicked")
    }

    /// A template that evaluates with an error cell is a broken
    /// recipe; every shipped template must compute clean. The Error
    /// Zoo is the exception ON PURPOSE: its contained failures are
    /// the content, asserted in its own test below.
    #[test]
    fn templates_have_no_error_cells() {
        with_big_stack(|| {
            for (name, sheet) in TEMPLATES {
                if *name == "Error Zoo" {
                    continue;
                }
                for (line, cell) in crate::answers::compute(sheet) {
                    assert!(!cell.is_error, "{name}: line {line}: {}", cell.text);
                }
            }
        });
    }

    /// The zoo's errors are the exhibit: every family present, all
    /// contained, and the sheet computes past the last cage.
    #[test]
    fn error_zoo_keeps_every_failure_caged() {
        with_big_stack(|| {
            let cells = crate::answers::compute(TEMPLATES[5].1);
            let errors: Vec<&str> = cells
                .values()
                .filter(|c| c.is_error)
                .map(|c| c.text.as_str())
                .collect();
            assert!(errors.len() >= 10, "the zoo lost cages: {errors:?}");
            for family in [
                "division by zero",
                "result out of range",
                "malformed number",
                "expected a value",
                "expected `)`",
                "nested too deeply",
                "expects 1 argument",
                "left hand side: Mass",
            ] {
                assert!(
                    errors.iter().any(|e| e.contains(family)),
                    "the zoo lost its {family} cage: {errors:?}"
                );
            }
            // containment: the sheet computes past the last cage
            let still_alive = cells
                .values()
                .filter(|c| !c.is_error)
                .any(|c| c.text == "42");
            assert!(still_alive, "6 * 7 stopped answering: {errors:?}");
        });
    }

    #[test]
    fn templates_answer_the_things_they_show_off() {
        with_big_stack(|| {
            let portfolio = crate::answers::compute(TEMPLATES[0].1);
            let texts: Vec<&str> = portfolio.values().map(|c| c.text.as_str()).collect();
            for expected in ["1,900", "1,640", "5,000", "8,540", "3,540"] {
                assert!(
                    texts.contains(&expected),
                    "portfolio lost {expected}: {texts:?}"
                );
            }
            let budget = crate::answers::compute(TEMPLATES[1].1);
            let texts: Vec<&str> = budget.values().map(|c| c.text.as_str()).collect();
            // rate renderings keep the written period (`950 /month`);
            // sums over mixed periods display in the largest involved,
            // so the /year lines read in /yr. The tagged views cover
            // planned and actual together; the paired *_actual
            // variables drive the variance section
            for expected in ["2248 /month", "-540.0 /yr", "18 /month", "384 /yr"] {
                assert!(
                    texts.contains(&expected),
                    "budget lost {expected}: {texts:?}"
                );
            }
            let trip = crate::answers::compute(TEMPLATES[2].1);
            let texts: Vec<&str> = trip.values().map(|c| c.text.as_str()).collect();
            // rate x duration is a plain amount; the total divides back
            // into a per-day rate
            for expected in ["990 €", "198 €/day"] {
                assert!(texts.contains(&expected), "trip lost {expected}: {texts:?}");
            }
            let complex = crate::answers::compute(TEMPLATES[3].1);
            let texts: Vec<&str> = complex.values().map(|c| c.text.as_str()).collect();
            // the scientist sheet: dimensional algebra, engine functions,
            // statistics, and recursion all answer cleanly
            for expected in [
                "99.768 J/L",    // ideal gas law
                "84944.9 J/mol", // Arrhenius activation energy
                "0.00251661 g",  // sample standard deviation
                "500 mL",        // dilution
                "4.45897",       // Henderson-Hasselbalch pH
                "59.8683 m",     // projectile range
                "120",           // combinatorics through recursion
            ] {
                assert!(
                    texts.contains(&expected),
                    "complex sample lost {expected}: {texts:?}"
                );
            }
            let stress = crate::answers::compute(TEMPLATES[4].1);
            // trim_end: the region model pads cells to a shared dot
            // column (the "1      " shapes), which is asserted over in
            // answers.rs; here only the numbers matter
            let texts: Vec<&str> = stress.values().map(|c| c.text.trim_end()).collect();
            // the torture sheet: wide answers, a 45-character tiny one,
            // padded region edges, and deep recursion all render
            for expected in [
                "1,371,737,997,260,000",
                "0.000000000000000000000000000000788860905221",
                "4,555",
                "12", // the tab-led line
                "14", // the glued 7+7
            ] {
                assert!(
                    texts.contains(&expected),
                    "stress lost {expected}: {texts:?}"
                );
            }
            let zoo = crate::answers::compute(TEMPLATES[5].1);
            let zoo_texts: Vec<&str> = zoo.values().map(|c| c.text.as_str()).collect();
            assert!(
                zoo_texts.contains(&"42"),
                "the zoo stopped computing past its cages: {zoo_texts:?}"
            );
            // the mortgage: the top-of-sheet headline cites the bottom-
            // sheet derivation through FORWARD references
            let mortgage = crate::answers::compute(TEMPLATES[6].1);
            // trim_end: the region model pads to a shared dot column
            let texts: Vec<&str> = mortgage.values().map(|c| c.text.trim_end()).collect();
            for expected in ["2,464.36275991", "887,170.593566", "467,170.593566"] {
                assert!(
                    texts.contains(&expected),
                    "mortgage lost {expected}: {texts:?}"
                );
            }
            // the invoice: relative percents do the discount and the tax
            let invoice = crate::answers::compute(TEMPLATES[7].1);
            let texts: Vec<&str> = invoice.values().map(|c| c.text.trim_end()).collect();
            for expected in ["3,550", "3,372.5", "708.225", "4,080.725"] {
                assert!(
                    texts.contains(&expected),
                    "invoice lost {expected}: {texts:?}"
                );
            }
        });
    }
}

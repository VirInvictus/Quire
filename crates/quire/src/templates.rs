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
];

#[cfg(test)]
mod tests {
    use super::TEMPLATES;

    /// A template that evaluates with an error cell is a broken
    /// recipe; every shipped template must compute clean.
    #[test]
    fn templates_have_no_error_cells() {
        for (name, sheet) in TEMPLATES {
            for (line, cell) in crate::answers::compute(sheet) {
                assert!(!cell.is_error, "{name}: line {line}: {}", cell.text);
            }
        }
    }

    #[test]
    fn templates_answer_the_things_they_show_off() {
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
    }
}

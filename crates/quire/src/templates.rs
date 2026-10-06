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
        for expected in ["2,750", "1,115", "1,075"] {
            assert!(
                texts.contains(&expected),
                "budget lost {expected}: {texts:?}"
            );
        }
    }
}

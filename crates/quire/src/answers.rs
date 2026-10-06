//! The bridge between the engine and the UI: one answer cell per
//! expression line, formatted and classified, keyed by line number.
//! Deliberately GTK-free so it tests headlessly.

use std::collections::HashMap;

use quire_eval::Outcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerCell {
    pub text: String,
    pub is_error: bool,
}

pub fn compute(sheet: &str) -> HashMap<u32, AnswerCell> {
    quire_eval::evaluate_sheet(sheet)
        .into_iter()
        .filter_map(|line| {
            let cell = match line.outcome? {
                Outcome::Value(v) => AnswerCell {
                    text: quire_eval::format_number(v),
                    is_error: false,
                },
                Outcome::Failed(e) => AnswerCell {
                    text: e.kind.to_string(),
                    is_error: true,
                },
            };
            Some((line.number as u32, cell))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_match_expression_lines_only() {
        let sheet = "milk = 3.50\n\nmilk * 2\n# head\noops = 1 / 0\n";
        let map = compute(sheet);
        assert_eq!(map.len(), 3);
        assert_eq!(map[&1].text, "3.5");
        assert!(!map[&1].is_error);
        assert_eq!(map[&3].text, "7");
        assert!(map[&5].is_error);
        assert_eq!(map[&5].text, "division by zero");
    }

    #[test]
    fn empty_sheet_has_no_cells() {
        assert!(compute("").is_empty());
    }
}

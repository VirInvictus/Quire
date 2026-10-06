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

/// Per-line answer formats, cycled with Alt+Up/Down (Phase 7).
/// Keyed by line number; the choice travels with the line NUMBER,
/// so it shifts when the sheet's structure shifts (the stable
/// line-ids gate is the real fix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFormat {
    Standard,
    Fixed2,
    Hex,
    Bin,
}

impl LineFormat {
    pub fn next(self) -> Self {
        match self {
            LineFormat::Standard => LineFormat::Fixed2,
            LineFormat::Fixed2 => LineFormat::Hex,
            LineFormat::Hex => LineFormat::Bin,
            LineFormat::Bin => LineFormat::Standard,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            LineFormat::Standard => LineFormat::Bin,
            LineFormat::Fixed2 => LineFormat::Standard,
            LineFormat::Hex => LineFormat::Fixed2,
            LineFormat::Bin => LineFormat::Hex,
        }
    }
}

pub fn compute(sheet: &str) -> HashMap<u32, AnswerCell> {
    compute_with_formats(sheet, &Default::default(), None)
}

pub fn compute_with_formats(
    sheet: &str,
    formats: &HashMap<u32, LineFormat>,
    max_decimals: Option<u32>,
) -> HashMap<u32, AnswerCell> {
    quire_eval::evaluate_sheet(sheet)
        .into_iter()
        .filter_map(|line| {
            let format = formats.get(&(line.number as u32)).copied();
            let cell = match line.outcome? {
                Outcome::Value(v) => AnswerCell {
                    text: match format {
                        None | Some(LineFormat::Standard) => match max_decimals {
                            Some(cap) => quire_eval::format_number_with(v, cap),
                            None => quire_eval::format_number(v),
                        },
                        Some(LineFormat::Fixed2) => format!("{v:.2}"),
                        // hex and bin are integer shapes; anything
                        // else stays on the previous format
                        Some(LineFormat::Hex) if v >= 0.0 && v.fract() == 0.0 => {
                            format!("{:#x}", v as i64)
                        }
                        Some(LineFormat::Bin) if v >= 0.0 && v.fract() == 0.0 => {
                            format!("{:#b}", v as i64)
                        }
                        _ => quire_eval::format_number(v),
                    },
                    is_error: false,
                },
                // unit-engine values arrive pre-rendered (the engine's
                // Display is the cell text, spec.md "Unit expressions");
                // format cycling does not apply to them
                Outcome::Quantity(v) => AnswerCell {
                    text: v.to_string(),
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

    #[test]
    fn decimal_caps_round_the_standard_rendering() {
        let sheet = "pct = 3540 / 8540 * 100\nfine = 1 / 3\n";
        let cells = compute_with_formats(sheet, &Default::default(), Some(2));
        assert_eq!(cells[&1].text, "41.45");
        // the lossy guard: a zero-decimal cap cannot zero out 1/3
        let cells = compute_with_formats(sheet, &Default::default(), Some(0));
        assert_eq!(cells[&2].text, "0.333333333333");
    }

    #[test]
    fn format_cycling_covers_the_shapes() {
        let sheet = "whole = 8540\npct = 40 * whole / 100\nneg = -3\n";
        // standard
        let formats = Default::default();
        let cells = compute_with_formats(sheet, &formats, None);
        assert_eq!(cells[&2].text, "3,416");
        // fixed two decimals: the portfolio allocation case
        let mut formats = HashMap::new();
        formats.insert(2, LineFormat::Fixed2);
        let cells = compute_with_formats(sheet, &formats, None);
        assert_eq!(cells[&2].text, "3416.00");
        // hex and bin apply to non-negative integers
        formats.insert(1, LineFormat::Hex);
        formats.insert(2, LineFormat::Bin);
        let cells = compute_with_formats(sheet, &formats, None);
        assert_eq!(cells[&1].text, "0x215c");
        assert_eq!(cells[&2].text, "0b110101011000");
        // negatives keep their previous format when hex/bin cannot apply
        formats.insert(3, LineFormat::Hex);
        let cells = compute_with_formats(sheet, &formats, None);
        assert_eq!(cells[&3].text, "-3");
        // the cycle order is Standard -> Fixed2 -> Hex -> Bin
        assert_eq!(LineFormat::Standard.next(), LineFormat::Fixed2);
        assert_eq!(LineFormat::Fixed2.next(), LineFormat::Hex);
        assert_eq!(LineFormat::Hex.next(), LineFormat::Bin);
        assert_eq!(LineFormat::Bin.next(), LineFormat::Standard);
        assert_eq!(LineFormat::Standard.previous(), LineFormat::Bin);
    }
}

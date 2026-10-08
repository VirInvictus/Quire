//! The bridge between the engine and the UI: one answer cell per
//! expression line, formatted and classified, keyed by line number.
//! Deliberately GTK-free so it tests headlessly.

use std::collections::HashMap;

use quire_eval::Outcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerCell {
    pub text: String,
    pub is_error: bool,
    /// The error's token span in CHAR offsets within the line, for
    /// the in-sheet red underline. None for values and for errors
    /// without a token position.
    pub error_span: Option<(usize, usize)>,
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

/// The error span as CHAR offsets within the line: token spans are
/// byte offsets, and TextIters count chars.
fn char_span(line_raw: &str, span: (usize, usize)) -> Option<(usize, usize)> {
    let clamp = |b: usize| line_raw.get(..b).map(|s| s.chars().count());
    Some((clamp(span.0)?, clamp(span.1)?))
}

pub fn compute_with_formats(
    sheet: &str,
    formats: &HashMap<u32, LineFormat>,
    max_decimals: Option<u32>,
) -> HashMap<u32, AnswerCell> {
    let mut map: HashMap<u32, AnswerCell> = quire_eval::evaluate_sheet(sheet)
        .into_iter()
        .filter_map(|line| {
            let format = formats.get(&(line.number as u32)).copied();
            let line_raw = sheet.lines().nth(line.number - 1).unwrap_or("");
            let cell = match line.outcome? {
                Outcome::Value(v) => AnswerCell {
                    error_span: None,
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
                // unit-engine values render through the engine's own
                // display (spec.md "Unit expressions"); format cycling
                // does not apply to them
                Outcome::Quantity(v) => AnswerCell {
                    error_span: None,
                    text: quire_eval::render(&v),
                    is_error: false,
                },
                Outcome::Failed(e) => AnswerCell {
                    text: e.kind.to_string(),
                    is_error: true,
                    error_span: char_span(line_raw, e.span),
                },
            };
            Some((line.number as u32, cell))
        })
        .collect();
    align_regions(sheet, &mut map);
    map
}

/// The region model (spec.md "Results"): within each heading region,
/// scalar answers share a decimal-point column. Integer answers pad
/// on the right so their implied dot sits where the fractional
/// answers' dots are; the renderer's right alignment then lines every
/// dot in the region up. Only plain number texts pad - quantities,
/// errors, hex and bin keep their shapes.
fn align_regions(text: &str, cells: &mut HashMap<u32, AnswerCell>) {
    let mut region = 0usize;
    let mut region_of: HashMap<u32, usize> = HashMap::new();
    for line in quire_eval::parse_sheet(text) {
        if line.kind == quire_eval::LineKind::Heading {
            region += 1;
        }
        region_of.insert(line.number as u32, region);
    }
    let mut max_frac: HashMap<usize, usize> = HashMap::new();
    for (num, cell) in cells.iter() {
        if cell.is_error {
            continue;
        }
        let Some(frac) = scalar_frac(&cell.text) else {
            continue;
        };
        if frac > 0 {
            let slot = max_frac.entry(region_of[num]).or_insert(0);
            if frac > *slot {
                *slot = frac;
            }
        }
    }
    for (num, cell) in cells.iter_mut() {
        if cell.is_error {
            continue;
        }
        let Some(frac) = scalar_frac(&cell.text) else {
            continue;
        };
        let Some(max) = max_frac.get(&region_of[num]) else {
            continue;
        };
        if *max == 0 {
            continue;
        }
        // an integer pads one extra column for its implied dot
        let pad = if frac == 0 { *max + 1 } else { *max - frac };
        if pad > 0 {
            cell.text.push_str(&" ".repeat(pad));
        }
    }
}

/// The fractional width of a plain number text (`1,270.50` -> 2), or
/// None for anything else (quantities, hex, bin, errors).
fn scalar_frac(text: &str) -> Option<usize> {
    let (int, frac) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text, None),
    };
    if !int
        .strip_prefix('-')
        .unwrap_or(int)
        .chars()
        .all(|c| c.is_ascii_digit() || c == ',')
    {
        return None;
    }
    match frac {
        Some(f) if f.chars().all(|c| c.is_ascii_digit()) => Some(f.len()),
        Some(_) => None,
        None => Some(0),
    }
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
        // region alignment: the region's only decimal (3.5) gives the
        // integer a two-space pad (digit + implied dot column)
        assert_eq!(map[&3].text, "7  ");
        assert!(map[&5].is_error);
        assert_eq!(map[&5].text, "division by zero");
    }

    #[test]
    fn empty_sheet_has_no_cells() {
        assert!(compute("").is_empty());
    }

    #[test]
    fn regions_share_a_decimal_column() {
        let sheet = "# A\n120\n3.5\n\n## B\n7\n2.25\n5000 g\n";
        let map = compute(sheet);
        // region A: max frac is 1, so the integer pads two (digit +
        // implied dot column)
        assert_eq!(map[&2].text, "120  ");
        assert_eq!(map[&3].text, "3.5");
        // region B: max frac is 2
        assert_eq!(map[&6].text, "7   ");
        assert_eq!(map[&7].text, "2.25");
        // quantities keep the engine's shape
        assert_eq!(map[&8].text, "5000 g");
    }

    #[test]
    fn integer_only_regions_do_not_pad() {
        let map = compute("10\n20\n");
        assert_eq!(map[&1].text, "10");
        assert_eq!(map[&2].text, "20");
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

//! Result formatting per spec.md: integer parts grouped in threes
//! with `,`; up to 12 significant digits; trailing zeros trimmed;
//! whole results print without a decimal part.

/// Format with a cap on decimal places (the app's answer-decimals
/// setting): the value rounds to `max_decimals` places, then runs
/// the standard pipeline. A value the cap would zero out keeps the
/// 12-significant rendering instead (the guard against lossy
/// display: `1/3` with a zero-decimal cap must not read as zero).
pub fn format_number_with(x: f64, max_decimals: u32) -> String {
    let scale = 10f64.powi(max_decimals.min(12) as i32);
    let rounded = (x * scale).round() / scale;
    if x != 0.0 && rounded == 0.0 {
        format_number(x)
    } else {
        format_number(rounded)
    }
}

/// Format a finite value for an answer cell.
pub fn format_number(x: f64) -> String {
    debug_assert!(x.is_finite(), "evaluator must not emit non-finite values");
    if x == 0.0 {
        // catches -0.0 too: negative zero is not a useful answer
        return "0".to_string();
    }
    let neg = x < 0.0;
    // `{:.11e}` gives one leading digit plus 11 fraction digits:
    // exactly 12 significant, already rounded, carry into the exponent
    // included.
    let sci = format!("{:.11e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').expect("e-notation always present");
    let exp: i32 = exp.parse().expect("exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    debug_assert_eq!(digits.len(), 12);

    let body = if exp >= 0 {
        let int_end = (exp as usize + 1).min(digits.len());
        let mut int_part: String = digits[..int_end].to_string();
        int_part.push_str(&"0".repeat(exp as usize + 1 - int_end));
        let frac = digits[int_end..].trim_end_matches('0');
        if frac.is_empty() {
            group(&int_part)
        } else {
            format!("{}.{}", group(&int_part), frac)
        }
    } else {
        let zeros = (-exp - 1) as usize;
        let frac_full = format!("{}{digits}", "0".repeat(zeros));
        let frac = frac_full.trim_end_matches('0');
        if frac.is_empty() {
            "0".to_string()
        } else {
            format!("0.{frac}")
        }
    };
    if neg { format!("-{body}") } else { body }
}

/// Insert `,` every three digits counting from the right.
fn group(int_part: &str) -> String {
    let bytes = int_part.as_bytes();
    let mut out = String::with_capacity(int_part.len() + int_part.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(b as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{format_number as f, format_number_with as fw};

    #[test]
    fn formats_whole_and_grouped() {
        assert_eq!(f(0.0), "0");
        assert_eq!(f(-0.0), "0");
        assert_eq!(f(4.0), "4");
        assert_eq!(f(100.0), "100");
        assert_eq!(f(1234567.89), "1,234,567.89");
        assert_eq!(f(1e12), "1,000,000,000,000");
        assert_eq!(f(-42.5), "-42.5");
    }

    #[test]
    fn caps_twelve_significant_digits() {
        // hides float noise: 0.1 + 0.2 is 0.30000000000000004
        assert_eq!(f(0.1 + 0.2), "0.3");
        assert_eq!(f(1.0 / 3.0), "0.333333333333");
        assert_eq!(f(2.0 / 3.0), "0.666666666667");
    }

    #[test]
    fn handles_small_and_huge() {
        assert_eq!(f(0.001), "0.001");
        assert_eq!(f(1e-7), "0.0000001");
        assert_eq!(f(1e13), "10,000,000,000,000");
    }

    #[test]
    fn trims_trailing_zeros() {
        assert_eq!(f(3.50), "3.5");
        assert_eq!(f(7.00), "7");
        assert_eq!(f(1.2300), "1.23");
    }

    #[test]
    fn decimal_caps_round_and_stay_grouped() {
        assert_eq!(fw(41.4519906323, 2), "41.45");
        assert_eq!(fw(8540.0 / 40.0, 2), "213.5");
        assert_eq!(fw(1234567.891, 2), "1,234,567.89");
        assert_eq!(fw(7.0, 2), "7");
        // the lossy guard: a cap that would zero the value out
        // keeps the full rendering
        assert_eq!(fw(1.0 / 3.0, 0), "0.333333333333");
    }
}

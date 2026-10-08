//! Auto list continuation: the decision for what Enter does on a
//! list line. Pure functions, so the table tests run headless; the
//! Enter wiring in `page.rs` is the thin part.

/// What Enter should do on a sheet line that opens with a list
/// marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListEnter {
    /// Content follows the marker: continue the list with this
    /// prefix (indent + marker, marker includes its trailing space),
    /// inserted after a newline at the cursor.
    Continue(String),
    /// The line is only a marker: the list ends. Delete the byte
    /// range from the line, then fall through to a plain newline
    /// (the empty-item exit every markdown editor shares).
    Exit(usize, usize),
}

/// The list continuation for `line`, or None when Enter must stay a
/// plain newline. Bullets repeat as-is; numbering increments. Only
/// `- `, `* `, `+ `, and `N.` / `N)` markers count, always followed
/// by a space, with the line's leading spaces/tabs preserved. The
/// caller guards this to lines the engine classifies as Text, so a
/// math line's Enter is never touched.
pub fn list_enter(line: &str) -> Option<ListEnter> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[indent_len..];

    let numbered = |r: &str| -> Option<(usize, String)> {
        let digits = r.len() - r.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 || digits > 9 {
            return None;
        }
        let separator = if r[digits..].starts_with(". ") {
            "."
        } else if r[digits..].starts_with(") ") {
            ")"
        } else {
            return None;
        };
        let n: u32 = r[..digits].parse().ok()?;
        Some((digits + 2, format!("{}{separator}", n + 1)))
    };

    let (marker_len, next_marker) = if rest.starts_with("- ") {
        (2, "-".to_string())
    } else if rest.starts_with("* ") {
        (2, "*".to_string())
    } else if rest.starts_with("+ ") {
        (2, "+".to_string())
    } else {
        let (len, n) = numbered(rest)?;
        (len, n)
    };

    let marker_end = indent_len + marker_len;
    if line[marker_end..].contains(|c: char| c != ' ' && c != '\t') {
        Some(ListEnter::Continue(format!(
            "{}{} ",
            &line[..indent_len],
            next_marker
        )))
    } else {
        // Nothing but spaces after the marker: an empty item, so the
        // list exits; the deletion swallows the trailing spaces too.
        Some(ListEnter::Exit(indent_len, line.len()))
    }
}

/// GFM task-list items: a list marker followed by `[ ]` or `[x]`.
/// Pure decisions; the Ctrl+click wiring in `page.rs` is thin.
///
/// Returns the byte offset of the checkbox's `[` and whether the box
/// is ticked. The box must close with `]` and the line must have
/// content or end after it (GFM's `- [ ]foo` is prose).
pub fn task_box(line: &str) -> Option<(usize, bool)> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[indent_len..];
    let after_marker = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ")
    {
        &rest[2..]
    } else {
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits > 0
            && digits <= 9
            && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") "))
        {
            &rest[digits + 2..]
        } else {
            return None;
        }
    };
    let boxed = after_marker.strip_prefix('[')?;
    let (checked, tail) = match boxed.chars().next()? {
        ' ' => (false, &boxed[1..]),
        'x' | 'X' => (true, &boxed[1..]),
        _ => return None,
    };
    let tail = tail.strip_prefix(']')?;
    if !(tail.is_empty() || tail.starts_with(' ')) {
        return None;
    }
    let at = line.len() - after_marker.len();
    Some((at, checked))
}

/// The line with its checkbox toggled, or None when the line is not
/// a task item.
pub fn toggle_task(line: &str) -> Option<String> {
    let (at, checked) = task_box(line)?;
    let mut out = line.to_string();
    let replacement = if checked { " " } else { "x" };
    out.replace_range(at + 1..at + 2, replacement);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cont(line: &str, prefix: &str) {
        assert_eq!(list_enter(line), Some(ListEnter::Continue(prefix.into())));
    }

    fn exit(line: &str, start: usize) {
        assert_eq!(list_enter(line), Some(ListEnter::Exit(start, line.len())));
    }

    #[test]
    fn bullets_repeat_with_indent() {
        cont("- buy milk", "- ");
        cont("  - nested item", "  - ");
        cont("* star item", "* ");
        cont("+ plus item", "+ ");
        cont("\t- tab-indented bullet", "\t- ");
    }

    #[test]
    fn numbering_increments_and_keeps_style() {
        cont("1. first", "2. ");
        cont("9. ninth", "10. ");
        cont("0. zeroth", "1. ");
        cont("3) paren style", "4) ");
        cont("  12. indented", "  13. ");
    }

    #[test]
    fn empty_items_exit_the_list() {
        exit("- ", 0);
        exit("  - ", 2);
        exit("2.  ", 0);
        exit("   +   ", 3);
        exit("7) ", 0);
    }

    #[test]
    fn non_markers_stay_plain_newlines() {
        assert_eq!(list_enter(""), None);
        assert_eq!(list_enter("plain prose"), None);
        assert_eq!(list_enter("-"), None);
        assert_eq!(list_enter("-item no space"), None);
        assert_eq!(list_enter("1.no space"), None);
        assert_eq!(list_enter("1."), None);
        assert_eq!(list_enter("mid - dash in prose"), None);
        assert_eq!(list_enter("9999999999. too many digits"), None);
    }

    #[test]
    fn task_boxes_recognize_and_toggle() {
        assert_eq!(task_box("- [ ] milk"), Some((2, false)));
        assert_eq!(task_box("- [x] milk"), Some((2, true)));
        assert_eq!(task_box("- [X] milk"), Some((2, true)));
        assert_eq!(task_box("  - [ ] indented"), Some((4, false)));
        assert_eq!(task_box("1. [ ] numbered"), Some((3, false)));
        assert_eq!(task_box("- [ ]"), Some((2, false)));
        // prose shapes are not checkboxes
        assert_eq!(task_box("- [ ]milk"), None);
        assert_eq!(task_box("- [y] milk"), None);
        assert_eq!(task_box("- milk"), None);
        assert_eq!(task_box("[ ] no marker"), None);
        assert_eq!(toggle_task("- [ ] milk"), Some("- [x] milk".into()));
        assert_eq!(toggle_task("- [X] milk"), Some("- [ ] milk".into()));
        assert_eq!(toggle_task("- milk"), None);
    }
}

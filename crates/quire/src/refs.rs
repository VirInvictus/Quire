//! Self-updating `&N` line references: the pure convergence logic.
//!
//! GTK-free by design (like `answers.rs`): the GTK layer reads mark
//! positions out of the buffer, calls [`converge`], and applies the
//! returned plan; every decision lives here where it can be
//! table-tested headlessly.
//!
//! The design (the fourth attempt at this feature; commits e5a593a,
//! bcc822e and b2ad51f all cascaded and were stripped in 2ab2932):
//!
//! - A referenced line's identity is a GtkTextMark, never a text
//!   diff. The GTK layer keeps one invisible right-gravity mark per
//!   referenced line (deduplicated); marks ride every edit -
//!   insert, delete, paste, undo, redo - inside the buffer's btree,
//!   and reading them fires no signals, so a rewrite can never
//!   re-arm the pipeline that caused it. Right gravity anchors the
//!   mark to the CONTENT: a line inserted at the target's start
//!   pushes the mark down with it, which is refs-follow-content.
//! - Each mark carries a `label`: the digits the affected `&N`
//!   tokens display. [`converge`] compares labels against the marks'
//!   actual positions and emits exactly the rewrites needed to make
//!   the text agree with reality; after one pass the two agree, so
//!   the pass is idempotent.
//! - Rewrites are applied blocked (the changed handler never sees
//!   them) and inside an irreversible action (they are not undo
//!   units), so undo/redo of the user's edit moves the marks back
//!   and the next pass converges the digits back. No fight is
//!   possible.
//! - A mark whose label names a line the sheet does not have yet (a
//!   forward reference) is created `clamped` to the last line and
//!   stays inert: it rewrites nothing until the line is born, then
//!   re-anchors at its label. If some other mark has moved onto that
//!   line by then, the born mark yields to it (one mark per line,
//!   and the GTK layer's table is keyed by label, so duplicates are
//!   forbidden, not merged).
//! - Hand-edited digits rebind positionally (spreadsheet-style): the
//!   retyped value looks up a fresh mark at that line, and a mark no
//!   ref claims anymore is garbage-collected.
//!
//! Only tokens the engine resolves participate: [`live_refs`] reads
//! Expression lines and skips function definitions (spec: bodies
//! cannot use refs); comments and prose never reach here.

use std::collections::{HashMap, HashSet};

use quire_eval::{LineKind, Stmt, Tok, parse, parse_sheet, tokenize};

/// One `&N` token the engine resolves: its one-based line, the
/// token's byte span within that line's raw text, and the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefToken {
    pub line: u32,
    pub span_in_line: (usize, usize),
    pub value: u32,
}

/// The `&N` tokens of a sheet that the engine will actually resolve:
/// expression lines only, function definitions excluded (bodies
/// cannot use refs), comments and prose invisible. `&0` is skipped;
/// the engine errors it and no mark should chase a line that cannot
/// exist.
pub fn live_refs(text: &str) -> Vec<RefToken> {
    let mut out = Vec::new();
    for line in parse_sheet(text) {
        if line.kind != LineKind::Expression {
            continue;
        }
        let Ok(tokens) = tokenize(&line.raw) else {
            continue;
        };
        // A function definition is the one expression shape whose
        // body must not renumber; parse failures (unit lines, mixed
        // lines) keep their tokens: the bridge resolves refs there.
        if let Ok(Stmt::FnDef(..)) = parse(&tokens) {
            continue;
        }
        for t in &tokens {
            if let Tok::LineRef(v) = t.tok
                && v >= 1
            {
                out.push(RefToken {
                    line: line.number as u32,
                    span_in_line: t.span,
                    value: v,
                });
            }
        }
    }
    out
}

/// The GTK layer's per-mark state: `label` is the digits currently
/// displayed for this mark's refs; `clamped` marks a mark created
/// while its label exceeded the sheet (a forward reference), which
/// stays inert until the line is born.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkState {
    pub label: u32,
    pub clamped: bool,
}

/// One token rewrite: replace the byte span in the one-based line
/// with `digits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub line: u32,
    pub span_in_line: (usize, usize),
    pub digits: String,
}

/// Everything one convergence pass wants done. The GTK layer applies
/// drops, then relabels, then creations (table order matters: drops
/// first free labels that relabels or creations reuse); the edits
/// splice back to front (descending position, so earlier splices do
/// not disturb later spans).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Converge {
    pub edits: Vec<Edit>,
    /// `(old label, new label)` for marks that moved.
    pub relabel: Vec<(u32, u32)>,
    /// Marks to create (`clamped` = anchor at the last line until
    /// the label's line is born).
    pub create: Vec<MarkState>,
    /// Labels to garbage-collect (unclaimed, unreadable, merged, or
    /// displaced marks).
    pub drop: Vec<u32>,
}

impl Converge {
    pub fn has_work(&self) -> bool {
        !(self.edits.is_empty()
            && self.relabel.is_empty()
            && self.create.is_empty()
            && self.drop.is_empty())
    }
}

/// Apply a plan's relabel list to a label-keyed table (the GTK layer's
/// mark table, or a test harness's stand-in). Relabels are a
/// SIMULTANEOUS rename: when refs target consecutive lines the plan
/// chains (25->26, 26->27, ...), so every relabeled entry must be
/// pulled out before any is reinserted - a sequential remove/insert
/// evicts the entry each new key collides with, collapsing the whole
/// table onto one orphaned entry (the mortgage-template bug).
pub fn apply_relabels<V>(table: &mut std::collections::HashMap<u32, V>, relabel: &[(u32, u32)]) {
    let mut moved = Vec::with_capacity(relabel.len());
    for (old, new) in relabel {
        if let Some(value) = table.remove(old) {
            moved.push((*new, value));
        }
    }
    for (new, value) in moved {
        table.insert(new, value);
    }
}

/// One convergence pass over a sheet.
///
/// `marks` is the GTK layer's current table; `positions` maps a label
/// to the mark's actual one-based line right now. The GTK layer may
/// omit entries: a missing or out-of-range position (the buffer's
/// phantom trailing line, an empty sheet) garbage-collects the mark,
/// because a mark with no trustworthy position must never adopt a
/// ref typed later. Returns the rewrites, relabels, creations, and
/// drops that make the text agree with the marks.
pub fn converge(text: &str, marks: &[MarkState], positions: &[(u32, u32)]) -> Converge {
    let line_count = text.lines().count() as u32;
    let tokens = live_refs(text);
    let claimed: HashSet<u32> = tokens.iter().map(|t| t.value).collect();
    let at = |label: u32| {
        positions
            .iter()
            .find(|(l, _)| *l == label)
            .map(|(_, current)| *current)
    };

    let mut out = Converge::default();

    // Live marks: unclamped, with a trustworthy position. Two marks
    // can land on the same line (deleting the line between their
    // targets); they can never separate again, so the first
    // (deterministically by label) keeps the position and the rest
    // are dropped, their refs following the keeper.
    let mut live: Vec<(u32, u32)> = Vec::new();
    for m in marks {
        if m.clamped {
            continue;
        }
        match at(m.label) {
            Some(current) if current >= 1 && current <= line_count => {
                live.push((m.label, current));
            }
            _ => out.drop.push(m.label),
        }
    }
    live.sort_unstable();
    let mut kept: Vec<(u32, u32)> = Vec::new();
    let mut follow: HashMap<u32, u32> = HashMap::new();
    for (label, current) in live {
        match kept.last() {
            Some(&(k_label, k_current)) if k_current == current => {
                follow.insert(label, k_label);
                out.drop.push(label);
            }
            _ => kept.push((label, current)),
        }
    }

    // Kept marks move to their actual position: label = current.
    // An unclaimed mark (its ref moved away or vanished) is dropped,
    // never relabelled: nothing displays its old digits anymore.
    let mut new_label: HashMap<u32, u32> = HashMap::new();
    for &(label, current) in &kept {
        new_label.insert(label, current);
        if claimed.contains(&label) {
            if label != current {
                out.relabel.push((label, current));
            }
        } else {
            out.drop.push(label);
        }
    }
    for (merged, keeper) in &follow {
        new_label.insert(*merged, new_label[keeper]);
    }
    let taken: HashSet<u32> = kept.iter().map(|&(_, current)| current).collect();

    // Clamped marks: inert forward refs wait; born ones re-anchor at
    // their label - unless another mark has taken that line, in
    // which case the live mark serves it and the clamped mark dies.
    let mut waiting: HashSet<u32> = HashSet::new();
    for m in marks.iter().filter(|m| m.clamped) {
        if m.label > line_count {
            waiting.insert(m.label);
        } else {
            out.drop.push(m.label);
            if !taken.contains(&m.label) && !new_label.values().any(|v| *v == m.label) {
                out.create.push(MarkState {
                    label: m.label,
                    clamped: false,
                });
            }
        }
    }
    // An inert mark whose label names a line a live mark now holds
    // is displaced: the label must stay unique in the table.
    for label in waiting.clone() {
        if taken.contains(&label) {
            waiting.remove(&label);
            out.drop.push(label);
        }
    }

    // Rewrites: each token's target is its mark's new label (or its
    // own value when the mark is inert, being born, or absent). The
    // span covers ONLY the digits: the `&` never leaves the line.
    // (The 0.5.1 cascade grew `&11` into `&111` partly because its
    // splices replaced the whole `&N` span with bare digits.)
    for t in &tokens {
        let target = new_label.get(&t.value).copied().unwrap_or(t.value);
        if target != t.value {
            out.edits.push(Edit {
                line: t.line,
                span_in_line: (t.span_in_line.0 + 1, t.span_in_line.1),
                digits: target.to_string(),
            });
        }
    }

    // Creations: FINAL token values nothing in the table will hold
    // after this pass (a ref rewritten to a new value claims the new
    // label, never the old one).
    let mut settled: HashSet<u32> = new_label.values().copied().collect();
    settled.extend(out.create.iter().map(|m| m.label));
    settled.extend(waiting.iter().copied());
    let mut final_targets: HashSet<u32> = HashSet::new();
    for t in &tokens {
        final_targets.insert(new_label.get(&t.value).copied().unwrap_or(t.value));
    }
    for value in final_targets {
        if !settled.contains(&value) {
            out.create.push(MarkState {
                label: value,
                clamped: value > line_count,
            });
            settled.insert(value);
        }
    }

    out.edits
        .sort_by_key(|e| std::cmp::Reverse((e.line, e.span_in_line.0)));
    out.relabel.sort_unstable();
    out.create.sort_by_key(|m| m.label);
    out.create.dedup();
    out.drop.sort_unstable();
    out.drop.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless stand-in for the buffer: the text plus one
    /// right-gravity line-start anchor per mark. `insert_lines` and
    /// `delete_lines` model what the btree does to such anchors,
    /// which is the whole contract the GTK layer relies on.
    struct Sheet {
        text: String,
        marks: Vec<MarkState>,
        /// each mark's actual position (one-based line)
        at: Vec<u32>,
    }

    impl Sheet {
        fn new(text: &str) -> Self {
            let mut sheet = Self {
                text: text.to_string(),
                marks: Vec::new(),
                at: Vec::new(),
            };
            sheet.settle();
            sheet
        }

        /// The positions the GTK layer would report.
        fn positions(&self) -> Vec<(u32, u32)> {
            self.marks
                .iter()
                .zip(&self.at)
                .map(|(m, at)| (m.label, *at))
                .collect()
        }

        /// Insert `count` blank lines at one-based line `at`; a
        /// right-gravity line-start anchor at or after the insert
        /// point moves down with the content.
        fn insert_lines(&mut self, at: u32, count: u32) {
            let mut lines: Vec<String> = self.text.lines().map(String::from).collect();
            let point = (at as usize).clamp(1, lines.len() + 1) - 1;
            lines.splice(point..point, vec![String::new(); count as usize]);
            self.set_text(lines.join("\n"));
            for a in &mut self.at {
                if *a >= at {
                    *a += count;
                }
            }
        }

        /// Delete one-based lines `from..from+count`; anchors inside
        /// the range collapse to the range start (the btree moves
        /// them to the deletion boundary); the boundary line's own
        /// anchor stays with its content, which moves up.
        fn delete_lines(&mut self, from: u32, count: u32) {
            let mut lines: Vec<String> = self.text.lines().map(String::from).collect();
            let start = (from as usize).saturating_sub(1);
            let end = (start + count as usize).min(lines.len());
            lines.drain(start..end);
            self.set_text(lines.join("\n"));
            let boundary = from.min(self.line_count().max(1));
            for a in &mut self.at {
                if *a >= from && *a < from + count {
                    *a = boundary;
                } else if *a >= from + count {
                    *a -= count;
                }
            }
        }

        fn set_text(&mut self, joined: String) {
            self.text = if joined.is_empty() || joined.ends_with('\n') {
                joined
            } else {
                format!("{joined}\n")
            };
        }

        fn line_count(&self) -> u32 {
            self.text.lines().count() as u32
        }

        /// One convergence pass, applied the way the GTK layer
        /// applies it. Returns whether anything happened.
        fn step(&mut self) -> bool {
            let c = converge(&self.text.clone(), &self.marks, &self.positions());
            if !c.has_work() {
                return false;
            }
            // edits, back to front: byte spans within a line, which
            // the model shares with the buffer for ASCII digits
            for e in &c.edits {
                let mut lines: Vec<String> = self.text.lines().map(String::from).collect();
                let raw = &mut lines[(e.line - 1) as usize];
                raw.replace_range(e.span_in_line.0..e.span_in_line.1, &e.digits);
                self.set_text(lines.join("\n"));
            }
            let mut kept_marks = Vec::new();
            let mut kept_at = Vec::new();
            for (m, at) in self.marks.iter().zip(&self.at) {
                if c.drop.contains(&m.label) {
                    continue;
                }
                let label = c
                    .relabel
                    .iter()
                    .find(|(old, _)| old == &m.label)
                    .map(|(_, new)| *new)
                    .unwrap_or(m.label);
                // any mark that survives a live pass is unclamped
                kept_marks.push(MarkState {
                    label,
                    clamped: false,
                });
                kept_at.push(*at);
            }
            for m in c.create {
                // created marks anchor at their label, clamped to the
                // sheet's last line for forward references
                let at = if m.clamped {
                    self.line_count().max(1)
                } else {
                    m.label
                };
                kept_marks.push(m);
                kept_at.push(at);
            }
            self.marks = kept_marks;
            self.at = kept_at;
            true
        }

        /// Converge to a fixed point (one pass in every tested case;
        /// the cap only proves termination).
        fn settle(&mut self) {
            for _ in 0..10 {
                if !self.step() {
                    return;
                }
            }
            panic!("convergence did not settle in 10 passes");
        }

        fn line(&self, n: u32) -> String {
            self.text.lines().nth(n as usize - 1).unwrap().to_string()
        }
    }

    fn labels(sheet: &Sheet) -> Vec<u32> {
        let mut v: Vec<u32> = sheet.marks.iter().map(|m| m.label).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn live_refs_reads_expression_lines_only() {
        let text = "\
5 + 5
x = &1 * 2
// &4 in a comment stays text
see line &3 for the total
f(a) = &2 + a
&1 + &2 + &7
&0
";
        let refs = live_refs(text);
        let seen: Vec<(u32, u32)> = refs.iter().map(|t| (t.line, t.value)).collect();
        assert_eq!(
            seen,
            vec![(2, 1), (6, 1), (6, 2), (6, 7)],
            "comments, prose, fn bodies, and &0 must not bind: {refs:?}"
        );
    }

    #[test]
    fn unit_lines_keep_their_tokens() {
        // the scalar parse fails on these, but the engine resolves
        // refs through the token scan; they are not definitions
        let text = "5 + 5\nbag = 2 kg + 300 g\n&1 kg\n";
        let refs = live_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].value, 1);
    }

    #[test]
    fn fresh_marks_anchor_at_the_authored_numbers() {
        let mut sheet = Sheet::new("5 + 5\n&1 * 3\n&2 - 1\n");
        assert_eq!(labels(&sheet), vec![1, 2]);
        assert_eq!(sheet.line(2), "&1 * 3");
        assert!(!sheet.step(), "a freshly loaded sheet is already converged");
    }

    #[test]
    fn insert_above_shifts_refs_down() {
        let mut sheet = Sheet::new("5 + 5\n&1 * 3\n&2 - 1\n");
        sheet.insert_lines(1, 1);
        sheet.settle();
        assert_eq!(sheet.line(3), "&2 * 3");
        assert_eq!(sheet.line(4), "&3 - 1");
    }

    #[test]
    fn deleting_the_target_clamps_to_the_successor() {
        let mut sheet = Sheet::new("5 + 5\n&1 * 3\n&2 - 1\n");
        sheet.delete_lines(1, 1);
        sheet.settle();
        // line 1's place is taken by the old line 2; &1 stays (the
        // successor rule), and the &2 mark rode its content up, so
        // both now point at line 1
        assert_eq!(sheet.line(1), "&1 * 3");
        assert_eq!(sheet.line(2), "&1 - 1");
        assert_eq!(labels(&sheet), vec![1]);
    }

    #[test]
    fn refs_above_the_edit_point_stay_put() {
        let mut sheet = Sheet::new("&1 + 1\n5\n6\n");
        sheet.insert_lines(3, 2);
        sheet.settle();
        assert_eq!(sheet.line(1), "&1 + 1");
    }

    #[test]
    fn chained_refs_shift_in_one_pass() {
        let mut sheet = Sheet::new("5\nx = &1\n&2 + 1\n");
        sheet.insert_lines(1, 1);
        let c = converge(&sheet.text.clone(), &sheet.marks, &sheet.positions());
        assert_eq!(c.edits.len(), 2, "both refs renumber in the same pass");
        sheet.settle();
        assert_eq!(sheet.line(3), "x = &2");
        assert_eq!(sheet.line(4), "&3 + 1");
    }

    #[test]
    fn digit_width_grows_cleanly() {
        let mut sheet = Sheet::new("9 + 0\n&1\n");
        sheet.insert_lines(1, 2);
        sheet.settle();
        assert_eq!(sheet.line(4), "&3");
        let mut sheet = Sheet::new("9 + 0\n9 + 1\n9 + 2\n&3\n");
        sheet.insert_lines(1, 1);
        sheet.settle();
        assert_eq!(sheet.line(5), "&4");
    }

    #[test]
    fn multiple_refs_on_one_line_independent() {
        let mut sheet = Sheet::new("a = 1\nb = 2\nc = 3\n&1 + &2 + &3\n");
        sheet.insert_lines(2, 1);
        sheet.settle();
        assert_eq!(sheet.line(5), "&1 + &3 + &4");
    }

    #[test]
    fn refs_to_one_target_share_one_mark() {
        let sheet = Sheet::new("5\n&1\n&1\n");
        assert_eq!(labels(&sheet), vec![1], "one mark per referenced line");
    }

    #[test]
    fn self_reference_stays_self_pointing() {
        let mut sheet = Sheet::new("&1 + 1\n");
        sheet.insert_lines(1, 1);
        sheet.settle();
        assert_eq!(sheet.line(2), "&2 + 1");
    }

    #[test]
    fn pasting_a_block_shifts_by_the_block() {
        let mut sheet = Sheet::new("7\n&1 * 2\n");
        sheet.insert_lines(1, 3);
        sheet.settle();
        assert_eq!(sheet.line(5), "&4 * 2");
    }

    #[test]
    fn undo_reverts_the_digits() {
        let mut sheet = Sheet::new("5 + 5\n&1 * 3\n");
        sheet.insert_lines(1, 1);
        sheet.settle();
        assert_eq!(sheet.line(3), "&2 * 3");
        // the user's undo: the inserted line goes away, marks ride
        // back, the next pass converges the digits back
        sheet.delete_lines(1, 1);
        sheet.settle();
        assert_eq!(sheet.line(2), "&1 * 3");
        assert_eq!(labels(&sheet), vec![1]);
    }

    #[test]
    fn colliding_marks_merge() {
        // deleting the line between two targets lands both marks on
        // the same line; one mark survives and both refs follow it
        let mut sheet = Sheet::new("a = 1\nb = 2\nc = 3\n&1 + &2 + &3\n");
        sheet.delete_lines(2, 1);
        sheet.settle();
        assert_eq!(sheet.line(3), "&1 + &2 + &2");
        assert_eq!(labels(&sheet), vec![1, 2]);
    }

    #[test]
    fn deleting_everything_is_benign() {
        let mut sheet = Sheet::new("5\n&1\n");
        sheet.delete_lines(1, 2);
        sheet.settle();
        assert_eq!(sheet.text.lines().count(), 0);
        // no ref survives to claim them, and their positions are
        // unreadable on an empty sheet: garbage-collected
        assert!(sheet.marks.is_empty());
    }

    #[test]
    fn forward_refs_stay_inert_until_the_line_is_born() {
        let mut sheet = Sheet::new("&3\n5\n");
        // the ref names line 3; the sheet has two lines: the mark is
        // clamped and inert, and nothing rewrites while it grows
        assert_eq!(labels(&sheet), vec![3]);
        sheet.insert_lines(1, 1);
        sheet.settle();
        assert_eq!(
            sheet.line(2),
            "&3",
            "inert while clamped; born at its label, no rewrite"
        );
        // from here it is a live mark and follows content
        sheet.insert_lines(1, 1);
        sheet.settle();
        assert_eq!(sheet.line(3), "&4");
    }

    #[test]
    fn a_born_forward_ref_yields_to_the_mark_that_took_its_line() {
        let mut sheet = Sheet::new("&4\n1\n&2\n");
        // &4 is forward (3-line sheet), &2 is live at line 2
        assert_eq!(labels(&sheet), vec![2, 4]);
        // two lines in at the top push the &2 mark onto line 4 -
        // the very line the forward ref is waiting for
        sheet.insert_lines(1, 2);
        sheet.settle();
        assert_eq!(sheet.line(5), "&4", "the live mark serves line 4");
        assert_eq!(
            labels(&sheet),
            vec![4],
            "the born mark yielded: one per line"
        );
    }

    #[test]
    fn hand_typed_digit_edits_rebind_positionally() {
        // the user retypes a ref's digits: the old mark loses its
        // last claim and is garbage-collected, the new value anchors
        // positionally (spreadsheet-style)
        let mut sheet = Sheet::new("5\n7\n&1 + 0\n");
        let mut lines: Vec<String> = sheet.text.lines().map(String::from).collect();
        lines[2] = "&2 + 0".to_string();
        sheet.set_text(lines.join("\n"));
        sheet.settle();
        assert_eq!(labels(&sheet), vec![2], "mark 1 is gone, 2 is anchored");
        assert_eq!(sheet.line(3), "&2 + 0");
    }

    /// The GTK layer's table mutation, modeled: relabels applied the
    /// way apply_ref_plan did BEFORE the fix - sequentially into a
    /// label-keyed map. Kept as the pin that the shared helper (which
    /// page.rs now calls) is a simultaneous rename, not a chain of
    /// keyed inserts.
    #[test]
    fn apply_relabels_is_a_simultaneous_rename() {
        let mut table: std::collections::HashMap<u32, u32> =
            [(25, 125), (26, 126), (27, 127), (28, 128)]
                .into_iter()
                .collect();
        let chain = [(25, 26), (26, 27), (27, 28), (28, 29)];
        super::apply_relabels(&mut table, &chain);
        let mut keys: Vec<u32> = table.keys().copied().collect();
        keys.sort();
        assert_eq!(keys, vec![26, 27, 28, 29]);
        // every value rode with its entry: the chain renamed the keys
        // without losing or swapping any
        for (old, new) in chain {
            assert_eq!(table[&new], 100 + old);
        }
    }

    /// The mortgage-template shape: four refs to a contiguous run of
    /// derivation lines, two Enters above the block. Under the old
    /// sequential keyed apply the table collapsed onto one mark and
    /// the second Enter froze the first three refs and dragged the
    /// last backward (26/27/28/27).
    #[test]
    fn mortgage_refs_survive_two_enters_above() {
        let mortgage = crate::templates::TEMPLATES
            .iter()
            .find(|(name, _)| name == &"Mortgage")
            .map(|(_, text)| *text)
            .expect("the mortgage template ships");
        let mut sheet = Sheet::new(mortgage);
        // Enter at the end of line 5, then Enter again: two blank
        // lines above the whole ref block
        sheet.insert_lines(6, 1);
        assert!(sheet.step());
        sheet.insert_lines(6, 1);
        assert!(sheet.step());
        let refs: Vec<u32> = crate::refs::live_refs(&sheet.text)
            .iter()
            .map(|t| t.value)
            .collect();
        assert_eq!(refs, vec![27, 28, 29, 30]);
        // the table stayed whole: one mark per reference
        assert_eq!(sheet.marks.len(), 4);
        // and the digits resolve to the derivation's values
        assert_eq!(sheet.text.lines().nth(10).map(str::trim), Some("&27"));
    }

    #[test]
    fn coalesced_and_sequential_inserts_converge_identically() {
        let mortgage = crate::templates::TEMPLATES
            .iter()
            .find(|(name, _)| name == &"Mortgage")
            .map(|(_, text)| *text)
            .expect("the mortgage template ships");
        let mut two_runs = Sheet::new(mortgage);
        two_runs.insert_lines(6, 1);
        two_runs.step();
        two_runs.insert_lines(6, 1);
        two_runs.step();
        let mut coalesced = Sheet::new(mortgage);
        coalesced.insert_lines(6, 2);
        coalesced.step();
        assert_eq!(two_runs.text, coalesced.text);
    }

    #[test]
    fn welcome_sheet_and_templates_converge_without_edits() {
        for (name, text) in [
            ("welcome", crate::page::WELCOME_SHEET),
            ("portfolio", crate::templates::TEMPLATES[0].1),
            ("budget", crate::templates::TEMPLATES[1].1),
            ("mortgage", crate::templates::TEMPLATES[6].1),
        ] {
            let mut sheet = Sheet::new(text);
            assert!(
                !sheet.step(),
                "{name} must load converged: refs are as authored"
            );
        }
    }

    #[test]
    fn empty_input_is_stable() {
        assert_eq!(converge("", &[], &[]), Converge::default());
        assert!(live_refs("").is_empty());
    }
}

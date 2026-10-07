//! The editor page: one buffer, one sheet view, one answers column,
//! and the document operations around them (open, save, external
//! change monitoring).

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use sourceview5::prelude::*;

use gtk4::prelude::TextViewExt;

use crate::answers;
use crate::renderer::AnswersRenderer;
use quire_eval::{SheetIndex, index_sheet};

/// Fired after each evaluation burst with the fresh sheet index.
type ReindexCallback = Box<dyn Fn(&SheetIndex)>;

use crate::{styles, view::QuireView};

/// First-run content: the tour. Every computational line must
/// evaluate (pinned by the test below) - a welcome sheet with an
/// error cell is a broken first impression.
const WELCOME_SHEET: &str = "\
# Welcome to Quire

// notes, lists, and math share one plain-text sheet, and every
// expression answers on its own line - live, as you type.

- lists read as markdown; Enter continues them
- math just calculates
- edit anything below and the answers follow

## Percents, the way you say them
200 + 15%
240 - 10%
15% of 80
answer * 2

## Variables, references, and totals
rent = 950 @fixed
groceries = 320 @fixed
fun = 150 @fun
total @fixed
total @fun
total

## Units come built in
5 kg + 300 g
2 hours + 30 minutes
26.2 miles -> km

## Functions
twice(x) = x * 2
twice(21)
2 kg + twice(2 kg)
rate(x) = x / 100
rate(40) of 90

## Recursion
fact(0) = 1
fact(n) = n * fact(n - 1)
fact(6)

## Currency, offline-first
// uncomment once rates are loaded (menu: Refresh currency rates):
// 50 USD -> EUR   // 46.5 €

## Dates
3 weeks from today
90 minutes from now
6 months ago

## Line references
&11 * 2

## Mix words with math
50 apples at 3 each
2 coffees plus 1 tea

## Hand-typed price snapshots
aapl = 10 * 190 @ 2026-10-06 @stocks
msft = 4 * 410 @ 2026-10-06 @stocks
bonds = 5000 @bonds
total @stocks
whole = total

## Editing
// Tab completes a variable name; Ctrl+C copies the answer
// Ctrl+B jumps to a definition; Ctrl+L toggles line numbers
// Alt+Up/Down cycles a line's format (fixed decimals, hex, bin)
// &4 references line 4's answer; the app renumbers as lines shift
// drag any text file onto the window to open it
// the menu sets how many decimals answers show
// errors underline the broken token: hover the red cell for the
// full message - try uncommenting this one:
// 3 kg + 5 m

// this sheet is yours - edit it, or start fresh with Ctrl+N
";

/// The net line shift between two sheet versions: Some((first
/// moved 1-based line, delta)) when k lines were inserted or
/// removed at a single point; None when the line count is unchanged
/// or the change is not a clean tail shift.
pub(crate) fn line_shift(old: &str, new: &str) -> Option<(usize, i32)> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let delta = new_lines.len() as i32 - old_lines.len() as i32;
    if delta == 0 {
        return None;
    }
    let i = old_lines
        .iter()
        .zip(new_lines.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(old_lines.len().min(new_lines.len()));
    let tail_matches = if delta > 0 {
        old_lines[i..] == new_lines[i + delta as usize..]
    } else {
        old_lines[i + (-delta) as usize..] == new_lines[i..]
    };
    if !tail_matches {
        return None;
    }
    Some((i + 1, delta))
}

/// The `&N` tokens of a sheet: (byte range, value) in order.
fn collect_refs(text: &str) -> Vec<((usize, usize), String)> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut consumed = 0usize;
    while let Some(rel) = rest.find('&') {
        let at = consumed + rel;
        let digits: String = rest[rel + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            rest = &rest[rel + 1..];
            consumed = at + 1;
            continue;
        }
        out.push(((at, at + 1 + digits.len()), digits.clone()));
        let skip = rel + 1 + digits.len();
        rest = &rest[skip..];
        consumed += skip;
    }
    out
}

pub struct QuirePage {
    pub view: QuireView,
    buffer: sourceview5::Buffer,
    renderer: AnswersRenderer,
    file: RefCell<Option<gio::File>>,
    monitor: RefCell<Option<gio::FileMonitor>>,
    /// guard so the file monitor ignores the app's own writes
    loading: Cell<bool>,
    /// called after each evaluation burst with the fresh sheet index
    on_reindex: RefCell<Option<ReindexCallback>>,
    /// the live answers map, for copy-answer and friends
    answers: RefCell<std::collections::HashMap<u32, answers::AnswerCell>>,
    /// the sheet text as of the last evaluation pass: diffed against
    /// the current text to detect line shifts for &N renumbering
    baseline: RefCell<String>,
    /// per-line answer formats (Alt+Up/Down), keyed by line number;
    /// the choice rides the NUMBER, so it shifts with edits until the
    /// stable line-ids gate lands
    formats: RefCell<std::collections::HashMap<u32, answers::LineFormat>>,
}

impl QuirePage {
    pub fn new() -> Rc<Self> {
        let buffer = sourceview5::Buffer::new(None::<&gtk4::TextTagTable>);
        let lang = styles::language();
        buffer.set_language(lang.as_ref());
        buffer.set_highlight_syntax(lang.is_some());
        if let Some(scheme) = styles::scheme(vir_gtk::portal::is_dark()) {
            buffer.set_style_scheme(Some(&scheme));
        }

        let view = QuireView::with_buffer(&buffer);
        view.set_wrap_mode(gtk4::WrapMode::Word);
        // breathing room; the answers column lives at the right edge
        view.set_top_margin(16);
        view.set_bottom_margin(16);
        view.set_left_margin(18);
        view.set_right_margin(6);
        view.add_css_class("quire-editor");

        let renderer = AnswersRenderer::with_buffer(&buffer);
        let gutter = sourceview5::prelude::ViewExt::gutter(&view, gtk4::TextWindowType::Right);
        gutter.insert(&renderer, 0);

        let page = Rc::new(Self {
            view: view.clone(),
            buffer: buffer.clone(),
            renderer,
            file: RefCell::new(None),
            monitor: RefCell::new(None),
            loading: Cell::new(false),
            on_reindex: RefCell::new(None),
            answers: RefCell::new(std::collections::HashMap::new()),
            baseline: RefCell::new(String::new()),
            formats: RefCell::new(std::collections::HashMap::new()),
        });
        Self::wire_evaluation(&page, &buffer);
        Self::wire_answer_tooltips(&page);
        // one delayed repaint: on a very first run the fonts were
        // installed moments ago and the earliest frames can resolve
        // text against a cold font cache; repainting once the map is
        // warm makes the column correct without a restart
        let repaint = page.renderer.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            repaint.queue_draw()
        });
        page.load(WELCOME_SHEET, None);
        page
    }

    /// The buffer, for signal wiring outside the page.
    pub fn buffer(&self) -> &sourceview5::Buffer {
        &self.buffer
    }

    /// The document file backing this sheet, if saved or opened.
    pub fn file(&self) -> Option<gio::File> {
        self.file.borrow().clone()
    }

    /// The sheet's display name: the file's basename or "Untitled".
    pub fn display_name(&self) -> String {
        self.file
            .borrow()
            .as_ref()
            .and_then(|f| f.basename())
            .map(|b| b.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Replace the whole sheet with `text` against `file` (None =
    /// untitled). Marks the buffer clean.
    pub fn load(self: &Rc<Self>, text: &str, file: Option<gio::File>) {
        self.loading.set(true);
        self.buffer.set_text(text);
        self.buffer.set_modified(false);
        self.loading.set(false);
        *self.file.borrow_mut() = file.clone();
        self.watch(file.as_ref());
        self.renderer.set_answers(answers::compute(text));
    }

    /// Save the sheet to `file`; returns the IO error message on
    /// failure. Marks the buffer clean and moves the document there.
    pub fn save_to(self: &Rc<Self>, file: &gio::File) -> Result<(), String> {
        let text = self
            .buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true);
        self.loading.set(true);
        let result = file
            .replace_contents(
                text.as_bytes(),
                None,
                false,
                gio::FileCreateFlags::REPLACE_DESTINATION,
                gio::Cancellable::NONE,
            )
            .map(|_| ())
            .map_err(|e| e.to_string());
        self.loading.set(false);
        match result {
            Ok(()) => {
                self.buffer.set_modified(false);
                *self.file.borrow_mut() = Some(file.clone());
                self.watch(Some(file));
                Ok(())
            }
            Err(message) => Err(message),
        }
    }

    /// Read `file` and load it; returns the IO error message on
    /// failure.
    pub fn open(self: &Rc<Self>, file: &gio::File) -> Result<(), String> {
        let (bytes, _) = file
            .load_contents(gio::Cancellable::NONE)
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        self.load(&text, Some(file.clone()));
        Ok(())
    }

    /// Start a new empty sheet with no file behind it.
    pub fn reset_to_new(self: &Rc<Self>) {
        self.load("", None);
    }

    /// Watch `file` for out-of-app writes: reload while the sheet is
    /// clean, stand down while dirty.
    pub fn watch(self: &Rc<Self>, file: Option<&gio::File>) {
        let Some(file) = file else {
            *self.monitor.borrow_mut() = None;
            return;
        };
        match file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
            Ok(monitor) => {
                let page = Rc::downgrade(self);
                monitor.connect_changed(move |_monitor, _other, _file, _event| {
                    let Some(page) = page.upgrade() else { return };
                    if page.loading.get() || page.buffer.is_modified() {
                        return;
                    }
                    let file = page.file.borrow().clone();
                    if let Some(file) = file
                        && page.open(&file).is_ok()
                    {
                        page.renderer.queue_draw();
                    }
                });
                *self.monitor.borrow_mut() = Some(monitor);
            }
            Err(_) => {
                *self.monitor.borrow_mut() = None;
            }
        }
    }

    /// Editing-key behaviors on the view: Tab completes the word
    /// before the cursor when exactly one bound variable matches;
    /// Ctrl+C with no selection copies the current line's answer;
    /// Ctrl+B jumps to the nearest definition above.
    pub fn wire_editing_keys(self: &Rc<Self>) {
        let controller = gtk4::EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let page = Rc::downgrade(self);
        controller.connect_key_pressed(move |_controller, keyval, _code, state| {
            let Some(page) = page.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = state.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let name = keyval.name().map(|n| n.to_string());

            if ctrl && name.as_deref() == Some("c") && !page.buffer.has_selection() {
                let line = page.cursor_iter().line() as u32 + 1;
                if let Some(cell) = page.answer_for_line(line) {
                    page.copy_to_clipboard(&cell.text);
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }

            if ctrl && name.as_deref() == Some("b") {
                if let Some(word) = page.word_at_cursor() {
                    let cursor_line = page.cursor_iter().line() as usize + 1;
                    if let Some((line, _)) = page
                        .index()
                        .assignments
                        .into_iter()
                        .rfind(|(l, n)| n == &word && *l < cursor_line)
                    {
                        page.goto_line(line);
                        return glib::Propagation::Stop;
                    }
                }
                return glib::Propagation::Proceed;
            }

            let alt = state.contains(gtk4::gdk::ModifierType::ALT_MASK);
            if alt && !ctrl && matches!(name.as_deref(), Some("Up") | Some("Down")) {
                let next = name.as_deref() == Some("Up");
                page.cycle_line_format(next);
                return glib::Propagation::Stop;
            }

            if name.as_deref() == Some("Tab") && !ctrl && page.complete_variable() {
                return glib::Propagation::Stop;
            }

            // Enter continues a list only on the editor's terms: a
            // plain Return (no modifiers, no selection) on a line the
            // engine classifies as Text. Math lines, headings, and
            // comments fall through to the default newline so the
            // sheet keeps evaluating.
            let plain_return = (name.as_deref() == Some("Return")
                || name.as_deref() == Some("KP_Enter"))
                && !ctrl
                && !state.contains(gtk4::gdk::ModifierType::SHIFT_MASK)
                && !state.contains(gtk4::gdk::ModifierType::ALT_MASK);
            if plain_return && !page.buffer.has_selection() && page.continue_list() {
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });
        self.view.add_controller(controller);
    }

    /// Continue the list on the cursor's line: insert the next
    /// marker after a newline, or exit the list when the line is an
    /// empty item. Returns whether Enter was handled; the caller
    /// falls back to the default newline otherwise.
    fn continue_list(&self) -> bool {
        let ins = self.cursor_iter();
        let mut line_start = ins;
        line_start.set_line_offset(0);
        let mut line_end = line_start;
        line_end.forward_to_line_end();
        let line = self.buffer.text(&line_start, &line_end, true).to_string();

        // The engine's shape rule decides what counts as a list
        // line: only Text lines carry markers, and an unbound bare
        // name must not be mistaken for one.
        if quire_eval::Line::new(1, line.as_str()).kind != quire_eval::LineKind::Text {
            return false;
        }
        match crate::lists::list_enter(&line) {
            Some(crate::lists::ListEnter::Continue(prefix)) => {
                self.buffer.begin_user_action();
                let mut at = ins;
                self.buffer
                    .insert_interactive(&mut at, &format!("\n{prefix}"), true);
                self.buffer.end_user_action();
                true
            }
            Some(crate::lists::ListEnter::Exit(start, end)) => {
                // The span is indent + marker + spaces, all ASCII, so
                // the string's byte offsets are iter char offsets.
                self.buffer.begin_user_action();
                let mut from = line_start;
                from.forward_chars(start as i32);
                let mut to = line_start;
                to.forward_chars(end as i32);
                self.buffer.delete_interactive(&mut from, &mut to, true);
                let mut at = self.cursor_iter();
                self.buffer.insert_interactive(&mut at, "\n", true);
                self.buffer.end_user_action();
                true
            }
            None => false,
        }
    }

    /// Rewrite `&N` reference tokens when lines shift: `from` is the
    // WIRING PENDING: insert_text/delete_range call this (see the
    // 0.4.0 todo); until then clippy's dead-code is expected.
    #[allow(dead_code)]
    /// Rewrite `&N` reference tokens when lines shift: `from` is the
    /// one-based line the shift starts at (the first line that
    /// moved), `delta` is how many lines it moved by. Pure text
    /// surgery on the in-memory sheet; disk only ever sees what the
    /// user typed or approved.
    fn renumber_refs(text: &str, from: usize, delta: i32) -> String {
        // the shift applies to the REFERENCED line: a ref moves when
        // its target sits at or after the edit point. Deletions clamp
        // refs whose target was removed to the successor position, so
        // they keep pointing at the line that took its place.
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(rel) = rest.find('&') {
            let before = &rest[..rel + 1];
            out.push_str(before);
            let after = &rest[rel + 1..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                rest = after;
                continue;
            }
            let n: i64 = digits.parse().unwrap_or(0);
            let moved = if n >= from as i64 {
                let moved = n + delta as i64;
                if delta < 0 {
                    moved.max(from as i64)
                } else {
                    moved.max(1)
                }
            } else {
                n
            };
            out.push_str(&moved.to_string());
            rest = &after[digits.len()..];
        }
        out.push_str(rest);
        out
    }

    /// Re-evaluate the whole sheet on change, coalesced to the next
    /// idle turn (the spec's debounce; whole-sheet evaluation is O(n)
    /// on tiny sheets, so one deferred pass per burst is plenty).
    /// Before evaluating, &N refs are renumbered if the diff against
    /// the baseline shows a pure line shift - the ONE place this
    /// happens, so signal cascades are impossible.
    fn wire_evaluation(page: &Rc<Self>, buffer: &sourceview5::Buffer) {
        let pending = Rc::new(Cell::new(false));
        buffer.connect_changed(glib::clone!(
            #[weak]
            page,
            #[strong]
            pending,
            move |_| {
                if pending.replace(true) {
                    return;
                }
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    page,
                    #[strong]
                    pending,
                    move || {
                        pending.set(false);
                        let text = page.buffer.text(
                            &page.buffer.start_iter(),
                            &page.buffer.end_iter(),
                            true,
                        );
                        let text = text.to_string();

                        // &N renumbering: diff the current text against
                        // the baseline. A pure line shift rewrites the
                        // refs and triggers a second pass to evaluate.
                        let baseline = page.baseline.borrow().clone();
                        if let Some((from, delta)) = line_shift(&baseline, &text)
                            && !collect_refs(&text).is_empty()
                        {
                            let updated = QuirePage::renumber_refs(&text, from, delta);
                            if updated != text {
                                page.baseline.replace(updated.clone());
                                page.buffer.set_text(&updated);
                                return; // the re-triggered pass evaluates
                            }
                        }
                        page.baseline.replace(text.clone());

                        let cells = answers::compute_with_formats(
                            &text,
                            &page.formats.borrow(),
                            crate::settings::answer_decimals(),
                        );
                        page.answers.replace(cells.clone());
                        page.apply_error_highlights(&cells);
                        page.renderer.set_answers(cells);
                        if let Some(f) = page.on_reindex.borrow().as_ref() {
                            f(&index_sheet(&text));
                        }
                    }
                ));
            }
        ));
    }

    /// Hovering a line shows that line's cell FULL text in a
    /// tooltip: the column clips long errors (numbat's messages) and
    /// long answers, and a clipped error is a useless error. The
    /// whole line is the hover target - sheet text included - so the
    /// tooltip never depends on landing inside the narrow column.
    fn wire_answer_tooltips(page: &Rc<Self>) {
        let page = page.clone();
        let view = page.view.clone();
        view.set_has_tooltip(true);
        view.connect_query_tooltip(move |view, x, y, _keyboard, tooltip| {
            let (_, buffer_y) = view.window_to_buffer_coords(gtk4::TextWindowType::Widget, x, y);
            let (line_iter, _) = view.line_at_y(buffer_y);
            let Some(cell) = page.answer_for_line(line_iter.line() as u32 + 1) else {
                return false;
            };
            tooltip.set_text(Some(&cell.text));
            true
        });
    }

    /// Register the callback fired with a fresh sheet index after
    /// each evaluation pass.
    pub fn set_on_reindex(&self, f: impl Fn(&SheetIndex) + 'static) {
        *self.on_reindex.borrow_mut() = Some(Box::new(f));
    }

    /// The current structural index of the sheet.
    pub fn index(&self) -> SheetIndex {
        let text = self
            .buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true);
        index_sheet(&text)
    }

    /// The stored answer cell for a one-based sheet line.
    pub fn answer_for_line(&self, line: u32) -> Option<answers::AnswerCell> {
        self.answers.borrow().get(&line).cloned()
    }

    /// Copy `text` to the system clipboard.
    pub fn copy_to_clipboard(&self, text: &str) {
        self.view.display().clipboard().set_text(text);
    }

    /// The insert-mark position as an iterator.
    fn cursor_iter(&self) -> gtk4::TextIter {
        let offset = self.buffer.cursor_position();
        let mut iter = self.buffer.start_iter();
        iter.set_offset(offset);
        iter
    }

    /// Place the cursor at a one-based line and scroll it into view.
    pub fn goto_line(&self, line: usize) {
        let mut iter = self.buffer.start_iter();
        iter.set_line(line.saturating_sub(1) as i32);
        self.buffer.place_cursor(&iter);
        self.view.scroll_to_iter(&mut iter, 0.0, false, 0.0, 0.0);
    }

    /// The identifier under (or ending at) the cursor, if any.
    pub fn word_at_cursor(&self) -> Option<String> {
        let (start, end) = {
            let ins = self.cursor_iter();
            let mut s = ins;
            let mut e = ins;
            while s.char().is_ascii_alphanumeric() || s.char() == '_' {
                s.backward_char();
            }
            while e.char().is_ascii_alphanumeric() || e.char() == '_' {
                e.forward_char();
            }
            (s, e)
        };
        let word = self.buffer.text(&start, &end, true).to_string();
        if word.is_empty() { None } else { Some(word) }
    }

    /// Complete the word before the cursor from variable names bound
    /// above it, NoteCalc's rule: act only on a unique match. Returns
    /// whether a completion was inserted.
    pub fn complete_variable(&self) -> bool {
        let ins = self.cursor_iter();
        let mut line_start = ins;
        line_start.set_line_offset(0);
        let prefix = self.buffer.text(&line_start, &ins, true).to_string();
        let prefix = prefix.trim_end().to_string();
        if prefix.is_empty()
            || !prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return false;
        }
        let cursor_line = ins.line() as usize + 1;
        let names: Vec<String> = self
            .index()
            .assignments
            .into_iter()
            .filter(|(line, _)| *line < cursor_line)
            .map(|(_, name)| name)
            .filter(|n| n.starts_with(&prefix) && *n != prefix)
            .collect();
        if names.len() != 1 {
            return false;
        }
        let completion = &names[0][prefix.len()..];
        let mut at = ins;
        self.buffer.insert_interactive(&mut at, completion, true);
        true
    }

    /// Cycle the cursor line's answer format (Alt+Up/Down). This is
    /// view state only: the sheet stays clean, and the choice rides
    /// the line number until stable line ids land.
    fn cycle_line_format(&self, next: bool) {
        let line = self.cursor_iter().line() as u32 + 1;
        let format = {
            let mut formats = self.formats.borrow_mut();
            let current = formats
                .get(&line)
                .copied()
                .unwrap_or(answers::LineFormat::Standard);
            let next_format = if next {
                current.next()
            } else {
                current.previous()
            };
            formats.insert(line, next_format);
            next_format
        };
        let _ = format;
        self.refresh_answers();
    }

    /// Recompute the answers map (formats included) and repaint.
    pub(crate) fn refresh_answers(&self) {
        let text = self
            .buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true);
        let cells = answers::compute_with_formats(
            &text,
            &self.formats.borrow(),
            crate::settings::answer_decimals(),
        );
        self.answers.replace(cells.clone());
        self.apply_error_highlights(&cells);
        self.renderer.set_answers(cells);
        self.renderer.queue_draw();
    }

    /// The token-level error underline: each failed line's error span
    /// (the exact token that failed) gets a red underline in the
    /// sheet. Previous highlights are cleared every pass - the map is
    /// rebuilt wholesale, so the tags follow.
    fn apply_error_highlights(&self, cells: &std::collections::HashMap<u32, answers::AnswerCell>) {
        let tag_table = self.buffer.tag_table();
        if tag_table.lookup("quire-error-token").is_none() {
            let tag = gtk4::TextTag::new(Some("quire-error-token"));
            tag.set_underline(gtk4::pango::Underline::Single);
            tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.769, 0.455, 0.431, 1.0)));
            tag_table.add(&tag);
        }
        let start = self.buffer.start_iter();
        let end = self.buffer.end_iter();
        self.buffer
            .remove_tag_by_name("quire-error-token", &start, &end);

        for (line_number, cell) in cells {
            let Some((char_start, char_end)) = cell.error_span else {
                continue;
            };
            let mut start = self.buffer.start_iter();
            start.set_line(*line_number as i32 - 1);
            start.forward_chars(char_start as i32);
            let mut end = self.buffer.start_iter();
            end.set_line(*line_number as i32 - 1);
            end.forward_chars(char_end as i32);
            self.buffer
                .apply_tag_by_name("quire-error-token", &start, &end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WELCOME_SHEET;

    /// The tour must evaluate clean: an error cell on the starter
    /// page is a broken first impression, and a silent regression in
    /// the engine would land exactly here.
    #[test]
    fn renumber_refs_shifts_refs_after_the_edit_point() {
        use super::QuirePage;
        let sheet = "5 + 5\n&1 * 3\n&2 - 1\n";
        // an insert at line 1 shifts every ref down one
        assert_eq!(
            QuirePage::renumber_refs(sheet, 1, 1),
            "5 + 5\n&2 * 3\n&3 - 1\n"
        );
        // deleting line 1: &1 targeted it, so it clamps to the
        // successor (&1); &2 moves up to &1
        assert_eq!(
            QuirePage::renumber_refs(sheet, 1, -1),
            "5 + 5\n&1 * 3\n&1 - 1\n"
        );
        // deleting line 2: &1 is above the edit, untouched; &2
        // targeted the deleted line, so it clamps to the successor
        assert_eq!(
            QuirePage::renumber_refs(sheet, 2, -1),
            "5 + 5\n&1 * 3\n&2 - 1\n"
        );
        // refs at or before the edit point are untouched
        let sheet = "5 + 5\n&1 * 3\n";
        assert_eq!(QuirePage::renumber_refs(sheet, 1, 0), sheet);
    }

    #[test]
    fn welcome_sheet_has_no_error_cells() {
        for (line, cell) in crate::answers::compute(WELCOME_SHEET) {
            assert!(!cell.is_error, "line {line}: {}", cell.text);
        }
    }

    #[test]
    fn welcome_sheet_carries_the_tour() {
        let cells = crate::answers::compute(WELCOME_SHEET);
        let texts: Vec<&str> = cells.values().map(|c| c.text.as_str()).collect();
        for expected in [
            "230", "216", "12", "24", "42", "36", "1,420", "1,270", "150", "5300 g", "150 min",
            "3,540",
        ] {
            assert!(
                texts.contains(&expected),
                "the tour lost its {expected} line: {texts:?}"
            );
        }
    }
}

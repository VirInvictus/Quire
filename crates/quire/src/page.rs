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

use crate::answers;
use crate::renderer::AnswersRenderer;
use crate::{styles, view::QuireView};

/// First-run content: the shortest sheet that shows what Quire is.
const WELCOME_SHEET: &str = "\
# Welcome to Quire

// notes and math share the sheet
groceries = 42.50
takeout = 23.75
total

// edit a line and the answers follow; or try:
200 + 15%
";

pub struct QuirePage {
    pub view: QuireView,
    buffer: sourceview5::Buffer,
    renderer: AnswersRenderer,
    file: RefCell<Option<gio::File>>,
    monitor: RefCell<Option<gio::FileMonitor>>,
    /// guard so the file monitor ignores the app's own writes
    loading: Cell<bool>,
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
        });
        Self::wire_evaluation(&buffer, &page.renderer);
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

    /// Re-evaluate the whole sheet on change, coalesced to the next
    /// idle turn (the spec's debounce; whole-sheet evaluation is O(n)
    /// on tiny sheets, so one deferred pass per burst is plenty).
    fn wire_evaluation(buffer: &sourceview5::Buffer, renderer: &AnswersRenderer) {
        let pending = Rc::new(Cell::new(false));
        buffer.connect_changed(glib::clone!(
            #[weak]
            buffer,
            #[weak]
            renderer,
            #[strong]
            pending,
            move |_| {
                if pending.replace(true) {
                    return;
                }
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    buffer,
                    #[weak]
                    renderer,
                    #[strong]
                    pending,
                    move || {
                        pending.set(false);
                        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
                        renderer.set_answers(answers::compute(&text));
                    }
                ));
            }
        ));
    }
}

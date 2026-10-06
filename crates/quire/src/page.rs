//! The editor page: one buffer, one sheet view, one answers column.

use std::cell::Cell;
use std::rc::Rc;

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
    renderer: AnswersRenderer,
}

impl QuirePage {
    pub fn new() -> Self {
        let buffer = sourceview5::Buffer::new(None::<&gtk4::TextTagTable>);
        let lang = styles::language();
        buffer.set_language(lang.as_ref());
        buffer.set_highlight_syntax(lang.is_some());
        if let Some(scheme) = styles::scheme(vir_gtk::portal::is_dark()) {
            buffer.set_style_scheme(Some(&scheme));
        }

        let view = QuireView::with_buffer(&buffer);
        view.set_wrap_mode(gtk4::WrapMode::Word);
        view.set_monospace(true);
        // breathing room; the answers column lives at the right edge
        view.set_top_margin(16);
        view.set_bottom_margin(16);
        view.set_left_margin(18);
        view.set_right_margin(6);
        view.add_css_class("quire-editor");

        let renderer = AnswersRenderer::with_buffer(&buffer);
        let gutter = sourceview5::prelude::ViewExt::gutter(&view, gtk4::TextWindowType::Right);
        gutter.insert(&renderer, 0);

        let page = Self {
            view: view.clone(),
            renderer,
        };
        Self::wire_evaluation(&buffer, &page.renderer);
        // one delayed repaint: on a very first run the fonts were
        // installed moments ago and the earliest frames can resolve
        // text against a cold font cache; repainting once the map is
        // warm makes the column correct without a restart
        let repaint = page.renderer.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            repaint.queue_draw()
        });
        buffer.set_text(WELCOME_SHEET);
        page
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

impl Default for QuirePage {
    fn default() -> Self {
        Self::new()
    }
}

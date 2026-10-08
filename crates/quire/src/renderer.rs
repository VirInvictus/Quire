//! The answers column: a custom GutterRenderer living in the view's
//! right gutter, painting one answer cell per expression line,
//! scroll-synced for free by the gutter machinery.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{Snapshot, gdk, graphene, pango};
use sourceview5::prelude::*;
use sourceview5::subclass::gutter_renderer::GutterRendererImpl;
use sourceview5::{GutterLines, GutterRenderer};

use crate::answers::AnswerCell;

/// Column width bounds in pixels. The column breathes to fit its
/// widest visible answer - dates are 19 characters and outgrew the
/// old fixed 168 - between this floor (room for a typical computed
/// value) and this ceiling (past it, answers ellipsize from the
/// left and the hover tooltip carries the full text).
pub const MIN_COLUMN_WIDTH: i32 = 168;
pub const MAX_COLUMN_WIDTH: i32 = 320;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AnswersRenderer {
        pub answers: RefCell<HashMap<u32, AnswerCell>>,
        pub buffer: RefCell<Option<sourceview5::Buffer>>,
        /// The current column width: fits the widest visible answer,
        /// clamped to the bounds above. Recomputed on set_answers.
        pub width: std::cell::Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AnswersRenderer {
        const NAME: &'static str = "QuireAnswersRenderer";
        type Type = super::AnswersRenderer;
        type ParentType = GutterRenderer;
    }

    impl ObjectImpl for AnswersRenderer {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            // CELL keeps each cell pinned to the first visual row of a
            // wrapped line; xalign 1.0 right-aligns the number.
            obj.set_alignment_mode(sourceview5::GutterRendererAlignmentMode::Cell);
            obj.set_xalign(1.0);
            obj.set_xpad(12);
            obj.add_css_class("quire-answers");
            obj.set_visible(false);
            self.width.set(MIN_COLUMN_WIDTH);
        }
    }

    impl WidgetImpl for AnswersRenderer {
        fn measure(&self, orientation: gtk4::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk4::Orientation::Horizontal => {
                    let w = self.width.get();
                    (w, w, -1, -1)
                }
                _ => self.parent_measure(orientation, for_size),
            }
        }

        /// Column chrome under everything: the widget's box is the
        /// full column height, so the tone and hairline run
        /// continuously, below the last line included. Painted BEFORE
        /// the base snapshot, whose line loop draws the text on top.
        fn snapshot(&self, snapshot: &Snapshot) {
            let obj = self.obj();
            let palette = crate::active_palette();
            let rgba = |hex: &str| gdk::RGBA::parse(hex).unwrap_or(gdk::RGBA::WHITE);
            let w = obj.width() as f32;
            let h = obj.height() as f32;
            let scale = obj.scale_factor() as f32;

            snapshot.append_color(&rgba(palette.bg_view), &graphene::Rect::new(0.0, 0.0, w, h));
            snapshot.append_color(
                &rgba(palette.grid),
                &graphene::Rect::new(0.0, 0.0, scale, h),
            );

            self.parent_snapshot(snapshot);
        }
    }

    impl GutterRendererImpl for AnswersRenderer {
        fn snapshot_line(&self, snapshot: &Snapshot, _lines: &GutterLines, line: u32) {
            // the gutter counts buffer lines from zero; the engine's
            // sheet line numbers count from one
            let cell = self.answers.borrow().get(&(line + 1)).cloned();
            let Some(cell) = cell else { return };

            let obj = self.obj();
            let palette = crate::active_palette();

            // A fresh layout per cell per frame. At notepad scale this
            // is nothing, and it can never hold a font that resolved
            // before the user's font caches were warm (the one bug a
            // cache bought us on cold first runs).
            let layout = obj.create_pango_layout(Some(&cell.text));
            layout.set_alignment(pango::Alignment::Right);
            layout.set_ellipsize(pango::EllipsizeMode::Start);
            let inner = self.width.get() - 2 * obj.xpad();
            // the layout spans the inner column, right-aligned inside
            // it, so its logical width IS the cell width handed to
            // align_cell: the glyphs end flush at the xpad edge and
            // never leave the renderer's clip
            layout.set_width(inner * pango::SCALE);
            let (_lw, lh) = layout.pixel_size();
            let (x, y) = obj.align_cell(line, inner as f32, lh as f32);
            // pixel-snap: fractional baselines raster one pixel up
            // into the line's clip edge and shave glyph tops
            let (x, y) = (x.round(), y.round());

            let style_name = if cell.is_error {
                "quire:error"
            } else {
                "quire:result"
            };
            let fallback = if cell.is_error {
                palette.err
            } else {
                palette.heading
            };
            let color = self
                .buffer
                .borrow()
                .as_ref()
                .and_then(|b| b.style_scheme())
                .and_then(|scheme| scheme.style(style_name))
                .and_then(|st| st.foreground())
                .and_then(|hex| gdk::RGBA::parse(&hex).ok())
                .unwrap_or_else(|| gdk::RGBA::parse(fallback).unwrap_or(gdk::RGBA::WHITE));

            snapshot.save();
            snapshot.translate(&graphene::Point::new(x, y));
            snapshot.append_layout(&layout, &color);
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct AnswersRenderer(ObjectSubclass<imp::AnswersRenderer>)
        @extends GutterRenderer, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for AnswersRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl AnswersRenderer {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// The sheet whose style scheme colors the cells.
    pub fn with_buffer(buffer: &sourceview5::Buffer) -> Self {
        let renderer = Self::new();
        renderer.imp().buffer.replace(Some(buffer.clone()));
        renderer
    }

    pub fn set_answers(&self, answers: HashMap<u32, AnswerCell>) {
        {
            let mut map = self.imp().answers.borrow_mut();
            *map = answers;
        }
        self.refit_column();
        self.set_visible(!self.imp().answers.borrow().is_empty());
        self.queue_draw();
    }

    /// Size the column to its widest visible answer, clamped. Measured
    /// with the same widget-font layout the cells draw with, so the
    /// fit is exact at any font; once per burst, not per frame.
    fn refit_column(&self) {
        let widest = self
            .imp()
            .answers
            .borrow()
            .values()
            .map(|cell| {
                let layout = self.create_pango_layout(Some(&cell.text));
                let (w, _) = layout.pixel_size();
                w
            })
            .max()
            .unwrap_or(0);
        let inner = (widest + 1).max(0); // one glyph of breathing room
        let xpad = self.xpad();
        let width = (inner + 2 * xpad).clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        if self.imp().width.get() != width {
            self.imp().width.set(width);
            // the gutter re-measures on resize requests
            self.queue_resize();
        }
    }
}

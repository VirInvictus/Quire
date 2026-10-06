//! The sheet view subclass. Carries the below-text paint layer that
//! the Phase 3 cursor-line expression highlight will use; column tone
//! and hairline live in the renderer's own snapshot, which proved to
//! be the only paint surface that reliably reaches the screen (the
//! Gutter widget ignores CSS backgrounds, and the view's below-text
//! layer is clipped to the text window, under the gutter child).

use gtk4::glib;
use gtk4::subclass::prelude::*;
use sourceview5::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct QuireView;

    #[glib::object_subclass]
    impl ObjectSubclass for QuireView {
        const NAME: &'static str = "QuireView";
        type Type = super::QuireView;
        type ParentType = sourceview5::View;
    }

    impl ObjectImpl for QuireView {}

    impl WidgetImpl for QuireView {}

    impl gtk4::subclass::text_view::TextViewImpl for QuireView {}

    impl ViewImpl for QuireView {}
}

glib::wrapper! {
    pub struct QuireView(ObjectSubclass<imp::QuireView>)
        @extends sourceview5::View, gtk4::TextView, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
            gtk4::Scrollable;
}

impl QuireView {
    pub fn with_buffer(buffer: &sourceview5::Buffer) -> Self {
        glib::Object::builder().property("buffer", buffer).build()
    }
}

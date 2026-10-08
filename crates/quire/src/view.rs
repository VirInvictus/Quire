//! The sheet view subclass. The cursor-line highlight lives in
//! page.rs as a full-height background TextTag - the native GTK
//! mechanism, which needs no custom paint layer (the sourceview
//! subclass trait in 0.11 exposes no below-text snapshot hook, and a
//! WidgetImpl::snapshot override cannot paint between the view's
//! background and its text).

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

//! The main window: header chrome over the editor page.

use gtk4::prelude::*;

use crate::page::QuirePage;

pub fn new(app: &gtk4::Application) -> gtk4::ApplicationWindow {
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Quire")
        .default_width(940)
        .default_height(680)
        .build();

    let header = gtk4::HeaderBar::new();
    header.set_show_title_buttons(true);
    let title = gtk4::Label::new(Some("Quire"));
    title.add_css_class("title");
    header.set_title_widget(Some(&title));
    window.set_titlebar(Some(&header));

    let page = QuirePage::new();
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_child(Some(&page.view));
    scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    // the answers column belongs to the view, so the scrollbar rides
    // outside it; the sheet fills the window edge to edge
    window.set_child(Some(&scrolled));

    window
}

//! Quire: a Soulver-style notepad calculator for Linux.
//!
//! Plain GTK4, Kanagawa Dragon through vir-gtk, engine in quire-eval.

mod answers;
mod fonts;
mod lists;
mod page;
mod renderer;
mod settings;
mod styles;
mod templates;
mod view;
mod window;

use gtk4::prelude::*;

/// App stylesheet, spliced with the active Kanagawa palette on every
/// dark/light flip. Tokens are vir-gtk's `%NAME%` replacements.
/// Column tone and hairline are painted by the answers renderer, not
/// here. No `line-height`: GTK clips TextView ascenders whenever the
/// property is set (verified at 150% and a 24px value); the cursor
/// color comes from the style scheme's `cursor` style.
const APP_CSS: &str = "\
textview.quire-editor {
  background-color: %BG_VIEW%;
  color: %FG%;
  font-family: \"JetBrains Mono\", monospace;
  font-size: 20px;
}

/* identical metrics on the answers column: its cells are measured
   against the view's line boxes, and a font mismatch clips glyph
   tops */
.quire-answers {
  font-family: \"JetBrains Mono\", monospace;
  font-size: 20px;
}";

fn resplice() {
    use vir_gtk::theme::{base_css, install_app_stylesheet, install_stylesheet};
    let palette = active_palette();
    install_stylesheet(&base_css(&palette));
    install_app_stylesheet(&palette.replace_tokens(APP_CSS));
}

pub fn active_palette() -> vir_gtk::theme::Palette {
    if vir_gtk::portal::is_dark() {
        vir_gtk::theme::Palette::dragon()
    } else {
        vir_gtk::theme::Palette::lotus()
    }
}

fn main() -> gtk4::glib::ExitCode {
    // fonts land before GTK builds its font map inside run(), so a
    // first run renders in JetBrains Mono without a restart
    fonts::ensure_installed();

    let app = gtk4::Application::builder()
        .application_id("io.github.virinvictus.Quire")
        .build();

    app.connect_activate(|app| {
        use vir_gtk::portal;
        portal::init(None, None, true);
        portal::connect_dark_changed(app, |_| resplice());
        resplice();

        // schema, language spec, and schemes must be in place before
        // the first window and buffer ask for them
        settings::install();
        styles::install();

        let win = window::QuireWindow::new(app);
        win.window().present();
    });

    app.run()
}

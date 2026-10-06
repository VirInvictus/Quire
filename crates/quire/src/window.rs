//! The main window: header chrome, document actions (new / open /
//! save / save-as), recents, the unsaved-changes guard, and the
//! line-numbers toggle.

use std::rc::Rc;

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::page::QuirePage;
use crate::settings;

#[derive(Clone)]
pub struct QuireWindow {
    app: gtk4::Application,
    window: gtk4::ApplicationWindow,
    title: gtk4::Label,
    menu_button: gtk4::MenuButton,
    outline_button: gtk4::MenuButton,
    page: Rc<QuirePage>,
    /// set when the user chose to discard unsaved changes, so the
    /// close-request handler lets the window die
    force_close: Rc<std::cell::Cell<bool>>,
}

impl QuireWindow {
    pub fn new(app: &gtk4::Application) -> Self {
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .title("Quire")
            .default_width(settings::get().int("window-width").max(480))
            .default_height(settings::get().int("window-height").max(320))
            .build();

        let title = gtk4::Label::new(Some("Untitled"));
        title.add_css_class("title");
        let header = gtk4::HeaderBar::new();
        header.set_title_widget(Some(&title));
        header.set_show_title_buttons(true);

        let menu_button = gtk4::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some("Menu"));
        header.pack_end(&menu_button);

        let outline_button = gtk4::MenuButton::new();
        outline_button.set_icon_name("view-list-symbolic");
        outline_button.set_tooltip_text(Some("Outline"));
        outline_button.set_visible(false);
        header.pack_start(&outline_button);

        window.set_titlebar(Some(&header));

        let page = QuirePage::new();
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&page.view));
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        window.set_child(Some(&scrolled));

        let win = Self {
            app: app.clone(),
            window: window.clone(),
            title,
            outline_button,
            menu_button,
            page: page.clone(),
            force_close: Rc::new(std::cell::Cell::new(false)),
        };

        // the sheet's number toggle lives in GSettings and drives the
        // view property directly
        settings::get()
            .bind("show-line-numbers", &page.view, "show-line-numbers")
            .build();

        win.add_actions();
        win.build_menu();
        win.wire_title_tracking();
        win.wire_close_guard();
        win.wire_drop_target();
        win.wire_outline();
        page.wire_editing_keys();

        window.present();
        win
    }

    /// Rebuild the outline menu whenever the sheet is re-indexed.
    fn wire_outline(&self) {
        let win = self.clone();
        self.page.set_on_reindex(move |index| {
            let menu = gio::Menu::new();
            for (line, title) in &index.headings {
                menu.append(
                    Some(&title.to_string()),
                    Some(&format!("win.goto-heading({line})")),
                );
            }
            win.outline_button.set_menu_model(Some(&menu));
            win.outline_button.set_visible(!index.headings.is_empty());
        });
    }

    /// Open a file dropped onto the window.
    fn wire_drop_target(&self) {
        let drop = gtk4::DropTarget::new(gio::File::static_type(), gtk4::gdk::DragAction::COPY);
        let win = self.clone();
        drop.connect_drop(move |_target, value, _x, _y| {
            let Ok(file) = value.get::<gio::File>() else {
                return false;
            };
            let Some(path) = file.path() else {
                return false;
            };
            win.open_path(path.to_string_lossy().to_string());
            true
        });
        self.window.add_controller(drop);
    }

    pub fn window(&self) -> &gtk4::ApplicationWindow {
        &self.window
    }

    fn add_actions(&self) {
        let win = self.clone();
        let new_action = gio::SimpleAction::new("new", None);
        new_action.connect_activate(move |_, _| win.action_new());
        self.window.add_action(&new_action);

        let win = self.clone();
        let open_action = gio::SimpleAction::new("open", None);
        open_action.connect_activate(move |_, _| win.action_open());
        self.window.add_action(&open_action);

        let win = self.clone();
        let save_action = gio::SimpleAction::new("save", None);
        save_action.connect_activate(move |_, _| win.action_save());
        self.window.add_action(&save_action);

        let win = self.clone();
        let save_as_action = gio::SimpleAction::new("save-as", None);
        save_as_action.connect_activate(move |_, _| win.action_save_as());
        self.window.add_action(&save_as_action);

        // stateful answer-decimals radio: the cap as its string state
        // (-1 = full); a change persists and repaints the sheet
        let win = self.clone();
        let decimals = settings::get().int("answer-decimals").to_string();
        let decimals_action = gio::SimpleAction::new_stateful(
            "answer-decimals",
            Some(glib::VariantTy::STRING),
            &decimals.to_variant(),
        );
        decimals_action.connect_activate(move |action, payload| {
            if let Some(v) = payload.and_then(|v| v.str()) {
                let parsed: i32 = v.parse().unwrap_or(-1);
                let _ = settings::get().set_int("answer-decimals", parsed);
                action.set_state(&v.to_variant());
                win.page.refresh_answers();
            }
        });
        self.window.add_action(&decimals_action);

        // parameterized new-from-template: the template id as payload
        let win = self.clone();
        let template_action = gio::SimpleAction::new("template", Some(glib::VariantTy::STRING));
        template_action.connect_activate(move |_, payload| {
            if let Some(id) = payload.and_then(|v| v.str()) {
                win.action_template(id);
            }
        });
        self.window.add_action(&template_action);

        // parameterized recent-open: the path as its string payload
        let win = self.clone();
        let recent_action = gio::SimpleAction::new("recent", Some(glib::VariantTy::STRING));
        recent_action.connect_activate(move |_, payload| {
            if let Some(path) = payload.and_then(|v| v.str()) {
                win.open_path(path.to_string());
            }
        });
        self.window.add_action(&recent_action);

        // stateful line-numbers toggle; the view itself is bound to
        // the GSettings key, so the action only flips the setting
        let initial = settings::get().boolean("show-line-numbers");
        let line_numbers =
            gio::SimpleAction::new_stateful("line-numbers", None, &initial.to_variant());
        {
            let settings = settings::get().clone();
            line_numbers.connect_activate(move |action, _| {
                let state = action
                    .state()
                    .and_then(|v| v.get::<bool>())
                    .unwrap_or(false);
                let state = !state;
                action.set_state(&state.to_variant());
                let _ = settings.set_boolean("show-line-numbers", state);
            });
        }
        self.window.add_action(&line_numbers);

        let win = self.clone();
        let goto_action = gio::SimpleAction::new("goto-heading", Some(&i32::static_variant_type()));
        goto_action.connect_activate(move |_, param| {
            if let Some(line) = param.and_then(|v| v.get::<i32>()) {
                win.page.goto_line(line.max(1) as usize);
            }
        });
        self.window.add_action(&goto_action);

        self.app.set_accels_for_action("win.new", &["<Primary>n"]);
        self.app.set_accels_for_action("win.open", &["<Primary>o"]);
        self.app.set_accels_for_action("win.save", &["<Primary>s"]);
        self.app
            .set_accels_for_action("win.save-as", &["<Primary><Shift>s"]);
        self.app
            .set_accels_for_action("win.line-numbers", &["<Primary>l"]);
    }

    fn build_menu(&self) {
        let menu = gio::Menu::new();

        let file_section = gio::Menu::new();
        file_section.append(Some("New"), Some("win.new"));
        file_section.append(Some("Open"), Some("win.open"));
        file_section.append(Some("Save"), Some("win.save"));
        file_section.append(Some("Save As…"), Some("win.save-as"));
        menu.append_section(None, &file_section);

        let view_section = gio::Menu::new();
        view_section.append(Some("Line numbers"), Some("win.line-numbers"));
        menu.append_section(None, &view_section);

        let template_section = gio::Menu::new();
        for (name, _) in crate::templates::TEMPLATES {
            let escaped = name.to_lowercase().replace('\'', "\\'");
            template_section.append(Some(name), Some(&format!("win.template('{escaped}')")));
        }
        menu.append_section(Some("New from template"), &template_section);

        let decimals_section = gio::Menu::new();
        for (label, id) in [
            ("Full", "-1"),
            ("0 decimals", "0"),
            ("1 decimal", "1"),
            ("2 decimals", "2"),
            ("3 decimals", "3"),
            ("4 decimals", "4"),
        ] {
            let item = gio::MenuItem::new(Some(label), None);
            let target = id.to_variant();
            item.set_action_and_target_value(Some("win.answer-decimals"), Some(&target));
            decimals_section.append_item(&item);
        }
        menu.append_section(Some("Answer decimals"), &decimals_section);

        let recent_section = gio::Menu::new();
        for path in settings::get().strv("recent-files") {
            let label = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            // detailed action strings are single-quoted; escape any
            // embedded quotes and backslashes in the path first
            let escaped = path.replace('\\', "\\\\").replace('\'', "\\'");
            recent_section.append(
                Some(&format!("Reopen {label}")),
                Some(&format!("win.recent('{escaped}')")),
            );
        }
        if recent_section.n_items() > 0 {
            menu.append_section(Some("Recent"), &recent_section);
        }

        self.menu_button.set_menu_model(Some(&menu));
    }

    fn wire_title_tracking(&self) {
        self.update_title();
        let win = self.clone();
        self.page
            .buffer()
            .connect_notify_local(Some("modified"), move |_, _| win.update_title());
    }

    fn update_title(&self) {
        let name = self.page.display_name();
        let marker = if self.page.is_modified() { "• " } else { "" };
        self.title.set_text(&format!("{marker}{name}"));
        self.window.set_title(Some(&format!("{name} — Quire")));
    }

    fn wire_close_guard(&self) {
        let win = self.clone();
        self.window.connect_close_request(move |_| {
            let size = win.window.default_size();
            let _ = settings::get().set_int("window-width", size.0);
            let _ = settings::get().set_int("window-height", size.1);

            if win.force_close.get() || !win.page.is_modified() {
                return glib::Propagation::Proceed;
            }
            win.confirm_discard(Box::new(glib::clone!(
                #[strong]
                win,
                move || {
                    win.force_close.set(true);
                    win.window.close();
                }
            )));
            glib::Propagation::Stop
        });
    }

    /// Present the unsaved-changes alert; `then` runs only when the
    /// user chooses Discard.
    fn confirm_discard(&self, then: Box<dyn FnOnce()>) {
        let alert = vir_gtk::widgets::Alert::new(
            Some("Discard unsaved changes?"),
            Some("The sheet has been modified since the last save."),
        );
        alert.add_response("cancel", "Cancel");
        alert.add_response("discard", "Discard");
        alert.set_response_appearance("discard", vir_gtk::widgets::Appearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        let then = std::cell::RefCell::new(Some(then));
        alert.connect_response(move |response| {
            if response == "discard"
                && let Some(then) = then.borrow_mut().take()
            {
                then();
            }
        });
        alert.present(Some(&self.window));
    }

    /// Run `then` immediately when the sheet is clean, otherwise ask
    /// first.
    fn with_discard_check(&self, then: impl FnOnce() + 'static) {
        if self.page.is_modified() {
            self.confirm_discard(Box::new(then));
        } else {
            then();
        }
    }

    /// New from a shipped template: load its content untitled (a
    /// save goes through save-as, like a fresh sheet).
    fn action_template(&self, id: &str) {
        let page = self.page.clone();
        let id = id.to_string();
        self.with_discard_check(move || {
            if let Some((_, sheet)) = crate::templates::TEMPLATES
                .iter()
                .find(|(name, _)| name.to_lowercase() == id)
            {
                page.load(sheet, None);
            }
        });
    }

    fn action_new(&self) {
        let page = self.page.clone();
        self.with_discard_check(move || {
            page.reset_to_new();
        });
    }

    /// Read the `ms` (maybe-string) last-folder key. The string
    /// getter panics on nothing-variants, so this must go through the
    /// typed variant API.
    fn last_folder(&self) -> String {
        settings::get()
            .value("last-folder")
            .get::<Option<String>>()
            .flatten()
            .unwrap_or_default()
    }

    fn action_open(&self) {
        let folder = self.last_folder();
        let page = self.page.clone();
        let win = self.clone();
        self.with_discard_check(move || {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Open Sheet");
            if !folder.is_empty() {
                let initial = gio::File::for_path(folder);
                dialog.set_initial_folder(Some(&initial));
            }
            dialog.open(
                Some(&win.window),
                gio::Cancellable::NONE,
                glib::clone!(
                    #[strong]
                    page,
                    #[strong]
                    win,
                    move |result| {
                        if let Ok(file) = result
                            && page.open(&file).is_ok()
                        {
                            if let Some(path) = file.path() {
                                settings::push_recent(&path.to_string_lossy());
                                let _ = settings::get().set_string(
                                    "last-folder",
                                    &path
                                        .parent()
                                        .map(|p| p.to_string_lossy())
                                        .unwrap_or_default(),
                                );
                            }
                            win.update_title();
                            win.build_menu();
                        }
                    }
                ),
            );
        });
    }

    fn open_path(&self, path: String) {
        let page = self.page.clone();
        let win = self.clone();
        self.with_discard_check(move || {
            let file = gio::File::for_path(&path);
            if page.open(&file).is_ok() {
                settings::push_recent(&path);
                win.update_title();
                win.build_menu();
            }
        });
    }

    fn action_save(&self) {
        match self.page.file() {
            Some(file) => {
                if let Err(message) = self.page.save_to(&file) {
                    self.title.set_text(&format!("Save failed: {message}"));
                } else {
                    self.update_title();
                }
            }
            None => self.action_save_as(),
        }
    }

    fn action_save_as(&self) {
        let folder = self.last_folder();
        let page = self.page.clone();
        let win = self.clone();
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Save Sheet As");
        dialog.set_initial_name(Some(&format!("{}.quire", page.display_name())));
        if !folder.is_empty() {
            let initial = gio::File::for_path(folder);
            dialog.set_initial_folder(Some(&initial));
        }
        dialog.save(
            Some(&self.window),
            gio::Cancellable::NONE,
            glib::clone!(
                #[strong]
                page,
                #[strong]
                win,
                move |result| {
                    if let Ok(file) = result
                        && page.save_to(&file).is_ok()
                    {
                        if let Some(path) = file.path() {
                            settings::push_recent(&path.to_string_lossy());
                            let _ = settings::get().set_string(
                                "last-folder",
                                &path
                                    .parent()
                                    .map(|p| p.to_string_lossy())
                                    .unwrap_or_default(),
                            );
                        }
                        win.update_title();
                        win.build_menu();
                    }
                }
            ),
        );
    }
}

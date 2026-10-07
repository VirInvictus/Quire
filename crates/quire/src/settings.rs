//! Application settings: a GSettings schema extracted and compiled at
//! startup into the user data dir (the same pattern as the fonts and
//! the styles - nothing is installed system-wide), then served as a
//! process-wide `gio::Settings`.

use std::path::PathBuf;

use gtk4::gio;
use gtk4::prelude::*;

const SCHEMA_XML: &[u8] = include_bytes!("../resources/io.github.virinvictus.Quire.gschema.xml");

const SCHEMA_ID: &str = "io.github.virinvictus.Quire";

/// Extract + compile the schema and make it discoverable. Must run
/// after GTK initialisation and before the first [`get`]. Safe to
/// have failed extraction: a missing compiled schema only means
/// [`get`] panics on first use, which cannot happen before a
/// successful install.
pub fn install() {
    let Some(schemas_dir) = schemas_dir() else {
        return;
    };
    if std::fs::create_dir_all(&schemas_dir).is_err() {
        return;
    }
    let xml_path = schemas_dir.join("io.github.virinvictus.Quire.gschema.xml");
    if std::fs::write(&xml_path, SCHEMA_XML).is_err() {
        return;
    }
    // only needed when the schema changed; recompiling is cheap
    let _ = std::process::Command::new("glib-compile-schemas")
        .arg(&schemas_dir)
        .output();
}

/// A fresh proxy over the compiled schema. `gio::Settings` is a thin
/// backend-backed object; creating one per call site is what the gio
/// docs intend and costs nothing.
pub fn get() -> gio::Settings {
    gio::Settings::new(SCHEMA_ID)
}

/// The answer-decimals cap: -1 means the full 12-significant
/// rendering; 0..=12 caps the decimal places.
pub fn answer_decimals() -> Option<u32> {
    match get().int("answer-decimals") {
        d if d < 0 => None,
        d => Some(d as u32),
    }
}

/// The currency refresh interval in hours (the ECB publishes daily;
/// 24 is the default).
pub fn currency_refresh_hours() -> i32 {
    get().int("currency-refresh-hours")
}

/// Prepends `path` to the recent-files list, deduplicating and
/// capping at eight entries.
pub fn push_recent(path: &str) {
    let mut recents: Vec<String> = get()
        .strv("recent-files")
        .iter()
        .map(|s| s.to_string())
        .collect();
    recents.retain(|p| p != path);
    recents.insert(0, path.to_string());
    recents.truncate(8);
    let _ = get().set_strv("recent-files", recents);
}

fn schemas_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("glib-2.0/schemas"))
}

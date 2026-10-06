//! Bundled language spec + style schemes. Same pattern as the fonts:
//! the files ship in `resources/styles/` (the .lang is a fork of
//! GtkSourceView's LGPL markdown.lang) and are extracted at startup
//! to the user data dir, where the GtkSourceView managers can load
//! them. gresource compilation waits for the Phase 5 build-system
//! decision.

use std::path::PathBuf;

const LANG: &[u8] = include_bytes!("../resources/styles/quire.lang");
const SCHEME_DARK: &[u8] = include_bytes!("../resources/styles/quire-dark.xml");
const SCHEME_LIGHT: &[u8] = include_bytes!("../resources/styles/quire-light.xml");

/// Write the bundled files out and point both managers at them. Must
/// run after GTK is initialised and before the first buffer asks for
/// a language: the default managers snapshot their search paths on
/// first use.
pub fn install() {
    let Some(base) = data_dir() else { return };
    write_out(&base.join("lang"), "quire.lang", LANG);
    write_out(&base.join("styles"), "quire-dark.xml", SCHEME_DARK);
    write_out(&base.join("styles"), "quire-light.xml", SCHEME_LIGHT);

    let lm = sourceview5::LanguageManager::default();
    let mut paths: Vec<String> = lm.search_path().iter().map(|p| p.to_string()).collect();
    if let Some(dir) = base.join("lang").to_str() {
        paths.push(dir.to_string());
    }
    let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    lm.set_search_path(&refs);

    let ssm = sourceview5::StyleSchemeManager::default();
    if let Some(dir) = base.join("styles").to_str() {
        ssm.prepend_search_path(dir);
    }
}

/// The `quire` language, if it loaded.
pub fn language() -> Option<sourceview5::Language> {
    sourceview5::LanguageManager::default().language("quire")
}

/// The sheet scheme for the active palette.
pub fn scheme(dark: bool) -> Option<sourceview5::StyleScheme> {
    let id = if dark { "quire-dark" } else { "quire-light" };
    sourceview5::StyleSchemeManager::default().scheme(id)
}

fn write_out(dir: &std::path::Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    let stale = std::fs::read(&path)
        .map(|old| old.len() != bytes.len())
        .unwrap_or(true);
    if stale && std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(&path, bytes);
    }
}

fn data_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("quire"))
}

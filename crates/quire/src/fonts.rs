//! Bundled JetBrains Mono (SIL OFL 1.1, files live in
//! `resources/fonts/` with the license text). Nothing may assume an
//! installed font: the app carries its own face and installs it under
//! the user fonts directory before GTK initializes its font map, so
//! even a first run renders in it.

use std::path::PathBuf;

const FONTS: &[(&str, &[u8])] = &[
    (
        "JetBrainsMono-Regular.ttf",
        include_bytes!("../resources/fonts/JetBrainsMono-Regular.ttf"),
    ),
    (
        "JetBrainsMono-Medium.ttf",
        include_bytes!("../resources/fonts/JetBrainsMono-Medium.ttf"),
    ),
    (
        "JetBrainsMono-Bold.ttf",
        include_bytes!("../resources/fonts/JetBrainsMono-Bold.ttf"),
    ),
];

/// Idempotent: writes a file only when missing or size-changed, and
/// refreshes the fontconfig cache only when something landed.
pub fn ensure_installed() {
    let Some(dir) = font_dir() else { return };
    let mut landed = false;
    for (name, bytes) in FONTS {
        let path = dir.join(name);
        let stale = std::fs::read(&path)
            .map(|old| old.len() != bytes.len())
            .unwrap_or(true);
        if stale && std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&path, bytes).is_ok() {
            landed = true;
        }
    }
    if landed {
        let _ = std::process::Command::new("fc-cache")
            .arg("-f")
            .arg(&dir)
            .output();
    }
}

fn font_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("fonts/Quire"))
}

//! The ECB reference-rate cache: fetch once, cache under
//! `~/.cache/quire/ecb.xml`, seed the unit engine from the cache at
//! startup. Stale rates fully work offline (spec.md "Currency");
//! the fetch runs on a background thread and never blocks a sheet.

use std::path::PathBuf;
use std::time::Duration;

const ECB_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// Seed from the cache if present, then refresh in the background
/// when the cache is older than the GSettings interval. Idempotent
/// per process: the engine's rates are set-once (spec.md
/// "Currency"), so a refresh applies from the next launch.
pub fn ensure() {
    match cached_xml() {
        Some(xml) => {
            quire_eval::set_exchange_rates(&xml);
            if stale() {
                spawn_fetch();
            }
        }
        None => spawn_fetch(),
    }
}

/// Force a background fetch (the menu's refresh item).
pub fn spawn_fetch() {
    std::thread::spawn(|| {
        if let Some(xml) = fetch() {
            let path = cache_path();
            if let Some(parent) = path.parent()
                && std::fs::create_dir_all(parent).is_ok()
                && std::fs::write(&path, &xml).is_ok()
            {
                quire_eval::set_exchange_rates(&xml);
            }
        }
    });
}

fn fetch() -> Option<String> {
    attohttpc::get(ECB_URL).send().ok()?.text().ok()
}

fn cached_xml() -> Option<String> {
    std::fs::read_to_string(cache_path()).ok()
}

/// The cache is stale when it is older than the GSettings interval
/// (or unreadable as a file at all).
fn stale() -> bool {
    let hours = crate::settings::currency_refresh_hours() as u64;
    let max_age = Duration::from_secs(hours.max(1) * 3600);
    std::fs::metadata(cache_path())
        .and_then(|m| m.modified())
        .map(|modified| modified.elapsed().unwrap_or(max_age) > max_age)
        .unwrap_or(true)
}

fn cache_path() -> PathBuf {
    cache_file_under(&cache_dir())
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn cache_file_under(dir: &std::path::Path) -> PathBuf {
    dir.join("quire/ecb.xml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_sits_under_quire_in_the_cache_dir() {
        assert!(cache_file_under(std::path::Path::new("/tmp/any")).ends_with("quire/ecb.xml"));
    }
}

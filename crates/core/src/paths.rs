use std::env;
use std::path::PathBuf;

/// Where Clipfinch keeps its database and settings.
///
/// `$CLIPFINCH_DATA_DIR`, else `$XDG_DATA_HOME/clipfinch`, else `~/.local/share/clipfinch`.
pub fn data_dir() -> PathBuf {
    if let Some(d) = env::var_os("CLIPFINCH_DATA_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(d);
    }
    if let Some(x) = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(x).join("clipfinch");
    }
    let home = env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".local/share/clipfinch")
}

pub fn db_path() -> PathBuf {
    data_dir().join("clipfinch.sqlite")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

fn autostart_file() -> PathBuf {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_else(|| ".".into())).join(".config")
        });
    base.join("autostart").join("clipfinch.desktop")
}

/// Enables or disables starting Clipfinch (in the background) at login.
pub fn set(enabled: bool, exe: &str) -> Result<()> {
    let path = autostart_file();
    if enabled {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let contents = format!(
            "[Desktop Entry]\nType=Application\nName=Clipfinch\nComment=Clipboard history\n\
             Exec={exe} --background\nIcon=clipfinch\nTerminal=false\nNoDisplay=true\n\
             X-GNOME-Autostart-enabled=true\n"
        );
        fs::write(path, contents)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn is_enabled() -> bool {
    autostart_file().exists()
}

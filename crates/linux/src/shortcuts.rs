//! Installs global shortcuts as GNOME custom keybindings.
//!
//! Wayland apps cannot grab global keys, so the compositor runs
//! `clipfinch --toggle` etc. for us. This writes those bindings through
//! `gsettings` (no GNOME-specific Rust bindings required).

use std::process::Command;

use anyhow::{bail, Context, Result};

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const BINDING_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const PATH_PREFIX: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";

pub struct Shortcut {
    pub slug: &'static str,
    pub name: &'static str,
    /// CLI flag passed to clipfinch, e.g. `--toggle`.
    pub flag: &'static str,
    /// GNOME accelerator syntax, e.g. `<Super><Shift>v`.
    pub binding: String,
}

pub fn all_slugs() -> [&'static str; 3] {
    [
        "clipfinch-popup",
        "clipfinch-queue",
        "clipfinch-queue-paste",
    ]
}

fn path_for(slug: &str) -> String {
    format!("{PATH_PREFIX}{slug}/")
}

fn gsettings(args: &[&str]) -> Result<String> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .context("could not run `gsettings` (is this a GNOME session?)")?;
    if !out.status.success() || (args.first() == Some(&"set") && !out.stderr.is_empty()) {
        bail!(
            "gsettings {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Parses `['/a/', '/b/']` (or `@as []`) into its string items.
fn parse_list(s: &str) -> Vec<String> {
    s.split('\'')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, p)| p.to_string())
        .collect()
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        "@as []".to_string()
    } else {
        format!(
            "[{}]",
            items
                .iter()
                .map(|p| format!("'{p}'"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn current_list() -> Result<Vec<String>> {
    Ok(parse_list(&gsettings(&[
        "get",
        MEDIA_KEYS,
        "custom-keybindings",
    ])?))
}

fn string_value(value: &str) -> String {
    format!(
        "'{}'",
        value
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

pub fn install(exe: &str, shortcuts: &[Shortcut]) -> Result<()> {
    anyhow::ensure!(
        std::env::var_os("CLIPFINCH_SHORTCUTS_UNAVAILABLE").is_none(),
        "host dconf settings are unavailable; shortcut registration is disabled"
    );
    anyhow::ensure!(!exe.chars().any(char::is_control), "invalid launcher path");
    let mut list = current_list()?;
    for s in shortcuts {
        let path = path_for(s.slug);
        if !list.contains(&path) {
            list.push(path.clone());
        }
        let schema = format!("{BINDING_SCHEMA}:{path}");
        let command = format!("'{}' {}", exe.replace('\'', "'\\''"), s.flag);
        gsettings(&["set", &schema, "name", &string_value(s.name)])?;
        gsettings(&["set", &schema, "command", &string_value(&command)])?;
        gsettings(&["set", &schema, "binding", &string_value(&s.binding)])?;
    }
    gsettings(&["set", MEDIA_KEYS, "custom-keybindings", &format_list(&list)])?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    anyhow::ensure!(
        std::env::var_os("CLIPFINCH_SHORTCUTS_UNAVAILABLE").is_none(),
        "host dconf settings are unavailable; shortcut removal is disabled"
    );
    let ours: Vec<String> = all_slugs().iter().map(|s| path_for(s)).collect();
    let list: Vec<String> = current_list()?
        .into_iter()
        .filter(|p| !ours.contains(p))
        .collect();
    gsettings(&["set", MEDIA_KEYS, "custom-keybindings", &format_list(&list)])?;
    for path in &ours {
        let schema = format!("{BINDING_SCHEMA}:{path}");
        let _ = gsettings(&["reset", &schema, "name"]);
        let _ = gsettings(&["reset", &schema, "command"]);
        let _ = gsettings(&["reset", &schema, "binding"]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_roundtrip() {
        assert!(parse_list("@as []").is_empty());
        let l = vec!["/a/".to_string(), "/b/".to_string()];
        assert_eq!(parse_list(&format_list(&l)), l);
        assert_eq!(format_list(&[]), "@as []");
    }
}

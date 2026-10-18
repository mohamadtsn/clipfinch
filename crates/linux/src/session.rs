use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

pub fn session_type() -> SessionType {
    match env::var("XDG_SESSION_TYPE").ok().as_deref() {
        Some("x11") => SessionType::X11,
        Some("wayland") => SessionType::Wayland,
        _ => {
            if env::var_os("WAYLAND_DISPLAY").is_some() {
                SessionType::Wayland
            } else if env::var_os("DISPLAY").is_some() {
                SessionType::X11
            } else {
                SessionType::Unknown
            }
        }
    }
}

pub fn is_gnome() -> bool {
    env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains("GNOME"))
        .unwrap_or(false)
}

/// Finds an executable on `$PATH`.
pub fn find_command(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

pub fn command_exists(name: &str) -> bool {
    find_command(name).is_some()
}

//! Which application is focused (X11 only).
//!
//! Wayland deliberately hides this from regular applications, so on a Wayland
//! session this returns `None`.

use crate::session::{session_type, SessionType};

pub fn active_window_class() -> Option<String> {
    if session_type() != SessionType::X11 {
        return None;
    }
    x11_active_class()
}

fn x11_active_class() -> Option<String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;
    let atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let reply = conn
        .get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let win = reply.value32()?.next()?;
    if win == 0 {
        return None;
    }
    let wm = conn
        .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
        .ok()?
        .reply()
        .ok()?;
    let parts: Vec<&[u8]> = wm
        .value
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .collect();
    parts
        .last()
        .map(|p| String::from_utf8_lossy(p).into_owned())
}

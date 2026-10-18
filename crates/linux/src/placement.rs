//! Window placement helpers for X11 sessions.
//!
//! Wayland does not let applications position their own windows, so none of
//! this is used there (the compositor decides where the popup appears).

pub fn pointer_position() -> Option<(i32, i32)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;
    let p = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((p.root_x as i32, p.root_y as i32))
}

pub fn screen_size() -> Option<(i32, i32)> {
    use x11rb::connection::Connection;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let s = &conn.setup().roots[screen_num];
    Some((s.width_in_pixels as i32, s.height_in_pixels as i32))
}

pub fn move_window(xid: u32, x: i32, y: i32) -> anyhow::Result<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt};

    let (conn, _) = x11rb::connect(None)?;
    conn.configure_window(xid, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;
    Ok(())
}

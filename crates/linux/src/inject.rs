//! Sending a paste keystroke to the focused application.
//!
//! * X11 session: XTest (reliable).
//! * Wayland session: `ydotool` if installed and its daemon is running.
//!   Wayland has no sanctioned way for a normal app to type into other apps.

use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use crate::session::{command_exists, session_type, SessionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    XTest,
    Ydotool,
    None,
}

pub fn available_method() -> Method {
    match session_type() {
        SessionType::X11 => Method::XTest,
        _ => {
            if command_exists("ydotool") {
                Method::Ydotool
            } else {
                Method::None
            }
        }
    }
}

pub fn describe() -> &'static str {
    match available_method() {
        Method::XTest => "XTest (automatic paste available)",
        Method::Ydotool => "ydotool (automatic paste available if ydotoold is running)",
        Method::None => "not available (install ydotool, or press Ctrl+V yourself)",
    }
}

/// Sends Ctrl+V (or Ctrl+Shift+V when `shift`).
pub fn send_paste(shift: bool) -> Result<(), String> {
    match available_method() {
        Method::XTest => xtest_paste(shift).map_err(|e| format!("XTest failed: {e}")),
        Method::Ydotool => ydotool_paste(shift),
        Method::None => Err("No keystroke injector available".to_string()),
    }
}

fn ydotool_paste(shift: bool) -> Result<(), String> {
    // Linux input event codes: LEFTCTRL=29, LEFTSHIFT=42, V=47.
    let seq: &[&str] = if shift {
        &["29:1", "42:1", "47:1", "47:0", "42:0", "29:0"]
    } else {
        &["29:1", "47:1", "47:0", "29:0"]
    };
    let status = Command::new("ydotool")
        .arg("key")
        .args(seq)
        .status()
        .map_err(|e| format!("could not run ydotool: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("ydotool failed (is ydotoold running?)".to_string())
    }
}

fn xtest_paste(shift: bool) -> anyhow::Result<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
    use x11rb::protocol::xtest::ConnectionExt as _;

    const XK_V: u32 = 0x0076;
    const XK_SHIFT_L: u32 = 0xffe1;
    const XK_CONTROL_L: u32 = 0xffe3;
    // Modifiers we release first so the user's held hotkey keys don't leak into Ctrl+V.
    const MODS: [u32; 8] = [
        0xffe1, 0xffe2, 0xffe3, 0xffe4, 0xffe9, 0xffea, 0xffeb, 0xffec,
    ];

    let (conn, screen_num) = x11rb::connect(None)?;
    let setup = conn.setup();
    let root = setup.roots[screen_num].root;
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    let mapping = conn.get_keyboard_mapping(min, max - min + 1)?.reply()?;
    let per = mapping.keysyms_per_keycode as usize;

    let keycode_of = |sym: u32| -> Option<u8> {
        mapping
            .keysyms
            .chunks(per)
            .position(|chunk| chunk.contains(&sym))
            .map(|i| min + i as u8)
    };
    let key_v = keycode_of(XK_V).ok_or_else(|| anyhow::anyhow!("no keycode for V"))?;
    let key_ctrl =
        keycode_of(XK_CONTROL_L).ok_or_else(|| anyhow::anyhow!("no keycode for Control"))?;
    let key_shift = keycode_of(XK_SHIFT_L);

    // Release any modifiers that are physically held right now.
    let pressed = conn.query_keymap()?.reply()?.keys;
    for sym in MODS {
        if let Some(code) = keycode_of(sym) {
            let byte = pressed[(code / 8) as usize];
            if byte & (1 << (code % 8)) != 0 {
                conn.xtest_fake_input(KEY_RELEASE_EVENT, code, 0, root, 0, 0, 0)?;
            }
        }
    }
    conn.flush()?;
    sleep(Duration::from_millis(15));

    let press = |code: u8, down: bool| -> anyhow::Result<()> {
        let kind = if down { KEY_PRESS_EVENT } else { KEY_RELEASE_EVENT };
        conn.xtest_fake_input(kind, code, 0, root, 0, 0, 0)?;
        Ok(())
    };
    press(key_ctrl, true)?;
    if shift {
        if let Some(s) = key_shift {
            press(s, true)?;
        }
    }
    press(key_v, true)?;
    press(key_v, false)?;
    if shift {
        if let Some(s) = key_shift {
            press(s, false)?;
        }
    }
    press(key_ctrl, false)?;
    conn.flush()?;
    Ok(())
}

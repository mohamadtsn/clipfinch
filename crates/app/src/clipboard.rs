//! Clipboard monitoring and writing through GDK.

use std::rc::Rc;
use std::time::Duration;

use clipfinch_core::classify;
use clipfinch_core::model::Format;
use clipfinch_core::text;
use clipfinch_linux::window_info;
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::ctx::Ctx;

/// Largest single representation we will read.
const MAX_ITEM_BYTES: usize = 32 * 1024 * 1024;
/// Largest total size of one clip.
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

pub fn start_monitor(ctx: Rc<Ctx>) {
    let Some(display) = gdk::Display::default() else {
        eprintln!("clipfinch: no display, clipboard monitoring disabled");
        return;
    };
    let clipboard = display.clipboard();
    {
        let ctx = ctx.clone();
        clipboard.connect_changed(move |cb| {
            // Changes we made ourselves (pasting a clip) are not new copies.
            if cb.is_local() {
                return;
            }
            schedule(ctx.clone(), cb.clone());
        });
    }
    // Pick up whatever is on the clipboard at startup.
    schedule(ctx, clipboard);
}

/// Debounced capture: only the newest change within ~120 ms is processed.
fn schedule(ctx: Rc<Ctx>, cb: gdk::Clipboard) {
    let generation = ctx.next_capture_gen();
    glib::spawn_future_local(async move {
        glib::timeout_future(Duration::from_millis(120)).await;
        if ctx.capture_gen() != generation {
            return;
        }
        capture(&ctx, &cb).await;
    });
}

async fn read_mime(cb: &gdk::Clipboard, mime: &str) -> Option<Vec<u8>> {
    let (stream, _) = cb.read_future(&[mime], glib::Priority::DEFAULT).await.ok()?;
    let mut out: Vec<u8> = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(64 * 1024, glib::Priority::DEFAULT)
            .await
            .ok()?;
        if chunk.is_empty() {
            break;
        }
        out.extend_from_slice(&chunk);
        if out.len() > MAX_ITEM_BYTES {
            return None;
        }
    }
    let _ = stream.close_future(glib::Priority::DEFAULT).await;
    Some(out)
}

async fn capture(ctx: &Rc<Ctx>, cb: &gdk::Clipboard) {
    let (paused, excluded) = {
        let st = ctx.state.borrow();
        (st.settings.paused, st.settings.excluded_apps.clone())
    };
    if paused {
        return;
    }

    let mimes: Vec<String> = cb
        .formats()
        .mime_types()
        .iter()
        .map(|s| s.to_string())
        .collect();
    if mimes.is_empty() {
        return;
    }
    // Password managers mark sensitive copies; never store those.
    if mimes.iter().any(|m| classify::is_secret_mime(m)) {
        return;
    }

    // Only available on X11 sessions; None on Wayland.
    let source = window_info::active_window_class();
    if let Some(app) = &source {
        if excluded.iter().any(|e| e.eq_ignore_ascii_case(app)) {
            return;
        }
    }

    let mut formats: Vec<Format> = Vec::new();
    let mut total = 0usize;
    for mime in mimes {
        if classify::is_ignored_mime(&mime) {
            continue;
        }
        if let Some(data) = read_mime(cb, &mime).await {
            if data.is_empty() {
                continue;
            }
            total += data.len();
            if total > MAX_TOTAL_BYTES {
                return;
            }
            formats.push(Format { mime, data });
        }
    }

    let Some(new_clip) = classify::build_new_clip(formats, source) else {
        return;
    };
    let label = text::truncate(new_clip.text.lines().next().unwrap_or(""), 80);
    let result = ctx.state.borrow_mut().store.upsert_clip(new_clip);
    match result {
        Ok((id, _)) => ctx.after_capture(id, label),
        Err(e) => eprintln!("clipfinch: could not store clip: {e:#}"),
    }
}

/// Puts every stored representation back on the clipboard.
pub fn set_formats(formats: &[Format]) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let providers: Vec<gdk::ContentProvider> = formats
        .iter()
        .map(|f| gdk::ContentProvider::for_bytes(&f.mime, &glib::Bytes::from(&f.data[..])))
        .collect();
    if providers.is_empty() {
        return;
    }
    let provider = gdk::ContentProvider::new_union(&providers);
    let _ = display.clipboard().set_content(Some(&provider));
}

/// Puts only plain text on the clipboard.
pub fn set_plain(text: &str) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gdk::ContentProvider::for_value(&text.to_value());
    let _ = display.clipboard().set_content(Some(&provider));
}

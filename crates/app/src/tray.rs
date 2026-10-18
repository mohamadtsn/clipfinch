//! System tray icon (StatusNotifierItem). On GNOME this needs the AppIndicator
//! extension, which Ubuntu enables by default.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::MenuItem;

use crate::ctx::TrayCmd;

struct ClipfinchTray {
    tx: async_channel::Sender<TrayCmd>,
    paused: Arc<AtomicBool>,
}

impl ksni::Tray for ClipfinchTray {
    fn id(&self) -> String {
        "clipfinch".into()
    }

    fn title(&self) -> String {
        "Clipfinch".into()
    }

    fn icon_name(&self) -> String {
        "clipfinch".into()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open clipboard history".into(),
                activate: Box::new(|t: &mut ClipfinchTray| {
                    let _ = t.tx.send_blocking(TrayCmd::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Paste queue".into(),
                activate: Box::new(|t: &mut ClipfinchTray| {
                    let _ = t.tx.send_blocking(TrayCmd::Queue);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Pause capture".into(),
                checked: self.paused.load(Ordering::Relaxed),
                activate: Box::new(|t: &mut ClipfinchTray| {
                    let _ = t.tx.send_blocking(TrayCmd::TogglePause);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings…".into(),
                activate: Box::new(|t: &mut ClipfinchTray| {
                    let _ = t.tx.send_blocking(TrayCmd::Settings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut ClipfinchTray| {
                    let _ = t.tx.send_blocking(TrayCmd::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(tx: async_channel::Sender<TrayCmd>, paused: Arc<AtomicBool>) {
    ksni::TrayService::new(ClipfinchTray { tx, paused }).spawn();
}

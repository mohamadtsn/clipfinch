//! The settings window.

use std::rc::Rc;

use clipfinch_core::settings::HistoryLimit;
use clipfinch_linux::session::session_type;
use clipfinch_linux::shortcuts::{self, Shortcut};
use clipfinch_linux::{autostart, inject};
use gtk::prelude::*;
use gtk::{gio, pango};

use crate::ctx::{exe_path, Ctx};

pub struct SettingsWindow {
    pub window: gtk::Window,
}

fn row(title: &str, subtitle: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.set_hexpand(true);
    let t = gtk::Label::new(Some(title));
    t.set_xalign(0.0);
    texts.append(&t);
    if !subtitle.is_empty() {
        let s = gtk::Label::new(Some(subtitle));
        s.set_xalign(0.0);
        s.set_wrap(true);
        s.add_css_class("dim-label");
        texts.append(&s);
    }
    widget.set_valign(gtk::Align::Center);
    line.append(&texts);
    line.append(widget);
    line
}

fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::new(None);
    l.set_markup(&format!("<b>{}</b>", gtk::glib::markup_escape_text(text)));
    l.set_xalign(0.0);
    l.set_margin_top(10);
    l
}

impl SettingsWindow {
    pub fn new(ctx: Rc<Ctx>) -> Rc<SettingsWindow> {
        let settings = ctx.state.borrow().settings.clone();

        let window = gtk::Window::builder()
            .title("Clipfinch Settings")
            .default_width(520)
            .default_height(640)
            .build();
        window.add_css_class("panel");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(18);
        root.set_margin_end(18);

        // --- General ---------------------------------------------------------
        root.append(&heading("General"));

        let labels: Vec<&str> = HistoryLimit::ALL.iter().map(|l| l.label()).collect();
        let dropdown = gtk::DropDown::from_strings(&labels);
        dropdown.set_selected(settings.history_limit.index());
        {
            let ctx = ctx.clone();
            dropdown.connect_selected_notify(move |d| {
                let limit = HistoryLimit::from_index(d.selected());
                ctx.update_settings(|s| s.history_limit = limit);
                ctx.apply_retention();
            });
        }
        root.append(&row(
            "Keep history for",
            "Pinned clips are never removed.",
            &dropdown,
        ));

        let pause = gtk::Switch::new();
        pause.set_active(settings.paused);
        {
            let ctx = ctx.clone();
            pause.connect_active_notify(move |sw| {
                let on = sw.is_active();
                ctx.update_settings(|s| s.paused = on);
            });
        }
        root.append(&row("Pause capture", "Nothing new is recorded while on.", &pause));

        let login = gtk::Switch::new();
        login.set_active(settings.open_at_login || autostart::is_enabled());
        {
            let ctx = ctx.clone();
            login.connect_active_notify(move |sw| {
                let on = sw.is_active();
                ctx.update_settings(|s| s.open_at_login = on);
                if let Err(e) = autostart::set(on, &exe_path()) {
                    eprintln!("clipfinch: autostart: {e:#}");
                }
            });
        }
        root.append(&row("Open at login", "Starts Clipfinch in the background.", &login));

        let shift = gtk::Switch::new();
        shift.set_active(settings.paste_with_shift);
        {
            let ctx = ctx.clone();
            shift.connect_active_notify(move |sw| {
                let on = sw.is_active();
                ctx.update_settings(|s| s.paste_with_shift = on);
            });
        }
        root.append(&row(
            "Paste with Ctrl+Shift+V",
            "Use this if you mostly paste into terminals.",
            &shift,
        ));

        // --- Privacy ---------------------------------------------------------
        root.append(&heading("Privacy"));
        let excluded = gtk::Entry::new();
        excluded.set_width_chars(24);
        excluded.set_placeholder_text(Some("keepassxc, bitwarden"));
        excluded.set_text(&settings.excluded_apps.join(", "));
        {
            let ctx = ctx.clone();
            excluded.connect_changed(move |e| {
                let apps: Vec<String> = e
                    .text()
                    .split(',')
                    .map(|a| a.trim().to_string())
                    .filter(|a| !a.is_empty())
                    .collect();
                ctx.update_settings(|s| s.excluded_apps = apps);
            });
        }
        root.append(&row(
            "Ignore copies from these apps",
            "Comma-separated window classes. Works on X11 sessions only; password managers \
             that mark their copies as secret are always ignored.",
            &excluded,
        ));

        let erase = gtk::Button::with_label("Erase history…");
        erase.add_css_class("destructive-action");
        {
            let ctx = ctx.clone();
            let window = window.clone();
            erase.connect_clicked(move |_| {
                let dialog = gtk::AlertDialog::builder()
                    .message("Erase clipboard history?")
                    .detail("Everything that is not on a pinboard will be deleted.")
                    .cancel_button(0)
                    .default_button(0)
                    .build();
                dialog.set_buttons(&["Cancel", "Erase"]);
                let ctx = ctx.clone();
                dialog.choose(Some(&window), gio::Cancellable::NONE, move |res| {
                    if let Ok(1) = res {
                        ctx.erase_history();
                    }
                });
            });
        }
        root.append(&row("Erase history", "Pinboards are kept.", &erase));

        // --- Shortcuts -------------------------------------------------------
        root.append(&heading("Global shortcuts"));
        let note = gtk::Label::new(Some(
            "Wayland apps cannot grab global keys, so Clipfinch registers GNOME custom \
             shortcuts that run `clipfinch --toggle` and friends. Use GNOME accelerator \
             syntax, e.g. <Super><Shift>v.",
        ));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.add_css_class("dim-label");
        root.append(&note);

        let popup_entry = gtk::Entry::new();
        popup_entry.set_text(&settings.popup_hotkey);
        let queue_entry = gtk::Entry::new();
        queue_entry.set_text(&settings.queue_hotkey);
        let paste_entry = gtk::Entry::new();
        paste_entry.set_text(&settings.queue_paste_hotkey);
        root.append(&row("Open history", "", &popup_entry));
        root.append(&row("Start / stop paste queue", "", &queue_entry));
        root.append(&row("Paste next queue item", "", &paste_entry));

        let status = gtk::Label::new(None);
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.set_wrap_mode(pango::WrapMode::WordChar);

        let install = gtk::Button::with_label("Install / update shortcuts");
        install.add_css_class("suggested-action");
        {
            let ctx = ctx.clone();
            let status = status.clone();
            let (pe, qe, ne) = (popup_entry.clone(), queue_entry.clone(), paste_entry.clone());
            install.connect_clicked(move |_| {
                let popup_key = pe.text().trim().to_string();
                let queue_key = qe.text().trim().to_string();
                let paste_key = ne.text().trim().to_string();
                ctx.update_settings(|s| {
                    s.popup_hotkey = popup_key.clone();
                    s.queue_hotkey = queue_key.clone();
                    s.queue_paste_hotkey = paste_key.clone();
                });
                let list = vec![
                    Shortcut {
                        slug: "clipfinch-popup",
                        name: "Clipfinch: open history",
                        flag: "--toggle",
                        binding: popup_key,
                    },
                    Shortcut {
                        slug: "clipfinch-queue",
                        name: "Clipfinch: paste queue",
                        flag: "--queue",
                        binding: queue_key,
                    },
                    Shortcut {
                        slug: "clipfinch-queue-paste",
                        name: "Clipfinch: paste next in queue",
                        flag: "--queue-paste",
                        binding: paste_key,
                    },
                ];
                match shortcuts::install(&exe_path(), &list) {
                    Ok(()) => status.set_text("Shortcuts installed."),
                    Err(e) => status.set_text(&format!("Could not install shortcuts: {e:#}")),
                }
            });
        }
        root.append(&install);
        root.append(&status);

        // --- System ----------------------------------------------------------
        root.append(&heading("This system"));
        let info = gtk::Label::new(Some(&format!(
            "Session: {:?}\nAutomatic paste: {}\nData folder: {}",
            session_type(),
            inject::describe(),
            clipfinch_core::paths::data_dir().display()
        )));
        info.set_xalign(0.0);
        info.set_wrap(true);
        info.set_selectable(true);
        info.add_css_class("dim-label");
        root.append(&info);

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&root)
            .build();
        window.set_child(Some(&scroller));

        Rc::new(SettingsWindow { window })
    }
}

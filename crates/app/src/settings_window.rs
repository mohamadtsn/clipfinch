//! The settings window.

use std::cell::Cell;
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
    line.add_css_class("settings-row");
    widget
        .as_ref()
        .update_property(&[gtk::accessible::Property::Label(title)]);
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.set_hexpand(true);
    let t = gtk::Label::new(Some(title));
    t.set_xalign(0.0);
    t.set_wrap(true);
    texts.append(&t);
    if !subtitle.is_empty() {
        let s = gtk::Label::new(Some(subtitle));
        s.set_xalign(0.0);
        s.set_wrap(true);
        s.add_css_class("secondary");
        s.set_max_width_chars(34);
        texts.append(&s);
    }
    widget.set_valign(gtk::Align::Center);
    line.append(&texts);
    line.append(widget);
    line
}

fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::new(None);
    l.set_text(text);
    l.add_css_class("section-heading");
    l.set_xalign(0.0);
    l
}

fn group(root: &gtk::Box, title: &str) -> gtk::Box {
    root.append(&heading(title));
    let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
    group.add_css_class("settings-group");
    root.append(&group);
    group
}

fn page(stack: &gtk::Stack, name: &str, title: &str, description: &str) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("settings-content");
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("page-title");
    content.append(&heading);
    let subtitle = gtk::Label::new(Some(description));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("page-description");
    content.append(&subtitle);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();
    stack.add_titled(&scroll, Some(name), title);
    content
}

fn save_login_setting(ctx: &Ctx, enabled: bool) -> anyhow::Result<()> {
    let exe = exe_path()?;
    autostart::set(enabled, &exe)?;
    ctx.update_settings(|s| s.open_at_login = enabled);
    Ok(())
}

impl SettingsWindow {
    pub fn new(ctx: Rc<Ctx>) -> Rc<SettingsWindow> {
        let settings = ctx.state.borrow().settings.clone();

        let window = gtk::Window::builder()
            .title("Clipfinch Settings")
            .default_width(780)
            .default_height(560)
            .build();
        window.add_css_class("panel");
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&gtk::Label::new(Some("Settings"))));
        window.set_titlebar(Some(&header));

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::None);
        let sidebar = gtk::StackSidebar::new();
        sidebar.set_stack(&stack);
        sidebar.set_size_request(156, -1);
        sidebar.add_css_class("settings-sidebar");
        let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        shell.append(&sidebar);
        shell.append(&stack);

        let root = page(
            &stack,
            "general",
            "General",
            "Manage capture, history and paste behavior.",
        );

        // --- General ---------------------------------------------------------
        let general = group(&root, "Capture & history");

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
        general.append(&row(
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
        general.append(&row(
            "Pause capture",
            "Nothing new is recorded while on.",
            &pause,
        ));

        let login = gtk::Switch::new();
        login.set_active(autostart::is_enabled());
        let login_status = gtk::Label::new(None);
        login_status.set_xalign(0.0);
        login_status.set_wrap(true);
        login_status.set_wrap_mode(pango::WrapMode::WordChar);
        login_status.add_css_class("error");
        {
            let ctx = ctx.clone();
            let status = login_status.clone();
            let updating = Cell::new(false);
            login.connect_active_notify(move |sw| {
                if updating.replace(true) {
                    return;
                }
                let on = sw.is_active();
                match save_login_setting(&ctx, on) {
                    Ok(()) => status.set_text(""),
                    Err(e) => {
                        status.set_text(&format!("Could not update login startup: {e:#}"));
                        sw.set_active(autostart::is_enabled());
                    }
                }
                updating.set(false);
            });
        }
        general.append(&row(
            "Open at login",
            "Starts Clipfinch in the background.",
            &login,
        ));

        root.append(&login_status);

        let shift = gtk::Switch::new();
        shift.set_active(settings.paste_with_shift);
        {
            let ctx = ctx.clone();
            shift.connect_active_notify(move |sw| {
                let on = sw.is_active();
                ctx.update_settings(|s| s.paste_with_shift = on);
            });
        }
        general.append(&row(
            "Paste with Ctrl+Shift+V",
            "Use this if you mostly paste into terminals.",
            &shift,
        ));

        // --- Privacy ---------------------------------------------------------
        let privacy_page = page(
            &stack,
            "privacy",
            "Privacy",
            "Choose what stays in your clipboard history.",
        );
        let privacy = group(&privacy_page, "Excluded apps");
        let excluded = gtk::Entry::new();
        excluded.set_width_chars(18);
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
        privacy.append(&row(
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
        let deletion = group(&privacy_page, "History data");
        deletion.append(&row("Erase history", "Pinboards are kept.", &erase));

        // --- Shortcuts -------------------------------------------------------
        let shortcuts_page = page(
            &stack,
            "shortcuts",
            "Shortcuts",
            "Open history and manage the paste queue from your keyboard.",
        );
        let note = gtk::Label::new(Some(
            "Use GNOME key notation, for example <Super><Shift>v. Apply your changes \
             to register them with the desktop.",
        ));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.add_css_class("secondary");
        note.set_margin_bottom(12);
        shortcuts_page.append(&note);
        let shortcuts_group = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shortcuts_group.add_css_class("settings-group");
        shortcuts_page.append(&shortcuts_group);

        let popup_entry = gtk::Entry::new();
        popup_entry.set_text(&settings.popup_hotkey);
        let queue_entry = gtk::Entry::new();
        queue_entry.set_text(&settings.queue_hotkey);
        let paste_entry = gtk::Entry::new();
        paste_entry.set_text(&settings.queue_paste_hotkey);
        shortcuts_group.append(&row("Open history", "", &popup_entry));
        shortcuts_group.append(&row("Start / stop paste queue", "", &queue_entry));
        shortcuts_group.append(&row("Paste next queue item", "", &paste_entry));

        let status = gtk::Label::new(None);
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.set_wrap_mode(pango::WrapMode::WordChar);

        let install = gtk::Button::with_label("Apply shortcuts");
        install.set_halign(gtk::Align::End);
        install.set_margin_top(12);
        install.add_css_class("suggested-action");
        {
            let ctx = ctx.clone();
            let status = status.clone();
            let (pe, qe, ne) = (
                popup_entry.clone(),
                queue_entry.clone(),
                paste_entry.clone(),
            );
            install.connect_clicked(move |_| {
                let popup_key = pe.text().trim().to_string();
                let queue_key = qe.text().trim().to_string();
                let paste_key = ne.text().trim().to_string();
                let list = vec![
                    Shortcut {
                        slug: "clipfinch-popup",
                        name: "Clipfinch: open history",
                        flag: "--toggle",
                        binding: popup_key.clone(),
                    },
                    Shortcut {
                        slug: "clipfinch-queue",
                        name: "Clipfinch: paste queue",
                        flag: "--queue",
                        binding: queue_key.clone(),
                    },
                    Shortcut {
                        slug: "clipfinch-queue-paste",
                        name: "Clipfinch: paste next in queue",
                        flag: "--queue-paste",
                        binding: paste_key.clone(),
                    },
                ];
                match exe_path().and_then(|exe| shortcuts::install(&exe, &list)) {
                    Ok(()) => {
                        ctx.update_settings(|s| {
                            s.popup_hotkey = popup_key;
                            s.queue_hotkey = queue_key;
                            s.queue_paste_hotkey = paste_key;
                        });
                        status.set_text("Shortcuts installed.");
                    }
                    Err(e) => status.set_text(&format!("Could not install shortcuts: {e:#}")),
                }
            });
        }
        shortcuts_page.append(&install);
        shortcuts_page.append(&status);

        // --- System ----------------------------------------------------------
        let system_page = page(
            &stack,
            "system",
            "System",
            "Desktop integration and local storage.",
        );
        let info = gtk::Label::new(Some(&format!(
            "Session: {:?}\nAutomatic paste: {}\nData folder: {}",
            session_type(),
            inject::describe(),
            clipfinch_core::paths::data_dir().display()
        )));
        info.set_xalign(0.0);
        info.set_wrap(true);
        info.set_wrap_mode(pango::WrapMode::WordChar);
        info.set_selectable(true);
        info.add_css_class("secondary");
        info.add_css_class("system-info");
        system_page.append(&info);
        window.set_child(Some(&shell));

        Rc::new(SettingsWindow { window })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a private display and CLIPFINCH_PRIVATE_TEST=1"]
    fn settings_pages_fit_long_paths_and_preserve_capture_toggle() {
        assert_eq!(std::env::var("CLIPFINCH_PRIVATE_TEST").as_deref(), Ok("1"));
        gtk::init().unwrap();
        let dir =
            std::env::temp_dir().join(format!("clipfinch-settings-test-{}", std::process::id()));
        let old_data = std::env::var_os("CLIPFINCH_DATA_DIR");
        std::env::set_var("CLIPFINCH_DATA_DIR", dir.join("x".repeat(200)));
        let app = gtk::Application::builder()
            .application_id("dev.clipfinch.SettingsTest")
            .build();
        let ctx = Ctx::new(&app).unwrap();
        let settings = SettingsWindow::new(ctx.clone());
        assert!(settings.window.measure(gtk::Orientation::Horizontal, -1).0 <= 780);
        let shell = settings.window.child().unwrap();
        let stack = shell
            .last_child()
            .unwrap()
            .downcast::<gtk::Stack>()
            .unwrap();
        for name in ["general", "privacy", "shortcuts", "system"] {
            stack.set_visible_child_name(name);
            assert_eq!(stack.visible_child_name().as_deref(), Some(name));
        }
        stack.set_visible_child_name("general");
        let mut widgets = vec![stack.visible_child().unwrap()];
        let mut pause = None;
        while let Some(widget) = widgets.pop() {
            if let Some(switch) = widget.downcast_ref::<gtk::Switch>() {
                pause = Some(switch.clone());
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                widgets.push(widget);
            }
        }
        let pause = pause.unwrap();
        pause.set_active(true);
        assert!(ctx.state.borrow().settings.paused);
        pause.set_active(false);
        assert!(!ctx.state.borrow().settings.paused);
        match old_data {
            Some(value) => std::env::set_var("CLIPFINCH_DATA_DIR", value),
            None => std::env::remove_var("CLIPFINCH_DATA_DIR"),
        }
        drop(settings);
        drop(ctx);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires an isolated display; run with xvfb-run"]
    fn failed_autostart_write_does_not_save_enabled_setting() {
        gtk::init().unwrap();
        let dir = std::env::temp_dir().join(format!("clipfinch-login-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("not-a-directory");
        std::fs::write(&config, "synthetic fixture").unwrap();
        let old_data = std::env::var_os("CLIPFINCH_DATA_DIR");
        let old_config = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("CLIPFINCH_DATA_DIR", dir.join("data"));
        std::env::set_var("XDG_CONFIG_HOME", &config);
        let app = gtk::Application::builder()
            .application_id("dev.clipfinch.Test")
            .build();
        let ctx = Ctx::new(&app).unwrap();
        let failed = save_login_setting(&ctx, true).is_err();
        let enabled = ctx.state.borrow().settings.open_at_login;
        for (name, value) in [
            ("CLIPFINCH_DATA_DIR", old_data),
            ("XDG_CONFIG_HOME", old_config),
        ] {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
        assert!(failed);
        assert!(!enabled);
    }
}

//! Shared application context: state, windows and high-level actions.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;

use clipfinch_core::model::Kind;
use clipfinch_core::paths;
use clipfinch_core::queue::PasteQueue;
use clipfinch_core::settings::Settings;
use clipfinch_core::store::{now_ms, Store};
use clipfinch_linux::inject;
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::clipboard;
use crate::popup::Popup;
use crate::queue_window::QueueWindow;
use crate::settings_window::SettingsWindow;

#[allow(dead_code)]
pub enum TrayCmd {
    Show,
    Queue,
    Settings,
    TogglePause,
    Quit,
}

pub struct AppState {
    pub store: Store,
    pub settings: Settings,
    pub queue: PasteQueue,
}

pub struct Ctx {
    pub app: gtk::Application,
    pub state: RefCell<AppState>,
    popup: RefCell<Option<Rc<Popup>>>,
    queue_win: RefCell<Option<Rc<QueueWindow>>>,
    settings_win: RefCell<Option<Rc<SettingsWindow>>>,
    capture_gen: Cell<u64>,
    hinted: Cell<bool>,
    paused_flag: Arc<AtomicBool>,
}

/// Containers register the host launcher rather than their internal binary.
pub fn exe_path() -> anyhow::Result<String> {
    resolve_exe_path(std::env::var_os("CLIPFINCH_LAUNCHER").as_deref())
}

fn resolve_exe_path(launcher: Option<&std::ffi::OsStr>) -> anyhow::Result<String> {
    let path = match launcher {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_exe().context("could not locate the running executable")?,
    };
    anyhow::ensure!(
        path.is_absolute(),
        "CLIPFINCH_LAUNCHER must be an absolute path"
    );
    let path = path.to_str().context("launcher path must be valid UTF-8")?;
    anyhow::ensure!(
        !path.chars().any(char::is_control),
        "launcher path must not contain control characters"
    );
    Ok(path.to_string())
}

#[allow(deprecated)]
pub fn load_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    if let Some(schema) = gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup("org.gnome.desktop.interface", true))
    {
        if schema.has_key("color-scheme") {
            let system = gio::Settings::new("org.gnome.desktop.interface");
            system
                .bind(
                    "color-scheme",
                    &settings,
                    "gtk-application-prefer-dark-theme",
                )
                .get_only()
                .no_sensitivity()
                .mapping(|value, _| {
                    value
                        .str()
                        .map(|scheme| (scheme == "prefer-dark").to_value())
                })
                .build();
        }
    }
    let provider = gtk::CssProvider::new();
    apply_css(
        &provider,
        settings.property("gtk-application-prefer-dark-theme"),
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    settings.connect_notify_local(
        Some("gtk-application-prefer-dark-theme"),
        move |settings, _| {
            apply_css(
                &provider,
                settings.property("gtk-application-prefer-dark-theme"),
            );
        },
    );
}

fn apply_css(provider: &gtk::CssProvider, dark: bool) {
    let colors = if dark {
        include_str!("palette-dark.css")
    } else {
        include_str!("palette-light.css")
    };
    provider.load_from_string(&format!("{colors}\n{}", include_str!("style.css")));
}

impl Ctx {
    pub fn new(app: &gtk::Application) -> anyhow::Result<Rc<Ctx>> {
        let store = Store::open(&paths::db_path())?;
        let settings = Settings::load(&paths::settings_path());
        let paused_flag = Arc::new(AtomicBool::new(settings.paused));
        Ok(Rc::new(Ctx {
            app: app.clone(),
            state: RefCell::new(AppState {
                store,
                settings,
                queue: PasteQueue::new(),
            }),
            popup: RefCell::new(None),
            queue_win: RefCell::new(None),
            settings_win: RefCell::new(None),
            capture_gen: Cell::new(0),
            hinted: Cell::new(false),
            paused_flag,
        }))
    }

    pub fn start(self: &Rc<Self>) {
        clipboard::start_monitor(self.clone());
        self.apply_retention();
        {
            let me = self.clone();
            glib::timeout_add_seconds_local(3600, move || {
                me.apply_retention();
                glib::ControlFlow::Continue
            });
        }
        #[cfg(feature = "tray")]
        {
            let (tx, rx) = async_channel::unbounded::<TrayCmd>();
            crate::tray::spawn(tx, self.paused_flag.clone());
            let me = self.clone();
            glib::spawn_future_local(async move {
                while let Ok(cmd) = rx.recv().await {
                    me.on_tray(cmd);
                }
            });
        }
    }

    // ---- capture bookkeeping -------------------------------------------------

    pub fn next_capture_gen(&self) -> u64 {
        let g = self.capture_gen.get() + 1;
        self.capture_gen.set(g);
        g
    }

    pub fn capture_gen(&self) -> u64 {
        self.capture_gen.get()
    }

    pub fn after_capture(self: &Rc<Self>, id: i64, label: String) {
        {
            let mut st = self.state.borrow_mut();
            if st.queue.is_active() {
                st.queue.push(id, label);
            }
        }
        self.refresh_ui();
    }

    pub fn refresh_ui(self: &Rc<Self>) {
        if let Some(p) = self.popup.borrow().clone() {
            if p.is_visible() {
                p.refresh();
            }
        }
        if let Some(q) = self.queue_win.borrow().clone() {
            q.refresh();
        }
    }

    // ---- windows -------------------------------------------------------------

    fn popup(self: &Rc<Self>) -> Rc<Popup> {
        if let Some(p) = self.popup.borrow().as_ref() {
            return p.clone();
        }
        let p = Popup::new(self.clone());
        *self.popup.borrow_mut() = Some(p.clone());
        p
    }

    pub fn show_popup(self: &Rc<Self>) {
        self.popup().show();
    }

    pub fn hide_popup(&self) {
        if let Some(p) = self.popup.borrow().clone() {
            p.hide();
        }
    }

    pub fn toggle_popup(self: &Rc<Self>) {
        let visible = self
            .popup
            .borrow()
            .as_ref()
            .map(|p| p.is_visible())
            .unwrap_or(false);
        if visible {
            self.hide_popup();
        } else {
            self.show_popup();
        }
    }

    pub fn show_settings(self: &Rc<Self>) {
        let existing = self.settings_win.borrow().clone();
        if let Some(w) = existing {
            if w.window.is_visible() {
                w.window.present();
                return;
            }
        }
        let w = SettingsWindow::new(self.clone());
        w.window.present();
        *self.settings_win.borrow_mut() = Some(w);
    }

    fn queue_window(self: &Rc<Self>) -> Rc<QueueWindow> {
        if let Some(q) = self.queue_win.borrow().as_ref() {
            return q.clone();
        }
        let q = QueueWindow::new(self.clone());
        *self.queue_win.borrow_mut() = Some(q.clone());
        q
    }

    // ---- commands ------------------------------------------------------------

    pub fn run_command(self: &Rc<Self>, name: &str) {
        match name {
            "show" => self.show_popup(),
            "toggle" => self.toggle_popup(),
            "queue" => self.queue_toggle(),
            "queue-paste" => self.queue_paste(),
            "settings" => self.show_settings(),
            "quit" => self.app.quit(),
            _ => {}
        }
    }

    pub fn on_tray(self: &Rc<Self>, cmd: TrayCmd) {
        match cmd {
            TrayCmd::Show => self.show_popup(),
            TrayCmd::Queue => self.queue_toggle(),
            TrayCmd::Settings => self.show_settings(),
            TrayCmd::TogglePause => self.update_settings(|s| s.paused = !s.paused),
            TrayCmd::Quit => self.app.quit(),
        }
    }

    // ---- settings ------------------------------------------------------------

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut st = self.state.borrow_mut();
        f(&mut st.settings);
        self.paused_flag
            .store(st.settings.paused, Ordering::Relaxed);
        if let Err(e) = st.settings.save(&paths::settings_path()) {
            eprintln!("clipfinch: could not save settings: {e:#}");
        }
    }

    pub fn apply_retention(&self) {
        let st = self.state.borrow();
        if let Some(ms) = st.settings.history_limit.millis() {
            let _ = st.store.purge_older_than(now_ms() - ms);
        }
        let _ = st.store.trim_history(5000);
    }

    pub fn erase_history(self: &Rc<Self>) {
        let _ = self.state.borrow().store.erase_history();
        self.refresh_ui();
    }

    // ---- paste ---------------------------------------------------------------

    /// Puts a clip on the clipboard. Returns false if it could not be loaded.
    pub fn copy_clip(&self, id: i64, plain: bool) -> bool {
        enum Payload {
            Plain(String),
            Formats(Vec<clipfinch_core::model::Format>),
        }
        let payload = {
            let st = self.state.borrow();
            let clip = match st.store.get(id) {
                Ok(Some(c)) => c,
                _ => return false,
            };
            if plain && clip.kind != Kind::Image {
                Payload::Plain(st.store.plain_text(id).unwrap_or(clip.text))
            } else {
                match st.store.formats(id) {
                    Ok(f) if !f.is_empty() => Payload::Formats(f),
                    _ => return false,
                }
            }
        };
        match payload {
            Payload::Plain(t) => clipboard::set_plain(&t),
            Payload::Formats(f) => clipboard::set_formats(&f),
        }
        true
    }

    /// Copies a clip, closes the popup and types Ctrl+V into the previous app.
    pub fn paste_clip(self: &Rc<Self>, id: i64, plain: bool) {
        if !self.copy_clip(id, plain) {
            return;
        }
        self.hide_popup();
        let _ = self.state.borrow().store.touch(id);
        self.schedule_paste(220);
    }

    fn schedule_paste(self: &Rc<Self>, delay_ms: u64) {
        let shift = self.state.borrow().settings.paste_with_shift;
        let me = self.clone();
        glib::spawn_future_local(async move {
            glib::timeout_future(Duration::from_millis(delay_ms)).await;
            if let Err(e) = inject::send_paste(shift) {
                me.hint_manual_paste(&e);
            }
        });
    }

    fn hint_manual_paste(&self, reason: &str) {
        if self.hinted.get() {
            return;
        }
        self.hinted.set(true);
        let n = gio::Notification::new("Copied to the clipboard");
        n.set_body(Some(&format!(
            "Press Ctrl+V to paste. Automatic paste is unavailable: {reason}"
        )));
        self.app.send_notification(Some("clipfinch-paste-hint"), &n);
    }

    fn notify(&self, id: &str, title: &str, body: &str) {
        let n = gio::Notification::new(title);
        n.set_body(Some(body));
        self.app.send_notification(Some(id), &n);
    }

    // ---- paste queue ---------------------------------------------------------

    pub fn queue_toggle(self: &Rc<Self>) {
        let active = {
            let mut st = self.state.borrow_mut();
            let on = !st.queue.is_active();
            st.queue.set_active(on);
            if !on {
                st.queue.clear();
            }
            on
        };
        if active {
            self.queue_window().show();
        } else {
            self.queue_window().hide();
            self.queue_window().refresh();
        }
    }

    pub fn queue_paste(self: &Rc<Self>) {
        let item = self.state.borrow_mut().queue.next();
        match item {
            Some(it) => {
                if self.copy_clip(it.clip_id, false) {
                    // Longer delay: the user may still be holding the shortcut keys.
                    self.schedule_paste(350);
                }
                self.queue_window().refresh();
            }
            None => self.notify(
                "clipfinch-queue-empty",
                "Paste queue is empty",
                "Start the queue, then copy several things.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    #[ignore = "requires an isolated display and GSETTINGS_BACKEND=memory"]
    fn system_theme_changes_update_the_running_application() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        gtk::init().unwrap();
        let provider = gtk::CssProvider::new();
        let errors = Rc::new(RefCell::new(Vec::new()));
        let collected = errors.clone();
        provider.connect_parsing_error(move |_, _, error| {
            collected.borrow_mut().push(error.to_string())
        });
        apply_css(&provider, false);
        apply_css(&provider, true);
        assert!(errors.borrow().is_empty(), "{:?}", errors.borrow());
        let system = gio::Settings::new("org.gnome.desktop.interface");
        system.set_string("color-scheme", "default").unwrap();
        load_css();
        let settings = gtk::Settings::default().unwrap();
        assert!(!settings.property::<bool>("gtk-application-prefer-dark-theme"));
        system.set_string("color-scheme", "prefer-dark").unwrap();
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(settings.property::<bool>("gtk-application-prefer-dark-theme"));
        system.set_string("color-scheme", "prefer-light").unwrap();
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(!settings.property::<bool>("gtk-application-prefer-dark-theme"));
    }

    #[test]
    fn launcher_override_preserves_absolute_paths_with_spaces() {
        assert_eq!(
            resolve_exe_path(Some(OsStr::new("/home/demo/clip finch/container.sh"))).unwrap(),
            "/home/demo/clip finch/container.sh"
        );
        assert!(std::path::Path::new(&resolve_exe_path(None).unwrap()).is_absolute());
    }

    #[test]
    fn launcher_override_rejects_relative_empty_and_control_characters() {
        for path in ["", "scripts/container.sh", "/tmp/launcher\nInjected=true"] {
            assert!(
                resolve_exe_path(Some(OsStr::new(path))).is_err(),
                "{path:?}"
            );
        }
    }
}

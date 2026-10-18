mod clipboard;
mod ctx;
mod dialogs;
mod popup;
mod queue_window;
mod settings_window;
#[cfg(feature = "tray")]
mod tray;

use std::cell::RefCell;
use std::rc::Rc;

use clipfinch_core::paths;
use clipfinch_core::settings::Settings;
use clipfinch_linux::session::{self, SessionType};
use clipfinch_linux::{autostart, inject, shortcuts};
use gtk::prelude::*;
use gtk::{gio, glib};

use ctx::Ctx;

const APP_ID: &str = "dev.clipfinch.Clipfinch";

const COMMANDS: &[(&str, &str)] = &[
    ("show", "Open the popup"),
    ("toggle", "Open or close the popup"),
    ("queue", "Start or stop the Paste Queue"),
    ("queue-paste", "Paste the next item of the Paste Queue"),
    ("settings", "Open the settings window"),
    ("quit", "Quit the running instance"),
    ("background", "Start in the background without opening anything"),
];

const HELP: &str = "\
Clipfinch - clipboard history for Linux

Usage: clipfinch [COMMAND]

Commands (sent to the running instance, which is started if needed):
  --show                  Open the popup
  --toggle                Open or close the popup
  --queue                 Start or stop the Paste Queue
  --queue-paste           Paste the next item of the Paste Queue
  --settings              Open the settings window
  --quit                  Quit the running instance
  --background            Start without opening anything

One-shot commands:
  --install-shortcuts     Register global shortcuts in GNOME
  --uninstall-shortcuts   Remove those shortcuts
  --autostart on|off      Start at login on/off
  --doctor                Print what works on this system
  --version, --help
";

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = handle_local_args(&args) {
        return code;
    }
    configure_backend();

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    for (name, desc) in COMMANDS {
        app.add_main_option(
            name,
            glib::Char::from(0u8),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            desc,
            None,
        );
    }

    let slot: Rc<RefCell<Option<Rc<Ctx>>>> = Rc::new(RefCell::new(None));

    {
        let slot = slot.clone();
        app.connect_startup(move |app| {
            // Keep running with no windows open.
            std::mem::forget(app.hold());
            ctx::load_css();
            match Ctx::new(app) {
                Ok(ctx) => {
                    ctx.start();
                    *slot.borrow_mut() = Some(ctx);
                }
                Err(e) => {
                    eprintln!("clipfinch: failed to start: {e:#}");
                    std::process::exit(1);
                }
            }
        });
    }

    {
        let slot = slot.clone();
        app.connect_local("command-line", false, move |values| {
            let cmdline = values[1]
                .get::<gio::ApplicationCommandLine>()
                .expect("command-line signal argument");
            let opts = cmdline.options_dict();
            let ctx = slot.borrow().clone();
            if let Some(ctx) = ctx {
                for (name, _) in COMMANDS {
                    if opts.contains(name) {
                        ctx.run_command(name);
                    }
                }
            }
            Some(0i32.to_value())
        });
    }

    app.run()
}

/// On a Wayland session GNOME only tells us about clipboard changes in the
/// background through XWayland, so prefer the X11 backend (falling back to
/// Wayland). Override with CLIPFINCH_BACKEND=wayland|x11.
fn configure_backend() {
    if std::env::var_os("GDK_BACKEND").is_some() {
        return;
    }
    match std::env::var("CLIPFINCH_BACKEND").ok().as_deref() {
        Some("wayland") => std::env::set_var("GDK_BACKEND", "wayland"),
        Some("x11") => std::env::set_var("GDK_BACKEND", "x11"),
        _ => {
            if session::session_type() == SessionType::Wayland
                && std::env::var_os("DISPLAY").is_some()
            {
                std::env::set_var("GDK_BACKEND", "x11,wayland");
            }
        }
    }
}

fn handle_local_args(args: &[String]) -> Option<glib::ExitCode> {
    let first = args.first()?.as_str();
    match first {
        "--version" | "-V" => {
            println!("clipfinch {}", env!("CARGO_PKG_VERSION"));
            Some(glib::ExitCode::SUCCESS)
        }
        "--help" | "-h" => {
            print!("{HELP}");
            Some(glib::ExitCode::SUCCESS)
        }
        "--install-shortcuts" => {
            let s = Settings::load(&paths::settings_path());
            let list = vec![
                shortcuts::Shortcut {
                    slug: "clipfinch-popup",
                    name: "Clipfinch: open history",
                    flag: "--toggle",
                    binding: s.popup_hotkey.clone(),
                },
                shortcuts::Shortcut {
                    slug: "clipfinch-queue",
                    name: "Clipfinch: paste queue",
                    flag: "--queue",
                    binding: s.queue_hotkey.clone(),
                },
                shortcuts::Shortcut {
                    slug: "clipfinch-queue-paste",
                    name: "Clipfinch: paste next in queue",
                    flag: "--queue-paste",
                    binding: s.queue_paste_hotkey.clone(),
                },
            ];
            Some(report(
                shortcuts::install(&ctx::exe_path(), &list),
                &format!(
                    "Shortcuts installed: {} (history), {} (queue), {} (queue paste)",
                    s.popup_hotkey, s.queue_hotkey, s.queue_paste_hotkey
                ),
            ))
        }
        "--uninstall-shortcuts" => Some(report(shortcuts::uninstall(), "Shortcuts removed")),
        "--autostart" => {
            let on = matches!(args.get(1).map(String::as_str), Some("on"));
            let off = matches!(args.get(1).map(String::as_str), Some("off"));
            if !on && !off {
                eprintln!("usage: clipfinch --autostart on|off");
                return Some(glib::ExitCode::FAILURE);
            }
            Some(report(
                autostart::set(on, &ctx::exe_path()),
                if on { "Autostart enabled" } else { "Autostart disabled" },
            ))
        }
        "--doctor" => {
            print_doctor();
            Some(glib::ExitCode::SUCCESS)
        }
        _ => None,
    }
}

fn report(result: anyhow::Result<()>, ok_message: &str) -> glib::ExitCode {
    match result {
        Ok(()) => {
            println!("{ok_message}");
            glib::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("clipfinch: {e:#}");
            glib::ExitCode::FAILURE
        }
    }
}

fn print_doctor() {
    println!("Session type      : {:?}", session::session_type());
    println!("GNOME             : {}", session::is_gnome());
    println!("DISPLAY           : {}", std::env::var("DISPLAY").unwrap_or_default());
    println!("Auto-paste        : {}", inject::describe());
    println!("gsettings         : {}", session::command_exists("gsettings"));
    println!("ydotool           : {}", session::command_exists("ydotool"));
    println!("Autostart enabled : {}", autostart::is_enabled());
    println!("Data directory    : {}", paths::data_dir().display());
    if session::session_type() == SessionType::Wayland {
        println!();
        println!("Wayland notes:");
        println!("  - the popup is placed by the compositor (not at the text cursor)");
        println!("  - source-app names and the excluded-apps list need an X11 session");
        println!("  - run `clipfinch --install-shortcuts` to get global shortcuts");
    }
}

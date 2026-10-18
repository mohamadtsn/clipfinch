use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// A tiny modal "enter some text" dialog.
pub fn prompt_text(
    parent: &gtk::Window,
    title: &str,
    initial: &str,
    on_ok: impl Fn(String) + 'static,
    on_close: impl Fn() + 'static,
) {
    let dialog = gtk::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .default_width(340)
        .resizable(false)
        .build();
    dialog.add_css_class("panel");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(14);
    content.set_margin_bottom(14);
    content.set_margin_start(14);
    content.set_margin_end(14);

    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_activates_default(true);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&ok);

    content.append(&entry);
    content.append(&buttons);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&ok));

    let on_ok = Rc::new(on_ok);
    {
        let dialog = dialog.clone();
        let entry = entry.clone();
        ok.connect_clicked(move |_| {
            on_ok(entry.text().to_string());
            dialog.close();
        });
    }
    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }
    dialog.connect_close_request(move |_| {
        on_close();
        glib::Propagation::Proceed
    });

    let esc = gtk::EventControllerKey::new();
    {
        let dialog = dialog.clone();
        esc.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                dialog.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    dialog.add_controller(esc);

    dialog.present();
    entry.grab_focus();
}

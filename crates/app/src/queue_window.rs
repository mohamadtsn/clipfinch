//! The Paste Queue window.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, pango};

use crate::ctx::Ctx;

pub struct QueueWindow {
    ctx: Rc<Ctx>,
    pub window: gtk::Window,
    list: gtk::ListBox,
    info: gtk::Label,
}

impl QueueWindow {
    pub fn new(ctx: Rc<Ctx>) -> Rc<QueueWindow> {
        let window = gtk::Window::builder()
            .title("Clipfinch – Paste Queue")
            .default_width(380)
            .default_height(440)
            .hide_on_close(true)
            .build();
        window.add_css_class("panel");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_margin_top(14);
        root.set_margin_bottom(14);
        root.set_margin_start(14);
        root.set_margin_end(14);

        let info = gtk::Label::new(None);
        info.set_wrap(true);
        info.set_xalign(0.0);
        root.append(&info);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("cards");
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        root.append(&scroller);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let reverse = gtk::Button::with_label("Reverse order");
        let clear = gtk::Button::with_label("Clear");
        let stop = gtk::Button::with_label("Stop queue");
        stop.add_css_class("destructive-action");
        buttons.append(&reverse);
        buttons.append(&clear);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        buttons.append(&spacer);
        buttons.append(&stop);
        root.append(&buttons);
        window.set_child(Some(&root));

        let this = Rc::new(QueueWindow {
            ctx,
            window,
            list,
            info,
        });

        {
            let weak = Rc::downgrade(&this);
            reverse.connect_clicked(move |_| {
                if let Some(q) = weak.upgrade() {
                    q.ctx.state.borrow_mut().queue.toggle_reverse();
                    q.refresh();
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            clear.connect_clicked(move |_| {
                if let Some(q) = weak.upgrade() {
                    q.ctx.state.borrow_mut().queue.clear();
                    q.refresh();
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            stop.connect_clicked(move |_| {
                if let Some(q) = weak.upgrade() {
                    let ctx = q.ctx.clone();
                    glib::idle_add_local_once(move || ctx.queue_toggle());
                }
            });
        }
        this
    }

    pub fn show(&self) {
        self.refresh();
        self.window.present();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn refresh(&self) {
        let (items, reversed, active, hotkey) = {
            let st = self.ctx.state.borrow();
            (
                st.queue.items().to_vec(),
                st.queue.is_reversed(),
                st.queue.is_active(),
                st.settings.queue_paste_hotkey.clone(),
            )
        };
        self.info.set_text(&if active {
            format!(
                "Queue is on – everything you copy is added. Press {hotkey} (or run \
                 `clipfinch --queue-paste`) to paste the next item. Order: {}.",
                if reversed { "newest first" } else { "oldest first" }
            )
        } else {
            "Queue is off.".to_string()
        });

        self.list.remove_all();
        let ordered: Vec<_> = if reversed {
            items.iter().rev().collect()
        } else {
            items.iter().collect()
        };
        for (n, item) in ordered.iter().enumerate() {
            let label = gtk::Label::new(Some(&format!("{}. {}", n + 1, item.label)));
            label.set_xalign(0.0);
            label.set_ellipsize(pango::EllipsizeMode::End);
            label.set_margin_top(4);
            label.set_margin_bottom(4);
            label.set_margin_start(6);
            if n == 0 {
                label.add_css_class("next");
            }
            self.list.append(&label);
        }
    }
}

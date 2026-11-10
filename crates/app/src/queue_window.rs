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
            .default_width(460)
            .default_height(440)
            .hide_on_close(true)
            .build();
        window.add_css_class("panel");
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&gtk::Label::new(Some("Paste queue"))));
        window.set_titlebar(Some(&header));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("queue-content");

        let info = gtk::Label::new(None);
        info.set_wrap(true);
        info.set_xalign(0.0);
        info.add_css_class("queue-summary");
        root.append(&info);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("queue-list");
        let empty = gtk::Label::new(Some("Copy something to add it to the queue."));
        empty.set_wrap(true);
        empty.add_css_class("placeholder");
        list.set_placeholder(Some(&empty));
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        root.append(&scroller);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.add_css_class("queue-actions");
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
            let hotkey = gtk::accelerator_parse(&hotkey)
                .map(|(key, modifiers)| gtk::accelerator_get_label(key, modifiers).to_string())
                .unwrap_or(hotkey);
            format!(
                "Copy to collect · {hotkey} pastes next\n{} · {} items",
                if reversed {
                    "newest first"
                } else {
                    "oldest first"
                },
                items.len()
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
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.add_css_class("queue-row");
            let order = gtk::Label::new(Some(&format!("{:02}", n + 1)));
            order.add_css_class("queue-order");
            row.append(&order);
            let label = gtk::Label::new(Some(&item.label));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.set_ellipsize(pango::EllipsizeMode::End);
            row.append(&label);
            if n == 0 {
                row.add_css_class("next");
                let next = gtk::Label::new(Some("Next"));
                next.add_css_class("secondary");
                row.append(&next);
            }
            self.list.append(&row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(widget: &gtk::Widget, name: &str) -> bool {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            if button.label().as_deref() == Some(name) {
                button.emit_clicked();
                return true;
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            if click(&widget, name) {
                return true;
            }
            child = widget.next_sibling();
        }
        false
    }

    #[test]
    #[ignore = "requires a private display and CLIPFINCH_PRIVATE_TEST=1"]
    fn queue_controls_preserve_order_and_active_state() {
        assert_eq!(std::env::var("CLIPFINCH_PRIVATE_TEST").as_deref(), Ok("1"));
        gtk::init().unwrap();
        let dir = std::env::temp_dir().join(format!("clipfinch-queue-test-{}", std::process::id()));
        let old_data = std::env::var_os("CLIPFINCH_DATA_DIR");
        std::env::set_var("CLIPFINCH_DATA_DIR", &dir);
        let app = gtk::Application::builder()
            .application_id("dev.clipfinch.QueueTest")
            .build();
        let ctx = Ctx::new(&app).unwrap();
        {
            let mut st = ctx.state.borrow_mut();
            st.queue.set_active(true);
            st.queue.push(1, "First synthetic clip".into());
            st.queue.push(2, "Second synthetic clip".into());
        }
        let queue = QueueWindow::new(ctx.clone());
        queue.refresh();
        assert!(queue.info.text().contains("2 items"));
        assert!(click(queue.window.upcast_ref(), "Reverse order"));
        assert!(ctx.state.borrow().queue.is_reversed());
        let next_row = queue.list.row_at_index(0).unwrap().child().unwrap();
        let label = next_row
            .first_child()
            .unwrap()
            .next_sibling()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        assert_eq!(label.text(), "Second synthetic clip");
        assert!(click(queue.window.upcast_ref(), "Clear"));
        assert!(ctx.state.borrow().queue.items().is_empty());
        assert!(ctx.state.borrow().queue.is_active());
        assert!(queue.list.row_at_index(0).is_none());
        assert!(click(queue.window.upcast_ref(), "Stop queue"));
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(!ctx.state.borrow().queue.is_active());
        match old_data {
            Some(value) => std::env::set_var("CLIPFINCH_DATA_DIR", value),
            None => std::env::remove_var("CLIPFINCH_DATA_DIR"),
        }
        drop(queue);
        drop(ctx);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

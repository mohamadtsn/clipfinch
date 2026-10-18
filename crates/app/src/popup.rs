//! The popup window: tabs, search, clip cards, preview and keyboard handling.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use clipfinch_core::classify;
use clipfinch_core::model::{Clip, Kind, Pinboard};
use clipfinch_core::search;
use clipfinch_core::store::{now_ms, Scope};
use clipfinch_core::text;
use clipfinch_linux::session::{session_type, SessionType};
use clipfinch_linux::placement;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};

use crate::ctx::Ctx;
use crate::dialogs;

const WIDTH: i32 = 460;
const HEIGHT: i32 = 560;
const HINT_LIST: &str =
    "Enter paste · Shift+Enter plain text · Space preview · Ctrl+1…9 quick paste · Esc close";
const HINT_PREVIEW: &str = "Space or Esc back · Enter paste";

#[derive(Default)]
struct Inner {
    tab: usize,
    pinboards: Vec<Pinboard>,
    rows: Vec<Clip>,
    previewing: bool,
}

pub struct Popup {
    ctx: Rc<Ctx>,
    pub window: gtk::Window,
    search: gtk::SearchEntry,
    tabs: gtk::Box,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    stack: gtk::Stack,
    preview_scroll: gtk::ScrolledWindow,
    preview_text: gtk::TextView,
    preview_image: gtk::Picture,
    status: gtk::Label,
    inner: RefCell<Inner>,
    was_active: Cell<bool>,
    suspend_autohide: Cell<bool>,
}

fn add_item(
    col: &gtk::Box,
    pop: &gtk::Popover,
    weak: &Weak<Popup>,
    label: &str,
    action: impl Fn(&Rc<Popup>) + 'static,
) {
    let b = gtk::Button::with_label(label);
    b.add_css_class("flat");
    if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
        l.set_xalign(0.0);
    }
    let weak = weak.clone();
    let pop = pop.clone();
    b.connect_clicked(move |_| {
        pop.popdown();
        if let Some(p) = weak.upgrade() {
            action(&p);
        }
    });
    col.append(&b);
}

fn body_label(s: &str, lines: i32) -> gtk::Label {
    let l = gtk::Label::new(Some(&text::truncate(s, 400)));
    l.add_css_class("body");
    l.set_xalign(0.0);
    l.set_hexpand(true);
    l.set_wrap(true);
    l.set_wrap_mode(pango::WrapMode::WordChar);
    l.set_lines(lines);
    l.set_ellipsize(pango::EllipsizeMode::End);
    l.set_max_width_chars(48);
    l
}

impl Popup {
    pub fn new(ctx: Rc<Ctx>) -> Rc<Popup> {
        let window = gtk::Window::builder()
            .title("Clipfinch")
            .default_width(WIDTH)
            .default_height(HEIGHT)
            .decorated(false)
            .resizable(false)
            .hide_on_close(true)
            .build();
        window.add_css_class("popup");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // Header: tabs + settings button (draggable).
        let handle = gtk::WindowHandle::new();
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("header");
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let tabs_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .child(&tabs)
            .build();
        let gear = gtk::Button::from_icon_name("emblem-system-symbolic");
        gear.add_css_class("flat");
        gear.set_focus_on_click(false);
        header.append(&tabs_scroll);
        header.append(&gear);
        handle.set_child(Some(&header));
        root.append(&handle);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search · try: mail link slides"));
        root.append(&search);

        // List page.
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.set_focusable(false);
        list.add_css_class("cards");
        let empty = gtk::Label::new(Some("Nothing here yet"));
        empty.add_css_class("placeholder");
        list.set_placeholder(Some(&empty));
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        scroller.set_focusable(false);

        // Preview page.
        let preview_text = gtk::TextView::new();
        preview_text.set_editable(false);
        preview_text.set_cursor_visible(false);
        preview_text.set_wrap_mode(gtk::WrapMode::WordChar);
        preview_text.add_css_class("preview");
        preview_text.set_focusable(false);
        let preview_scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .child(&preview_text)
            .build();
        let preview_image = gtk::Picture::new();
        preview_image.set_vexpand(true);
        preview_image.set_visible(false);
        let preview_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        preview_box.add_css_class("preview-box");
        preview_box.append(&preview_scroll);
        preview_box.append(&preview_image);

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(&scroller, Some("list"));
        stack.add_named(&preview_box, Some("preview"));
        root.append(&stack);

        let status = gtk::Label::new(Some(HINT_LIST));
        status.add_css_class("hint");
        status.set_xalign(0.0);
        root.append(&status);

        window.set_child(Some(&root));

        let popup = Rc::new(Popup {
            ctx,
            window,
            search,
            tabs,
            list,
            scroller,
            stack,
            preview_scroll,
            preview_text,
            preview_image,
            status,
            inner: RefCell::new(Inner::default()),
            was_active: Cell::new(false),
            suspend_autohide: Cell::new(false),
        });
        popup.wire(&gear);
        popup
    }

    fn wire(self: &Rc<Self>, gear: &gtk::Button) {
        let weak = Rc::downgrade(self);

        {
            let weak = weak.clone();
            self.search.connect_search_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.refresh_list();
                }
            });
        }
        {
            let weak = weak.clone();
            self.list.connect_row_activated(move |_, row| {
                if let Some(p) = weak.upgrade() {
                    p.paste_index(row.index() as usize, false);
                }
            });
        }
        {
            let weak = weak.clone();
            gear.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.ctx.hide_popup();
                    p.ctx.show_settings();
                }
            });
        }

        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let weak = weak.clone();
            key.connect_key_pressed(move |_, keyval, _, state| match weak.upgrade() {
                Some(p) => p.on_key(keyval, state),
                None => glib::Propagation::Proceed,
            });
        }
        {
            let weak = weak.clone();
            key.connect_key_released(move |_, keyval, _, _| {
                if keyval == gdk::Key::Control_L || keyval == gdk::Key::Control_R {
                    if let Some(p) = weak.upgrade() {
                        p.window.remove_css_class("hold-ctrl");
                    }
                }
            });
        }
        self.window.add_controller(key);

        {
            let weak = weak.clone();
            self.window.connect_is_active_notify(move |w| {
                let Some(p) = weak.upgrade() else { return };
                if w.is_active() {
                    p.was_active.set(true);
                } else {
                    w.remove_css_class("hold-ctrl");
                    if p.was_active.get() && !p.suspend_autohide.get() {
                        p.hide();
                    }
                }
            });
        }
    }

    // ---- show / hide ---------------------------------------------------------

    pub fn show(self: &Rc<Self>) {
        self.search.set_text("");
        {
            let mut i = self.inner.borrow_mut();
            i.tab = 0;
            i.previewing = false;
        }
        self.stack.set_visible_child_name("list");
        self.status.set_text(HINT_LIST);
        self.refresh();
        self.was_active.set(false);
        self.place();
        self.window.present();
        self.search.grab_focus();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
        self.window.remove_css_class("hold-ctrl");
        self.was_active.set(false);
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// Opens near the mouse pointer on X11 sessions. Wayland does not let apps
    /// position windows, so there the compositor decides.
    fn place(&self) {
        if session_type() != SessionType::X11 {
            return;
        }
        let Some((px, py)) = placement::pointer_position() else {
            return;
        };
        let (sw, sh) = placement::screen_size().unwrap_or((1920, 1080));
        let x = (px + 8).min(sw - WIDTH).max(0);
        let y = (py + 16).min(sh - HEIGHT).max(0);
        gtk::prelude::WidgetExt::realize(&self.window);
        if let Some(surface) = self.window.surface() {
            if let Ok(x11) = surface.downcast::<gdk4_x11::X11Surface>() {
                let _ = placement::move_window(x11.xid() as u32, x, y);
            }
        }
    }

    // ---- data ----------------------------------------------------------------

    pub fn refresh(self: &Rc<Self>) {
        self.reload_pinboards();
        self.rebuild_tabs();
        self.refresh_list();
    }

    fn reload_pinboards(&self) {
        let pbs = self
            .ctx
            .state
            .borrow()
            .store
            .pinboards()
            .unwrap_or_default();
        let mut i = self.inner.borrow_mut();
        if i.tab > pbs.len() {
            i.tab = 0;
        }
        i.pinboards = pbs;
    }

    fn refresh_list(self: &Rc<Self>) {
        let (tab, pinboards) = {
            let i = self.inner.borrow();
            (i.tab, i.pinboards.clone())
        };
        let scope = if tab == 0 {
            Scope::History
        } else {
            match pinboards.get(tab - 1) {
                Some(p) => Scope::Pinboard(p.id),
                None => Scope::History,
            }
        };
        let clips = self
            .ctx
            .state
            .borrow()
            .store
            .list(scope, 1000)
            .unwrap_or_default();
        let query = search::parse(&self.search.text());
        let rows: Vec<Clip> = clips
            .into_iter()
            .filter(|c| {
                query.matches(c.kind, &c.text, c.source_app.as_deref(), c.title.as_deref())
            })
            .take(200)
            .collect();

        self.list.remove_all();
        for (idx, clip) in rows.iter().enumerate() {
            let row = self.build_card(idx, clip, &pinboards);
            self.list.append(&row);
        }
        self.inner.borrow_mut().rows = rows;
        self.scroller.vadjustment().set_value(0.0);
        self.select_index(0);
    }

    fn clip_at(&self, idx: usize) -> Option<Clip> {
        self.inner.borrow().rows.get(idx).cloned()
    }

    fn selected_index(&self) -> Option<usize> {
        self.list.selected_row().map(|r| r.index() as usize)
    }

    fn selected_clip(&self) -> Option<Clip> {
        self.selected_index().and_then(|i| self.clip_at(i))
    }

    fn select_index(&self, idx: usize) {
        if let Some(row) = self.list.row_at_index(idx as i32) {
            self.list.select_row(Some(&row));
            self.ensure_visible(&row);
        }
    }

    fn move_selection(&self, delta: i32) {
        let n = self.inner.borrow().rows.len() as i32;
        if n == 0 {
            return;
        }
        let cur = self.selected_index().map(|i| i as i32).unwrap_or(-1);
        self.select_index((cur + delta).clamp(0, n - 1) as usize);
    }

    fn ensure_visible(&self, row: &gtk::ListBoxRow) {
        let adj = self.scroller.vadjustment();
        if let Some(p) = row.compute_point(&self.list, &gtk::graphene::Point::new(0.0, 0.0)) {
            let top = p.y() as f64;
            let bottom = top + row.height() as f64;
            if top < adj.value() {
                adj.set_value(top);
            } else if bottom > adj.value() + adj.page_size() {
                adj.set_value(bottom - adj.page_size());
            }
        }
    }

    // ---- tabs ----------------------------------------------------------------

    fn rebuild_tabs(self: &Rc<Self>) {
        while let Some(child) = self.tabs.first_child() {
            self.tabs.remove(&child);
        }
        let (tab, pinboards) = {
            let i = self.inner.borrow();
            (i.tab, i.pinboards.clone())
        };
        self.add_tab("Clipboard", 0, tab == 0, None);
        for (i, p) in pinboards.iter().enumerate() {
            self.add_tab(&p.name, i + 1, tab == i + 1, Some(p.clone()));
        }
        let plus = gtk::Button::with_label("+");
        plus.add_css_class("tab");
        plus.set_focus_on_click(false);
        plus.set_tooltip_text(Some("New pinboard (Ctrl+N)"));
        let weak = Rc::downgrade(self);
        plus.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.new_pinboard();
            }
        });
        self.tabs.append(&plus);
    }

    fn add_tab(self: &Rc<Self>, label: &str, index: usize, active: bool, pinboard: Option<Pinboard>) {
        let btn = gtk::Button::with_label(label);
        btn.add_css_class("tab");
        btn.set_focus_on_click(false);
        if active {
            btn.add_css_class("active");
        }
        let weak = Rc::downgrade(self);

        {
            let weak = weak.clone();
            btn.connect_clicked(move |_| {
                let weak = weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(p) = weak.upgrade() {
                        p.set_tab(index);
                    }
                });
            });
        }

        // Drop a card on a tab to move it there ("Clipboard" tab = unpin).
        let target_pb = pinboard.as_ref().map(|p| p.id);
        let target = gtk::DropTarget::new(glib::Type::I64, gdk::DragAction::COPY);
        {
            let weak = weak.clone();
            target.connect_drop(move |_, value, _, _| {
                let Ok(id) = value.get::<i64>() else {
                    return false;
                };
                let weak = weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(p) = weak.upgrade() {
                        let _ = p.ctx.state.borrow().store.pin(id, target_pb);
                        p.refresh_list();
                    }
                });
                true
            });
        }
        {
            let b = btn.clone();
            target.connect_enter(move |_, _, _| {
                b.add_css_class("drop-hover");
                gdk::DragAction::COPY
            });
        }
        {
            let b = btn.clone();
            target.connect_leave(move |_| b.remove_css_class("drop-hover"));
        }
        btn.add_controller(target);

        // Right click on a pinboard tab: rename / delete.
        if let Some(pb) = pinboard {
            let pop = gtk::Popover::new();
            let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let rename_pb = pb.clone();
            add_item(&col, &pop, &weak, "Rename…", move |p| p.rename_pinboard(&rename_pb));
            let delete_pb = pb.clone();
            add_item(&col, &pop, &weak, "Delete pinboard", move |p| {
                p.delete_pinboard(&delete_pb)
            });
            pop.set_child(Some(&col));
            pop.set_parent(&btn);
            {
                let pop = pop.clone();
                btn.connect_destroy(move |_| pop.unparent());
            }
            let right = gtk::GestureClick::new();
            right.set_button(3);
            right.connect_pressed(move |_, _, _, _| pop.popup());
            btn.add_controller(right);
        }

        self.tabs.append(&btn);
    }

    fn set_tab(self: &Rc<Self>, tab: usize) {
        self.inner.borrow_mut().tab = tab;
        self.rebuild_tabs();
        self.refresh_list();
        self.search.grab_focus();
    }

    fn switch_tab(self: &Rc<Self>, delta: i32) {
        let n = (self.inner.borrow().pinboards.len() + 1) as i32;
        let cur = self.inner.borrow().tab as i32;
        self.set_tab((cur + delta).rem_euclid(n) as usize);
    }

    // ---- dialogs -------------------------------------------------------------

    fn ask(
        self: &Rc<Self>,
        title: &str,
        initial: &str,
        on_ok: impl Fn(&Rc<Popup>, String) + 'static,
    ) {
        self.suspend_autohide.set(true);
        let ok_weak = Rc::downgrade(self);
        let close_weak = Rc::downgrade(self);
        dialogs::prompt_text(
            &self.window,
            title,
            initial,
            move |text| {
                if let Some(p) = ok_weak.upgrade() {
                    on_ok(&p, text);
                }
            },
            move || {
                if let Some(p) = close_weak.upgrade() {
                    p.suspend_autohide.set(false);
                    p.window.present();
                    p.search.grab_focus();
                }
            },
        );
    }

    fn new_pinboard(self: &Rc<Self>) {
        self.ask("New pinboard", "", |p, name| {
            let name = name.trim().to_string();
            if name.is_empty() {
                return;
            }
            let created = p.ctx.state.borrow().store.create_pinboard(&name);
            if created.is_ok() {
                p.reload_pinboards();
                let idx = p.inner.borrow().pinboards.len();
                p.set_tab(idx);
            }
        });
    }

    fn rename_pinboard(self: &Rc<Self>, pb: &Pinboard) {
        let id = pb.id;
        self.ask("Rename pinboard", &pb.name, move |p, name| {
            let name = name.trim().to_string();
            if name.is_empty() {
                return;
            }
            let _ = p.ctx.state.borrow().store.rename_pinboard(id, &name);
            p.refresh();
        });
    }

    fn delete_pinboard(self: &Rc<Self>, pb: &Pinboard) {
        let _ = self.ctx.state.borrow().store.delete_pinboard(pb.id);
        self.inner.borrow_mut().tab = 0;
        self.refresh();
    }

    fn rename_clip(self: &Rc<Self>, clip: &Clip) {
        let id = clip.id;
        self.ask(
            "Rename clip",
            clip.title.as_deref().unwrap_or(""),
            move |p, text| {
                let t = text.trim();
                let title = if t.is_empty() { None } else { Some(t) };
                let _ = p.ctx.state.borrow().store.rename(id, title);
                p.refresh_list();
            },
        );
    }

    // ---- clip actions --------------------------------------------------------

    fn paste_index(self: &Rc<Self>, idx: usize, plain: bool) {
        if let Some(c) = self.clip_at(idx) {
            self.ctx.paste_clip(c.id, plain);
        }
    }

    fn copy_clip(self: &Rc<Self>, clip: &Clip) {
        self.ctx.copy_clip(clip.id, false);
        self.ctx.hide_popup();
    }

    fn delete_clip(self: &Rc<Self>, clip: &Clip) {
        let _ = self.ctx.state.borrow().store.delete(clip.id);
        self.refresh_list();
    }

    fn pin_clip(self: &Rc<Self>, clip: &Clip, pinboard: Option<i64>) {
        let _ = self.ctx.state.borrow().store.pin(clip.id, pinboard);
        self.refresh_list();
    }

    fn open_clip(self: &Rc<Self>, clip: &Clip) {
        let uri = match clip.kind {
            Kind::Link => Some(clip.text.trim().to_string()),
            Kind::File => clip
                .text
                .lines()
                .next()
                .map(|p| gio::File::for_path(p).uri().to_string()),
            _ => None,
        };
        if let Some(uri) = uri {
            let _ = gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE);
            self.ctx.hide_popup();
        }
    }

    fn open_preview(self: &Rc<Self>, clip: &Clip) {
        if clip.kind == Kind::Image {
            let formats = self
                .ctx
                .state
                .borrow()
                .store
                .formats(clip.id)
                .unwrap_or_default();
            let texture = classify::best_image(&formats)
                .and_then(|f| gdk::Texture::from_bytes(&glib::Bytes::from(&f.data[..])).ok());
            match texture {
                Some(t) => {
                    self.preview_image.set_paintable(Some(&t));
                    self.preview_image.set_visible(true);
                    self.preview_scroll.set_visible(false);
                }
                None => {
                    self.preview_text.buffer().set_text(&clip.text);
                    self.preview_image.set_visible(false);
                    self.preview_scroll.set_visible(true);
                }
            }
        } else {
            let full = self
                .ctx
                .state
                .borrow()
                .store
                .plain_text(clip.id)
                .unwrap_or_else(|_| clip.text.clone());
            self.preview_text
                .buffer()
                .set_text(&text::take_chars(&full, 200_000));
            self.preview_image.set_visible(false);
            self.preview_scroll.set_visible(true);
        }
        self.stack.set_visible_child_name("preview");
        self.status.set_text(HINT_PREVIEW);
        self.inner.borrow_mut().previewing = true;
    }

    fn close_preview(&self) {
        self.stack.set_visible_child_name("list");
        self.status.set_text(HINT_LIST);
        self.inner.borrow_mut().previewing = false;
    }

    // ---- keyboard ------------------------------------------------------------

    fn on_key(self: &Rc<Self>, keyval: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        use glib::Propagation::{Proceed, Stop};

        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let previewing = self.inner.borrow().previewing;
        let search_empty = self.search.text().is_empty();
        let is = |k: gdk::Key| keyval == k;
        let is_enter = is(gdk::Key::Return) || is(gdk::Key::KP_Enter);

        if is(gdk::Key::Control_L) || is(gdk::Key::Control_R) {
            self.window.add_css_class("hold-ctrl");
            return Proceed;
        }

        if previewing {
            if is(gdk::Key::Escape) || is(gdk::Key::space) {
                self.close_preview();
                return Stop;
            }
            if is_enter {
                self.close_preview();
                if let Some(i) = self.selected_index() {
                    self.paste_index(i, shift);
                }
                return Stop;
            }
            return Proceed;
        }

        if is(gdk::Key::Escape) {
            if search_empty {
                self.ctx.hide_popup();
            } else {
                self.search.set_text("");
            }
            return Stop;
        }
        if is(gdk::Key::Up) {
            if ctrl {
                self.select_index(0);
            } else {
                self.move_selection(-1);
            }
            return Stop;
        }
        if is(gdk::Key::Down) {
            if ctrl {
                let last = self.inner.borrow().rows.len().saturating_sub(1);
                self.select_index(last);
            } else {
                self.move_selection(1);
            }
            return Stop;
        }
        if is(gdk::Key::Page_Up) {
            self.move_selection(-5);
            return Stop;
        }
        if is(gdk::Key::Page_Down) {
            self.move_selection(5);
            return Stop;
        }
        if is_enter {
            if let Some(i) = self.selected_index() {
                self.paste_index(i, shift);
            }
            return Stop;
        }
        if is(gdk::Key::Tab) {
            self.switch_tab(1);
            return Stop;
        }
        if is(gdk::Key::ISO_Left_Tab) {
            self.switch_tab(-1);
            return Stop;
        }
        if search_empty && !ctrl && is(gdk::Key::Left) {
            self.switch_tab(-1);
            return Stop;
        }
        if search_empty && !ctrl && is(gdk::Key::Right) {
            self.switch_tab(1);
            return Stop;
        }
        if search_empty && !ctrl && is(gdk::Key::space) {
            if let Some(c) = self.selected_clip() {
                self.open_preview(&c);
            }
            return Stop;
        }
        if ctrl && is(gdk::Key::BackSpace) {
            if let Some(c) = self.selected_clip() {
                if self.inner.borrow().tab != 0 {
                    self.pin_clip(&c, None); // remove from the pinboard
                } else {
                    self.delete_clip(&c);
                }
            }
            return Stop;
        }

        if ctrl {
            if let Some(ch) = keyval.to_unicode().map(|c| c.to_ascii_lowercase()) {
                match ch {
                    '1'..='9' => {
                        self.paste_index(ch as usize - '1' as usize, false);
                        return Stop;
                    }
                    'c' if self.search.selection_bounds().is_none() => {
                        if let Some(c) = self.selected_clip() {
                            self.copy_clip(&c);
                        }
                        return Stop;
                    }
                    'o' => {
                        if let Some(c) = self.selected_clip() {
                            self.open_clip(&c);
                        }
                        return Stop;
                    }
                    'r' => {
                        if let Some(c) = self.selected_clip() {
                            self.rename_clip(&c);
                        }
                        return Stop;
                    }
                    'n' => {
                        self.new_pinboard();
                        return Stop;
                    }
                    ',' => {
                        self.ctx.hide_popup();
                        self.ctx.show_settings();
                        return Stop;
                    }
                    _ => {}
                }
            }
        }
        Proceed
    }

    // ---- cards ---------------------------------------------------------------

    fn build_card(self: &Rc<Self>, idx: usize, clip: &Clip, pinboards: &[Pinboard]) -> gtk::ListBoxRow {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class("card");

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let kind = gtk::Label::new(Some(clip.kind.label()));
        kind.add_css_class("kind");
        head.append(&kind);
        if let Some(app) = &clip.source_app {
            let l = gtk::Label::new(Some(app));
            l.add_css_class("app");
            l.set_ellipsize(pango::EllipsizeMode::End);
            head.append(&l);
        }
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        head.append(&spacer);
        let time = gtk::Label::new(Some(&text::relative_time(now_ms(), clip.updated_at)));
        time.add_css_class("time");
        head.append(&time);
        if idx < 9 {
            let badge = gtk::Label::new(Some(&format!("Ctrl+{}", idx + 1)));
            badge.add_css_class("badge");
            head.append(&badge);
        }
        let more = self.build_more_menu(clip, pinboards);
        head.append(&more);
        card.append(&head);

        if let Some(t) = &clip.title {
            let l = gtk::Label::new(Some(t));
            l.add_css_class("title");
            l.set_xalign(0.0);
            l.set_ellipsize(pango::EllipsizeMode::End);
            card.append(&l);
        }

        match clip.kind {
            Kind::Text | Kind::Link => card.append(&body_label(&clip.text, 3)),
            Kind::Image => {
                let tex = clip
                    .thumb
                    .as_ref()
                    .and_then(|b| gdk::Texture::from_bytes(&glib::Bytes::from(&b[..])).ok());
                if let Some(tex) = tex {
                    let pic = gtk::Picture::new();
                    pic.set_paintable(Some(&tex));
                    pic.set_halign(gtk::Align::Start);
                    pic.set_content_fit(gtk::ContentFit::ScaleDown);
                    card.append(&pic);
                }
                card.append(&body_label(&clip.text, 1));
            }
            Kind::File => {
                let total = clip.text.lines().count();
                let mut names: Vec<String> = clip
                    .text
                    .lines()
                    .take(4)
                    .map(|p| text::file_name(p).to_string())
                    .collect();
                if total > 4 {
                    names.push(format!("… and {} more", total - 4));
                }
                card.append(&body_label(&names.join("\n"), 5));
            }
            Kind::Color => {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                if let Some((r, g, b, a)) = text::parse_color(&clip.text) {
                    let bytes = glib::Bytes::from(&[r, g, b, a][..]);
                    let tex = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &bytes, 4);
                    let swatch = gtk::Picture::new();
                    swatch.set_paintable(Some(&tex));
                    swatch.set_content_fit(gtk::ContentFit::Fill);
                    swatch.set_size_request(36, 36);
                    swatch.add_css_class("swatch");
                    row.append(&swatch);
                }
                row.append(&body_label(&clip.text, 1));
                card.append(&row);
            }
        }

        // Right click opens the same menu as the "⋯" button.
        let right = gtk::GestureClick::new();
        right.set_button(3);
        {
            let more = more.clone();
            right.connect_pressed(move |_, _, _, _| more.popup());
        }
        card.add_controller(right);

        // Drag onto a tab to pin.
        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::COPY);
        let id = clip.id;
        drag.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&id.to_value())));
        card.add_controller(drag);

        let row = gtk::ListBoxRow::new();
        row.set_focusable(false);
        row.set_child(Some(&card));
        row
    }

    fn build_more_menu(self: &Rc<Self>, clip: &Clip, pinboards: &[Pinboard]) -> gtk::MenuButton {
        let weak = Rc::downgrade(self);
        let pop = gtk::Popover::new();
        let col = gtk::Box::new(gtk::Orientation::Vertical, 2);

        let c = clip.clone();
        add_item(&col, &pop, &weak, "Paste", move |p| p.ctx.paste_clip(c.id, false));
        if clip.kind != Kind::Image {
            let c = clip.clone();
            add_item(&col, &pop, &weak, "Paste as plain text", move |p| {
                p.ctx.paste_clip(c.id, true)
            });
        }
        let c = clip.clone();
        add_item(&col, &pop, &weak, "Copy", move |p| p.copy_clip(&c));
        if matches!(clip.kind, Kind::Link | Kind::File) {
            let c = clip.clone();
            add_item(&col, &pop, &weak, "Open", move |p| p.open_clip(&c));
        }
        let c = clip.clone();
        add_item(&col, &pop, &weak, "Rename…", move |p| p.rename_clip(&c));
        for pb in pinboards {
            if clip.pinboard_id == Some(pb.id) {
                continue;
            }
            let c = clip.clone();
            let pid = pb.id;
            add_item(&col, &pop, &weak, &format!("Pin to “{}”", pb.name), move |p| {
                p.pin_clip(&c, Some(pid))
            });
        }
        if clip.pinboard_id.is_some() {
            let c = clip.clone();
            add_item(&col, &pop, &weak, "Remove from pinboard", move |p| {
                p.pin_clip(&c, None)
            });
        }
        let c = clip.clone();
        add_item(&col, &pop, &weak, "Delete", move |p| p.delete_clip(&c));

        pop.set_child(Some(&col));
        let mb = gtk::MenuButton::new();
        mb.set_label("⋯");
        mb.add_css_class("more");
        mb.set_focus_on_click(false);
        mb.set_popover(Some(&pop));
        mb
    }
}

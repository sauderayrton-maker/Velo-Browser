use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use glib::clone;

#[derive(Clone)]
pub struct BookmarksPanel {
    pub window: libadwaita::Window,
    list: gtk4::ListBox,
    navigate: Rc<dyn Fn(String)>,
}

impl BookmarksPanel {
    pub fn build(
        parent: &libadwaita::ApplicationWindow,
        navigate: Rc<dyn Fn(String)>,
    ) -> Self {
        let window = libadwaita::Window::builder()
            .title("Bookmarks")
            .default_width(420)
            .default_height(580)
            .transient_for(parent)
            .css_classes(vec!["panel-window"])
            .build();

        let header = libadwaita::HeaderBar::builder()
            .show_start_title_buttons(false)
            .show_end_title_buttons(true)
            .css_classes(vec!["panel-bar"])
            .build();

        let title_lbl = gtk4::Label::builder()
            .label("BOOKMARKS")
            .css_classes(vec!["panel-title"])
            .build();
        header.set_title_widget(Some(&title_lbl));

        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["panel-list"])
            .build();

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&list)
            .vexpand(true)
            .build();

        let toolbar = libadwaita::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&scrolled));
        window.set_content(Some(&toolbar));

        let key_ctl = gtk4::EventControllerKey::new();
        window.add_controller(key_ctl.clone());
        key_ctl.connect_key_pressed(clone!(
            #[weak] window,
            #[upgrade_or] glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    window.set_visible(false);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        ));

        BookmarksPanel { window, list, navigate }
    }

    pub fn is_open(&self) -> bool { self.window.is_visible() }

    pub fn show(&self) {
        self.window.present();
        self.refresh();
    }

    pub fn hide(&self) { self.window.set_visible(false); }

    #[allow(dead_code)]
    pub fn toggle(&self) {
        if self.is_open() { self.hide(); } else { self.show(); }
    }

    fn refresh(&self) {
        let list = self.list.clone();
        let navigate = Rc::clone(&self.navigate);
        let window = self.window.clone();

        crate::backend::fetch_bookmarks(move |bookmarks| {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            for bm in bookmarks {
                let nav = Rc::clone(&navigate);
                let win = window.clone();
                let url_nav = bm.url.clone();
                let row = make_row(&bm.url, &bm.title, bm.id, &list);
                row.connect_activate(move |_| {
                    nav(url_nav.clone());
                    win.set_visible(false);
                });
                list.append(&row);
            }
        });
    }
}

fn make_row(url: &str, title: &str, id: Option<i64>, list: &gtk4::ListBox) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::builder()
        .css_classes(vec!["panel-row"])
        .activatable(true)
        .build();

    let hbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .margin_start(14)
        .margin_end(8)
        .margin_top(9)
        .margin_bottom(9)
        .spacing(8)
        .build();

    let body = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(3)
        .hexpand(true)
        .build();

    let display_title = if title.is_empty() { url } else { title };

    let title_lbl = gtk4::Label::builder()
        .label(display_title)
        .css_classes(vec!["row-title"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(38)
        .build();

    let url_lbl = gtk4::Label::builder()
        .label(url)
        .css_classes(vec!["row-meta"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(38)
        .build();

    body.append(&title_lbl);
    body.append(&url_lbl);

    let del_btn = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .css_classes(vec!["flat", "row-delete-btn"])
        .valign(gtk4::Align::Center)
        .build();

    if let Some(bm_id) = id {
        del_btn.connect_clicked(clone!(#[weak] list, #[weak] row, move |_| {
            crate::backend::remove_bookmark(bm_id);
            list.remove(&row);
        }));
    } else {
        del_btn.set_sensitive(false);
    }

    hbox.append(&body);
    hbox.append(&del_btn);
    row.set_child(Some(&hbox));
    row
}

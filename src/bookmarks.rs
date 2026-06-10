use std::rc::Rc;
use gtk4::prelude::*;
use glib::clone;

#[derive(Clone)]
pub struct BookmarksPanel {
    pub root: gtk4::Box,
    list: gtk4::ListBox,
    navigate: Rc<dyn Fn(String)>,
    close: Rc<dyn Fn()>,
}

impl BookmarksPanel {
    pub fn build(navigate: Rc<dyn Fn(String)>, close: Rc<dyn Fn()>) -> Self {
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

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        root.append(&scrolled);

        BookmarksPanel { root, list, navigate, close }
    }

    pub fn refresh(&self) {
        let list = self.list.clone();
        let navigate = Rc::clone(&self.navigate);
        let close = Rc::clone(&self.close);

        crate::backend::fetch_bookmarks(move |bookmarks| {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            for bm in bookmarks {
                let (row, body) = make_row(&bm.url, &bm.title, bm.id, &list);
                let url = bm.url.clone();

                // Keyboard activation (Enter/Space on a focused row)
                let nav = Rc::clone(&navigate);
                let cl = Rc::clone(&close);
                let u = url.clone();
                row.connect_activate(move |_| {
                    nav(u.clone());
                    cl();
                });

                // Mouse click on the row body — ListBoxRow's "activate" signal
                // doesn't fire on click. Attached to `body` (not the whole row)
                // so it doesn't fight with the delete button.
                let nav = Rc::clone(&navigate);
                let cl = Rc::clone(&close);
                let click = gtk4::GestureClick::new();
                click.connect_released(move |_, _, _, _| {
                    nav(url.clone());
                    cl();
                });
                body.add_controller(click);

                list.append(&row);
            }
        });
    }
}

fn make_row(url: &str, title: &str, id: Option<i64>, list: &gtk4::ListBox) -> (gtk4::ListBoxRow, gtk4::Box) {
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
    (row, body)
}

use std::rc::Rc;
use gtk4::prelude::*;
use glib::clone;

#[derive(Clone)]
pub struct HistoryPanel {
    pub root: gtk4::Box,
    list: gtk4::ListBox,
    navigate: Rc<dyn Fn(String)>,
    close: Rc<dyn Fn()>,
}

impl HistoryPanel {
    pub fn build(navigate: Rc<dyn Fn(String)>, close: Rc<dyn Fn()>) -> Self {
        let search = gtk4::SearchEntry::builder()
            .placeholder_text("Filter history…")
            .css_classes(vec!["panel-search"])
            .margin_start(14)
            .margin_end(14)
            .margin_top(10)
            .margin_bottom(4)
            .build();

        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["panel-list"])
            .build();

        let search_ref = search.clone();
        list.set_filter_func(move |row| {
            let q = search_ref.text().to_lowercase();
            if q.is_empty() { return true; }
            row.widget_name().to_lowercase().contains(&q)
        });

        search.connect_search_changed(clone!(#[weak] list, move |_| {
            list.invalidate_filter();
        }));

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&list)
            .vexpand(true)
            .build();

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        root.append(&search);
        root.append(&scrolled);

        HistoryPanel { root, list, navigate, close }
    }

    pub fn refresh(&self) {
        let list = self.list.clone();
        let navigate = Rc::clone(&self.navigate);
        let close = Rc::clone(&self.close);

        crate::backend::fetch_history(move |entries| {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            for entry in entries {
                let row = make_row(
                    &entry.url,
                    &entry.title,
                    entry.visited_at.as_deref().unwrap_or(""),
                );
                row.set_widget_name(&format!(
                    "{} {}",
                    entry.title.to_lowercase(),
                    entry.url.to_lowercase()
                ));

                let url = entry.url.clone();

                // Keyboard activation (Enter/Space on a focused row)
                let nav = Rc::clone(&navigate);
                let cl = Rc::clone(&close);
                let u = url.clone();
                row.connect_activate(move |_| {
                    nav(u.clone());
                    cl();
                });

                // Mouse click — ListBoxRow's "activate" signal doesn't fire on click
                let nav = Rc::clone(&navigate);
                let cl = Rc::clone(&close);
                let click = gtk4::GestureClick::new();
                click.connect_released(move |_, _, _, _| {
                    nav(url.clone());
                    cl();
                });
                row.add_controller(click);

                list.append(&row);
            }
        });
    }
}

fn make_row(url: &str, title: &str, time: &str) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::builder()
        .css_classes(vec!["panel-row"])
        .activatable(true)
        .build();

    let body = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .margin_start(14)
        .margin_end(14)
        .margin_top(9)
        .margin_bottom(9)
        .spacing(3)
        .build();

    let display_title = if title.is_empty() { url } else { title };

    let title_lbl = gtk4::Label::builder()
        .label(display_title)
        .css_classes(vec!["row-title"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(44)
        .build();

    let meta = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();

    let url_lbl = gtk4::Label::builder()
        .label(url)
        .css_classes(vec!["row-meta"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(36)
        .hexpand(true)
        .build();

    let time_lbl = gtk4::Label::builder()
        .label(time)
        .css_classes(vec!["row-time"])
        .halign(gtk4::Align::End)
        .build();

    meta.append(&url_lbl);
    meta.append(&time_lbl);
    body.append(&title_lbl);
    body.append(&meta);
    row.set_child(Some(&body));
    row
}

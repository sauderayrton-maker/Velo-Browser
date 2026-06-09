use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use glib::clone;

#[derive(Clone)]
pub struct HistoryPanel {
    pub window: libadwaita::Window,
    list: gtk4::ListBox,
    navigate: Rc<dyn Fn(String)>,
}

impl HistoryPanel {
    pub fn build(
        parent: &libadwaita::ApplicationWindow,
        navigate: Rc<dyn Fn(String)>,
    ) -> Self {
        let window = libadwaita::Window::builder()
            .title("History")
            .default_width(420)
            .default_height(660)
            .transient_for(parent)
            .css_classes(vec!["panel-window"])
            .build();

        let header = libadwaita::HeaderBar::builder()
            .show_start_title_buttons(false)
            .show_end_title_buttons(true)
            .css_classes(vec!["panel-bar"])
            .build();

        let title_lbl = gtk4::Label::builder()
            .label("HISTORY")
            .css_classes(vec!["panel-title"])
            .build();
        header.set_title_widget(Some(&title_lbl));

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

        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        content.append(&search);
        content.append(&scrolled);

        let toolbar = libadwaita::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
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

        HistoryPanel { window, list, navigate }
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

        crate::backend::fetch_history(move |entries| {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            for entry in entries {
                let nav = Rc::clone(&navigate);
                let win = window.clone();
                let url = entry.url.clone();
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
                row.connect_activate(move |_| {
                    nav(url.clone());
                    win.set_visible(false);
                });
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

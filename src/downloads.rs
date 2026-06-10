use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use glib::clone;
use webkit6::Download;

#[derive(Clone, Copy, PartialEq)]
enum DlState {
    Active,
    Done,
    Failed,
}

#[derive(Clone)]
pub struct DownloadsPanel {
    pub root: gtk4::Box,
    list: gtk4::ListBox,
}

impl DownloadsPanel {
    pub fn build() -> Self {
        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["panel-list"])
            .build();

        let placeholder = gtk4::Label::builder()
            .label("No downloads yet")
            .css_classes(vec!["row-meta"])
            .margin_top(24)
            .build();
        list.set_placeholder(Some(&placeholder));

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

        DownloadsPanel { root, list }
    }

    /// Adds a row tracking `download` and wires up live progress, completion,
    /// and failure updates. Called as soon as WebKit reports a new download.
    pub fn add_download(&self, download: &Download) {
        let download = download.clone();
        let suggested = download
            .request()
            .and_then(|r| r.uri())
            .map(|u| u.to_string())
            .unwrap_or_default();
        let initial_name = suggested
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("Download")
            .to_string();

        let (row, title_lbl, meta_lbl, progress, action_btn) = make_row(&initial_name);
        self.list.prepend(&row);

        let state = Rc::new(Cell::new(DlState::Active));
        let dest_path: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));

        // Filename becomes known once WebKit settles on a destination.
        download.connect_created_destination(clone!(
            #[weak] title_lbl,
            #[strong] dest_path,
            move |_, dest_uri| {
                if let Ok((path, _)) = glib::filename_from_uri(dest_uri) {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        title_lbl.set_label(name);
                    }
                    *dest_path.borrow_mut() = Some(path);
                }
            }
        ));

        download.connect_estimated_progress_notify(clone!(
            #[weak] progress,
            #[weak] meta_lbl,
            move |d| {
                let frac = d.estimated_progress();
                progress.set_fraction(frac);
                let received = format_size(d.received_data_length());
                meta_lbl.set_label(&format!("{:.0}% · {received}", frac * 100.0));
            }
        ));

        download.connect_finished(clone!(
            #[weak] progress,
            #[weak] meta_lbl,
            #[weak] action_btn,
            #[strong] state,
            move |_| {
                state.set(DlState::Done);
                progress.set_fraction(1.0);
                progress.set_visible(false);
                meta_lbl.set_label("Done");
                action_btn.set_icon_name("folder-symbolic");
                action_btn.set_tooltip_text(Some("Show in folder"));
            }
        ));

        download.connect_failed(clone!(
            #[weak] progress,
            #[weak] meta_lbl,
            #[weak] action_btn,
            #[strong] state,
            move |_, _err| {
                state.set(DlState::Failed);
                progress.set_visible(false);
                meta_lbl.set_label("Failed");
                action_btn.set_icon_name("user-trash-symbolic");
                action_btn.set_tooltip_text(Some("Remove"));
            }
        ));

        action_btn.connect_clicked(clone!(
            #[strong] download,
            #[strong] state,
            #[strong] dest_path,
            #[weak] row,
            #[weak(rename_to = list)] self.list,
            move |_| match state.get() {
                DlState::Active => download.cancel(),
                DlState::Done => {
                    if let Some(path) = dest_path.borrow().as_ref() {
                        let file = gtk4::gio::File::for_path(path);
                        gtk4::FileLauncher::new(Some(&file))
                            .open_containing_folder(None::<&gtk4::Window>, gtk4::gio::Cancellable::NONE, |_| {});
                    }
                }
                DlState::Failed => list.remove(&row),
            }
        ));
    }
}

fn make_row(name: &str) -> (gtk4::ListBoxRow, gtk4::Label, gtk4::Label, gtk4::ProgressBar, gtk4::Button) {
    let row = gtk4::ListBoxRow::builder()
        .css_classes(vec!["panel-row"])
        .activatable(false)
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
        .spacing(4)
        .hexpand(true)
        .build();

    let title_lbl = gtk4::Label::builder()
        .label(name)
        .css_classes(vec!["row-title"])
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .max_width_chars(38)
        .build();

    let progress = gtk4::ProgressBar::builder()
        .fraction(0.0)
        .build();

    let meta_lbl = gtk4::Label::builder()
        .label("Starting…")
        .css_classes(vec!["row-meta"])
        .halign(gtk4::Align::Start)
        .build();

    body.append(&title_lbl);
    body.append(&progress);
    body.append(&meta_lbl);

    let action_btn = gtk4::Button::builder()
        .icon_name("process-stop-symbolic")
        .tooltip_text("Cancel")
        .css_classes(vec!["flat", "row-delete-btn"])
        .valign(gtk4::Align::Center)
        .build();

    hbox.append(&body);
    hbox.append(&action_btn);
    row.set_child(Some(&hbox));
    (row, title_lbl, meta_lbl, progress, action_btn)
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

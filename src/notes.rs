use gtk4::prelude::*;

/// Builds the Notes sidebar page: a plain-text scratchpad that auto-saves on
/// every keystroke. Returned as a `Widget` so it can be hosted in the main
/// window's sidebar `Stack` alongside the other applets.
pub fn build_widget() -> gtk4::Widget {
    let text_view = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::WordChar)
        .left_margin(22)
        .right_margin(22)
        .top_margin(18)
        .bottom_margin(18)
        .pixels_below_lines(3)
        .css_classes(vec!["notes-view"])
        .build();

    let buf = text_view.buffer();
    buf.set_text(&std::fs::read_to_string(notes_path()).unwrap_or_default());

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(&text_view)
        .vexpand(true)
        .build();

    // Auto-save every keystroke
    buf.connect_changed(|b| {
        let (start, end) = (b.start_iter(), b.end_iter());
        let _ = std::fs::write(notes_path(), b.text(&start, &end, false).as_str());
    });

    scrolled.upcast()
}

fn notes_path() -> std::path::PathBuf {
    let mut p = glib::user_data_dir();
    p.push("velo");
    std::fs::create_dir_all(&p).ok();
    p.push("notes.txt");
    p
}

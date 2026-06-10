use gtk4::prelude::*;
use libadwaita::prelude::*;

pub fn build(parent: &libadwaita::ApplicationWindow) -> libadwaita::Window {
    let win = libadwaita::Window::builder()
        .title("Notes")
        .default_width(360)
        .default_height(520)
        .transient_for(parent)
        .build();

    let header = libadwaita::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(true)
        .build();

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

    let toolbar = libadwaita::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));
    win.set_content(Some(&toolbar));

    // Auto-save every keystroke
    buf.connect_changed(|b| {
        let (start, end) = (b.start_iter(), b.end_iter());
        let _ = std::fs::write(notes_path(), b.text(&start, &end, false).as_str());
    });

    // Hide rather than destroy on the system close button, so notes can be
    // reopened with their scroll position preserved.
    win.set_hide_on_close(true);

    // Escape hides without destroying (preserves scroll position)
    let key_ctl = gtk4::EventControllerKey::new();
    key_ctl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    win.add_controller(key_ctl.clone());
    key_ctl.connect_key_pressed(glib::clone!(
        #[weak] win,
        #[upgrade_or] glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                win.set_visible(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));

    win
}

pub fn toggle(win: &libadwaita::Window) {
    if win.is_visible() {
        win.set_visible(false);
    } else {
        win.present();
    }
}

fn notes_path() -> std::path::PathBuf {
    let mut p = glib::user_data_dir();
    p.push("velo");
    std::fs::create_dir_all(&p).ok();
    p.push("notes.txt");
    p
}

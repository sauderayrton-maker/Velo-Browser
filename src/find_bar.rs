use gtk4::prelude::*;
use webkit6::prelude::*;
use webkit6::FindOptions;

const SEARCH_OPTS: u32 = FindOptions::CASE_INSENSITIVE.bits() | FindOptions::WRAP_AROUND.bits();
const MAX_MATCHES: u32 = 1000;

/// In-page "Find" bar (Ctrl+F) — slides down from the tab bar and drives the
/// active tab's `WebKitFindController`.
#[derive(Clone)]
pub struct FindBar {
    pub root: gtk4::Revealer,
    entry: gtk4::SearchEntry,
}

impl FindBar {
    pub fn build(tab_view: &libadwaita::TabView) -> Self {
        let entry = gtk4::SearchEntry::builder()
            .placeholder_text("Find in page")
            .hexpand(true)
            .build();

        let prev_btn = gtk4::Button::builder()
            .icon_name("go-up-symbolic")
            .tooltip_text("Previous match (Shift+Enter)")
            .css_classes(vec!["flat"])
            .build();

        let next_btn = gtk4::Button::builder()
            .icon_name("go-down-symbolic")
            .tooltip_text("Next match (Enter)")
            .css_classes(vec!["flat"])
            .build();

        let close_btn = gtk4::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text("Close (Esc)")
            .css_classes(vec!["flat"])
            .build();

        let bar = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .css_classes(vec!["find-bar"])
            .build();
        bar.append(&entry);
        bar.append(&prev_btn);
        bar.append(&next_btn);
        bar.append(&close_btn);

        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .reveal_child(false)
            .build();
        revealer.set_child(Some(&bar));

        entry.connect_search_changed(glib::clone!(
            #[weak] tab_view,
            move |entry| {
                let text = entry.text();
                with_find_controller(&tab_view, |fc| {
                    if text.is_empty() {
                        fc.search_finish();
                    } else {
                        fc.search(&text, SEARCH_OPTS, MAX_MATCHES);
                    }
                });
            }
        ));

        entry.connect_activate(glib::clone!(
            #[weak] tab_view,
            move |entry| {
                if entry.text().is_empty() { return; }
                with_find_controller(&tab_view, |fc| fc.search_next());
            }
        ));

        // Shift+Enter searches backwards.
        let key_ctl = gtk4::EventControllerKey::new();
        key_ctl.connect_key_pressed(glib::clone!(
            #[weak] tab_view,
            #[upgrade_or] glib::Propagation::Proceed,
            move |_, keyval, _keycode, mods| {
                if keyval == gtk4::gdk::Key::Return
                    && mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK)
                {
                    with_find_controller(&tab_view, |fc| fc.search_previous());
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        ));
        entry.add_controller(key_ctl);

        next_btn.connect_clicked(glib::clone!(
            #[weak] tab_view,
            move |_| with_find_controller(&tab_view, |fc| fc.search_next())
        ));

        prev_btn.connect_clicked(glib::clone!(
            #[weak] tab_view,
            move |_| with_find_controller(&tab_view, |fc| fc.search_previous())
        ));

        let find_bar = FindBar { root: revealer, entry };

        close_btn.connect_clicked(glib::clone!(
            #[strong] find_bar,
            #[weak] tab_view,
            move |_| find_bar.close(&tab_view)
        ));

        find_bar
    }

    /// Reveals the bar and focuses the search entry.
    pub fn open(&self) {
        self.root.set_reveal_child(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
    }

    /// Hides the bar, clears highlighted matches, and resets the entry.
    pub fn close(&self, tab_view: &libadwaita::TabView) {
        self.root.set_reveal_child(false);
        with_find_controller(tab_view, |fc| fc.search_finish());
        self.entry.set_text("");
    }

    pub fn is_open(&self) -> bool {
        self.root.reveals_child()
    }
}

fn with_find_controller<F: FnOnce(&webkit6::FindController)>(tab_view: &libadwaita::TabView, f: F) {
    let Some(page) = tab_view.selected_page() else { return };
    let Some(wv) = page.child().downcast::<webkit6::WebView>().ok() else { return };
    let Some(fc) = wv.find_controller() else { return };
    f(&fc);
}

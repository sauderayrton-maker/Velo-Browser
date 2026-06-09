use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use webkit6::prelude::*;
use webkit6::{LoadEvent, WebView};

use crate::engine;
use crate::history::HistoryPanel;
use crate::bookmarks::BookmarksPanel;

const NEWTAB_HTML: &str = include_str!("newtab.html");
const NEWTAB_URI: &str = "about:newtab";

pub fn build_browser_window(app: &libadwaita::Application) -> libadwaita::ApplicationWindow {
    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("Velo")
        .default_width(1440)
        .default_height(940)
        .build();

    let tab_view = libadwaita::TabView::new();
    let tab_bar = libadwaita::TabBar::builder()
        .view(&tab_view)
        .expand_tabs(false)
        .build();

    let (header, url_bar, back_btn, forward_btn, reload_btn) = build_header(&tab_view);

    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.add_top_bar(&tab_bar);
    toolbar_view.set_content(Some(&tab_view));
    window.set_content(Some(&toolbar_view));

    // Shared navigate callback for both panels
    let navigate: Rc<dyn Fn(String)> = Rc::new(glib::clone!(
        #[weak] tab_view,
        #[upgrade_or] (),
        move |url: String| {
            let uri = normalize_url(&url);
            with_webview(&tab_view, |wv| wv.load_uri(&uri));
        }
    ));

    let history_panel = HistoryPanel::build(&window, Rc::clone(&navigate));
    let bookmarks_panel = BookmarksPanel::build(&window, Rc::clone(&navigate));

    // Dim the tab view when a panel is open — "environment reacts"
    // Connect to each panel's window hide signal to remove dim when last panel closes
    let hp_win = history_panel.window.clone();
    let bp_win = bookmarks_panel.window.clone();
    history_panel.window.connect_hide(glib::clone!(
        #[weak] tab_view,
        #[weak] bp_win,
        move |_| {
            if !bp_win.is_visible() {
                tab_view.remove_css_class("browse-dim");
            }
        }
    ));
    bookmarks_panel.window.connect_hide(glib::clone!(
        #[weak] tab_view,
        #[weak] hp_win,
        move |_| {
            if !hp_win.is_visible() {
                tab_view.remove_css_class("browse-dim");
            }
        }
    ));

    // Bookmark star button
    let star_btn = gtk4::Button::builder()
        .icon_name("bookmark-new-symbolic")
        .tooltip_text("Bookmark page (Ctrl+D)")
        .css_classes(vec!["flat"])
        .build();
    header.pack_end(&star_btn);

    star_btn.connect_clicked(glib::clone!(
        #[weak] url_bar,
        #[weak] tab_view,
        move |_| {
            let url = url_bar.text().to_string();
            if url.is_empty() || url == "about:blank" { return; }
            let title = tab_view
                .selected_page()
                .and_then(|p| page_webview(&p))
                .and_then(|wv| wv.title())
                .map(|t| t.to_string())
                .unwrap_or_else(|| url.clone());
            crate::backend::add_bookmark(url, title);
        }
    ));

    open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, NEWTAB_URI, true);

    let notes_win = crate::notes::build(&window);

    tab_view.connect_selected_page_notify(glib::clone!(
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] url_bar,
        move |tv| {
            if let Some(page) = tv.selected_page() {
                if let Some(wv) = page_webview(&page) {
                    sync_nav(&back_btn, &forward_btn, &url_bar, &wv);
                }
            }
        }
    ));

    setup_shortcuts(
        &window, &tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn,
        &notes_win, &history_panel, &bookmarks_panel,
    );

    load_css();
    window
}

fn build_header(
    tab_view: &libadwaita::TabView,
) -> (
    libadwaita::HeaderBar,
    gtk4::Entry,
    gtk4::Button,
    gtk4::Button,
    gtk4::Button,
) {
    let back_btn = nav_button("go-previous-symbolic", "Back");
    let forward_btn = nav_button("go-next-symbolic", "Forward");
    let reload_btn = nav_button("view-refresh-symbolic", "Reload");
    back_btn.set_sensitive(false);
    forward_btn.set_sensitive(false);

    let nav_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(0)
        .css_classes(vec!["linked"])
        .build();
    nav_box.append(&back_btn);
    nav_box.append(&forward_btn);
    nav_box.append(&reload_btn);

    let url_bar = gtk4::Entry::builder()
        .placeholder_text("Search or navigate")
        .hexpand(true)
        .width_chars(60)
        .css_classes(vec!["url-bar"])
        .build();
    url_bar.set_input_purpose(gtk4::InputPurpose::Url);

    let focus_ctl = gtk4::EventControllerFocus::new();
    url_bar.add_controller(focus_ctl.clone());
    focus_ctl.connect_enter(glib::clone!(
        #[weak] url_bar,
        move |_| url_bar.select_region(0, -1)
    ));

    let new_tab_btn = nav_button("tab-new-symbolic", "New Tab");
    let menu_btn = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu")
        .css_classes(vec!["flat"])
        .build();

    let header = libadwaita::HeaderBar::new();
    header.pack_start(&nav_box);
    header.set_title_widget(Some(&url_bar));
    header.pack_end(&menu_btn);
    header.pack_end(&new_tab_btn);

    back_btn.connect_clicked(glib::clone!(
        #[weak] tab_view,
        move |_| with_webview(&tab_view, |wv| wv.go_back())
    ));
    forward_btn.connect_clicked(glib::clone!(
        #[weak] tab_view,
        move |_| with_webview(&tab_view, |wv| wv.go_forward())
    ));
    reload_btn.connect_clicked(glib::clone!(
        #[weak] tab_view,
        move |_| with_webview(&tab_view, |wv| {
            if wv.is_loading() { wv.stop_loading(); } else { wv.reload(); }
        })
    ));

    url_bar.connect_activate(glib::clone!(
        #[weak] tab_view,
        move |entry| {
            let uri = normalize_url(&entry.text());
            with_webview(&tab_view, |wv| wv.load_uri(&uri));
        }
    ));

    new_tab_btn.connect_clicked(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] reload_btn,
        move |_| {
            open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, NEWTAB_URI, true);
            url_bar.grab_focus();
        }
    ));

    (header, url_bar, back_btn, forward_btn, reload_btn)
}

pub fn open_tab(
    tab_view: &libadwaita::TabView,
    url_bar: &gtk4::Entry,
    back_btn: &gtk4::Button,
    forward_btn: &gtk4::Button,
    reload_btn: &gtk4::Button,
    url: &str,
    select: bool,
) -> libadwaita::TabPage {
    let webview = engine::create_webview();
    if url == NEWTAB_URI {
        webview.load_html(NEWTAB_HTML, None::<&str>);
    } else {
        webview.load_uri(url);
    }

    let page = tab_view.append(&webview);
    page.set_title("New Tab");

    if select {
        tab_view.set_selected_page(&page);
    }

    webview.connect_uri_notify(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] page,
        move |wv| {
            if is_selected(&tab_view, &page) {
                let uri = wv.uri().unwrap_or_default();
                url_bar.set_text(if is_newtab_uri(&uri) { "" } else { &uri });
            }
        }
    ));

    webview.connect_title_notify(glib::clone!(
        #[weak] page,
        move |wv| {
            let title = wv.title().filter(|t| !t.is_empty())
                .unwrap_or_else(|| glib::GString::from("New Tab"));
            page.set_title(&title);
        }
    ));

    webview.connect_load_changed(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] page,
        move |wv, event| {
            if is_selected(&tab_view, &page) {
                back_btn.set_sensitive(wv.can_go_back());
                forward_btn.set_sensitive(wv.can_go_forward());
                let uri = wv.uri().unwrap_or_default();
                url_bar.set_text(if is_newtab_uri(&uri) { "" } else { &uri });
            }
            if event == LoadEvent::Finished {
                let uri = wv.uri().unwrap_or_default().to_string();
                let title = wv.title().unwrap_or_default().to_string();
                if !is_newtab_uri(&uri) && !uri.is_empty() {
                    crate::backend::record_visit(uri, title);
                }
            }
        }
    ));

    webview.connect_is_loading_notify(glib::clone!(
        #[weak] reload_btn,
        #[weak] tab_view,
        #[weak] page,
        move |wv| {
            if is_selected(&tab_view, &page) {
                if wv.is_loading() {
                    reload_btn.set_icon_name("process-stop-symbolic");
                    reload_btn.set_tooltip_text(Some("Stop"));
                } else {
                    reload_btn.set_icon_name("view-refresh-symbolic");
                    reload_btn.set_tooltip_text(Some("Reload"));
                }
            }
        }
    ));

    webview.connect_estimated_load_progress_notify(glib::clone!(
        #[weak] page,
        move |wv| page.set_loading(wv.is_loading())
    ));

    page
}

fn setup_shortcuts(
    window: &libadwaita::ApplicationWindow,
    tab_view: &libadwaita::TabView,
    url_bar: &gtk4::Entry,
    back_btn: &gtk4::Button,
    forward_btn: &gtk4::Button,
    reload_btn: &gtk4::Button,
    notes_win: &libadwaita::Window,
    history_panel: &HistoryPanel,
    bookmarks_panel: &BookmarksPanel,
) {
    use gtk4::gdk::{Key, ModifierType};

    let key_ctl = gtk4::EventControllerKey::new();
    key_ctl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    window.add_controller(key_ctl.clone());

    let hp = history_panel.clone();
    let bp = bookmarks_panel.clone();

    key_ctl.connect_key_pressed(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] reload_btn,
        #[weak] notes_win,
        #[upgrade_or] glib::Propagation::Proceed,
        move |_, keyval, _keycode, mods| {
            let ctrl  = mods.contains(ModifierType::CONTROL_MASK);
            let shift = mods.contains(ModifierType::SHIFT_MASK);
            let alt   = mods.contains(ModifierType::ALT_MASK);

            match keyval {
                Key::F5 => {
                    if ctrl {
                        with_webview(&tab_view, |wv| wv.reload_bypass_cache());
                    } else {
                        with_webview(&tab_view, |wv| wv.reload());
                    }
                    return glib::Propagation::Stop;
                }
                Key::F6 => {
                    url_bar.grab_focus();
                    url_bar.select_region(0, -1);
                    return glib::Propagation::Stop;
                }
                Key::Escape => {
                    with_webview(&tab_view, |wv| {
                        if wv.is_loading() { wv.stop_loading(); }
                    });
                    return glib::Propagation::Proceed;
                }
                Key::Left if alt => {
                    with_webview(&tab_view, |wv| wv.go_back());
                    return glib::Propagation::Stop;
                }
                Key::Right if alt => {
                    with_webview(&tab_view, |wv| wv.go_forward());
                    return glib::Propagation::Stop;
                }
                Key::Tab if ctrl && !shift => {
                    cycle_tab(&tab_view, true);
                    return glib::Propagation::Stop;
                }
                Key::Tab if ctrl && shift => {
                    cycle_tab(&tab_view, false);
                    return glib::Propagation::Stop;
                }
                Key::ISO_Left_Tab if ctrl => {
                    cycle_tab(&tab_view, false);
                    return glib::Propagation::Stop;
                }
                _ => {}
            }

            if let Some(c) = keyval.to_unicode() {
                let ch = c.to_lowercase().next().unwrap_or(c);
                match ch {
                    'n' if ctrl && !shift => {
                        crate::notes::toggle(&notes_win);
                        return glib::Propagation::Stop;
                    }
                    't' if ctrl && !shift => {
                        open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn,
                                 NEWTAB_URI, true);
                        url_bar.grab_focus();
                        return glib::Propagation::Stop;
                    }
                    'w' if ctrl => {
                        close_current_tab(&tab_view);
                        return glib::Propagation::Stop;
                    }
                    'l' if ctrl => {
                        url_bar.grab_focus();
                        url_bar.select_region(0, -1);
                        return glib::Propagation::Stop;
                    }
                    'r' if ctrl && !shift => {
                        with_webview(&tab_view, |wv| wv.reload());
                        return glib::Propagation::Stop;
                    }
                    'r' if ctrl && shift => {
                        with_webview(&tab_view, |wv| wv.reload_bypass_cache());
                        return glib::Propagation::Stop;
                    }
                    'h' if ctrl && !shift => {
                        let opening = !hp.is_open();
                        bp.hide();
                        if opening {
                            hp.show();
                            tab_view.add_css_class("browse-dim");
                        } else {
                            hp.hide();
                        }
                        return glib::Propagation::Stop;
                    }
                    'b' if ctrl && !shift => {
                        let opening = !bp.is_open();
                        hp.hide();
                        if opening {
                            bp.show();
                            tab_view.add_css_class("browse-dim");
                        } else {
                            bp.hide();
                        }
                        return glib::Propagation::Stop;
                    }
                    'd' if ctrl => {
                        let url = url_bar.text().to_string();
                        if !url.is_empty() && url != "about:blank" {
                            let title = tab_view
                                .selected_page()
                                .and_then(|p| page_webview(&p))
                                .and_then(|wv| wv.title())
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| url.clone());
                            crate::backend::add_bookmark(url, title);
                        }
                        return glib::Propagation::Stop;
                    }
                    '=' | '+' if ctrl => {
                        with_webview(&tab_view, |wv| {
                            wv.set_zoom_level((wv.zoom_level() * 1.1).min(5.0));
                        });
                        return glib::Propagation::Stop;
                    }
                    '-' if ctrl => {
                        with_webview(&tab_view, |wv| {
                            wv.set_zoom_level((wv.zoom_level() / 1.1).max(0.25));
                        });
                        return glib::Propagation::Stop;
                    }
                    '0' if ctrl => {
                        with_webview(&tab_view, |wv| wv.set_zoom_level(1.0));
                        return glib::Propagation::Stop;
                    }
                    '1'..='8' if ctrl => {
                        let idx = (ch as i32) - ('1' as i32);
                        if idx < tab_view.n_pages() {
                            tab_view.set_selected_page(&tab_view.nth_page(idx));
                        }
                        return glib::Propagation::Stop;
                    }
                    '9' if ctrl => {
                        let last = tab_view.n_pages() - 1;
                        if last >= 0 {
                            tab_view.set_selected_page(&tab_view.nth_page(last));
                        }
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }

            glib::Propagation::Proceed
        }
    ));
}

// ── Tab helpers ───────────────────────────────────────────────────────────────

fn close_current_tab(tab_view: &libadwaita::TabView) {
    if tab_view.n_pages() <= 1 { return; }
    if let Some(page) = tab_view.selected_page() {
        tab_view.close_page(&page);
    }
}

fn cycle_tab(tab_view: &libadwaita::TabView, forward: bool) {
    let n = tab_view.n_pages();
    if n <= 1 { return; }
    if let Some(page) = tab_view.selected_page() {
        let pos = tab_view.page_position(&page);
        let next = if forward { (pos + 1) % n } else { (pos - 1 + n) % n };
        tab_view.set_selected_page(&tab_view.nth_page(next));
    }
}

// ── Widget helpers ────────────────────────────────────────────────────────────

fn nav_button(icon: &str, tooltip: &str) -> gtk4::Button {
    gtk4::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .css_classes(vec!["flat"])
        .build()
}

fn with_webview<F: Fn(&WebView)>(tab_view: &libadwaita::TabView, f: F) {
    if let Some(page) = tab_view.selected_page() {
        if let Some(wv) = page_webview(&page) {
            f(&wv);
        }
    }
}

fn page_webview(page: &libadwaita::TabPage) -> Option<WebView> {
    page.child().downcast::<WebView>().ok()
}

fn is_selected(tab_view: &libadwaita::TabView, page: &libadwaita::TabPage) -> bool {
    tab_view.selected_page().map_or(false, |p| p.as_ptr() == page.as_ptr())
}

fn sync_nav(
    back_btn: &gtk4::Button,
    forward_btn: &gtk4::Button,
    url_bar: &gtk4::Entry,
    wv: &WebView,
) {
    back_btn.set_sensitive(wv.can_go_back());
    forward_btn.set_sensitive(wv.can_go_forward());
    let uri = wv.uri().unwrap_or_default();
    url_bar.set_text(if is_newtab_uri(&uri) { "" } else { &uri });
}

fn is_newtab_uri(uri: &str) -> bool {
    uri.is_empty() || uri == "about:blank"
}

pub fn normalize_url(input: &str) -> String {
    let s = input.trim();
    if s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("file://")
        || s.starts_with("about:")
    {
        return s.to_string();
    }
    if s.contains('.') && !s.contains(' ') && !s.is_empty() {
        return format!("https://{s}");
    }
    format!("https://www.google.com/search?q={}", s.replace(' ', "+"))
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use webkit6::prelude::*;
use webkit6::{LoadEvent, WebView};

use crate::engine;
use crate::history::HistoryPanel;
use crate::bookmarks::BookmarksPanel;
use crate::downloads::DownloadsPanel;

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

    let (header, url_bar, back_btn, forward_btn, reload_btn, new_tab_btn, menu_btn) = build_header(&tab_view);

    // ── Sidebar — docked applet panel (History / Bookmarks / Notes / Downloads) ──
    let split_view = libadwaita::OverlaySplitView::builder()
        .sidebar_position(gtk4::PackType::End)
        .min_sidebar_width(320.0)
        .max_sidebar_width(400.0)
        .sidebar_width_fraction(0.26)
        .show_sidebar(false)
        .build();
    split_view.set_content(Some(&tab_view));

    let sidebar_title = gtk4::Label::builder()
        .css_classes(vec!["panel-title"])
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .build();

    let sidebar_close = gtk4::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Close (Esc)")
        .css_classes(vec!["flat"])
        .valign(gtk4::Align::Center)
        .build();

    let sidebar_header = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .css_classes(vec!["sidebar-header"])
        .build();
    sidebar_header.append(&sidebar_title);
    sidebar_header.append(&sidebar_close);

    let sidebar_stack = gtk4::Stack::new();

    let sidebar_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .css_classes(vec!["velo-sidebar"])
        .build();
    sidebar_box.append(&sidebar_header);
    sidebar_box.append(&sidebar_stack);
    split_view.set_sidebar(Some(&sidebar_box));

    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.add_top_bar(&tab_bar);
    toolbar_view.set_content(Some(&split_view));
    window.set_content(Some(&toolbar_view));

    // Closes the sidebar and lifts the "environment reacts" dim from the tab view.
    let close_sidebar: Rc<dyn Fn()> = Rc::new(glib::clone!(
        #[weak] split_view,
        #[weak] tab_view,
        #[upgrade_or] (),
        move || {
            split_view.set_show_sidebar(false);
            tab_view.remove_css_class("browse-dim");
        }
    ));

    sidebar_close.connect_clicked(glib::clone!(
        #[strong] close_sidebar,
        move |_| close_sidebar()
    ));

    // Shared navigate callback for panels that link out to pages
    let navigate: Rc<dyn Fn(String)> = Rc::new(glib::clone!(
        #[weak] tab_view,
        #[upgrade_or] (),
        move |url: String| {
            let uri = normalize_url(&url);
            with_webview(&tab_view, |wv| wv.load_uri(&uri));
        }
    ));

    let history_panel = HistoryPanel::build(Rc::clone(&navigate), Rc::clone(&close_sidebar));
    let bookmarks_panel = BookmarksPanel::build(Rc::clone(&navigate), Rc::clone(&close_sidebar));
    let downloads_panel = DownloadsPanel::build();
    let notes_widget = crate::notes::build_widget();

    sidebar_stack.add_named(&history_panel.root, Some("history"));
    sidebar_stack.add_named(&bookmarks_panel.root, Some("bookmarks"));
    sidebar_stack.add_named(&notes_widget, Some("notes"));
    sidebar_stack.add_named(&downloads_panel.root, Some("downloads"));

    // Wire WebKit's download signals into the Downloads applet
    engine::set_download_handler(glob_download_handler(downloads_panel.clone()));

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
        move |_| bookmark_current_page(&tab_view, &url_bar)
    ));

    open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, NEWTAB_URI, true);

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
        &split_view, &sidebar_stack, &sidebar_title, &history_panel, &bookmarks_panel,
    );

    setup_menu(
        &window, &menu_btn, &tab_view, &url_bar, &new_tab_btn, &reload_btn,
        &split_view, &sidebar_stack, &sidebar_title, &history_panel, &bookmarks_panel,
    );

    load_css();
    window
}

/// Builds the closure registered with the engine's download handler — kept
/// separate so its type stays a plain `impl Fn(webkit6::Download)`.
fn glob_download_handler(panel: DownloadsPanel) -> impl Fn(webkit6::Download) + 'static {
    move |download| panel.add_download(&download)
}

fn build_header(
    tab_view: &libadwaita::TabView,
) -> (
    libadwaita::HeaderBar,
    gtk4::Entry,
    gtk4::Button,
    gtk4::Button,
    gtk4::Button,
    gtk4::Button,
    gtk4::MenuButton,
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

    let brand = gtk4::Label::builder()
        .label("VELO")
        .css_classes(vec!["brand-mark"])
        .build();

    let header = libadwaita::HeaderBar::new();
    header.pack_start(&brand);
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

    (header, url_bar, back_btn, forward_btn, reload_btn, new_tab_btn, menu_btn)
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
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
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
        #[weak] split_view,
        #[weak] sidebar_stack,
        #[weak] sidebar_title,
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
                    if split_view.shows_sidebar() {
                        split_view.set_show_sidebar(false);
                        tab_view.remove_css_class("browse-dim");
                        return glib::Propagation::Stop;
                    }
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
                        toggle_notes(&split_view, &sidebar_stack, &sidebar_title, &tab_view);
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
                        toggle_history(&split_view, &sidebar_stack, &sidebar_title, &tab_view, &hp);
                        return glib::Propagation::Stop;
                    }
                    'b' if ctrl && !shift => {
                        toggle_bookmarks(&split_view, &sidebar_stack, &sidebar_title, &tab_view, &bp);
                        return glib::Propagation::Stop;
                    }
                    'j' if ctrl && !shift => {
                        toggle_downloads(&split_view, &sidebar_stack, &sidebar_title, &tab_view);
                        return glib::Propagation::Stop;
                    }
                    'd' if ctrl => {
                        bookmark_current_page(&tab_view, &url_bar);
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

// ── Menu ──────────────────────────────────────────────────────────────────────

fn setup_menu(
    window: &libadwaita::ApplicationWindow,
    menu_btn: &gtk4::MenuButton,
    tab_view: &libadwaita::TabView,
    url_bar: &gtk4::Entry,
    new_tab_btn: &gtk4::Button,
    reload_btn: &gtk4::Button,
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    history_panel: &HistoryPanel,
    bookmarks_panel: &BookmarksPanel,
) {
    let add_action = |name: &str| {
        let action = gio::SimpleAction::new(name, None);
        window.add_action(&action);
        action
    };

    let hp = history_panel.clone();
    let bp = bookmarks_panel.clone();

    add_action("new-tab").connect_activate(glib::clone!(
        #[weak] new_tab_btn,
        move |_, _| new_tab_btn.emit_clicked()
    ));

    add_action("bookmark-page").connect_activate(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        move |_, _| bookmark_current_page(&tab_view, &url_bar)
    ));

    add_action("toggle-history").connect_activate(glib::clone!(
        #[weak] tab_view,
        #[weak] split_view,
        #[weak] sidebar_stack,
        #[weak] sidebar_title,
        #[strong] hp,
        move |_, _| toggle_history(&split_view, &sidebar_stack, &sidebar_title, &tab_view, &hp)
    ));

    add_action("toggle-bookmarks").connect_activate(glib::clone!(
        #[weak] tab_view,
        #[weak] split_view,
        #[weak] sidebar_stack,
        #[weak] sidebar_title,
        #[strong] bp,
        move |_, _| toggle_bookmarks(&split_view, &sidebar_stack, &sidebar_title, &tab_view, &bp)
    ));

    add_action("toggle-notes").connect_activate(glib::clone!(
        #[weak] tab_view,
        #[weak] split_view,
        #[weak] sidebar_stack,
        #[weak] sidebar_title,
        move |_, _| toggle_notes(&split_view, &sidebar_stack, &sidebar_title, &tab_view)
    ));

    add_action("toggle-downloads").connect_activate(glib::clone!(
        #[weak] tab_view,
        #[weak] split_view,
        #[weak] sidebar_stack,
        #[weak] sidebar_title,
        move |_, _| toggle_downloads(&split_view, &sidebar_stack, &sidebar_title, &tab_view)
    ));

    add_action("reload").connect_activate(glib::clone!(
        #[weak] reload_btn,
        move |_, _| reload_btn.emit_clicked()
    ));

    add_action("zoom-in").connect_activate(glib::clone!(
        #[weak] tab_view,
        move |_, _| with_webview(&tab_view, |wv| {
            wv.set_zoom_level((wv.zoom_level() * 1.1).min(5.0));
        })
    ));

    add_action("zoom-out").connect_activate(glib::clone!(
        #[weak] tab_view,
        move |_, _| with_webview(&tab_view, |wv| {
            wv.set_zoom_level((wv.zoom_level() / 1.1).max(0.25));
        })
    ));

    add_action("zoom-reset").connect_activate(glib::clone!(
        #[weak] tab_view,
        move |_, _| with_webview(&tab_view, |wv| wv.set_zoom_level(1.0))
    ));

    add_action("check-update").connect_activate(glib::clone!(
        #[weak] window,
        move |_, _| check_for_updates(&window)
    ));

    let menu = gio::Menu::new();

    let nav = gio::Menu::new();
    nav.append(Some("New Tab"), Some("win.new-tab"));
    nav.append(Some("Reload"), Some("win.reload"));
    menu.append_section(None, &nav);

    let applets = gio::Menu::new();
    applets.append(Some("Bookmark This Page"), Some("win.bookmark-page"));
    applets.append(Some("History"), Some("win.toggle-history"));
    applets.append(Some("Bookmarks"), Some("win.toggle-bookmarks"));
    applets.append(Some("Notes"), Some("win.toggle-notes"));
    applets.append(Some("Downloads"), Some("win.toggle-downloads"));
    menu.append_section(None, &applets);

    let zoom = gio::Menu::new();
    zoom.append(Some("Zoom In"), Some("win.zoom-in"));
    zoom.append(Some("Zoom Out"), Some("win.zoom-out"));
    zoom.append(Some("Reset Zoom"), Some("win.zoom-reset"));
    menu.append_section(None, &zoom);

    let updates = gio::Menu::new();
    updates.append(Some("Check for Updates…"), Some("win.check-update"));
    menu.append_section(None, &updates);

    menu_btn.set_menu_model(Some(&menu));
}

// ── Panel & action helpers ──────────────────────────────────────────────────────

/// Shows/hides the sidebar and switches its active page. Returns `true` if
/// the sidebar ended up open on `name` (so callers can refresh fresh data).
fn toggle_panel(
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    tab_view: &libadwaita::TabView,
    name: &str,
    title: &str,
) -> bool {
    let already_showing = split_view.shows_sidebar()
        && sidebar_stack.visible_child_name().as_deref() == Some(name);

    if already_showing {
        split_view.set_show_sidebar(false);
        tab_view.remove_css_class("browse-dim");
        return false;
    }

    sidebar_stack.set_visible_child_name(name);
    sidebar_title.set_label(title);
    split_view.set_show_sidebar(true);
    tab_view.add_css_class("browse-dim");
    true
}

fn toggle_history(
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    tab_view: &libadwaita::TabView,
    hp: &HistoryPanel,
) {
    if toggle_panel(split_view, sidebar_stack, sidebar_title, tab_view, "history", "HISTORY") {
        hp.refresh();
    }
}

fn toggle_bookmarks(
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    tab_view: &libadwaita::TabView,
    bp: &BookmarksPanel,
) {
    if toggle_panel(split_view, sidebar_stack, sidebar_title, tab_view, "bookmarks", "BOOKMARKS") {
        bp.refresh();
    }
}

fn toggle_notes(
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    tab_view: &libadwaita::TabView,
) {
    toggle_panel(split_view, sidebar_stack, sidebar_title, tab_view, "notes", "NOTES");
}

fn toggle_downloads(
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    tab_view: &libadwaita::TabView,
) {
    toggle_panel(split_view, sidebar_stack, sidebar_title, tab_view, "downloads", "DOWNLOADS");
}

fn bookmark_current_page(tab_view: &libadwaita::TabView, url_bar: &gtk4::Entry) {
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

// ── Self-update ────────────────────────────────────────────────────────────────

fn check_for_updates(window: &libadwaita::ApplicationWindow) {
    crate::update::check_for_update(glib::clone!(
        #[weak] window,
        #[upgrade_or] (),
        move |result| match result {
            crate::update::CheckResult::UpToDate => show_alert(
                &window,
                "Velo is up to date",
                &format!("You're running the latest version (commit {}).", crate::update::CURRENT_COMMIT),
            ),
            crate::update::CheckResult::Available { local, remote } => {
                let dialog = gtk4::AlertDialog::builder()
                    .modal(true)
                    .message("Update available")
                    .detail(format!(
                        "Velo {local} → {remote} is available.\n\n\
                         Pull, build, and install the update now? \
                         You may be asked for your password to finish installing."
                    ))
                    .buttons(["Later", "Update Now"])
                    .cancel_button(0)
                    .default_button(1)
                    .build();

                dialog.choose(Some(&window), gtk4::gio::Cancellable::NONE, glib::clone!(
                    #[weak] window,
                    #[upgrade_or] (),
                    move |response| {
                        if matches!(response, Ok(1)) {
                            start_update(&window);
                        }
                    }
                ));
            }
            crate::update::CheckResult::Unavailable(msg) => {
                show_alert(&window, "Can't check for updates", &msg)
            }
        }
    ));
}

fn start_update(window: &libadwaita::ApplicationWindow) {
    let progress = progress_dialog(
        window,
        "Updating Velo…",
        "Pulling, building, and installing the latest version.\nThis may take a few minutes.",
    );

    crate::update::run_update(glib::clone!(
        #[weak] window,
        #[strong] progress,
        #[upgrade_or] (),
        move |result| {
            progress.close();
            match result {
                crate::update::UpdateResult::Success => {
                    let dialog = gtk4::AlertDialog::builder()
                        .modal(true)
                        .message("Update complete")
                        .detail("Velo has been updated. Restart now to use the new version?")
                        .buttons(["Later", "Restart Now"])
                        .cancel_button(0)
                        .default_button(1)
                        .build();

                    dialog.choose(Some(&window), gtk4::gio::Cancellable::NONE, |response| {
                        if matches!(response, Ok(1)) {
                            crate::update::restart();
                        }
                    });
                }
                crate::update::UpdateResult::Failed(msg) => {
                    show_alert(&window, "Update failed", &msg)
                }
            }
        }
    ));
}

fn show_alert(window: &libadwaita::ApplicationWindow, message: &str, detail: &str) {
    gtk4::AlertDialog::builder()
        .modal(true)
        .message(message)
        .detail(detail)
        .buttons(["OK"])
        .build()
        .show(Some(window));
}

/// A small modal "working" dialog with a spinner, shown while an update runs
/// in the background. Caller closes it via the returned handle.
fn progress_dialog(parent: &libadwaita::ApplicationWindow, title: &str, body: &str) -> gtk4::Window {
    let spinner = gtk4::Spinner::builder()
        .spinning(true)
        .width_request(32)
        .height_request(32)
        .halign(gtk4::Align::Center)
        .build();

    let title_lbl = gtk4::Label::builder()
        .label(title)
        .css_classes(vec!["title-4"])
        .build();

    let body_lbl = gtk4::Label::builder()
        .label(body)
        .wrap(true)
        .justify(gtk4::Justification::Center)
        .css_classes(vec!["dim-label"])
        .build();

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(14)
        .margin_top(28)
        .margin_bottom(28)
        .margin_start(28)
        .margin_end(28)
        .build();
    content.append(&spinner);
    content.append(&title_lbl);
    content.append(&body_lbl);

    let win = gtk4::Window::builder()
        .transient_for(parent)
        .modal(true)
        .resizable(false)
        .deletable(false)
        .destroy_with_parent(true)
        .build();
    win.set_child(Some(&content));
    win.present();
    win
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

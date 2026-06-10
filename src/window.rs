use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use serde::Deserialize;
use webkit6::prelude::*;
use webkit6::{
    javascriptcore, LoadEvent, NavigationPolicyDecision, NavigationType, PolicyDecisionType, WebView,
};

use crate::engine;
use crate::find_bar::FindBar;
use crate::history::HistoryPanel;
use crate::bookmarks::BookmarksPanel;
use crate::downloads::DownloadsPanel;
use crate::passwords_panel::PasswordsPanel;

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

    // Overlay for in-page toasts (e.g. "Save password for example.com?")
    let toast_overlay = libadwaita::ToastOverlay::new();

    let (header, url_bar, back_btn, forward_btn, reload_btn, new_tab_btn, menu_btn) =
        build_header(&tab_view, &toast_overlay);

    // ── Sidebar — docked applet panel (History / Bookmarks / Notes / Downloads) ──
    let split_view = libadwaita::OverlaySplitView::builder()
        .sidebar_position(gtk4::PackType::End)
        .min_sidebar_width(320.0)
        .max_sidebar_width(400.0)
        .sidebar_width_fraction(0.26)
        .show_sidebar(false)
        .build();
    toast_overlay.set_child(Some(&tab_view));
    split_view.set_content(Some(&toast_overlay));

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

    let find_bar = FindBar::build(&tab_view);

    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.add_top_bar(&tab_bar);
    toolbar_view.add_top_bar(&find_bar.root);
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
    let passwords_panel = PasswordsPanel::build();
    let notes_widget = crate::notes::build_widget();

    sidebar_stack.add_named(&history_panel.root, Some("history"));
    sidebar_stack.add_named(&bookmarks_panel.root, Some("bookmarks"));
    sidebar_stack.add_named(&notes_widget, Some("notes"));
    sidebar_stack.add_named(&downloads_panel.root, Some("downloads"));
    sidebar_stack.add_named(&passwords_panel.root, Some("passwords"));

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

    open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, &toast_overlay, NEWTAB_URI, true);

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
        &window, &tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, &toast_overlay, &find_bar,
        &split_view, &sidebar_stack, &sidebar_title, &history_panel, &bookmarks_panel, &passwords_panel,
    );

    setup_menu(
        &window, &menu_btn, &tab_view, &url_bar, &new_tab_btn, &reload_btn, &find_bar,
        &split_view, &sidebar_stack, &sidebar_title, &history_panel, &bookmarks_panel, &passwords_panel,
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
    toast_overlay: &libadwaita::ToastOverlay,
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

    let brand = gtk4::Button::builder()
        .label("VELO")
        .css_classes(vec!["brand-mark", "flat"])
        .tooltip_text("Go to start page")
        .build();

    brand.connect_clicked(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        move |_| {
            with_webview(&tab_view, |wv| wv.load_html(NEWTAB_HTML, None::<&str>));
            url_bar.set_text("");
        }
    ));

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
        #[weak] toast_overlay,
        move |_| {
            open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, &toast_overlay, NEWTAB_URI, true);
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
    toast_overlay: &libadwaita::ToastOverlay,
    url: &str,
    select: bool,
) -> libadwaita::TabPage {
    let webview = engine::create_webview();
    if url == NEWTAB_URI {
        webview.load_html(NEWTAB_HTML, None::<&str>);
    } else {
        webview.load_uri(url);
    }

    add_tab_for_webview(tab_view, url_bar, back_btn, forward_btn, reload_btn, toast_overlay, webview, select)
}

/// Wires a `WebView`'s signals into a new tab page — shared by `open_tab`
/// and the popup/new-window handling below, which builds the `WebView`
/// itself (via `connect_create`) before it has a tab to live in.
fn add_tab_for_webview(
    tab_view: &libadwaita::TabView,
    url_bar: &gtk4::Entry,
    back_btn: &gtk4::Button,
    forward_btn: &gtk4::Button,
    reload_btn: &gtk4::Button,
    toast_overlay: &libadwaita::ToastOverlay,
    webview: WebView,
    select: bool,
) -> libadwaita::TabPage {
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

    webview.connect_favicon_notify(glib::clone!(
        #[weak] page,
        move |wv| page.set_icon(wv.favicon().as_ref())
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
                    crate::backend::record_visit(uri.clone(), title);
                    try_autofill(wv, &uri);
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

    // ── Popups & target="_blank"/window.open() — open in a new background tab ──
    webview.connect_create(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] reload_btn,
        #[weak] toast_overlay,
        #[upgrade_or] None,
        move |wv, _nav_action| {
            let new_view = engine::create_related_webview(wv);
            let new_page = add_tab_for_webview(
                &tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, &toast_overlay, new_view.clone(), false,
            );
            new_view.connect_ready_to_show(glib::clone!(
                #[weak] tab_view,
                #[strong] new_page,
                #[upgrade_or] (),
                move |_| tab_view.set_selected_page(&new_page)
            ));
            Some(new_view.upcast::<gtk4::Widget>())
        }
    ));

    // ── Ctrl/middle-click links and target="_blank" navigations — open in a new tab ──
    webview.connect_decide_policy(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] reload_btn,
        #[weak] toast_overlay,
        #[upgrade_or] false,
        move |_wv, decision, decision_type| {
            let Some(nav_decision) = decision.downcast_ref::<NavigationPolicyDecision>() else {
                return false;
            };
            let Some(action) = nav_decision.navigation_action() else {
                return false;
            };

            let opens_new_tab = match decision_type {
                PolicyDecisionType::NewWindowAction => true,
                PolicyDecisionType::NavigationAction => {
                    action.navigation_type() == NavigationType::LinkClicked
                        && (action.mouse_button() == 2
                            || action.modifiers() & gtk4::gdk::ModifierType::CONTROL_MASK.bits() != 0)
                }
                _ => false,
            };

            if !opens_new_tab {
                return false;
            }

            let Some(uri) = action.request().and_then(|r| r.uri()) else {
                return false;
            };

            open_tab(&tab_view, &url_bar, &back_btn, &forward_btn, &reload_btn, &toast_overlay, &uri, false);
            decision.ignore();
            true
        }
    ));

    // ── Saved-login capture — prompts to save new/changed credentials ──
    if let Some(ucm) = webview.user_content_manager() {
        ucm.connect_script_message_received(Some(engine::PASSWORD_MESSAGE_HANDLER), glib::clone!(
            #[weak] webview,
            #[weak] toast_overlay,
            move |_ucm, value| handle_password_message(&webview, &toast_overlay, value)
        ));
    }

    page
}

fn setup_shortcuts(
    window: &libadwaita::ApplicationWindow,
    tab_view: &libadwaita::TabView,
    url_bar: &gtk4::Entry,
    back_btn: &gtk4::Button,
    forward_btn: &gtk4::Button,
    reload_btn: &gtk4::Button,
    toast_overlay: &libadwaita::ToastOverlay,
    find_bar: &FindBar,
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    history_panel: &HistoryPanel,
    bookmarks_panel: &BookmarksPanel,
    passwords_panel: &PasswordsPanel,
) {
    use gtk4::gdk::{Key, ModifierType};

    let key_ctl = gtk4::EventControllerKey::new();
    key_ctl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    window.add_controller(key_ctl.clone());

    let hp = history_panel.clone();
    let bp = bookmarks_panel.clone();
    let pp = passwords_panel.clone();
    let fb = find_bar.clone();

    key_ctl.connect_key_pressed(glib::clone!(
        #[weak] tab_view,
        #[weak] url_bar,
        #[weak] back_btn,
        #[weak] forward_btn,
        #[weak] reload_btn,
        #[weak] toast_overlay,
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
                    if fb.is_open() {
                        fb.close(&tab_view);
                        return glib::Propagation::Stop;
                    }
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
                                 &toast_overlay, NEWTAB_URI, true);
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
                    'p' if ctrl && !shift => {
                        toggle_passwords(&split_view, &sidebar_stack, &sidebar_title, &tab_view, &pp);
                        return glib::Propagation::Stop;
                    }
                    'f' if ctrl && !shift => {
                        fb.open();
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
    find_bar: &FindBar,
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    history_panel: &HistoryPanel,
    bookmarks_panel: &BookmarksPanel,
    passwords_panel: &PasswordsPanel,
) {
    let add_action = |name: &str| {
        let action = gio::SimpleAction::new(name, None);
        window.add_action(&action);
        action
    };

    let hp = history_panel.clone();
    let bp = bookmarks_panel.clone();
    let pp = passwords_panel.clone();

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

    add_action("toggle-passwords").connect_activate(glib::clone!(
        #[weak] tab_view,
        #[weak] split_view,
        #[weak] sidebar_stack,
        #[weak] sidebar_title,
        #[strong] pp,
        move |_, _| toggle_passwords(&split_view, &sidebar_stack, &sidebar_title, &tab_view, &pp)
    ));

    add_action("reload").connect_activate(glib::clone!(
        #[weak] reload_btn,
        move |_, _| reload_btn.emit_clicked()
    ));

    add_action("find-in-page").connect_activate(glib::clone!(
        #[strong] find_bar,
        move |_, _| find_bar.open()
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

    add_action("clear-browsing-data").connect_activate(glib::clone!(
        #[weak] window,
        #[weak] tab_view,
        move |_, _| confirm_clear_browsing_data(&window, &tab_view)
    ));

    let menu = gio::Menu::new();

    let nav = gio::Menu::new();
    nav.append(Some("New Tab"), Some("win.new-tab"));
    nav.append(Some("Reload"), Some("win.reload"));
    nav.append(Some("Find in Page"), Some("win.find-in-page"));
    menu.append_section(None, &nav);

    let applets = gio::Menu::new();
    applets.append(Some("Bookmark This Page"), Some("win.bookmark-page"));
    applets.append(Some("History"), Some("win.toggle-history"));
    applets.append(Some("Bookmarks"), Some("win.toggle-bookmarks"));
    applets.append(Some("Notes"), Some("win.toggle-notes"));
    applets.append(Some("Downloads"), Some("win.toggle-downloads"));
    applets.append(Some("Passwords"), Some("win.toggle-passwords"));
    menu.append_section(None, &applets);

    let zoom = gio::Menu::new();
    zoom.append(Some("Zoom In"), Some("win.zoom-in"));
    zoom.append(Some("Zoom Out"), Some("win.zoom-out"));
    zoom.append(Some("Reset Zoom"), Some("win.zoom-reset"));
    menu.append_section(None, &zoom);

    let privacy = gio::Menu::new();
    privacy.append(Some("Clear Browsing Data…"), Some("win.clear-browsing-data"));
    menu.append_section(None, &privacy);

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

fn toggle_passwords(
    split_view: &libadwaita::OverlaySplitView,
    sidebar_stack: &gtk4::Stack,
    sidebar_title: &gtk4::Label,
    tab_view: &libadwaita::TabView,
    pp: &PasswordsPanel,
) {
    if toggle_panel(split_view, sidebar_stack, sidebar_title, tab_view, "passwords", "PASSWORDS") {
        pp.refresh();
    }
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

// ── Password manager — autofill on load, capture on submit ─────────────────────

/// Looks up the most recently saved credential for `uri`'s origin and, if
/// found, fills it into the page's login form via `window.__veloFill`.
fn try_autofill(wv: &WebView, uri: &str) {
    let Some(origin) = crate::passwords::origin_from_uri(uri) else { return };
    let mut creds = crate::passwords::find_credentials_for_origin(&origin);
    creds.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    let Some(cred) = creds.into_iter().next() else { return };

    let username = cred.username.clone();
    crate::passwords::get_credential(origin, username, glib::clone!(
        #[weak] wv,
        #[upgrade_or] (),
        move |password| {
            let Some(password) = password else { return };
            let script = format!(
                "window.__veloFill && window.__veloFill({}, {})",
                js_string(&cred.username),
                js_string(&password),
            );
            wv.evaluate_javascript(&script, None, None, gtk4::gio::Cancellable::NONE, |_| {});
        }
    ));
}

/// JSON-encodes a string for splicing into an injected JS snippet.
fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

#[derive(Deserialize)]
struct LoginMessage {
    username: String,
    password: String,
}

/// Handles a login submission posted by `autofill.js`. If it differs from
/// any saved credential for this origin, offers to save it via a toast.
fn handle_password_message(webview: &WebView, toast_overlay: &libadwaita::ToastOverlay, value: &javascriptcore::Value) {
    if !value.is_string() { return; }
    let text = value.to_str();
    let Ok(msg) = serde_json::from_str::<LoginMessage>(&text) else { return };
    if msg.password.is_empty() { return; }

    let Some(uri) = webview.uri() else { return };
    let Some(origin) = crate::passwords::origin_from_uri(&uri) else { return };

    let LoginMessage { username, password } = msg;
    let username_for_toast = username.clone();
    let password_for_check = password.clone();
    let origin_for_toast = origin.clone();

    crate::passwords::get_credential(origin, username.clone(), glib::clone!(
        #[weak] toast_overlay,
        #[upgrade_or] (),
        move |existing| {
            if existing.as_deref() == Some(password_for_check.as_str()) {
                return;
            }
            show_save_password_toast(&toast_overlay, origin_for_toast, username_for_toast, password);
        }
    ));
}

/// Shows a toast offering to store a new or changed login for `origin`.
fn show_save_password_toast(toast_overlay: &libadwaita::ToastOverlay, origin: String, username: String, password: String) {
    let host = origin.split("://").nth(1).unwrap_or(origin.as_str()).to_string();
    let toast = libadwaita::Toast::builder()
        .title(format!("Save password for {host}?"))
        .button_label("Save")
        .timeout(8)
        .build();

    toast.connect_button_clicked(move |_| {
        crate::passwords::save_credential(origin.clone(), username.clone(), password.clone(), |_| {});
    });

    toast_overlay.add_toast(toast);
}

/// Asks for confirmation, then wipes cookies, cache, and site storage —
/// signing the user out of every site — and reloads the active tab.
fn confirm_clear_browsing_data(window: &libadwaita::ApplicationWindow, tab_view: &libadwaita::TabView) {
    let dialog = gtk4::AlertDialog::builder()
        .modal(true)
        .message("Clear browsing data?")
        .detail(
            "Cookies, cache, and site storage (including saved logins like Google) \
             will be deleted and you'll be signed out of every site. Saved passwords \
             in the Velo Passwords applet are not affected."
        )
        .buttons(["Cancel", "Clear Data"])
        .cancel_button(0)
        .default_button(0)
        .build();

    dialog.choose(Some(window), gtk4::gio::Cancellable::NONE, glib::clone!(
        #[weak] tab_view,
        #[upgrade_or] (),
        move |response| {
            if matches!(response, Ok(1)) {
                engine::clear_browsing_data(glib::clone!(
                    #[weak] tab_view,
                    #[upgrade_or] (),
                    move || with_webview(&tab_view, |wv| wv.reload())
                ));
            }
        }
    ));
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

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

// Import WebViewExt directly — avoids ambiguity with gtk4::WidgetExt::settings().
use webkit6::prelude::WebViewExt;
use webkit6::{
    Download, HardwareAccelerationPolicy, NetworkSession, UserContentInjectedFrames,
    UserContentManager, UserScript, UserScriptInjectionTime, WebContext, WebsiteDataTypes, WebView,
};

/// Name of the script message handler that the autofill script posts saved
/// (and new) login submissions to. See `src/autofill.js`.
pub const PASSWORD_MESSAGE_HANDLER: &str = "veloPasswords";

const AUTOFILL_SCRIPT: &str = include_str!("autofill.js");

thread_local! {
    // One shared context for all tabs: single network process, shared cache and cookies.
    static CONTEXT: OnceCell<WebContext> = const { OnceCell::new() };
    // Persistent network session — keeps cookies, local storage, IndexedDB, and
    // HTTP cache on disk so logins (e.g. Google) survive restarts.
    static SESSION: OnceCell<NetworkSession> = const { OnceCell::new() };
    static DOWNLOAD_HANDLER: RefCell<Option<Rc<dyn Fn(Download)>>> = const { RefCell::new(None) };
}

pub fn create_webview() -> WebView {
    CONTEXT.with(|ctx_cell| {
        let first_init = ctx_cell.get().is_none();
        let ctx = ctx_cell.get_or_init(WebContext::new);

        SESSION.with(|session_cell| {
            let session = session_cell.get_or_init(create_persistent_session);

            let ucm = create_user_content_manager();
            let webview = WebView::builder()
                .web_context(ctx)
                .network_session(session)
                .user_content_manager(&ucm)
                .build();
            configure_webview(&webview);

            if first_init {
                init_downloads(session);
            }

            webview
        })
    })
}

/// Builds the on-disk network session used by every tab — cookies, cache,
/// HSTS, and site storage are kept under `~/.local/share/velo/webkit` and
/// `~/.cache/velo/webkit` so they persist across restarts.
fn create_persistent_session() -> NetworkSession {
    let data_dir = glib::user_data_dir().join("velo/webkit");
    let cache_dir = glib::user_cache_dir().join("velo/webkit");
    std::fs::create_dir_all(&data_dir).ok();
    std::fs::create_dir_all(&cache_dir).ok();
    let session = NetworkSession::new(data_dir.to_str(), cache_dir.to_str());
    if let Some(data_manager) = session.website_data_manager() {
        data_manager.set_favicons_enabled(true);
    }
    session
}

/// Creates a new WebView related to `parent` — used for popups and
/// `target="_blank"`/`window.open()` requests so the new view shares the
/// same web context, cookies, and process group as the page that opened it.
pub fn create_related_webview(parent: &WebView) -> WebView {
    let ucm = create_user_content_manager();
    let webview = WebView::builder()
        .related_view(parent)
        .user_content_manager(&ucm)
        .build();
    configure_webview(&webview);
    webview
}

/// Builds a `UserContentManager` carrying the password capture/autofill
/// script and registers the message handler it posts logins to.
fn create_user_content_manager() -> UserContentManager {
    let ucm = UserContentManager::new();
    let script = UserScript::new(
        AUTOFILL_SCRIPT,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    );
    ucm.add_script(&script);
    ucm.register_script_message_handler(PASSWORD_MESSAGE_HANDLER, None);
    ucm
}

fn configure_webview(webview: &WebView) {
    // settings() returns Option<Settings> — always Some for a live WebView.
    let settings = webview.settings().expect("WebView has no settings");
    settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);
    settings.set_enable_javascript(true);
    settings.set_enable_developer_extras(true);
    settings.set_enable_smooth_scrolling(true);
    settings.set_javascript_can_open_windows_automatically(false);
    settings.set_media_playback_requires_user_gesture(true);
    // Don't initialize media stream pipeline at startup — avoids GStreamer NULL
    // errors on systems missing gst-plugins-good (autoaudiosink).
    settings.set_enable_media_stream(false);
    // Media Source Extensions — required for adaptive streaming (YouTube, etc.)
    settings.set_enable_mediasource(true);
    settings.set_user_agent_with_application_details(
        Some("Velo"),
        Some(env!("CARGO_PKG_VERSION")),
    );
}

/// Registers the callback invoked on the GTK main thread whenever a new
/// download starts. Replaces any previously registered handler.
pub fn set_download_handler(handler: impl Fn(Download) + 'static) {
    DOWNLOAD_HANDLER.with(|h| *h.borrow_mut() = Some(Rc::new(handler)));
}

/// Wipes all on-disk and in-memory site data (cookies, cache, local/session
/// storage, IndexedDB, service workers, HSTS/ITP state) for the shared
/// session — signs the user out of every site. `on_done` runs on the GTK
/// main thread once the wipe completes.
pub fn clear_browsing_data<F: FnOnce() + 'static>(on_done: F) {
    let session = SESSION.with(|s| s.get().cloned());
    let manager = session.and_then(|s| s.website_data_manager());
    let Some(manager) = manager else {
        on_done();
        return;
    };

    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    manager.clear(WebsiteDataTypes::all(), glib::TimeSpan(0), gio::Cancellable::NONE, {
        let done = done.clone();
        move |_| done.store(true, std::sync::atomic::Ordering::SeqCst)
    });

    let on_done = RefCell::new(Some(on_done));
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if !done.load(std::sync::atomic::Ordering::SeqCst) {
            return glib::ControlFlow::Continue;
        }
        if let Some(f) = on_done.borrow_mut().take() {
            f();
        }
        glib::ControlFlow::Break
    });
}

fn init_downloads(session: &NetworkSession) {
    session.connect_download_started(|_session, download| {
        download.connect_decide_destination(|download, suggested_filename| {
            let dest = unique_download_path(suggested_filename);
            if let Ok(uri) = glib::filename_to_uri(&dest, None) {
                download.set_destination(&uri);
            }
            true
        });

        DOWNLOAD_HANDLER.with(|h| {
            if let Some(handler) = h.borrow().as_ref() {
                handler(download.clone());
            }
        });
    });
}

/// Picks a free path in the user's Downloads directory, appending " (1)",
/// " (2)", etc. before the extension to avoid clobbering existing files.
fn unique_download_path(suggested_filename: &str) -> PathBuf {
    let dir = glib::user_special_dir(glib::UserDirectory::Downloads)
        .unwrap_or_else(|| glib::home_dir().join("Downloads"));
    std::fs::create_dir_all(&dir).ok();

    let name = if suggested_filename.is_empty() { "download" } else { suggested_filename };
    let path = std::path::Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
    let ext = path.extension().and_then(|s| s.to_str());

    let mut candidate = dir.join(name);
    let mut n = 1;
    while candidate.exists() {
        candidate = match ext {
            Some(ext) => dir.join(format!("{stem} ({n}).{ext}")),
            None => dir.join(format!("{stem} ({n})")),
        };
        n += 1;
    }
    candidate
}

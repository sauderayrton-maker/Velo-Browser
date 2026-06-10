use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

// Import WebViewExt directly — avoids ambiguity with gtk4::WidgetExt::settings().
use webkit6::prelude::WebViewExt;
use webkit6::{Download, HardwareAccelerationPolicy, NetworkSession, WebContext, WebView};

thread_local! {
    // One shared context for all tabs: single network process, shared cache and cookies.
    static CONTEXT: OnceCell<WebContext> = const { OnceCell::new() };
    static DOWNLOAD_HANDLER: RefCell<Option<Rc<dyn Fn(Download)>>> = const { RefCell::new(None) };
}

pub fn create_webview() -> WebView {
    CONTEXT.with(|cell| {
        let first_init = cell.get().is_none();
        let ctx = cell.get_or_init(WebContext::new);

        let webview = WebView::builder().web_context(ctx).build();

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

        if first_init {
            init_downloads();
        }

        webview
    })
}

/// Registers the callback invoked on the GTK main thread whenever a new
/// download starts. Replaces any previously registered handler.
pub fn set_download_handler(handler: impl Fn(Download) + 'static) {
    DOWNLOAD_HANDLER.with(|h| *h.borrow_mut() = Some(Rc::new(handler)));
}

fn init_downloads() {
    let Some(session) = NetworkSession::default() else { return };
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

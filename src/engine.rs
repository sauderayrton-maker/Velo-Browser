use std::cell::OnceCell;
// Import WebViewExt directly — avoids ambiguity with gtk4::WidgetExt::settings().
use webkit6::prelude::WebViewExt;
use webkit6::{HardwareAccelerationPolicy, WebContext, WebView};

thread_local! {
    // One shared context for all tabs: single network process, shared cache and cookies.
    static CONTEXT: OnceCell<WebContext> = const { OnceCell::new() };
}

pub fn create_webview() -> WebView {
    CONTEXT.with(|cell| {
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
        settings.set_user_agent_with_application_details(
            Some("Velo"),
            Some(env!("CARGO_PKG_VERSION")),
        );

        webview
    })
}

use std::cell::OnceCell;
use webkit6::{prelude::*, HardwareAccelerationPolicy, WebContext, WebView};

thread_local! {
    // One shared context for all tabs: single network process, shared cache and cookies.
    static CONTEXT: OnceCell<WebContext> = const { OnceCell::new() };
}

pub fn create_webview() -> WebView {
    CONTEXT.with(|cell| {
        let ctx = cell.get_or_init(build_context);

        let webview = WebView::builder().web_context(ctx).build();

        let settings = webview.settings();
        // Always use the GPU compositor — eliminates software fallback stalls.
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

fn build_context() -> WebContext {
    // Persist cache and site data between sessions.
    let cache_dir = {
        let mut p = glib::user_cache_dir();
        p.push("velo");
        p.to_string_lossy().into_owned()
    };
    let data_dir = {
        let mut p = glib::user_data_dir();
        p.push("velo");
        p.to_string_lossy().into_owned()
    };

    let data_mgr = webkit6::WebsiteDataManager::builder()
        .base_cache_directory(&cache_dir)
        .base_data_directory(&data_dir)
        .build();

    WebContext::builder()
        .website_data_manager(&data_mgr)
        .build()
}

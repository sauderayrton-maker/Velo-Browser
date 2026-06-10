use gtk4::prelude::*;
use std::time::Duration;

mod backend;
mod bookmarks;
mod downloads;
mod engine;
mod history;
mod notes;
mod window;

fn main() -> glib::ExitCode {
    ensure_backend_running();

    let app = libadwaita::Application::builder()
        .application_id("com.velo.Browser")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(|app| {
        libadwaita::StyleManager::default()
            .set_color_scheme(libadwaita::ColorScheme::ForceDark);
        window::build_browser_window(app).present();
    });

    app.run()
}

/// Starts the velo-backend service if it's not already reachable on :7777.
/// Falls back to a Docker-managed instance if velo-backend isn't installed —
/// either way, the browser works fine without it (history/bookmarks just no-op).
fn ensure_backend_running() {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(200))
        .timeout(Duration::from_millis(200))
        .build();

    if agent.get("http://127.0.0.1:7777/api/health").call().is_ok() {
        return;
    }

    let data_dir = xdg_data_dir().join("velo");
    std::fs::create_dir_all(&data_dir).ok();

    if std::process::Command::new("velo-backend")
        .env("DATA_DIR", &data_dir)
        .spawn()
        .is_ok()
    {
        // Give it a moment to bind the socket before tabs start loading.
        std::thread::sleep(Duration::from_millis(450));
    }
}

fn xdg_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".local/share")
}

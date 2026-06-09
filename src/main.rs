use gtk4::prelude::*;

mod backend;
mod bookmarks;
mod engine;
mod history;
mod notes;
mod window;

fn main() -> glib::ExitCode {
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

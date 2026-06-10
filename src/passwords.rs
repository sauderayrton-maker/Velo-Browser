use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Metadata for one saved login. The password itself never touches this
/// struct or disk — it lives in the OS keyring (Secret Service / KWallet),
/// keyed by `velo:<origin>` + username.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CredentialMeta {
    pub origin: String,
    pub username: String,
    pub saved_at: String,
}

fn index_path() -> PathBuf {
    let dir = glib::user_data_dir().join("velo");
    std::fs::create_dir_all(&dir).ok();
    dir.join("passwords.json")
}

fn keyring_service(origin: &str) -> String {
    format!("velo:{origin}")
}

/// Extracts a stable `scheme://host[:port]` origin from a page URI for
/// matching saved logins — paths, queries, and credentials are dropped.
pub fn origin_from_uri(uri: &str) -> Option<String> {
    let parsed = glib::Uri::parse(uri, glib::UriFlags::NONE).ok()?;
    let scheme = parsed.scheme();
    let host = parsed.host()?;
    let port = parsed.port();
    if port > 0 {
        Some(format!("{scheme}://{host}:{port}"))
    } else {
        Some(format!("{scheme}://{host}"))
    }
}

/// Reads the local credential index (origins, usernames, save times). This
/// is a small local JSON file, so it's read synchronously.
pub fn list_credentials() -> Vec<CredentialMeta> {
    std::fs::read_to_string(index_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Returns saved logins for the given origin.
pub fn find_credentials_for_origin(origin: &str) -> Vec<CredentialMeta> {
    list_credentials()
        .into_iter()
        .filter(|c| c.origin == origin)
        .collect()
}

fn write_index(entries: &[CredentialMeta]) {
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        std::fs::write(index_path(), json).ok();
    }
}

/// Polls `slot` on the GTK main loop until the background thread fills it
/// in, then hands the result to `on_done`. Mirrors the pattern in backend.rs.
fn deliver<T: Send + 'static, F: FnOnce(T) + 'static>(slot: Arc<Mutex<Option<T>>>, on_done: F) {
    let cb = RefCell::new(Some(on_done));
    glib::timeout_add_local(Duration::from_millis(40), move || {
        if let Some(data) = slot.lock().unwrap().take() {
            if let Some(f) = cb.borrow_mut().take() {
                f(data);
            }
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

/// Saves (or overwrites) a credential: the password goes to the OS keyring
/// and the origin/username/timestamp go into the local index. Calls
/// `on_done(true)` once the write completes successfully.
pub fn save_credential<F: FnOnce(bool) + 'static>(
    origin: String,
    username: String,
    password: String,
    on_done: F,
) {
    let slot: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let slot_bg = Arc::clone(&slot);

    std::thread::spawn(move || {
        let ok = Entry::new(&keyring_service(&origin), &username)
            .and_then(|e| e.set_password(&password))
            .is_ok();

        if ok {
            let mut entries = list_credentials();
            entries.retain(|c| !(c.origin == origin && c.username == username));
            entries.push(CredentialMeta {
                origin,
                username,
                saved_at: glib::DateTime::now_local()
                    .and_then(|d| d.format_iso8601())
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
            });
            write_index(&entries);
        }

        *slot_bg.lock().unwrap() = Some(ok);
    });

    deliver(slot, on_done);
}

/// Looks up the password for `origin`+`username` in the OS keyring.
pub fn get_credential<F: FnOnce(Option<String>) + 'static>(
    origin: String,
    username: String,
    on_done: F,
) {
    let slot: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
    let slot_bg = Arc::clone(&slot);

    std::thread::spawn(move || {
        let password = Entry::new(&keyring_service(&origin), &username)
            .and_then(|e| e.get_password())
            .ok();
        *slot_bg.lock().unwrap() = Some(password);
    });

    deliver(slot, on_done);
}

/// Removes a saved credential from both the OS keyring and the local index.
/// Calls `on_done(true)` once the removal completes successfully.
pub fn delete_credential<F: FnOnce(bool) + 'static>(
    origin: String,
    username: String,
    on_done: F,
) {
    let slot: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let slot_bg = Arc::clone(&slot);

    std::thread::spawn(move || {
        let ok = Entry::new(&keyring_service(&origin), &username)
            .and_then(|e| e.delete_credential())
            .is_ok();

        if ok {
            let mut entries = list_credentials();
            entries.retain(|c| !(c.origin == origin && c.username == username));
            write_index(&entries);
        }

        *slot_bg.lock().unwrap() = Some(ok);
    });

    deliver(slot, on_done);
}

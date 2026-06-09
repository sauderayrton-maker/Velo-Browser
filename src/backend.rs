use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BASE: &str = "http://127.0.0.1:7777";

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout(Duration::from_secs(8))
        .build()
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct HistoryEntry {
    pub id: Option<i64>,
    pub url: String,
    pub title: String,
    pub visited_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Bookmark {
    pub id: Option<i64>,
    pub url: String,
    pub title: String,
    pub created_at: Option<String>,
}

pub fn record_visit(url: String, title: String) {
    std::thread::spawn(move || {
        let entry = HistoryEntry { url, title, ..Default::default() };
        let _ = agent().post(&format!("{BASE}/api/history")).send_json(&entry);
    });
}

/// Fetches history in a background thread, delivers result to the GTK main thread via callback.
pub fn fetch_history<F: FnOnce(Vec<HistoryEntry>) + 'static>(on_done: F) {
    let slot: Arc<Mutex<Option<Vec<HistoryEntry>>>> = Arc::new(Mutex::new(None));
    let slot_bg = Arc::clone(&slot);

    std::thread::spawn(move || {
        let data = agent()
            .get(&format!("{BASE}/api/history"))
            .call()
            .ok()
            .and_then(|r| r.into_json::<Vec<HistoryEntry>>().ok())
            .unwrap_or_default();
        *slot_bg.lock().unwrap() = Some(data);
    });

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

/// Fetches bookmarks in a background thread, delivers result to the GTK main thread via callback.
pub fn fetch_bookmarks<F: FnOnce(Vec<Bookmark>) + 'static>(on_done: F) {
    let slot: Arc<Mutex<Option<Vec<Bookmark>>>> = Arc::new(Mutex::new(None));
    let slot_bg = Arc::clone(&slot);

    std::thread::spawn(move || {
        let data = agent()
            .get(&format!("{BASE}/api/bookmarks"))
            .call()
            .ok()
            .and_then(|r| r.into_json::<Vec<Bookmark>>().ok())
            .unwrap_or_default();
        *slot_bg.lock().unwrap() = Some(data);
    });

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

pub fn add_bookmark(url: String, title: String) {
    std::thread::spawn(move || {
        let bm = Bookmark { url, title, ..Default::default() };
        let _ = agent().post(&format!("{BASE}/api/bookmarks")).send_json(&bm);
    });
}

pub fn remove_bookmark(id: i64) {
    std::thread::spawn(move || {
        let _ = agent().delete(&format!("{BASE}/api/bookmarks/{id}")).call();
    });
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct HistoryEntry {
    id: Option<i64>,
    url: String,
    title: String,
    visited_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Bookmark {
    id: Option<i64>,
    url: String,
    title: String,
    created_at: Option<String>,
}

#[tokio::main]
async fn main() {
    let path = std::env::var("DATA_DIR").unwrap_or_else(|_| "/data".into());
    std::fs::create_dir_all(&path).ok();
    let db_path = format!("{path}/velo.db");

    let conn = Connection::open(&db_path).expect("open db");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            visited_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS bookmarks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .expect("init schema");

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/history", get(get_history).post(add_history))
        .route("/api/bookmarks", get(get_bookmarks).post(add_bookmark))
        .route("/api/bookmarks/:id", delete(delete_bookmark))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7777")
        .await
        .expect("bind");
    println!("velo-backend listening on :7777");
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> &'static str {
    "ok"
}

async fn get_history(State(s): State<AppState>) -> Json<Vec<HistoryEntry>> {
    let db = s.db.lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT id, url, title, visited_at FROM history
             ORDER BY visited_at DESC LIMIT 500",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at: row.get(3)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(rows)
}

async fn add_history(
    State(s): State<AppState>,
    Json(entry): Json<HistoryEntry>,
) -> StatusCode {
    let db = s.db.lock().unwrap();
    db.execute(
        "INSERT INTO history (url, title) VALUES (?1, ?2)",
        params![entry.url, entry.title],
    )
    .ok();
    StatusCode::CREATED
}

async fn get_bookmarks(State(s): State<AppState>) -> Json<Vec<Bookmark>> {
    let db = s.db.lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT id, url, title, created_at FROM bookmarks
             ORDER BY created_at DESC",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(rows)
}

async fn add_bookmark(
    State(s): State<AppState>,
    Json(bm): Json<Bookmark>,
) -> StatusCode {
    let db = s.db.lock().unwrap();
    db.execute(
        "INSERT OR IGNORE INTO bookmarks (url, title) VALUES (?1, ?2)",
        params![bm.url, bm.title],
    )
    .ok();
    StatusCode::CREATED
}

async fn delete_bookmark(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> StatusCode {
    let db = s.db.lock().unwrap();
    db.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])
        .ok();
    StatusCode::NO_CONTENT
}

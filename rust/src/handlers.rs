use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::fs;
use tracing::error;

use crate::{
    models::{CardFormAddressInput, CardFormEmailInput, CardFormPhoneInput, CardInput, HealthResponse},
    store,
};

pub struct AppState {
    pub conn: Arc<Mutex<Connection>>,
    pub uploads_dir: String,
}

fn internal_error(msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    error!("{}", msg);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": msg.to_string()})),
    )
}

fn not_found(msg: &str) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg})))
}

fn bad_request(msg: &str) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg})))
}

// ────────────────────────────────────────────────────────────────────────────
// Request logging (nginx-style)
// ────────────────────────────────────────────────────────────────────────────

fn log_request(method: &Method, uri: &Uri, headers: &HeaderMap, status: StatusCode) {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let referer = headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    tracing::info!(
        "{} {} {} {} \"{}\" \"{}\"",
        method.as_str(),
        uri.path(),
        uri.query().unwrap_or(""),
        status.as_u16(),
        user_agent,
        referer
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Multipart helpers
// ────────────────────────────────────────────────────────────────────────────

struct MultipartFields {
    text: std::collections::HashMap<String, String>,
    photo: Option<(String, Vec<u8>)>, // (original filename, bytes)
}

async fn collect_multipart(mut multipart: Multipart) -> Result<MultipartFields, String> {
    let mut text = std::collections::HashMap::new();
    let mut photo: Option<(String, Vec<u8>)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| format!("multipart error: {e}"))?
    {
        let name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().map(|s| s.to_string());

        if name == "photo" && filename.is_some() {
            let fname = filename.unwrap();
            let data = field
                .bytes()
                .await
                .map_err(|e| format!("read photo error: {e}"))?;
            if data.len() > 5 * 1024 * 1024 {
                return Err("photo exceeds 5MB limit".to_string());
            }
            photo = Some((fname, data.to_vec()));
        } else {
            let value = field
                .text()
                .await
                .map_err(|e| format!("read field error: {e}"))?;
            text.insert(name, value);
        }
    }

    Ok(MultipartFields { text, photo })
}

fn parse_card_input(fields: &MultipartFields) -> Result<CardInput, String> {
    let name = fields
        .text
        .get("name")
        .cloned()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "name is required".to_string())?;

    let phones: Vec<CardFormPhoneInput> = fields
        .text
        .get("phones")
        .map(|s| serde_json::from_str(s).unwrap_or_default())
        .unwrap_or_default();

    let emails: Vec<CardFormEmailInput> = fields
        .text
        .get("emails")
        .map(|s| serde_json::from_str(s).unwrap_or_default())
        .unwrap_or_default();

    let addresses: Vec<CardFormAddressInput> = fields
        .text
        .get("addresses")
        .map(|s| serde_json::from_str(s).unwrap_or_default())
        .unwrap_or_default();

    let tags: Vec<String> = fields
        .text
        .get("tags")
        .map(|s| serde_json::from_str(s).unwrap_or_default())
        .unwrap_or_default();

    Ok(CardInput {
        name,
        title: fields.text.get("title").cloned().unwrap_or_default(),
        company: fields.text.get("company").cloned().unwrap_or_default(),
        website: fields.text.get("website").cloned().unwrap_or_default(),
        notes: fields.text.get("notes").cloned().unwrap_or_default(),
        phones,
        emails,
        addresses,
        tags,
    })
}

async fn save_photo(
    uploads_dir: &str,
    card_id: i64,
    filename: &str,
    data: &[u8],
) -> Result<String, String> {
    // validate extension
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return Err("only jpg, png, webp photos are allowed".to_string());
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let new_filename = format!("card_{card_id}_{timestamp}.{ext}");
    let path = std::path::Path::new(uploads_dir).join(&new_filename);

    fs::create_dir_all(uploads_dir)
        .await
        .map_err(|e| format!("create uploads dir: {e}"))?;

    fs::write(&path, data)
        .await
        .map_err(|e| format!("write photo: {e}"))?;

    Ok(format!("uploads/{new_filename}"))
}

async fn remove_file_if_exists(uploads_dir: &str, photo_path: &str) {
    if photo_path.is_empty() {
        return;
    }
    // photo_path stored as "uploads/filename"
    let filename = photo_path.trim_start_matches("uploads/");
    let path = std::path::Path::new(uploads_dir).join(filename);
    let _ = fs::remove_file(path).await;
}

// ────────────────────────────────────────────────────────────────────────────
// Handlers
// ────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub tag: Option<String>,
}

pub async fn list_cards(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let conn = state.conn.clone();
    let q = params.q.clone();
    let tag = params.tag.clone();

    let result = tokio::task::spawn_blocking(move || {
        store::list_cards(&conn, q.as_deref(), tag.as_deref())
    })
    .await;

    match result {
        Ok(Ok(cards)) => (StatusCode::OK, Json(json!(cards))).into_response(),
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn get_card(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.conn.clone();

    let result =
        tokio::task::spawn_blocking(move || store::get_card(&conn, id)).await;

    match result {
        Ok(Ok(Some(card))) => (StatusCode::OK, Json(json!(card))).into_response(),
        Ok(Ok(None)) => not_found("card not found").into_response(),
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn create_card(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> impl IntoResponse {
    let fields = match collect_multipart(multipart).await {
        Ok(f) => f,
        Err(e) => return bad_request(&e).into_response(),
    };

    let input = match parse_card_input(&fields) {
        Ok(i) => i,
        Err(e) => return bad_request(&e).into_response(),
    };

    let photo_data = fields.photo;
    let conn = state.conn.clone();
    let uploads_dir = state.uploads_dir.clone();

    // Insert card first to get the ID
    let result =
        tokio::task::spawn_blocking(move || store::create_card(&conn, &input)).await;

    let card_id = match result {
        Ok(Ok(id)) => id,
        Ok(Err(e)) => return internal_error(e).into_response(),
        Err(e) => return internal_error(e).into_response(),
    };

    // Save photo if provided
    if let Some((filename, data)) = photo_data {
        match save_photo(&uploads_dir, card_id, &filename, &data).await {
            Ok(photo_path) => {
                let conn2 = state.conn.clone();
                let path_clone = photo_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    store::update_card_photo(&conn2, card_id, &path_clone)
                })
                .await;
            }
            Err(e) => return internal_error(e).into_response(),
        }
    }

    // Fetch and return
    let conn3 = state.conn.clone();
    let result =
        tokio::task::spawn_blocking(move || store::get_card(&conn3, card_id)).await;

    match result {
        Ok(Ok(Some(card))) => (StatusCode::CREATED, Json(json!(card))).into_response(),
        Ok(Ok(None)) => internal_error("card created but not found").into_response(),
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn update_card(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> impl IntoResponse {
    // Verify card exists
    let conn = state.conn.clone();
    let exists = tokio::task::spawn_blocking(move || store::get_card(&conn, id)).await;
    match exists {
        Ok(Ok(None)) => return not_found("card not found").into_response(),
        Ok(Err(e)) => return internal_error(e).into_response(),
        Err(e) => return internal_error(e).into_response(),
        _ => {}
    }

    let fields = match collect_multipart(multipart).await {
        Ok(f) => f,
        Err(e) => return bad_request(&e).into_response(),
    };

    let input = match parse_card_input(&fields) {
        Ok(i) => i,
        Err(e) => return bad_request(&e).into_response(),
    };

    let photo_data = fields.photo;
    let uploads_dir = state.uploads_dir.clone();

    let conn2 = state.conn.clone();
    let update_result =
        tokio::task::spawn_blocking(move || store::update_card(&conn2, id, &input)).await;

    match update_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return internal_error(e).into_response(),
        Err(e) => return internal_error(e).into_response(),
    }

    // Save new photo if provided
    if let Some((filename, data)) = photo_data {
        // Get old photo path (so we can delete the file), then clear it
        let conn3 = state.conn.clone();
        let old_path = tokio::task::spawn_blocking(move || store::delete_card_photo(&conn3, id))
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten()
            .unwrap_or_default();
        remove_file_if_exists(&uploads_dir, &old_path).await;

        match save_photo(&uploads_dir, id, &filename, &data).await {
            Ok(photo_path) => {
                let conn4 = state.conn.clone();
                let path_clone = photo_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    store::update_card_photo(&conn4, id, &path_clone)
                })
                .await;
            }
            Err(e) => return internal_error(e).into_response(),
        }
    }

    // Fetch and return updated card
    let conn5 = state.conn.clone();
    let result =
        tokio::task::spawn_blocking(move || store::get_card(&conn5, id)).await;

    match result {
        Ok(Ok(Some(card))) => (StatusCode::OK, Json(json!(card))).into_response(),
        Ok(Ok(None)) => not_found("card not found").into_response(),
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn delete_card(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> impl IntoResponse {
    let conn = state.conn.clone();
    let uploads_dir = state.uploads_dir.clone();

    let result =
        tokio::task::spawn_blocking(move || store::delete_card(&conn, id)).await;

    let (response, status) = match result {
        Ok(Ok(Some(old_photo))) => {
            remove_file_if_exists(&uploads_dir, &old_photo).await;
            (StatusCode::NO_CONTENT.into_response(), StatusCode::NO_CONTENT)
        }
        Ok(Ok(None)) => {
            let resp = not_found("card not found").into_response();
            let status = StatusCode::NOT_FOUND;
            (resp, status)
        }
        Ok(Err(e)) => {
            let resp = internal_error(e).into_response();
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            (resp, status)
        }
        Err(e) => {
            let resp = internal_error(e).into_response();
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            (resp, status)
        }
    };

    log_request(&method, &uri, &headers, status);
    response
}

pub async fn upload_photo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> impl IntoResponse {
    // Verify card exists
    let conn = state.conn.clone();
    let exists =
        tokio::task::spawn_blocking(move || store::get_card(&conn, id)).await;
    match exists {
        Ok(Ok(None)) => return not_found("card not found").into_response(),
        Ok(Err(e)) => return internal_error(e).into_response(),
        Err(e) => return internal_error(e).into_response(),
        _ => {}
    }

    let fields = match collect_multipart(multipart).await {
        Ok(f) => f,
        Err(e) => return bad_request(&e).into_response(),
    };

    let (filename, data) = match fields.photo {
        Some(p) => p,
        None => return bad_request("no photo field provided").into_response(),
    };

    let uploads_dir = state.uploads_dir.clone();

    // Delete old photo
    let conn2 = state.conn.clone();
    let old_path = tokio::task::spawn_blocking(move || store::delete_card_photo(&conn2, id))
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten()
        .unwrap_or_default();
    remove_file_if_exists(&uploads_dir, &old_path).await;

    // Save new photo
    let photo_path = match save_photo(&uploads_dir, id, &filename, &data).await {
        Ok(p) => p,
        Err(e) => return internal_error(e).into_response(),
    };

    let conn3 = state.conn.clone();
    let path_clone = photo_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        store::update_card_photo(&conn3, id, &path_clone)
    })
    .await;

    match result {
        Ok(Ok(())) => {
            let photo_url = format!("/{photo_path}");
            (StatusCode::OK, Json(json!({"photo_url": photo_url}))).into_response()
        }
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn delete_photo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.conn.clone();
    let uploads_dir = state.uploads_dir.clone();

    let result =
        tokio::task::spawn_blocking(move || store::delete_card_photo(&conn, id)).await;

    match result {
        Ok(Ok(Some(old_path))) => {
            remove_file_if_exists(&uploads_dir, &old_path).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(Ok(None)) => not_found("card not found").into_response(),
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn list_tags(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let conn = state.conn.clone();
    let result =
        tokio::task::spawn_blocking(move || store::list_tags(&conn)).await;

    match result {
        Ok(Ok(tags)) => (StatusCode::OK, Json(json!(tags))).into_response(),
        Ok(Err(e)) => internal_error(e).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let conn = state.conn.clone();
    let db_ok = tokio::task::spawn_blocking(move || {
        let c = conn.lock().unwrap();
        c.execute_batch("SELECT 1").is_ok()
    })
    .await
    .unwrap_or(false);

    let resp = HealthResponse {
        status: "ok".to_string(),
        db: if db_ok { "ok".to_string() } else { "error".to_string() },
    };
    (StatusCode::OK, Json(json!(resp)))
}

// ────────────────────────────────────────────────────────────────────────────
// Static file serving
// ────────────────────────────────────────────────────────────────────────────

pub async fn serve_uploads(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    // Prevent path traversal
    if filename.contains("..") || filename.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid filename").into_response();
    }

    let path = std::path::Path::new(&state.uploads_dir).join(&filename);

    match fs::read(&path).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(data))
                .unwrap()
                .into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "file not found").into_response(),
    }
}

pub async fn serve_index() -> impl IntoResponse {
    use crate::Asset;

    match Asset::get("index.html") {
        Some(content) => {
            let bytes = content.data.into_owned();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(bytes))
                .unwrap()
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
    }
}

//! Serving the built Vue application.
//!
//! dirtybase serves `public/` at `/assets` only, so the SPA is served here
//! instead: `dist/index.html` for `/` and for every client-side route, and real
//! files from `dist/` when one matches. Until the frontend is built the same
//! placeholder page the previous backend showed is returned, which keeps the API
//! usable on its own.

use axum::response::{IntoResponse, Response};

use crate::http::{Wildcard, cache_headers};

/// Where `npm run build` writes the frontend.
fn dist_dir() -> std::path::PathBuf {
    std::env::var("PADDOCK_DIST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("./dist"))
}

/// The page shown when `dist/` has not been built.
fn placeholder() -> String {
    "<!doctype html><meta charset=\"utf-8\"><title>Paddock Manager API</title>\
     <body style=\"font:16px system-ui;padding:2rem\">\
     <h1>Paddock Manager API</h1>\
     <p>The UI is not built yet. Run <code>npm run build</code> for production,\
     or <code>npm run dev</code> and open the Vite dev server.</p></body>"
        .to_string()
}

fn html_response(body: String, status: axum::http::StatusCode) -> Response {
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

/// Read a file from `dist/`, refusing paths that try to escape it.
async fn dist_file(relative: &str) -> Option<Vec<u8>> {
    let clean = relative.trim_start_matches('/');
    if clean.is_empty() || clean.contains("..") {
        return None;
    }
    tokio::fs::read(dist_dir().join(clean)).await.ok()
}

/// `GET /`
pub async fn index() -> Response {
    match dist_file("index.html").await {
        Some(bytes) => (
            [(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/html; charset=utf-8"),
            )],
            cache_headers(0),
            bytes,
        )
            .into_response(),
        None => html_response(placeholder(), axum::http::StatusCode::OK),
    }
}

/// `GET /{*rest}`
///
/// Anything without a file extension is a client-side route, so it gets
/// `index.html` and the router takes over. A request for a real file that does
/// not exist is a 404 rather than a silently wrong page.
pub async fn fallback(Wildcard(path): Wildcard) -> Response {
    let last = path.rsplit('/').next().unwrap_or_default();
    if last.contains('.') {
        if let Some(bytes) = dist_file(&path).await {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            return (
                [(
                    axum::http::header::CONTENT_TYPE,
                    mime.essence_str().to_string(),
                )],
                cache_headers(3600),
                bytes,
            )
                .into_response();
        }
        return crate::http::ApiError::not_found("Not found").into_response();
    }

    match dist_file("index.html").await {
        Some(bytes) => (
            [(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/html; charset=utf-8"),
            )],
            cache_headers(0),
            bytes,
        )
            .into_response(),
        None => html_response(placeholder(), axum::http::StatusCode::OK),
    }
}

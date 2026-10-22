//! HTTP handlers.
//!
//! Handlers return `ApiResult<T>`: `Ok` serialises as bare JSON (no envelope)
//! and `Err` becomes `{"error": "..."}` with the status attached. The frontend
//! reads `payload.error` on failure, so that shape is part of the contract.
//!
//! Handlers take their database handle as `CtxExt<Db>`, which dirtybase resolves
//! from the request context.

pub mod dashboard;
pub mod entries;
pub mod farms;
pub mod mixes;
pub mod photos;
pub mod spa;
pub mod tasks;

use axum::Json;
use axum::extract::FromRequest;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// A handler result: bare JSON on success, `{ "error": ... }` on failure.
pub type ApiResult<T> = Result<T, ApiError>;

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, msg)
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }
    /// 404 that also aborts with a message, for the routes where both apply.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::internal(e.to_string())
    }
}

/// 204 No Content, the response every delete returns.
pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Path parameters, already deserialized by axum into a `HashMap<String, String>`
/// substitute. `Path<(i64,)>` works for fixed-shape routes; this is for routes
/// with a variable tail such as the PDF path.
#[derive(Debug, Clone)]
pub struct Wildcard(pub String);

impl<S> FromRequest<S> for Wildcard
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request(
        req: axum::extract::Request,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let path = req.uri().path().to_string();
        Ok(Self(path))
    }
}

/// Collect query parameters into a plain map.
///
/// Repeated keys keep the last value, which matches how the frontend sends
/// filters.
#[derive(Debug, Clone, Default)]
pub struct Query(pub HashMap<String, String>);

impl<S> FromRequest<S> for Query
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request(
        req: axum::extract::Request,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let query = req.uri().query().unwrap_or("");
        let mut map = HashMap::new();
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (k, v) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            map.insert(percent_decode(k), percent_decode(v));
        }
        Ok(Self(map))
    }
}

impl Query {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0
            .get(key)
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
    }
    pub fn str_or(&self, key: &str, fallback: &str) -> String {
        self.get(key).unwrap_or(fallback).to_string()
    }
    pub fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.trim().parse().ok())
    }
    /// Only `"true"` counts as set, matching the original query handling.
    pub fn flag(&self, key: &str) -> bool {
        self.get(key) == Some("true")
    }
    pub fn i64_or(&self, key: &str, fallback: i64) -> i64 {
        self.i64(key).unwrap_or(fallback)
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// An already-parsed request body, so a handler can pull several optional keys
/// without caring whether the client sent them.
#[derive(Debug, Clone)]
pub struct Body(pub Map<String, Value>);

impl<S> FromRequest<S> for Body
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request(
        req: axum::extract::Request,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let bytes = match axum::body::to_bytes(req.into_body(), 4 * 1024 * 1024).await {
            Ok(b) => b,
            Err(_) => return Ok(Self(Map::new())),
        };
        let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Ok(Self(match parsed {
            Value::Object(m) => m,
            _ => Map::new(),
        }))
    }
}

impl Body {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn str(&self, key: &str) -> Option<String> {
        self.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Trimmed string, treating absent and non-string as empty.
    pub fn text(&self, key: &str) -> String {
        self.str(key).unwrap_or_default().trim().to_string()
    }

    /// String preferring `key`, falling back to `alt` for snake/camel variants.
    pub fn text_any(&self, key: &str, alt: &str) -> String {
        let a = self.text(key);
        if a.is_empty() { self.text(alt) } else { a }
    }

    pub fn i64(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::String(s)) => s.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn i64_any(&self, key: &str, alt: &str) -> Option<i64> {
        self.i64(key).or_else(|| self.i64(alt))
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(Value::Number(n)) => n.as_f64().filter(|v| v.is_finite()),
            Some(Value::String(s)) => {
                let t = s.trim();
                if t.is_empty() { None } else { t.parse().ok() }
            }
            _ => None,
        }
    }

    /// Float where an empty string means "not set" rather than zero, so a
    /// cleared numeric field stores NULL.
    pub fn f64_blank_is_null(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(Value::String(s)) if s.trim().is_empty() => None,
            _ => self.f64(key),
        }
    }

    /// Optional string where an empty string clears the column.
    pub fn opt_text(&self, key: &str) -> Option<String> {
        self.str(key).map(|s| s.trim().to_string())
    }

    /// Like `opt_text` but preferring `key` and falling back to `alt`, which is
    /// how the snake/camel variants of the same field are merged.
    pub fn opt_text_any(&self, key: &str, alt: &str) -> Option<String> {
        self.opt_text(key).or_else(|| self.opt_text(alt))
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| match v {
            Value::Bool(b) => Some(*b),
            Value::Number(n) => Some(n.as_f64().unwrap_or(0.0) != 0.0),
            Value::String(s) => Some(matches!(s.trim(), "true" | "1" | "yes")),
            _ => None,
        })
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn object(&self, key: &str) -> Option<Value> {
        self.get(key).filter(|v| v.is_object()).cloned()
    }

    pub fn array(&self, key: &str) -> Option<Vec<Value>> {
        self.get(key).and_then(|v| v.as_array()).cloned()
    }

    pub fn value(&self, key: &str) -> Value {
        self.get(key).cloned().unwrap_or(Value::Null)
    }
}

/// Content-Disposition for a download, so the browser saves with the intended
/// name.
pub fn attachment(filename: &str) -> HeaderValue {
    let safe = filename.replace(['"', '\r', '\n'], "");
    HeaderValue::from_str(&format!("attachment; filename=\"{safe}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// Parse an id path segment.
pub fn parse_id(segment: &str) -> ApiResult<i64> {
    segment
        .parse::<i64>()
        .map_err(|_| ApiError::bad_request("Invalid id"))
}

pub fn cache_headers(max_age_secs: u32) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "cache-control",
        HeaderValue::from_str(&format!("public, max-age={max_age_secs}"))
            .unwrap_or_else(|_| HeaderValue::from_static("public, max-age=0")),
    );
    h
}

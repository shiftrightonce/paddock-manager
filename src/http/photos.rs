//! Photo upload and serving, plus the external lookup links and the reference
//! data endpoints.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use dirtybase_contract::app_contract::CtxExt;
use serde_json::json;

use crate::db::*;
use crate::domain;
use crate::http::{ApiError, ApiResult, Body, Query, cache_headers};
use crate::models::*;
use crate::reference;

/// Maximum photos per request and per file.
const MAX_FILES: usize = 8;
const MAX_FILE_BYTES: usize = 12 * 1024 * 1024;

/// `POST /api/entries/{id}/photos`
///
/// Multipart bodies are read through axum's extractor so photo bytes survive
/// intact; the parser is not hand-rolled because a photo is exactly the kind of
/// payload a naive string split corrupts.
pub async fn upload_photos(
    CtxExt(db): CtxExt<Db>,
    Path(entry_id): Path<i64>,
    mut multipart: axum::extract::Multipart,
) -> ApiResult<Response> {
    // Confirm the entry exists before storing anything.
    if find_by_id(&db, Entry::table_name(), entry_id)
        .await
        .is_none()
    {
        return Err(ApiError::not_found("Entry not found"));
    }

    let mut created = Vec::new();
    let mut count = 0usize;

    loop {
        let field = multipart.next_field().await;
        let field = match field {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return Err(ApiError::bad_request(format!("Malformed upload: {e}"))),
        };

        let Some(filename) = field.file_name().map(|n| n.to_string()) else {
            // Non-file form fields (the caption field) are ignored.
            continue;
        };
        let mime = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        count += 1;
        if count > MAX_FILES {
            return Err(ApiError::bad_request("Too many files"));
        }

        let bytes = field
            .bytes()
            .await
            .map_err(|e| ApiError::bad_request(format!("Could not read upload: {e}")))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(ApiError::bad_request("File too large"));
        }

        let (stored, original, mime, size) =
            crate::uploads::store(&filename, &mime, &bytes).map_err(ApiError::bad_request)?;

        let row = json!({
            "entry_id": entry_id,
            "filename": stored,
            "original_name": original,
            "mime": mime,
            "size": size,
            "created_at": domain::dates::now_stamp(),
        });

        match insert_returning(&db, EntryPhoto::table_name(), &row).await {
            Ok(stored) => created.push(stored),
            Err(e) => {
                crate::uploads::remove_file(&col(&row, "filename"));
                return Err(ApiError::internal(e.to_string()));
            }
        }
    }

    if created.is_empty() {
        return Err(ApiError::bad_request("No files were uploaded"));
    }

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(created.iter().map(photo_json).collect::<Vec<_>>()),
    )
        .into_response())
}

/// `DELETE /api/photos/{id}`
pub async fn destroy_photo(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    let row = find_by_id(&db, EntryPhoto::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Photo not found"))?;
    // Remove the row first, then the file, so a failed unlink cannot leave an
    // orphan row pointing at nothing.
    delete_by_id(&db, EntryPhoto::table_name(), id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    crate::uploads::remove_file(&col(&row, "filename"));
    Ok(crate::http::no_content())
}

/// `GET /uploads/{filename}`
pub async fn serve_upload(Path(filename): Path<String>) -> Response {
    // Reject anything that is not a plain filename.
    if filename.is_empty() || filename.contains('/') || filename.contains("..") {
        return ApiError::not_found("Not found").into_response();
    }

    let path = crate::uploads::upload_dir().join(&filename);
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(
                    axum::http::header::CONTENT_TYPE,
                    mime.essence_str().to_string(),
                )],
                cache_headers(30 * 24 * 3600),
                bytes,
            )
                .into_response()
        }
        Err(_) => ApiError::not_found("Not found").into_response(),
    }
}

/// `GET /api/health`
pub async fn health(CtxExt(db): CtxExt<Db>) -> Response {
    let reachable = db.is_writable();
    let data_dir = std::env::var("PADDOCK_DATA_DIR").unwrap_or_else(|_| "./data".into());
    let body = json!({ "ok": reachable, "dataDir": data_dir });
    let status = if reachable {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };
    (status, axum::Json(body)).into_response()
}

const PUBCRIS_BASE: &str = "https://portal.apvma.gov.au/pubcris/PublicSearchServlet";
const PUBCRIS_SEARCH: &str =
    "https://portal.apvma.gov.au/pubcris/publicsearch.do?p_auth=search&p_ps=2&p_max_rows=50";
const BOM_BASE: &str = "https://www.bom.gov.au/climate/data/";

/// `GET /api/links/pubcris?q=`
///
/// Links out only: registration and label PDFs live behind APVMA's portal and
/// plantback intervals are free text inside those PDFs, so nothing is parsed.
pub async fn pubcris_link(q: Query) -> Response {
    let query = q.get("q").unwrap_or("").trim();
    let url = if query.is_empty() {
        PUBCRIS_SEARCH.to_string()
    } else {
        format!(
            "{PUBCRIS_BASE}?queryString={}&searchBy=contains&searchIn=All",
            urlencode(query)
        )
    };
    axum::Json(json!({ "url": url })).into_response()
}

/// `GET /api/links/bom?q=`
pub async fn bom_link(q: Query) -> Response {
    let query = q.get("q").unwrap_or("").trim();
    let url = if query.is_empty() {
        BOM_BASE.to_string()
    } else {
        format!("{BOM_BASE}?ref=index&q={}", urlencode(query))
    };
    axum::Json(json!({ "url": url })).into_response()
}

fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// `GET /api/reference/chemicals?q=`
pub async fn reference_chemicals(q: Query) -> Response {
    let query = q.str_or("q", "").to_lowercase();
    let items: Vec<_> = reference::CHEMICALS
        .iter()
        .filter(|c| {
            query.is_empty()
                || c.product.to_lowercase().contains(&query)
                || c.active.to_lowercase().contains(&query)
                || c.moa.to_lowercase().contains(&query)
        })
        .map(|c| {
            json!({
                "product": c.product,
                "active": c.active,
                "moa": c.moa,
                "moaDescription": reference::moa_description(c.moa),
                "strength": c.strength,
                "strengthUnit": c.strength_unit,
                "residual": c.residual,
                "description": c.note,
            })
        })
        .collect();
    axum::Json(items).into_response()
}

/// `GET /api/reference/lookup?name=...`
///
/// Exact product-name match on either list, as the entry dialogs use it to fill
/// in active/MOA or the N-P-K figures when the operator types a known name.
pub async fn reference_lookup(q: Query) -> Response {
    let name = q.str_or("name", "");
    let wanted = name.trim().to_lowercase();

    let chemical = reference::CHEMICALS
        .iter()
        .find(|c| !wanted.is_empty() && c.product.to_lowercase() == wanted)
        .map(|c| {
            json!({
                "product": c.product,
                "active": c.active,
                "moa": c.moa,
                "strength": c.strength,
                "strengthUnit": c.strength_unit,
                "residual": c.residual,
                "note": c.note,
            })
        });
    let fertiliser = reference::find_fertiliser(&name).map(|f| {
        json!({
            "product": f.product,
            "n": f.n,
            "p2o5": f.p2o5,
            "k2o": f.k2o,
            "rateUnit": f.rate_unit,
        })
    });

    axum::Json(json!({ "chemical": chemical, "fertiliser": fertiliser })).into_response()
}

/// `GET /api/reference/crops`
pub async fn reference_crops() -> Response {
    let items: Vec<_> = reference::CROPS
        .iter()
        .map(|c| {
            json!({
                "crop": c.name,
                "n": c.n,
                "p": c.p,
                "k": c.k,
                "fixedN": c.fixed_n,
                "typicalYield": c.typical_yield,
            })
        })
        .collect();
    axum::Json(items).into_response()
}

/// `GET /api/reference/fertilisers`
pub async fn reference_fertilisers() -> Response {
    let items: Vec<_> = reference::FERTILISERS
        .iter()
        .map(|f| {
            json!({
                "product": f.product,
                "n": f.n,
                "p2o5": f.p2o5,
                "k2o": f.k2o,
                "rateUnit": f.rate_unit,
            })
        })
        .collect();
    axum::Json(items).into_response()
}

/// `GET /api/reference/options`
pub async fn reference_options() -> Response {
    axum::Json(json!({
        "soilTypes": reference::SOIL_TYPES,
        "irrigationMethods": reference::IRRIGATION_METHODS,
        "weedDensityUnits": reference::WEED_DENSITY_UNITS,
        "soilMoistureUnits": reference::SOIL_MOISTURE_UNITS,
        "harvestUnits": reference::HARVEST_UNITS,
        "entryTypes": reference::ENTRY_TYPES,
        "rateUnits": reference::RATE_UNITS,
        "strengthUnits": reference::STRENGTH_UNITS,
        "taskTypes": reference::TASK_TYPES,
        "repeatIntervals": reference::REPEAT_INTERVALS,
        "moaDescriptions": reference::MOA_DESCRIPTIONS
            .iter()
            .map(|(k, v)| json!({ "group": k, "description": v }))
            .collect::<Vec<_>>(),
    }))
    .into_response()
}

/// `POST /api/entries/{id}/photos` JSON fallback for clients that cannot send
/// raw multipart. Accepts base64 payloads.
pub async fn upload_photos_json(
    CtxExt(db): CtxExt<Db>,
    Path(entry_id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    if find_by_id(&db, Entry::table_name(), entry_id)
        .await
        .is_none()
    {
        return Err(ApiError::not_found("Entry not found"));
    }

    let items = body.array("photos").unwrap_or_default();
    if items.len() > MAX_FILES {
        return Err(ApiError::bad_request("Too many files"));
    }

    let mut created = Vec::new();
    for item in &items {
        let filename_hint = domain::entries::str_of(item, "filename");
        let mime = {
            let m = domain::entries::str_of(item, "mime");
            if m.is_empty() {
                "image/jpeg".to_string()
            } else {
                m
            }
        };
        let payload = domain::entries::str_of(item, "data");
        let bytes = decode_base64(&payload)
            .ok_or_else(|| ApiError::bad_request("Could not decode photo data"))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(ApiError::bad_request("File too large"));
        }

        let (filename, original, mime, size) =
            crate::uploads::store(&filename_hint, &mime, &bytes).map_err(ApiError::bad_request)?;

        let row = json!({
            "entry_id": entry_id,
            "filename": filename,
            "original_name": original,
            "mime": mime,
            "size": size,
            "created_at": domain::dates::now_stamp(),
        });
        let stored = insert_returning(&db, EntryPhoto::table_name(), &row)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        created.push(stored);
    }

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(created.iter().map(photo_json).collect::<Vec<_>>()),
    )
        .into_response())
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        lookup[*c as usize] = i as u8;
    }

    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;

    for b in input.bytes() {
        if b == b'=' || b.is_ascii_whitespace() {
            continue;
        }
        let value = lookup[b as usize];
        if value == 255 {
            return None;
        }
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_path_rejects_traversal() {
        // `store` writes into the upload directory using the generated name, so
        // the only filename that reaches disk is server-chosen.
        let (stored, original, _, size) =
            crate::uploads::store("../../etc/passwd", "image/png", b"x").unwrap();
        assert!(!stored.contains('/'));
        assert!(!stored.contains(".."));
        assert_eq!(original, "../../etc/passwd");
        assert_eq!(size, 1);
        crate::uploads::remove_file(&stored);
    }

    #[test]
    fn base64_round_trips() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("").unwrap(), Vec::<u8>::new());
        assert!(decode_base64("not*valid").is_none());
    }

    #[test]
    fn urlencoding_escapes_spaces_and_ampersands() {
        assert_eq!(urlencode("Roundup 540"), "Roundup%20540");
        assert_eq!(urlencode("a&b"), "a%26b");
    }
}

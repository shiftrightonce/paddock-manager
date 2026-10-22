//! Timeline entry endpoints: create, update, delete, list and the derived
//! fields attached to each entry.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;
use dirtybase_contract::app_contract::CtxExt;
use serde_json::{Value, json};

use crate::db::*;
use crate::domain;
use crate::domain::entries::{auto_title, sanitise, str_of, truthy};
use crate::http::{ApiError, ApiResult, Body, Query, no_content};
use crate::models::*;

/// Attach `photos`, and type-specific derived fields, to an entry row.
pub async fn decorate_entry(db: &Db, row: &Value, records: &[domain::ResistanceRow]) -> Value {
    let id = as_i64(row, "id");
    let entry_type = col(row, "type");
    let data = as_value(row, "data");

    let photos: Vec<Value> = photos_of_entry(db, id)
        .await
        .iter()
        .map(photo_json)
        .collect();

    let mut out = entry_json(row);
    if let Some(obj) = out.as_object_mut() {
        obj.insert("photos".into(), json!(photos));
    }

    match entry_type.as_str() {
        "chemical_application" => {
            let products: Vec<domain::SprayProduct> = data["products"]
                .as_array()
                .map(|a| a.iter().map(domain::SprayProduct::from_value).collect())
                .unwrap_or_default();
            let conflicts = domain::check_conflicts(&products, records);
            let has_conflict = conflicts.iter().any(|c| c.conflict);
            if let Some(obj) = out.as_object_mut() {
                obj.insert(
                    "conflicts".into(),
                    json!(conflicts.iter().map(|c| c.to_json()).collect::<Vec<_>>()),
                );
                obj.insert("hasConflict".into(), json!(has_conflict));
            }
        }
        "water" => {
            let total = domain::entries::num_or_zero(&data, "rainfallMm")
                + domain::entries::num_or_zero(&data, "irrigationMm");
            if let Some(obj) = out.as_object_mut() {
                obj.insert("waterTotal".into(), json!(total));
            }
        }
        "planting" => {
            let lines = data["fertilisers"].as_array().cloned().unwrap_or_default();
            let mut n = 0.0;
            let mut p = 0.0;
            let mut k = 0.0;
            let mut known = false;
            for f in &lines {
                let c = domain::fertiliser_nutrients(f);
                if c.known {
                    known = true;
                    n += c.n;
                    p += c.p;
                    k += c.k;
                }
            }
            if let Some(obj) = out.as_object_mut() {
                obj.insert("fertiliserCount".into(), json!(lines.len()));
                obj.insert(
                    "fertiliserContribution".into(),
                    json!({ "n": n, "p": p, "k": k, "known": known }),
                );
            }
        }
        _ => {}
    }

    out
}

fn resistance_rows(rows: &[Value]) -> Vec<domain::ResistanceRow> {
    rows.iter()
        .map(|r| domain::ResistanceRow {
            id: as_i64(r, "id"),
            weed_species: col(r, "weed_species"),
            product: col(r, "product"),
            active: col(r, "active"),
            moa_group: col(r, "moa_group"),
            source: col(r, "source"),
            confirmed_on: opt_col(r, "confirmed_on"),
        })
        .collect()
}

/// `GET /api/entries?fieldId=&type=&from=&to=&limit=`
pub async fn list(CtxExt(db): CtxExt<Db>, q: Query) -> ApiResult<Response> {
    let field_id = q
        .i64("fieldId")
        .or_else(|| q.i64("field_id"))
        .ok_or_else(|| ApiError::bad_request("fieldId is required"))?;

    let field = find_by_id(&db, Field::table_name(), field_id)
        .await
        .ok_or_else(|| ApiError::not_found("Field not found"))?;

    let records = resistance_rows(&resistance_of_field(&db, field_id).await);

    let wanted_type = q.get("type").map(|s| s.to_string());
    let from = q.get("from").and_then(domain::dates::parse);
    let to = q.get("to").and_then(domain::dates::parse);

    let mut rows = entries_of_field(&db, field_id).await;
    rows.retain(|e| {
        if let Some(t) = &wanted_type {
            if col(e, "type") != *t {
                return false;
            }
        }
        let Some(d) = domain::dates::parse(&col(e, "entry_date")) else {
            return true;
        };
        if let Some(f) = from {
            if d < f {
                return false;
            }
        }
        if let Some(t) = to {
            if d > t {
                return false;
            }
        }
        true
    });

    let mut decorated = Vec::with_capacity(rows.len());
    for r in &rows {
        decorated.push(decorate_entry(&db, r, &records).await);
    }

    // The limit applies after decoration so counts stay accurate.
    if let Some(n) = q.i64("limit") {
        if n >= 0 && (n as usize) < decorated.len() {
            decorated.truncate(n as usize);
        }
    }

    let _ = field;
    Ok(axum::Json(decorated).into_response())
}

/// `GET /api/entries/{id}`
pub async fn detail(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    let row = find_by_id(&db, Entry::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Entry not found"))?;
    let field_id = as_i64(&row, "field_id");
    let records = resistance_rows(&resistance_of_field(&db, field_id).await);
    Ok(axum::Json(decorate_entry(&db, &row, &records).await).into_response())
}

/// `POST /api/entries`
pub async fn create(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let field_id = body
        .i64_any("fieldId", "field_id")
        .ok_or_else(|| ApiError::bad_request("Field is required"))?;

    if find_by_id(&db, Field::table_name(), field_id)
        .await
        .is_none()
    {
        return Err(ApiError::bad_request("Field is required"));
    }

    let entry_type = body.text("type");
    if !crate::reference::ENTRY_TYPES.contains(&entry_type.as_str()) {
        return Err(ApiError::bad_request("Unknown entry type"));
    }

    let entry_date = body
        .opt_text_any("entryDate", "entry_date")
        .filter(|s| domain::dates::parse(s).is_some())
        .unwrap_or_else(domain::dates::today_iso);

    let data = sanitise(&entry_type, &body.value("data"));
    // An omitted title is generated; a supplied one is always respected.
    let title = {
        let supplied = body.text_any("title", "title");
        if supplied.is_empty() {
            auto_title(&entry_type, &data)
        } else {
            supplied
        }
    };

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "field_id": field_id,
        "type": entry_type,
        "entry_date": entry_date,
        "title": title,
        "notes": body.text("notes"),
        "data": data.to_string(),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, Entry::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let records = load_resistance(&db, as_i64(&saved, "field_id")).await;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(decorate_entry(&db, &saved, &records).await),
    )
        .into_response())
}

/// `PUT /api/entries/{id}`
///
/// A title the operator typed by hand is preserved across edits. A generated
/// title is refreshed so it keeps matching the payload.
pub async fn update(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, Entry::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Entry not found"))?;

    let old_type = col(&existing, "type");
    let old_data = as_value(&existing, "data");
    let old_title = col(&existing, "title");
    let old_auto = auto_title(&old_type, &old_data);

    let entry_type = {
        let t = body.text("type");
        if t.is_empty() { old_type.clone() } else { t }
    };
    if !crate::reference::ENTRY_TYPES.contains(&entry_type.as_str()) {
        return Err(ApiError::bad_request("Unknown entry type"));
    }

    let data = if body.has("data") {
        sanitise(&entry_type, &body.value("data"))
    } else {
        // Type unchanged: keep the stored payload. Type changed: reinterpret it.
        if entry_type == old_type {
            old_data.clone()
        } else {
            sanitise(&entry_type, &old_data)
        }
    };

    let title = {
        let supplied = body.text("title");
        if !supplied.is_empty() {
            supplied
        } else if !body.has("title") && old_title == old_auto {
            auto_title(&entry_type, &data)
        } else {
            old_title.clone()
        }
    };

    let entry_date = body
        .opt_text_any("entryDate", "entry_date")
        .filter(|s| domain::dates::parse(s).is_some())
        .unwrap_or_else(|| col(&existing, "entry_date"));

    let row = json!({
        "id": id,
        "field_id": body.i64_any("fieldId", "field_id").unwrap_or_else(|| as_i64(&existing, "field_id")),
        "type": entry_type,
        "entry_date": entry_date,
        "title": title,
        "notes": if body.has("notes") { body.text("notes") } else { col(&existing, "notes") },
        "data": data.to_string(),
        "updated_at": domain::dates::now_stamp(),
    });

    let saved = update_returning(&db, Entry::table_name(), &row, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let field_id = as_i64(&saved, "field_id");
    let records = load_resistance(&db, field_id).await;
    Ok(axum::Json(decorate_entry(&db, &saved, &records).await).into_response())
}

/// `DELETE /api/entries/{id}`
pub async fn destroy(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, Entry::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Entry not found"));
    }
    crate::cleanup::delete_entry(&db, id).await;
    delete_by_id(&db, Entry::table_name(), id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(no_content())
}

/// Save the current tank mix as a reusable named mix.
pub async fn save_mix(
    CtxExt(db): CtxExt<Db>,
    Path(field_id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let name = body.text("name");
    if name.is_empty() {
        return Err(ApiError::bad_request("Mix name is required"));
    }
    let products = body.array("products").unwrap_or_default();
    if products.is_empty() {
        return Err(ApiError::bad_request("A mix needs at least one product"));
    }

    let stamp = domain::dates::now_stamp();
    let mix_row = json!({
        "name": name,
        "notes": body.text("notes"),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, TankMix::table_name(), &mix_row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mix_id = as_i64(&saved, "id");
    insert_mix_products(&db, mix_id, &products).await?;

    let _ = field_id;
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(json!({
            "id": mix_id,
            "name": name,
            "notes": body.text("notes"),
            "products": products,
        })),
    )
        .into_response())
}

/// Insert mix product lines, dropping blanks and re-indexing positions.
async fn insert_mix_products(db: &Db, mix_id: i64, products: &[Value]) -> ApiResult<()> {
    let mut position = 0i64;
    for p in products {
        let name = str_of(p, "product");
        if name.is_empty() {
            continue;
        }
        // `json!` cannot hold a multi-statement block, so the defaults are
        // resolved before the row is built.
        let moa = str_of(p, "moa");
        let moa_group = if moa.is_empty() {
            str_of(p, "moa_group")
        } else {
            moa
        };
        let strength_unit = str_of(p, "strengthUnit");
        let strength_unit = if strength_unit.is_empty() {
            "g/L".to_string()
        } else {
            strength_unit
        };
        let rate_unit = str_of(p, "rateUnit");
        let rate_unit = if rate_unit.is_empty() {
            "L/ha".to_string()
        } else {
            rate_unit
        };

        let row = json!({
            "mix_id": mix_id,
            "position": position,
            "product": name,
            "active": str_of(p, "active"),
            "moa_group": moa_group,
            "strength": domain::entries::num_of(p, "strength"),
            "strength_unit": strength_unit,
            "rate": domain::entries::num_of(p, "rate"),
            "rate_unit": rate_unit,
            "residual": if truthy(p.get("residual")) { 1 } else { 0 },
        });
        insert(db, TankMixProduct::table_name(), &row)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        position += 1;
    }
    Ok(())
}

/// Resistance records as domain rows, used by the PDF and the entry decoration.
pub async fn load_resistance(db: &Db, field_id: i64) -> Vec<domain::ResistanceRow> {
    resistance_rows(&resistance_of_field(db, field_id).await)
}

/// Spray events for a field, used by the PDF.
pub async fn load_sprays(db: &Db, field_id: i64) -> Vec<domain::SprayEvent> {
    entries_of_field(db, field_id)
        .await
        .iter()
        .filter(|e| col(e, "type") == "chemical_application")
        .map(|e| domain::SprayEvent {
            id: as_i64(e, "id"),
            date: col(e, "entry_date"),
            products: as_value(e, "data")["products"]
                .as_array()
                .map(|a| a.iter().map(domain::SprayProduct::from_value).collect())
                .unwrap_or_default(),
        })
        .collect()
}

/// Water events for a field.
pub async fn load_water_events(db: &Db, field_id: i64) -> Vec<domain::WaterEvent> {
    entries_of_field(db, field_id)
        .await
        .iter()
        .filter(|e| col(e, "type") == "water")
        .map(|e| {
            let data = as_value(e, "data");
            domain::WaterEvent {
                id: as_i64(e, "id"),
                date: col(e, "entry_date"),
                rainfall_mm: domain::entries::num_or_zero(&data, "rainfallMm"),
                irrigation_mm: domain::entries::num_or_zero(&data, "irrigationMm"),
                method: domain::entries::str_of(&data, "method"),
            }
        })
        .collect()
}

/// Today as a naive local date, re-exported for handlers.
pub fn ref_date() -> NaiveDate {
    domain::dates::today()
}

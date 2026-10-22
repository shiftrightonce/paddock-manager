//! Saved tank mixes and tank-mix compatibility flags.
//!
//! Compatibility is entirely user-maintained: there is no external dataset, and
//! a saved mix is implicitly treated as known compatible.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use dirtybase_contract::app_contract::CtxExt;
use serde_json::{Value, json};

use crate::db::*;
use crate::domain;
use crate::http::{ApiError, ApiResult, Body, no_content};
use crate::models::*;

/// Canonical key for an unordered product pair: lowercase, sorted, joined.
pub fn pair_key(a: &str, b: &str) -> (String, String, String) {
    let (first, second) = if a.to_lowercase() <= b.to_lowercase() {
        (a.trim().to_string(), b.trim().to_string())
    } else {
        (b.trim().to_string(), a.trim().to_string())
    };
    let key = format!("{}::{}", first.to_lowercase(), second.to_lowercase());
    (first, second, key)
}

/// Normalise mix product lines: drop blanks, re-index, fill units.
fn clean_products(products: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for p in products {
        let name = domain::entries::str_of(p, "product");
        if name.is_empty() {
            continue;
        }
        let strength_unit = domain::entries::str_of(p, "strengthUnit");
        let rate_unit = domain::entries::str_of(p, "rateUnit");
        let moa = domain::entries::str_of(p, "moa");
        out.push(json!({
            "product": name,
            "active": domain::entries::str_of(p, "active"),
            "moa": if moa.is_empty() { domain::entries::str_of(p, "moa_group") } else { moa },
            "strength": domain::entries::num_of(p, "strength"),
            "strengthUnit": if strength_unit.is_empty() { "g/L".to_string() } else { strength_unit },
            "rate": domain::entries::num_of(p, "rate"),
            "rateUnit": if rate_unit.is_empty() { "L/ha".to_string() } else { rate_unit },
            "residual": domain::entries::truthy(p.get("residual")),
        }));
    }
    out
}

/// `GET /api/mixes`
pub async fn list_mixes(CtxExt(db): CtxExt<Db>) -> ApiResult<Response> {
    let mut rows = json_rows(
        db.select_from_table(TankMix::table_name(), |_| {})
            .fetch_all()
            .await
            .unwrap_or_default(),
    );
    rows.sort_by(|a, b| {
        col(a, "name")
            .to_lowercase()
            .cmp(&col(b, "name").to_lowercase())
    });

    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        let products = products_of_mix(&db, as_i64(r, "id"))
            .await
            .iter()
            .map(mix_product_json)
            .collect();
        out.push(mix_json(r, products));
    }

    Ok(axum::Json(out).into_response())
}

/// `POST /api/mixes`
pub async fn create_mix(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let name = body.text("name");
    if name.is_empty() {
        return Err(ApiError::bad_request("Mix name is required"));
    }

    let products = clean_products(&body.array("products").unwrap_or_default());
    if products.is_empty() {
        return Err(ApiError::bad_request("A mix needs at least one product"));
    }

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "name": name,
        "notes": body.text("notes"),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, TankMix::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mix_id = as_i64(&saved, "id");
    replace_mix_products(&db, mix_id, &products).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(mix_json(&saved, products)),
    )
        .into_response())
}

/// `PUT /api/mixes/{id}`
pub async fn update_mix(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, TankMix::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Mix not found"))?;

    let name = if body.has("name") {
        let n = body.text("name");
        if n.is_empty() {
            return Err(ApiError::bad_request("Mix name is required"));
        }
        n
    } else {
        col(&existing, "name")
    };

    update_returning(
        &db,
        TankMix::table_name(),
        &json!({
            "id": id,
            "name": name,
            "notes": if body.has("notes") { body.text("notes") } else { col(&existing, "notes") },
            "updated_at": domain::dates::now_stamp(),
        }),
        id,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    // Products are only replaced when a non-empty list is supplied, so editing
    // the name alone leaves the mix intact.
    let incoming = clean_products(&body.array("products").unwrap_or_default());
    if !incoming.is_empty() {
        replace_mix_products(&db, id, &incoming).await?;
    }

    let products = products_of_mix(&db, id)
        .await
        .iter()
        .map(mix_product_json)
        .collect();
    let updated = find_by_id(&db, TankMix::table_name(), id)
        .await
        .unwrap_or(existing);

    Ok(axum::Json(mix_json(&updated, products)).into_response())
}

/// `DELETE /api/mixes/{id}`
pub async fn destroy_mix(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, TankMix::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Mix not found"));
    }
    crate::cleanup::delete_mix(&db, id).await;
    Ok(no_content())
}

async fn replace_mix_products(db: &Db, mix_id: i64, products: &[Value]) -> ApiResult<()> {
    _ = db
        .delete(TankMixProduct::table_name(), |q| {
            q.is_eq("mix_id", mix_id);
        })
        .await;

    for (index, p) in products.iter().enumerate() {
        insert(
            db,
            TankMixProduct::table_name(),
            &json!({
                "mix_id": mix_id,
                "position": index as i64,
                "product": p["product"].as_str().unwrap_or_default(),
                "active": p["active"].as_str().unwrap_or_default(),
                "moa_group": p["moa"].as_str().unwrap_or_default(),
                "strength": p["strength"],
                "strength_unit": p["strengthUnit"].as_str().unwrap_or_default(),
                "rate": p["rate"],
                "rate_unit": p["rateUnit"].as_str().unwrap_or_default(),
                "residual": if p["residual"].as_bool().unwrap_or(false) { 1 } else { 0 },
            }),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(())
}

/// `GET /api/compatibility`
pub async fn list_flags(CtxExt(db): CtxExt<Db>) -> ApiResult<Response> {
    let rows = flags(&db).await;
    Ok(axum::Json(rows.iter().map(flag_json).collect::<Vec<_>>()).into_response())
}

/// `POST /api/compatibility`
///
/// Re-flagging an existing pair returns it with 200 rather than duplicating.
pub async fn create_flag(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let a = body.text_any("product_a", "productA");
    let b = body.text_any("product_b", "productB");
    if a.is_empty() || b.is_empty() {
        return Err(ApiError::bad_request("Two products are required"));
    }
    if a.eq_ignore_ascii_case(&b) {
        return Err(ApiError::bad_request(
            "A product cannot be flagged against itself",
        ));
    }

    let (first, second, key) = pair_key(&a, &b);

    if let Some(existing) =
        find_by_id_key(&db, CompatibilityFlag::table_name(), "pair_key", &key).await
    {
        return Ok(axum::Json(flag_json(&existing)).into_response());
    }

    let row = json!({
        "product_a": first,
        "product_b": second,
        "pair_key": key,
        "note": body.text("note"),
        "created_at": domain::dates::now_stamp(),
    });

    let saved = insert_returning(&db, CompatibilityFlag::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(flag_json(&saved)),
    )
        .into_response())
}

/// `DELETE /api/compatibility/{id}`
pub async fn destroy_flag(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, CompatibilityFlag::table_name(), id)
        .await
        .is_none()
    {
        return Err(ApiError::not_found("Flag not found"));
    }
    delete_by_id(&db, CompatibilityFlag::table_name(), id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(no_content())
}

/// `POST /api/compatibility/check`
///
/// Checks every unordered pair in the submitted mix against the flag list and
/// reports which products in the mix are involved, in the order supplied.
pub async fn check_compatibility(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let inputs = body.array("products").unwrap_or_default();

    let names: Vec<String> = inputs
        .iter()
        .map(|p| match p {
            Value::String(s) => s.trim().to_string(),
            other => domain::entries::str_of(other, "product"),
        })
        .filter(|s| !s.is_empty())
        .collect();

    let flag_rows = flags(&db).await;
    let mut conflicts: Vec<Value> = Vec::new();

    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            let key = pair_key(&names[i], &names[j]).2;
            for f in &flag_rows {
                if col(f, "pair_key") == key {
                    conflicts.push(json!({
                        "id": as_i64(f, "id"),
                        "product_a": col(f, "product_a"),
                        "product_b": col(f, "product_b"),
                        "note": col(f, "note"),
                        "inMix": [names[i].clone(), names[j].clone()],
                    }));
                }
            }
        }
    }

    Ok(axum::Json(json!({
        "productCount": names.len(),
        "ok": conflicts.is_empty(),
        "conflicts": conflicts,
    }))
    .into_response())
}

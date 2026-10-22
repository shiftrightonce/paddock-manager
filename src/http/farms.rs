//! Farm and field endpoints.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;
use dirtybase_contract::app_contract::CtxExt;
use serde_json::{Value, json};

use crate::db::*;
use crate::domain::{self, Level};
use crate::http::{ApiError, ApiResult, Body, Query, no_content};
use crate::models::*;

/// One field row plus the derived status keys, as the field views expect them.
async fn decorate_field(db: &Db, field: &Value, ref_date: NaiveDate) -> Value {
    let field_id = as_i64(field, "id");
    let resistance = crate::http::entries::load_resistance(db, field_id).await;
    let sprays = crate::http::entries::load_sprays(db, field_id).await;
    let status = domain::field_status(&resistance, &sprays, ref_date);

    // Merge the derived status keys alongside the field columns.
    let mut base = field_json(field);
    if let (Some(base_map), Some(status_map)) =
        (base.as_object_mut(), status.to_json().as_object().cloned())
    {
        for (k, v) in status_map {
            base_map.insert(k, v);
        }
    }
    base
}

/// Build the status decoration for a set of fields.
async fn decorate_fields(db: &Db, fields: &[Value], ref_date: NaiveDate) -> Vec<Value> {
    let mut out = Vec::with_capacity(fields.len());
    for f in fields {
        out.push(decorate_field(db, f, ref_date).await);
    }
    out
}

/// One field's derived status, as JSON.
pub async fn field_status_json(db: &Db, field_id: i64, ref_date: NaiveDate) -> Value {
    field_status_for(db, field_id, ref_date).await.to_json()
}

/// One field's status plus the loaders the rollups need for attention items.
async fn field_status_for(db: &Db, field_id: i64, ref_date: NaiveDate) -> domain::FieldStatus {
    let resistance: Vec<domain::ResistanceRow> =
        crate::http::entries::load_resistance(db, field_id).await;
    let sprays: Vec<domain::SprayEvent> = crate::http::entries::load_sprays(db, field_id).await;
    domain::field_status(&resistance, &sprays, ref_date)
}

/// Attention items for one farm: one per resistance record, one per field with
/// an active residual.
pub async fn farm_attention(
    db: &Db,
    farm_id: i64,
    farm_name: &str,
    ref_date: NaiveDate,
) -> Vec<domain::Attention> {
    let mut items = Vec::new();

    for f in fields_of_farm(db, farm_id).await {
        let field_id = as_i64(&f, "id");
        let field_name = col(&f, "name");

        for r in resistance_of_field(db, field_id).await {
            items.push(domain::attention_item_for_record(
                farm_id,
                farm_name,
                field_id,
                &field_name,
                &domain::ResistanceRow {
                    id: as_i64(&r, "id"),
                    weed_species: col(&r, "weed_species"),
                    product: col(&r, "product"),
                    active: col(&r, "active"),
                    moa_group: col(&r, "moa_group"),
                    source: col(&r, "source"),
                    confirmed_on: opt_col(&r, "confirmed_on"),
                },
            ));
        }

        let status = field_status_for(db, field_id, ref_date).await;
        if let Some(lr) = &status.last_residual {
            if lr.active() {
                items.push(domain::attention_item_for_residual(
                    farm_id,
                    farm_name,
                    field_id,
                    &field_name,
                    lr,
                ));
            }
        }
    }

    domain::sort_attention(&mut items);
    items
}

/// Farm summary: field list with status, attention items and upcoming tasks.
pub async fn farm_rollup(db: &Db, farm_id: i64, ref_date: NaiveDate) -> Option<Value> {
    let farm = find_by_id(db, Farm::table_name(), farm_id).await?;
    let farm_name = col(&farm, "name");

    let fields = decorate_fields(db, &fields_of_farm(db, farm_id).await, ref_date).await;
    let attention = farm_attention(db, farm_id, &farm_name, ref_date).await;
    let upcoming: Vec<Value> = tasks_with_names(db, Some(farm_id), None, false, None)
        .await
        .iter()
        .map(|t| t.to_json(ref_date))
        .collect();

    let count = |level: Level| {
        fields
            .iter()
            .filter(|f| f["status"] == level.as_str())
            .count()
    };

    Some(json!({
        "farm": farm_json(&farm),
        "fields": fields,
        "attention": attention.iter().map(|a| a.to_json()).collect::<Vec<_>>(),
        "upcoming": upcoming,
        "counts": {
            "fields": fields.len(),
            "red": count(Level::Red),
            "amber": count(Level::Amber),
            "green": count(Level::Green),
        },
    }))
}

/// Every attention item across every farm, sorted.
///
/// The dashboard renders this list and the farm list counts it, so both go
/// through here: a farm's `attentionCount` includes its due tasks, not just its
/// resistance and residual items.
pub async fn all_attention(db: &Db, ref_date: NaiveDate) -> Vec<domain::Attention> {
    let mut attention: Vec<domain::Attention> = Vec::new();

    for f in farms(db).await {
        let id = as_i64(&f, "id");
        let name = col(&f, "name");
        attention.extend(farm_attention(db, id, &name, ref_date).await);
    }

    // Tasks are farm-wide and may be linked to a field, which resolves the farm.
    for t in tasks_with_names(db, None, None, false, None).await {
        let due = domain::dates::parse(&t.due_date);
        if due.map(|d| d <= ref_date).unwrap_or(false) {
            attention.push(domain::attention_item_for_task(
                t.effective_farm_id(),
                t.farm_name.as_deref().unwrap_or("Farm task"),
                t.field_id,
                t.field_name.as_deref().unwrap_or_default(),
                t.id,
                &t.title,
                &t.due_date,
                ref_date,
            ));
        }
    }

    domain::sort_attention(&mut attention);
    attention
}

/// `GET /api/farms`
pub async fn list(CtxExt(db): CtxExt<Db>, q: Query) -> ApiResult<Response> {
    let ref_date = domain::dates::today();
    let all = farms(&db).await;
    let attention = all_attention(&db, ref_date).await;
    let mut out = Vec::with_capacity(all.len());

    for f in &all {
        let id = as_i64(f, "id");
        let fields = decorate_fields(&db, &fields_of_farm(&db, id).await, ref_date).await;
        let count = |level: Level| {
            fields
                .iter()
                .filter(|x| x["status"] == level.as_str())
                .count()
        };
        let mut item = farm_json(f);
        if let Some(obj) = item.as_object_mut() {
            obj.insert("fieldCount".into(), json!(fields.len()));
            obj.insert("redCount".into(), json!(count(Level::Red)));
            obj.insert("amberCount".into(), json!(count(Level::Amber)));
            obj.insert(
                "attentionCount".into(),
                json!(attention.iter().filter(|a| a.farm_id == Some(id)).count()),
            );
        }
        out.push(item);
    }

    let _ = q;
    Ok(axum::Json(out).into_response())
}

/// `POST /api/farms`
pub async fn create(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let name = body.text("name");
    if name.is_empty() {
        return Err(ApiError::bad_request("Farm name is required"));
    }

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "name": name,
        "location": body.text("location"),
        "weather_station": body.text_any("weather_station", "weatherStation"),
        "notes": body.text("notes"),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, Farm::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(farm_json(&saved)),
    )
        .into_response())
}

/// `GET /api/farms/{id}`
pub async fn detail(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    let ref_date = domain::dates::today();
    farm_rollup(&db, id, ref_date)
        .await
        .map(|v| axum::Json(v).into_response())
        .ok_or_else(|| ApiError::not_found("Farm not found"))
}

/// `PUT /api/farms/{id}`
pub async fn update(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, Farm::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Farm not found"))?;

    // Absent keys fall back to the stored value; a supplied empty name is
    // rejected rather than silently blanking the farm.
    let name = if body.has("name") {
        let n = body.text("name");
        if n.is_empty() {
            return Err(ApiError::bad_request("Farm name is required"));
        }
        n
    } else {
        col(&existing, "name")
    };

    let row = json!({
        "id": id,
        "name": name,
        "location": if body.has("location") { body.text("location") } else { col(&existing, "location") },
        "weather_station": if body.has("weather_station") {
            body.text("weather_station")
        } else if body.has("weatherStation") {
            body.text("weatherStation")
        } else {
            col(&existing, "weather_station")
        },
        "notes": if body.has("notes") { body.text("notes") } else { col(&existing, "notes") },
        "updated_at": domain::dates::now_stamp(),
    });

    let saved = update_returning(&db, Farm::table_name(), &row, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(axum::Json(farm_json(&saved)).into_response())
}

/// `DELETE /api/farms/{id}`
pub async fn destroy(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, Farm::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Farm not found"));
    }
    crate::cleanup::delete_farm(&db, id).await;
    Ok(no_content())
}

/// `GET /api/fields?farmId=&status=`
pub async fn list_fields(CtxExt(db): CtxExt<Db>, q: Query) -> ApiResult<Response> {
    let ref_date = domain::dates::today();

    let farm_names: HashMapLocal = {
        let mut m = std::collections::HashMap::new();
        for f in farms(&db).await {
            m.insert(as_i64(&f, "id"), col(&f, "name"));
        }
        m
    };

    let rows = match q.i64("farmId") {
        Some(fid) => fields_of_farm(&db, fid).await,
        None => all_fields(&db).await,
    };

    let mut decorated = decorate_fields(&db, &rows, ref_date).await;

    // The status filter applies after decoration, so it sees the derived value.
    if let Some(wanted) = q.get("status") {
        decorated.retain(|f| f["status"] == wanted);
    }

    let out: Vec<Value> = decorated
        .into_iter()
        .map(|mut f| {
            let farm_id = f["farm_id"].as_i64().unwrap_or_default();
            if let Some(obj) = f.as_object_mut() {
                obj.insert(
                    "farm_name".into(),
                    json!(farm_names.get(&farm_id).cloned().unwrap_or_default()),
                );
            }
            f
        })
        .collect();

    Ok(axum::Json(out).into_response())
}

type HashMapLocal = std::collections::HashMap<i64, String>;

/// `POST /api/fields`
pub async fn create_field(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let farm_id = body
        .i64_any("farm_id", "farmId")
        .ok_or_else(|| ApiError::bad_request("Parent farm is required"))?;
    let name = body.text("name");
    if name.is_empty() {
        return Err(ApiError::bad_request("Field name is required"));
    }

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "farm_id": farm_id,
        "name": name,
        "location": body.text("location"),
        "area_ha": body.f64_blank_is_null("area_ha"),
        "soil_type": body.text("soil_type"),
        "notes": body.text("notes"),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, Field::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(field_json(&saved)),
    )
        .into_response())
}

/// `GET /api/fields/{id}`
pub async fn field_detail(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    let field = find_by_id(&db, Field::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Field not found"))?;
    let ref_date = domain::dates::today();

    let decorated = decorate_field(&db, &field, ref_date).await;

    let farm = find_by_id(&db, Farm::table_name(), as_i64(&field, "farm_id")).await;

    // Future-dated entries are hidden so planning ahead does not fill the
    // timeline with entries the operator has not logged yet.
    let entries: Vec<Value> = entries_of_field(&db, id)
        .await
        .iter()
        .filter(|e| {
            domain::dates::parse(&col(e, "entry_date"))
                .map(|d| d <= ref_date)
                .unwrap_or(true)
        })
        .map(entry_json)
        .collect();

    let resistance = resistance_of_field(&db, id)
        .await
        .iter()
        .map(resistance_json)
        .collect::<Vec<_>>();

    let scheduled: Vec<Value> = tasks_with_names(&db, None, Some(id), false, None)
        .await
        .iter()
        .map(|t| t.to_json(ref_date))
        .collect();

    Ok(axum::Json(json!({
        "field": decorated,
        "farm": farm.as_ref().map(farm_json),
        "entries": entries,
        "water": water_window_for(&db, id, 7, ref_date).await.to_json(),
        "resistance": resistance,
        "scheduled": scheduled,
        "lastResidual": decorated.get("lastResidual").cloned().unwrap_or(Value::Null),
        "seasons": seasons_of_field(&db, id).await.iter().map(season_json).collect::<Vec<_>>(),
    }))
    .into_response())
}

/// `PUT /api/fields/{id}`
pub async fn update_field(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, Field::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Field not found"))?;

    let name = if body.has("name") {
        let n = body.text("name");
        if n.is_empty() {
            return Err(ApiError::bad_request("Field name is required"));
        }
        n
    } else {
        col(&existing, "name")
    };

    let row = json!({
        "id": id,
        "farm_id": body.i64_any("farm_id", "farmId").unwrap_or_else(|| as_i64(&existing, "farm_id")),
        "name": name,
        "location": if body.has("location") { body.text("location") } else { col(&existing, "location") },
        "area_ha": if body.has("area_ha") { body.f64_blank_is_null("area_ha") } else { as_opt_f64(&existing, "area_ha") },
        "soil_type": if body.has("soil_type") { body.text("soil_type") } else { col(&existing, "soil_type") },
        "notes": if body.has("notes") { body.text("notes") } else { col(&existing, "notes") },
        "updated_at": domain::dates::now_stamp(),
    });

    let saved = update_returning(&db, Field::table_name(), &row, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let ref_date = domain::dates::today();
    Ok(axum::Json(decorate_field(&db, &saved, ref_date).await).into_response())
}

/// `DELETE /api/fields/{id}`
pub async fn destroy_field(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, Field::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Field not found"));
    }
    crate::cleanup::delete_field(&db, id).await;
    Ok(no_content())
}

/// Water input over a window, as `water::water_window_for` builds it.
async fn water_window_for(
    db: &Db,
    field_id: i64,
    days: i64,
    ref_date: NaiveDate,
) -> domain::WaterWindow {
    let events: Vec<domain::WaterEvent> = entries_of_field(db, field_id)
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
        .collect();

    domain::water_input(&events, days, ref_date)
}

pub async fn field_water(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    q: Query,
) -> ApiResult<Response> {
    let days = q.i64_or("days", 7).clamp(1, 365);
    let window = water_window_for(&db, id, days, domain::dates::today()).await;
    Ok(axum::Json(window.to_json()).into_response())
}

pub async fn field_last_residual(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
) -> ApiResult<Response> {
    let status = field_status_for(&db, id, domain::dates::today()).await;
    Ok(axum::Json(
        status
            .last_residual
            .map(|r| r.to_json())
            .unwrap_or(Value::Null),
    )
    .into_response())
}

pub async fn field_moa_history(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    q: Query,
) -> ApiResult<Response> {
    let months = q.i64_or("months", 12).clamp(1, 120);
    let ref_date = domain::dates::today();

    let records = crate::http::entries::load_resistance(&db, id).await;
    let sprays = crate::http::entries::load_sprays(&db, id).await;

    let history = domain::moa_history(&sprays, &records, months, ref_date);
    Ok(axum::Json(history.iter().map(|h| h.to_json()).collect::<Vec<_>>()).into_response())
}

pub async fn field_nutrient_budget(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    q: Query,
) -> ApiResult<Response> {
    let crop = q.str_or("crop", "");
    let target = q
        .i64("targetYield")
        .map(|v| v as f64)
        .or_else(|| q.get("targetYield").and_then(|v| v.parse::<f64>().ok()));

    let budget = crate::nutrient::budget_for_field(&db, id, &crop, target).await;
    Ok(axum::Json(budget.to_json()).into_response())
}

/// `POST /api/fields/{id}/resistance/check`
pub async fn field_resistance_check(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let records = crate::http::entries::load_resistance(&db, id).await;

    let products: Vec<domain::SprayProduct> = body
        .array("products")
        .unwrap_or_default()
        .iter()
        .map(domain::SprayProduct::from_input)
        .collect();

    let result = domain::check_conflicts(&products, &records);
    Ok(axum::Json(result.iter().map(|r| r.to_json()).collect::<Vec<_>>()).into_response())
}

pub async fn field_resistance_list(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
) -> ApiResult<Response> {
    let rows = resistance_of_field(&db, id).await;
    Ok(axum::Json(rows.iter().map(resistance_json).collect::<Vec<_>>()).into_response())
}

pub async fn field_resistance_create(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    if find_by_id(&db, Field::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Field not found"));
    }
    let species = body.text_any("weed_species", "weedSpecies");
    if species.is_empty() {
        return Err(ApiError::bad_request("Weed species is required"));
    }

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "field_id": id,
        "weed_species": species,
        "product": body.text("product"),
        "active": body.text("active"),
        // moa_group is the canonical column; "moa" is the accepted alias.
        "moa_group": body.text_any("moa_group", "moa"),
        "source": body.text("source"),
        "confirmed_on": body.opt_text("confirmed_on"),
        "notes": body.text("notes"),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, ResistanceRecord::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(resistance_json(&saved)),
    )
        .into_response())
}

pub async fn field_seasons(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    let rows = seasons_of_field(&db, id).await;
    Ok(axum::Json(rows.iter().map(season_json).collect::<Vec<_>>()).into_response())
}

pub async fn field_seasons_create(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    if find_by_id(&db, Field::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Field not found"));
    }
    let crop = body.text("crop");
    if crop.is_empty() {
        return Err(ApiError::bad_request("Crop is required"));
    }

    let year = body
        .i64_any("season_year", "seasonYear")
        .unwrap_or_else(|| domain::dates::year_of(domain::dates::today()));

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "field_id": id,
        "season_year": year,
        "crop": crop,
        "sown_date": body.opt_text("sown_date"),
        "harvested": if body.bool("harvested").unwrap_or(false) { 1 } else { 0 },
        "chemicals": body.text("chemicals"),
        "notes": body.text("notes"),
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, SeasonHistory::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(season_json(&saved)),
    )
        .into_response())
}

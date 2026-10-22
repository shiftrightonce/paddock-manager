//! Scheduled tasks and completion.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use dirtybase_contract::app_contract::CtxExt;
use serde_json::{Value, json};

use crate::db::*;
use crate::domain;
use crate::http::{ApiError, ApiResult, Body, Query, no_content};
use crate::models::*;

/// Next due date for a repeating task, counted from the current due date.
pub fn next_due(due: &str, interval: &str) -> String {
    let Some(d) = domain::dates::parse(due) else {
        return domain::dates::today_iso();
    };
    let next = match interval {
        "weekly" => domain::dates::add_days(d, 7),
        "fortnightly" => domain::dates::add_days(d, 14),
        "monthly" => domain::dates::months_ago(d, -1),
        _ => return due.to_string(),
    };
    next.format(domain::dates::ISO).to_string()
}

/// `GET /api/tasks?farmId=&fieldId=&includeCompleted=&limit=`
pub async fn list(CtxExt(db): CtxExt<Db>, q: Query) -> ApiResult<Response> {
    let ref_date = domain::dates::today();
    let tasks = tasks_with_names(
        &db,
        q.i64("farmId"),
        q.i64("fieldId"),
        q.flag("includeCompleted"),
        Some(q.i64_or("limit", 200).clamp(1, 5000)),
    )
    .await;

    let out: Vec<Value> = tasks.iter().map(|t| t.to_json(ref_date)).collect();
    Ok(axum::Json(out).into_response())
}

/// `POST /api/tasks`
pub async fn create(CtxExt(db): CtxExt<Db>, body: Body) -> ApiResult<Response> {
    let title = body.text("title");
    if title.is_empty() {
        return Err(ApiError::bad_request("Task title is required"));
    }

    let task_type = {
        let t = body.text_any("taskType", "task_type");
        if crate::reference::TASK_TYPES.contains(&t.as_str()) {
            t
        } else {
            "custom".to_string()
        }
    };
    let interval = {
        let i = body.text_any("repeatInterval", "repeat_interval");
        if crate::reference::REPEAT_INTERVALS.contains(&i.as_str()) {
            i
        } else {
            "none".to_string()
        }
    };

    let field_id = body.i64_any("fieldId", "field_id");
    let field = match field_id {
        Some(fid) => Some(
            find_by_id(&db, Field::table_name(), fid)
                .await
                .ok_or_else(|| ApiError::not_found("Field not found"))?,
        ),
        None => None,
    };

    // A field-linked task belongs to that field's farm unless told otherwise.
    let field_farm = field.as_ref().map(|f| as_i64(f, "farm_id"));
    let farm_id = match (body.i64_any("farmId", "farm_id"), field_farm) {
        (Some(requested), Some(actual)) if requested != actual => {
            return Err(ApiError::bad_request(
                "Task farm does not match the field farm",
            ));
        }
        (Some(requested), _) => Some(requested),
        (None, Some(actual)) => Some(actual),
        (None, None) => None,
    };

    let due = body
        .opt_text_any("due_date", "dueDate")
        .filter(|s| domain::dates::parse(s).is_some())
        .unwrap_or_else(domain::dates::today_iso);

    let stamp = domain::dates::now_stamp();
    let row = json!({
        "farm_id": farm_id,
        "field_id": field_id,
        "task_type": task_type,
        "title": title,
        "notes": body.text("notes"),
        "due_date": due,
        "repeat_interval": interval,
        "completed_at": Value::Null,
        "created_at": stamp,
        "updated_at": stamp,
    });

    let saved = insert_returning(&db, Task::table_name(), &row)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let ref_date = domain::dates::today();
    Ok((
        axum::http::StatusCode::CREATED,
        axum::Json(TaskRow::from_row(&saved).to_json(ref_date)),
    )
        .into_response())
}

/// `PUT /api/tasks/{id}`
pub async fn update(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, Task::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Task not found"))?;

    let pick_str = |key: &str, alt: &str, fallback: String| -> String {
        if body.has(key) {
            body.text(key)
        } else if body.has(alt) {
            body.text(alt)
        } else {
            fallback
        }
    };

    // `json!` cannot hold a multi-statement block, so the defaults are
    // resolved before the row is built.
    let task_type = pick_str("taskType", "task_type", col(&existing, "task_type"));
    let task_type = if crate::reference::TASK_TYPES.contains(&task_type.as_str()) {
        task_type
    } else {
        "custom".to_string()
    };

    let due_date = pick_str("due_date", "dueDate", col(&existing, "due_date"));
    let due_date = if domain::dates::parse(&due_date).is_some() {
        due_date
    } else {
        col(&existing, "due_date")
    };

    let repeat_interval = pick_str(
        "repeatInterval",
        "repeat_interval",
        col(&existing, "repeat_interval"),
    );
    let repeat_interval = if crate::reference::REPEAT_INTERVALS.contains(&repeat_interval.as_str())
    {
        repeat_interval
    } else {
        "none".to_string()
    };

    let completed_at = if body.has("completed_at") {
        body.opt_text("completed_at")
    } else {
        opt_col(&existing, "completed_at")
    };

    let row = json!({
        "id": id,
        "farm_id": body.i64_any("farmId", "farm_id").or_else(|| as_opt_i64(&existing, "farm_id")),
        "field_id": body.i64_any("fieldId", "field_id").or_else(|| as_opt_i64(&existing, "field_id")),
        "task_type": task_type,
        "title": pick_str("title", "title", col(&existing, "title")),
        "notes": pick_str("notes", "notes", col(&existing, "notes")),
        "due_date": due_date,
        "repeat_interval": repeat_interval,
        "completed_at": completed_at,
        "updated_at": domain::dates::now_stamp(),
    });

    let saved = update_returning(&db, Task::table_name(), &row, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let ref_date = domain::dates::today();
    Ok(axum::Json(TaskRow::from_row(&saved).to_json(ref_date)).into_response())
}

/// `DELETE /api/tasks/{id}`
pub async fn destroy(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, Task::table_name(), id).await.is_none() {
        return Err(ApiError::not_found("Task not found"));
    }
    delete_by_id(&db, Task::table_name(), id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(no_content())
}

/// `POST /api/tasks/{id}/complete`
///
/// A repeating task rolls forward and stays open; a one-off task is marked
/// complete. Completing a field-linked task logs an inspection entry on that
/// field's timeline.
pub async fn complete(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, Task::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Task not found"))?;

    let interval = col(&existing, "repeat_interval");
    let title = col(&existing, "title");
    let field_id = as_opt_i64(&existing, "field_id");
    let due = col(&existing, "due_date");
    let today = domain::dates::today_iso();

    let repeating = interval != "none";
    let rescheduled_to = if repeating {
        let next = next_due(&due, &interval);
        patch_by_id(
            &db,
            Task::table_name(),
            &json!({ "due_date": next, "completed_at": Value::Null, "updated_at": domain::dates::now_stamp() }),
            id,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        Some(next)
    } else {
        patch_by_id(
            &db,
            Task::table_name(),
            &json!({ "completed_at": today, "updated_at": domain::dates::now_stamp() }),
            id,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        None
    };

    // Whole-farm tasks have no field to log against.
    let entry = match field_id {
        Some(fid) => {
            let outcome = {
                let n = body.text("notes");
                if n.is_empty() {
                    col(&existing, "notes")
                } else {
                    n
                }
            };
            // The inspection records the task title as its type, which is what
            // makes the timeline entry readable at a glance.
            let data = domain::entries::sanitise(
                "inspection",
                &json!({ "taskId": id, "taskType": title, "outcome": outcome }),
            );
            let stamp = domain::dates::now_stamp();
            let row = json!({
                "field_id": fid,
                "type": "inspection",
                "entry_date": today,
                "title": format!("Inspection - {title}"),
                "notes": outcome,
                "data": data.to_string(),
                "created_at": stamp,
                "updated_at": stamp,
            });
            let saved = insert_returning(&db, Entry::table_name(), &row)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            Some(entry_json(&saved))
        }
        None => None,
    };

    let task = find_by_id(&db, Task::table_name(), id)
        .await
        .map(|r| {
            let mut t = task_json(&r);
            if let (Some(obj), Some(next)) = (t.as_object_mut(), &rescheduled_to) {
                obj.insert("next_due".into(), json!(next));
            }
            t
        })
        .unwrap_or(Value::Null);

    Ok(axum::Json(json!({
        "task": task,
        "entry": entry,
        "rescheduledTo": rescheduled_to,
    }))
    .into_response())
}

fn task_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "farm_id": as_opt_i64(row, "farm_id"),
        "field_id": as_opt_i64(row, "field_id"),
        "task_type": col(row, "task_type"),
        "title": col(row, "title"),
        "notes": col(row, "notes"),
        "due_date": col(row, "due_date"),
        "repeat_interval": col(row, "repeat_interval"),
        "completed_at": opt_col(row, "completed_at"),
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

/// `PUT /api/resistance/{id}`
pub async fn update_resistance(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, ResistanceRecord::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Record not found"))?;

    let species = {
        let s = body.text_any("weed_species", "weedSpecies");
        if s.is_empty() {
            return Err(ApiError::bad_request("Weed species is required"));
        }
        s
    };

    let row = json!({
        "id": id,
        "field_id": as_i64(&existing, "field_id"),
        "weed_species": species,
        "product": if body.has("product") { body.text("product") } else { col(&existing, "product") },
        "active": if body.has("active") { body.text("active") } else { col(&existing, "active") },
        "moa_group": if body.has("moa_group") {
            body.text("moa_group")
        } else if body.has("moa") {
            body.text("moa")
        } else {
            col(&existing, "moa_group")
        },
        "source": if body.has("source") { body.text("source") } else { col(&existing, "source") },
        "confirmed_on": if body.has("confirmed_on") {
            body.opt_text("confirmed_on")
        } else {
            opt_col(&existing, "confirmed_on")
        },
        "notes": if body.has("notes") { body.text("notes") } else { col(&existing, "notes") },
        "updated_at": domain::dates::now_stamp(),
    });

    let saved = update_returning(&db, ResistanceRecord::table_name(), &row, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(axum::Json(resistance_json(&saved)).into_response())
}

/// `DELETE /api/resistance/{id}`
pub async fn destroy_resistance(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
) -> ApiResult<Response> {
    if find_by_id(&db, ResistanceRecord::table_name(), id)
        .await
        .is_none()
    {
        return Err(ApiError::not_found("Record not found"));
    }
    delete_by_id(&db, ResistanceRecord::table_name(), id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(no_content())
}

/// `PUT /api/seasons/{id}`
pub async fn update_season(
    CtxExt(db): CtxExt<Db>,
    Path(id): Path<i64>,
    body: Body,
) -> ApiResult<Response> {
    let existing = find_by_id(&db, SeasonHistory::table_name(), id)
        .await
        .ok_or_else(|| ApiError::not_found("Season not found"))?;

    let crop = {
        let c = body.text("crop");
        if c.is_empty() {
            return Err(ApiError::bad_request("Crop is required"));
        }
        c
    };

    let row = json!({
        "id": id,
        "field_id": as_i64(&existing, "field_id"),
        "season_year": body.i64_any("season_year", "seasonYear")
            .unwrap_or_else(|| as_i64(&existing, "season_year")),
        "crop": crop,
        "sown_date": if body.has("sown_date") {
            body.opt_text("sown_date")
        } else {
            opt_col(&existing, "sown_date")
        },
        "harvested": if body.has("harvested") {
            if body.bool("harvested").unwrap_or(false) { 1 } else { 0 }
        } else {
            as_i64(&existing, "harvested")
        },
        "chemicals": if body.has("chemicals") { body.text("chemicals") } else { col(&existing, "chemicals") },
        "notes": if body.has("notes") { body.text("notes") } else { col(&existing, "notes") },
        "updated_at": domain::dates::now_stamp(),
    });

    let saved = update_returning(&db, SeasonHistory::table_name(), &row, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(axum::Json(season_json(&saved)).into_response())
}

/// `DELETE /api/seasons/{id}`
pub async fn destroy_season(CtxExt(db): CtxExt<Db>, Path(id): Path<i64>) -> ApiResult<Response> {
    if find_by_id(&db, SeasonHistory::table_name(), id)
        .await
        .is_none()
    {
        return Err(ApiError::not_found("Season not found"));
    }
    delete_by_id(&db, SeasonHistory::table_name(), id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(no_content())
}

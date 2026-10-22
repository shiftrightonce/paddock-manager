//! Database access and row decoding.
//!
//! Rows come back from `Manager` as maps of stringified values, so decoding is
//! centralised here. `row_*` helpers read a column as text, `as_i64`, `as_f64`
//! and `as_json` interpret it, and the models convert to the JSON shapes the
//! frontend consumes.

use dirtybase_contract::db_contract::base::manager::Manager;
use dirtybase_contract::db_contract::base::query::QueryBuilder;
use dirtybase_contract::db_contract::field_values::FieldValue;
use dirtybase_contract::db_contract::query_result::QueryResult;
use dirtybase_contract::db_contract::types::ColumnAndValue;
use serde_json::{Map, Value, json};

use crate::models::*;

pub type Db = Manager;

/// Flatten a driver row into a JSON object.
///
/// Rows arrive as typed column maps; every scalar becomes a JSON string, number
/// or null, which is what `col`/`as_i64`/`as_f64`/`as_object` below expect.
pub fn row(record: impl IntoIterator<Item = (String, FieldValue)>) -> Value {
    let map: Map<String, Value> = record
        .into_iter()
        .filter(|(k, _)| k != "internal_id")
        .map(|(k, v)| (k, field_to_json(&v)))
        .collect();
    Value::Object(map)
}

/// One `FieldValue` as JSON.
///
/// `FieldValue` is an internally tagged enum (`{"t":..,"v":..}`), so it cannot be
/// serialised directly: that would turn every column into an object and quietly
/// lose the value. Timestamps use the same text formats as the previous backend.
pub fn field_to_json(value: &FieldValue) -> Value {
    match value {
        FieldValue::Null | FieldValue::NotSet => Value::Null,
        FieldValue::I8(v) => json!(v),
        FieldValue::I16(v) => json!(v),
        FieldValue::I32(v) => json!(v),
        FieldValue::U32(v) => json!(v),
        FieldValue::U64(v) => json!(v),
        FieldValue::I64(v) => json!(v),
        FieldValue::F64(v) => json!(v),
        FieldValue::String(v) => Value::String(v.clone()),
        FieldValue::Boolean(v) => json!(v),
        FieldValue::Object(v) => {
            let map: Map<String, Value> = v
                .iter()
                .map(|(k, inner)| (k.clone(), field_to_json(inner)))
                .collect();
            Value::Object(map)
        }
        FieldValue::Array(v) => Value::Array(v.iter().map(field_to_json).collect()),
        // No column stores binary; photos are files on disk plus a filename row.
        FieldValue::Binary(v) => {
            Value::String(v.iter().map(|byte| format!("{byte:02x}")).collect())
        }
        FieldValue::Uuid(v) => Value::String(v.to_string()),
        FieldValue::DateTime(v) | FieldValue::Timestamp(v) => {
            Value::String(v.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        FieldValue::Date(v) => Value::String(v.format("%Y-%m-%d").to_string()),
        FieldValue::Time(v) => Value::String(v.format("%H:%M:%S").to_string()),
        FieldValue::Failable { field, .. } => field_to_json(field),
    }
}

/// The JSON form of a fetched row.
pub fn json_rows(records: Vec<dirtybase_db::types::StructuredColumnAndValue>) -> Vec<Value> {
    records
        .into_iter()
        .map(|r| row(r.fields_ref().clone()))
        .collect()
}

/// Insert/update columns from a JSON object. `null` values are kept as SQL NULL.
pub fn columns(obj: &Value) -> ColumnAndValue {
    match obj.as_object() {
        Some(map) => map
            .iter()
            .map(|(k, v)| (k.clone(), FieldValue::from(v.clone())))
            .collect(),
        None => ColumnAndValue::new(),
    }
}

/// Insert one row and return the new id.
pub async fn insert(db: &Db, table: &str, obj: &Value) -> Result<i64, anyhow::Error> {
    let result: QueryResult = db.insert(table, columns(obj)).await?;
    Ok(result.last_insert_id())
}

/// Insert a row and read it back, so the response body is what was stored.
///
/// The original API returned the inserted row including its new id, and the
/// frontend navigates using it.
pub async fn insert_returning(db: &Db, table: &str, obj: &Value) -> Result<Value, anyhow::Error> {
    let id = insert(db, table, obj).await?;
    find_by_id(db, table, id)
        .await
        .ok_or_else(|| anyhow::anyhow!("row disappeared after insert"))
}

/// Update one row by primary key and read it back.
///
/// The primary key is dropped from the payload: callers pass the row they read
/// earlier, and re-setting `id` is at best a no-op.
pub async fn update_returning(
    db: &Db,
    table: &str,
    obj: &Value,
    id: i64,
) -> Result<Value, anyhow::Error> {
    let mut payload = obj.clone();
    if let Some(map) = payload.as_object_mut() {
        map.remove("id");
    }
    db.update(table, columns(&payload), |q| {
        q.is_eq("id", id);
    })
    .await?;
    find_by_id(db, table, id)
        .await
        .ok_or_else(|| anyhow::anyhow!("row disappeared after update"))
}

/// Update columns without reading the row back, for partial updates.
pub async fn patch_by_id(db: &Db, table: &str, obj: &Value, id: i64) -> Result<(), anyhow::Error> {
    let mut payload = obj.clone();
    if let Some(map) = payload.as_object_mut() {
        map.remove("id");
    }
    db.update(table, columns(&payload), |q| {
        q.is_eq("id", id);
    })
    .await
}

/// Delete one row by primary key.
pub async fn delete_where(
    db: &Db,
    table: &str,
    column: &str,
    value: i64,
) -> Result<(), anyhow::Error> {
    db.delete(table, |q| {
        q.is_eq(column, value);
    })
    .await
}

pub fn col(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn as_i64(row: &Value, key: &str) -> i64 {
    match row.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or_default(),
        Some(Value::String(s)) => s.trim().parse().unwrap_or_default(),
        _ => 0,
    }
}

pub fn as_opt_i64(row: &Value, key: &str) -> Option<i64> {
    match row.get(key) {
        Some(Value::Null) | None => None,
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() { None } else { t.parse().ok() }
        }
        _ => None,
    }
}

pub fn as_f64(row: &Value, key: &str) -> f64 {
    match row.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or_default(),
        Some(Value::String(s)) => s.trim().parse().unwrap_or_default(),
        _ => 0.0,
    }
}

pub fn as_opt_f64(row: &Value, key: &str) -> Option<f64> {
    match row.get(key) {
        Some(Value::Null) | None => None,
        Some(Value::Number(n)) => n.as_f64().filter(|v| v.is_finite()),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<f64>().ok().filter(|v| v.is_finite())
            }
        }
        _ => None,
    }
}

/// Parse a JSON object column, falling back to `{}` for blanks or malformed text.
pub fn as_object(row: &Value, key: &str) -> Map<String, Value> {
    let raw = col(row, key);
    serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn as_value(row: &Value, key: &str) -> Value {
    Value::Object(as_object(row, key))
}

pub fn farm_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "name": col(row, "name"),
        "location": col(row, "location"),
        "weather_station": col(row, "weather_station"),
        "notes": col(row, "notes"),
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

pub fn field_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "farm_id": as_i64(row, "farm_id"),
        "name": col(row, "name"),
        "location": col(row, "location"),
        "area_ha": as_opt_f64(row, "area_ha"),
        "soil_type": col(row, "soil_type"),
        "notes": col(row, "notes"),
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

pub fn entry_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "field_id": as_i64(row, "field_id"),
        "type": col(row, "type"),
        "entry_date": col(row, "entry_date"),
        "title": col(row, "title"),
        "notes": col(row, "notes"),
        "data": as_value(row, "data"),
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

pub fn resistance_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "field_id": as_i64(row, "field_id"),
        "weed_species": col(row, "weed_species"),
        "product": col(row, "product"),
        "active": col(row, "active"),
        "moa_group": col(row, "moa_group"),
        "source": col(row, "source"),
        "confirmed_on": col(row, "confirmed_on"),
        "notes": col(row, "notes"),
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

pub fn season_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "field_id": as_i64(row, "field_id"),
        "season_year": as_i64(row, "season_year"),
        "crop": col(row, "crop"),
        "sown_date": col(row, "sown_date"),
        "harvested": as_i64(row, "harvested"),
        "chemicals": col(row, "chemicals"),
        "notes": col(row, "notes"),
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

pub fn photo_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "entry_id": as_i64(row, "entry_id"),
        "filename": col(row, "filename"),
        "original_name": col(row, "original_name"),
        "mime": col(row, "mime"),
        "size": as_i64(row, "size"),
        "created_at": col(row, "created_at"),
    })
}

/// Normalised task row shared by the task list, farm rollups and the PDF.
#[derive(Clone, Debug)]
pub struct TaskRow {
    pub id: i64,
    pub farm_id: Option<i64>,
    pub field_id: Option<i64>,
    pub task_type: String,
    pub title: String,
    pub notes: String,
    pub due_date: String,
    pub repeat_interval: String,
    pub completed_at: Option<String>,
    pub field_name: Option<String>,
    pub field_farm_id: Option<i64>,
    pub farm_name: Option<String>,
}

impl TaskRow {
    pub fn from_row(row: &Value) -> Self {
        Self {
            id: as_i64(row, "id"),
            farm_id: as_opt_i64(row, "farm_id"),
            field_id: as_opt_i64(row, "field_id"),
            task_type: col(row, "task_type"),
            title: col(row, "title"),
            notes: col(row, "notes"),
            due_date: col(row, "due_date"),
            repeat_interval: col(row, "repeat_interval"),
            completed_at: opt_col(row, "completed_at"),
            field_name: opt_col(row, "field_name"),
            field_farm_id: as_opt_i64(row, "field_farm_id"),
            farm_name: opt_col(row, "farm_name"),
        }
    }

    /// A field-linked task resolves its farm through the field.
    pub fn effective_farm_id(&self) -> Option<i64> {
        self.farm_id.or(self.field_farm_id)
    }

    pub fn to_json(&self, ref_date: NaiveDateAlias) -> Value {
        let due = crate::domain::dates::parse(&self.due_date);
        let overdue = due.map(|d| d < ref_date).unwrap_or(false);
        let due_today = due.map(|d| d == ref_date).unwrap_or(false);
        json!({
            "id": self.id,
            "farm_id": self.farm_id,
            "field_id": self.field_id,
            "task_type": self.task_type,
            "title": self.title,
            "notes": self.notes,
            "due_date": self.due_date,
            "repeat_interval": self.repeat_interval,
            "completed_at": self.completed_at,
            "field_name": self.field_name,
            "farm_name": self.farm_name,
            "overdue": overdue,
            "dueToday": due_today,
        })
    }
}

pub type NaiveDateAlias = chrono::NaiveDate;

pub fn opt_col(row: &Value, key: &str) -> Option<String> {
    let v = col(row, key);
    (!v.is_empty()).then_some(v)
}

pub fn mix_json(row: &Value, products: Vec<Value>) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "name": col(row, "name"),
        "notes": col(row, "notes"),
        "products": products,
        "created_at": col(row, "created_at"),
        "updated_at": col(row, "updated_at"),
    })
}

pub fn mix_product_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "mix_id": as_i64(row, "mix_id"),
        "position": as_i64(row, "position"),
        "product": col(row, "product"),
        "active": col(row, "active"),
        "moa_group": col(row, "moa_group"),
        "strength": as_opt_f64(row, "strength"),
        "strength_unit": col(row, "strength_unit"),
        "rate": as_opt_f64(row, "rate"),
        "rate_unit": col(row, "rate_unit"),
        "residual": as_i64(row, "residual") != 0,
    })
}

pub fn flag_json(row: &Value) -> Value {
    json!({
        "id": as_i64(row, "id"),
        "product_a": col(row, "product_a"),
        "product_b": col(row, "product_b"),
        "pair_key": col(row, "pair_key"),
        "note": col(row, "note"),
        "created_at": col(row, "created_at"),
    })
}

/// Look up one row by primary key.
pub async fn find_by_id(db: &Db, table: &str, id: i64) -> Option<Value> {
    find_one_where(db, table, |q| {
        q.is_eq("id", id);
    })
    .await
}

/// Look up one row by any single column value, for the tables whose natural key
/// is not the id (the compatibility `pair_key`, a mix name).
pub async fn find_by_id_key(db: &Db, table: &str, column: &str, value: &str) -> Option<Value> {
    find_one_where(db, table, |q| {
        q.is_eq(column, value.to_string());
    })
    .await
}

async fn find_one_where<F>(db: &Db, table: &str, filter: F) -> Option<Value>
where
    F: FnOnce(&mut QueryBuilder),
{
    db.select_from_table(table, filter)
        .fetch_one()
        .await
        .ok()
        .flatten()
        .map(|r| row(r.fields_ref().clone()))
}

pub async fn delete_by_id(db: &Db, table: &str, id: i64) -> Result<(), anyhow::Error> {
    delete_where(db, table, "id", id).await
}

pub async fn farms(db: &Db) -> Vec<Value> {
    let mut rows = db
        .select_from_table(Farm::table_name(), |_| {})
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        col(a, "name")
            .to_lowercase()
            .cmp(&col(b, "name").to_lowercase())
    });
    rows
}

pub async fn fields_of_farm(db: &Db, farm_id: i64) -> Vec<Value> {
    let mut rows = db
        .select_from_table(Field::table_name(), |q| {
            q.is_eq("farm_id", farm_id);
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        col(a, "name")
            .to_lowercase()
            .cmp(&col(b, "name").to_lowercase())
    });
    rows
}

pub async fn all_fields(db: &Db) -> Vec<Value> {
    let mut rows = db
        .select_from_table(Field::table_name(), |_| {})
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        col(a, "name")
            .to_lowercase()
            .cmp(&col(b, "name").to_lowercase())
    });
    rows
}

pub async fn entries_of_field(db: &Db, field_id: i64) -> Vec<Value> {
    let mut rows = db
        .select_from_table(Entry::table_name(), |q| {
            q.is_eq("field_id", field_id);
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    // Newest first, with the id as a tiebreak for same-day entries.
    rows.sort_by(|a, b| {
        col(b, "entry_date")
            .cmp(&col(a, "entry_date"))
            .then_with(|| as_i64(b, "id").cmp(&as_i64(a, "id")))
    });
    rows
}

pub async fn resistance_of_field(db: &Db, field_id: i64) -> Vec<Value> {
    let mut rows = db
        .select_from_table(ResistanceRecord::table_name(), |q| {
            q.is_eq("field_id", field_id);
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    // Confirmed records first, then most recently confirmed.
    rows.sort_by(|a, b| {
        let ca = opt_col(a, "confirmed_on");
        let cb = opt_col(b, "confirmed_on");
        ca.is_none()
            .cmp(&cb.is_none())
            .then_with(|| cb.cmp(&ca))
            .then_with(|| as_i64(b, "id").cmp(&as_i64(a, "id")))
    });
    rows
}

pub async fn photos_of_entry(db: &Db, entry_id: i64) -> Vec<Value> {
    let mut rows = db
        .select_from_table(EntryPhoto::table_name(), |q| {
            q.is_eq("entry_id", entry_id);
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| as_i64(a, "id").cmp(&as_i64(b, "id")));
    rows
}

pub async fn seasons_of_field(db: &Db, field_id: i64) -> Vec<Value> {
    let mut rows = db
        .select_from_table(SeasonHistory::table_name(), |q| {
            q.is_eq("field_id", field_id);
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        as_i64(b, "season_year")
            .cmp(&as_i64(a, "season_year"))
            .then_with(|| as_i64(b, "id").cmp(&as_i64(a, "id")))
    });
    rows
}

pub async fn products_of_mix(db: &Db, mix_id: i64) -> Vec<Value> {
    let mut rows = db
        .select_from_table(TankMixProduct::table_name(), |q| {
            q.is_eq("mix_id", mix_id);
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        as_i64(a, "position")
            .cmp(&as_i64(b, "position"))
            .then_with(|| as_i64(a, "id").cmp(&as_i64(b, "id")))
    });
    rows
}

pub async fn flags(db: &Db) -> Vec<Value> {
    let mut rows = db
        .select_from_table(CompatibilityFlag::table_name(), |_q| {})
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        col(a, "product_a")
            .cmp(&col(b, "product_a"))
            .then_with(|| col(a, "product_b").cmp(&col(b, "product_b")))
    });
    rows
}

/// Incomplete tasks ordered by due date, optionally scoped to a farm or field.
///
/// Farm scoping matches either the task's own farm or the farm of the field it
/// points at, so field-linked tasks are included by their farm.
pub async fn tasks_filtered(
    db: &Db,
    farm_id: Option<i64>,
    field_id: Option<i64>,
    include_completed: bool,
    limit: Option<i64>,
) -> Vec<TaskRow> {
    let rows = db
        .select_from_table(Task::table_name(), |q| {
            if let Some(fid) = field_id {
                q.is_eq("field_id", fid);
            }
            if !include_completed {
                q.is_null("completed_at");
            }
        })
        .fetch_all()
        .await
        .map(json_rows)
        .unwrap_or_default();

    let mut out: Vec<TaskRow> = rows.iter().map(TaskRow::from_row).collect();
    out.retain(|t| farm_id.is_none() || t.effective_farm_id() == farm_id);
    // Completed tasks sink below open ones.
    out.sort_by(|a, b| {
        a.completed_at
            .is_some()
            .cmp(&b.completed_at.is_some())
            .then_with(|| a.due_date.cmp(&b.due_date))
            .then_with(|| a.id.cmp(&b.id))
    });
    if let Some(n) = limit {
        out.truncate(n as usize);
    }
    out
}

/// Tasks joined to their field/farm names, for display.
pub async fn tasks_with_names(
    db: &Db,
    farm_id: Option<i64>,
    field_id: Option<i64>,
    include_completed: bool,
    limit: Option<i64>,
) -> Vec<TaskRow> {
    let tasks = tasks_filtered(db, farm_id, field_id, include_completed, None).await;
    if tasks.is_empty() {
        return Vec::new();
    }

    let farm_rows = farms(db).await;
    let farm_names: std::collections::HashMap<i64, String> = farm_rows
        .iter()
        .map(|f| (as_i64(f, "id"), col(f, "name")))
        .collect();
    let field_rows = all_fields(db).await;
    let field_info: std::collections::HashMap<i64, (String, i64)> = field_rows
        .iter()
        .map(|f| (as_i64(f, "id"), (col(f, "name"), as_i64(f, "farm_id"))))
        .collect();

    let mut out: Vec<TaskRow> = tasks
        .into_iter()
        .map(|mut t| {
            if let Some((name, fid)) = t.field_id.and_then(|id| field_info.get(&id)) {
                t.field_name = Some(name.clone());
                t.field_farm_id = Some(*fid);
            }
            if t.farm_id.is_none() {
                t.farm_name = t
                    .effective_farm_id()
                    .and_then(|id| farm_names.get(&id).cloned());
            } else {
                t.farm_name = t.farm_id.and_then(|id| farm_names.get(&id).cloned());
            }
            t
        })
        .collect();

    if let Some(n) = limit {
        out.truncate(n as usize);
    }
    out
}

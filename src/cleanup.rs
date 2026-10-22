//! Referential cleanup.
//!
//! Deleting a farm or field has to reach everything hanging off it. Doing this
//! in code keeps the behaviour explicit, and lets the photo files on disk be
//! removed along with their rows, which database cascades cannot do.

use crate::db::*;
use crate::models::*;

/// Delete an entry's photo rows and their files.
pub async fn delete_entry(db: &Db, entry_id: i64) {
    for row in photos_of_entry(db, entry_id).await {
        crate::uploads::remove_file(&col(&row, "filename"));
    }
    delete_where(db, EntryPhoto::table_name(), "entry_id", entry_id).await;
}

pub async fn delete_field(db: &Db, field_id: i64) {
    for e in entries_of_field(db, field_id).await {
        delete_entry(db, as_i64(&e, "id")).await;
    }
    delete_where(db, Entry::table_name(), "field_id", field_id).await;
    delete_where(db, ResistanceRecord::table_name(), "field_id", field_id).await;
    delete_where(db, SeasonHistory::table_name(), "field_id", field_id).await;
    delete_where(db, Task::table_name(), "field_id", field_id).await;
    delete_row(db, Field::table_name(), field_id).await;
}

pub async fn delete_farm(db: &Db, farm_id: i64) {
    for f in fields_of_farm(db, farm_id).await {
        delete_field(db, as_i64(&f, "id")).await;
    }
    delete_where(db, Task::table_name(), "farm_id", farm_id).await;
    delete_row(db, Farm::table_name(), farm_id).await;
}

/// Delete a mix and its product rows.
pub async fn delete_mix(db: &Db, mix_id: i64) {
    delete_where(db, TankMixProduct::table_name(), "mix_id", mix_id).await;
    delete_row(db, TankMix::table_name(), mix_id).await;
}

/// Remove rows matching one column value.
pub async fn delete_where(db: &Db, table: &str, column: &str, value: i64) {
    let _ = crate::db::delete_where(db, table, column, value).await;
}

/// Delete one row by id, logging a failure instead of hiding it.
///
/// The row is already gone or the caller has checked it exists, so this only
/// runs on a database error; logging keeps that visible.
async fn delete_row(db: &Db, table: &str, id: i64) {
    if let Err(e) = delete_by_id(db, table, id).await {
        tracing::error!("could not delete {table} {id}: {e:#}");
    }
}

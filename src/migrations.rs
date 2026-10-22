//! Schema creation.
//!
//! The app ships with the schema from the previous Node backend already in
//! `data/paddock.sqlite`, so migrations run on every boot and have to cope with
//! two cases: the table is missing, in which case it is created in full, or the
//! table is already there, in which case nothing happens.
//!
//! The blueprint below is therefore the *fallback* for a new database. It is
//! written to match `server/schema.sql` column for column, which `models.rs`
//! asserts, so a fresh database behaves exactly like the shipped one.
//!
//! `create_table_schema` errors when the table exists, which is why every table
//! is guarded by `has_table` first rather than by catching the error.

use dirtybase_contract::app_contract::Context;
use dirtybase_contract::db_contract::base::manager::Manager;
use dirtybase_contract::db_contract::base::table::TableBlueprint;
use dirtybase_contract::db_contract::migration::Migration;

use crate::models::*;

pub struct PaddockMigration;

/// Create `table` from `build` when it is missing.
///
/// `create_table_schema` fails on an existing table, so the existence check is
/// what makes repeat boots no-ops.
async fn ensure_table<F>(manager: &Manager, table: &str, build: F) -> Result<(), anyhow::Error>
where
    F: Fn(&mut TableBlueprint) + Copy + Send,
{
    if manager.has_table(table).await? {
        return Ok(());
    }
    manager.create_table_schema(table, build).await
}

#[dirtybase_contract::async_trait]
impl Migration for PaddockMigration {
    async fn up(&self, manager: &Manager, _: &Context) -> Result<(), anyhow::Error> {
        ensure_table(manager, Farm::table_name(), |t| {
            t.id(None);
            t.string(Farm::col_name_for_name()).set_is_nullable(false);
            t.string(Farm::col_name_for_location())
                .default_is_empty_string();
            t.string(Farm::col_name_for_weather_station())
                .default_is_empty_string();
            t.text(Farm::col_name_for_notes()).default_is_empty_string();
            t.string(Farm::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(Farm::col_name_for_updated_at())
                .set_is_nullable(false);
        })
        .await?;

        ensure_table(manager, Field::table_name(), |t| {
            t.id(None);
            t.integer(Field::col_name_for_farm_id())
                .set_is_nullable(false)
                .references_with_cascade_delete(Farm::table_name(), "id");
            t.string(Field::col_name_for_name()).set_is_nullable(false);
            t.string(Field::col_name_for_location())
                .default_is_empty_string();
            t.number(Field::col_name_for_area_ha());
            t.string(Field::col_name_for_soil_type())
                .default_is_empty_string();
            t.text(Field::col_name_for_notes())
                .default_is_empty_string();
            t.string(Field::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(Field::col_name_for_updated_at())
                .set_is_nullable(false);
            t.index(&[Field::col_name_for_farm_id()]);
        })
        .await?;

        ensure_table(manager, Entry::table_name(), |t| {
            t.id(None);
            t.integer(Entry::col_name_for_field_id())
                .set_is_nullable(false)
                .references_with_cascade_delete(Field::table_name(), "id");
            t.string(Entry::col_name_for_entry_type())
                .set_is_nullable(false);
            t.string(Entry::col_name_for_entry_date())
                .set_is_nullable(false);
            t.string(Entry::col_name_for_title())
                .default_is_empty_string();
            t.text(Entry::col_name_for_notes())
                .default_is_empty_string();
            t.text(Entry::col_name_for_data()).default_is_empty_string();
            t.string(Entry::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(Entry::col_name_for_updated_at())
                .set_is_nullable(false);
            t.index(&[
                Entry::col_name_for_field_id(),
                Entry::col_name_for_entry_date(),
            ]);
            t.index(&[Entry::col_name_for_entry_type()]);
        })
        .await?;

        ensure_table(manager, EntryPhoto::table_name(), |t| {
            t.id(None);
            t.integer(EntryPhoto::col_name_for_entry_id())
                .set_is_nullable(false)
                .references_with_cascade_delete(Entry::table_name(), "id");
            t.string(EntryPhoto::col_name_for_filename())
                .set_is_nullable(false);
            t.string(EntryPhoto::col_name_for_original_name())
                .default_is_empty_string();
            t.string(EntryPhoto::col_name_for_mime())
                .default_is_empty_string();
            t.integer(EntryPhoto::col_name_for_size()).default_is_zero();
            t.string(EntryPhoto::col_name_for_created_at())
                .set_is_nullable(false);
            t.index(&[EntryPhoto::col_name_for_entry_id()]);
        })
        .await?;

        ensure_table(manager, ResistanceRecord::table_name(), |t| {
            t.id(None);
            t.integer(ResistanceRecord::col_name_for_field_id())
                .set_is_nullable(false)
                .references_with_cascade_delete(Field::table_name(), "id");
            t.string(ResistanceRecord::col_name_for_weed_species())
                .set_is_nullable(false);
            t.string(ResistanceRecord::col_name_for_product())
                .default_is_empty_string();
            t.string(ResistanceRecord::col_name_for_active())
                .default_is_empty_string();
            t.string(ResistanceRecord::col_name_for_moa_group())
                .default_is_empty_string();
            t.string(ResistanceRecord::col_name_for_source())
                .default_is_empty_string();
            t.string(ResistanceRecord::col_name_for_confirmed_on());
            t.text(ResistanceRecord::col_name_for_notes())
                .default_is_empty_string();
            t.string(ResistanceRecord::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(ResistanceRecord::col_name_for_updated_at())
                .set_is_nullable(false);
            t.index(&[ResistanceRecord::col_name_for_field_id()]);
        })
        .await?;

        ensure_table(manager, Task::table_name(), |t| {
            t.id(None);
            t.integer(Task::col_name_for_farm_id())
                .references_with_cascade_delete(Farm::table_name(), "id");
            t.integer(Task::col_name_for_field_id())
                .references_with_cascade_delete(Field::table_name(), "id");
            t.string(Task::col_name_for_task_type())
                .default_is_empty_string();
            t.string(Task::col_name_for_title()).set_is_nullable(false);
            t.text(Task::col_name_for_notes()).default_is_empty_string();
            t.string(Task::col_name_for_due_date())
                .set_is_nullable(false);
            t.string(Task::col_name_for_repeat_interval())
                .default_is_empty_string();
            t.string(Task::col_name_for_completed_at());
            t.string(Task::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(Task::col_name_for_updated_at())
                .set_is_nullable(false);
            t.index(&[
                Task::col_name_for_due_date(),
                Task::col_name_for_completed_at(),
            ]);
        })
        .await?;

        ensure_table(manager, TankMix::table_name(), |t| {
            t.id(None);
            t.string(TankMix::col_name_for_name())
                .set_is_nullable(false);
            t.text(TankMix::col_name_for_notes())
                .default_is_empty_string();
            t.string(TankMix::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(TankMix::col_name_for_updated_at())
                .set_is_nullable(false);
        })
        .await?;

        ensure_table(manager, TankMixProduct::table_name(), |t| {
            t.id(None);
            t.integer(TankMixProduct::col_name_for_mix_id())
                .set_is_nullable(false)
                .references_with_cascade_delete(TankMix::table_name(), "id");
            t.integer(TankMixProduct::col_name_for_position())
                .default_is_zero();
            t.string(TankMixProduct::col_name_for_product())
                .default_is_empty_string();
            t.string(TankMixProduct::col_name_for_active())
                .default_is_empty_string();
            t.string(TankMixProduct::col_name_for_moa_group())
                .default_is_empty_string();
            t.number(TankMixProduct::col_name_for_strength());
            t.string(TankMixProduct::col_name_for_strength_unit())
                .default_is_empty_string();
            t.number(TankMixProduct::col_name_for_rate());
            t.string(TankMixProduct::col_name_for_rate_unit())
                .default_is_empty_string();
            t.integer(TankMixProduct::col_name_for_residual())
                .default_is_zero();
            t.index(&[TankMixProduct::col_name_for_mix_id()]);
        })
        .await?;

        ensure_table(manager, CompatibilityFlag::table_name(), |t| {
            t.id(None);
            t.string(CompatibilityFlag::col_name_for_product_a())
                .set_is_nullable(false);
            t.string(CompatibilityFlag::col_name_for_product_b())
                .set_is_nullable(false);
            t.string(CompatibilityFlag::col_name_for_pair_key())
                .set_is_unique(true)
                .set_is_nullable(false);
            t.text(CompatibilityFlag::col_name_for_note())
                .default_is_empty_string();
            t.string(CompatibilityFlag::col_name_for_created_at())
                .set_is_nullable(false);
        })
        .await?;

        ensure_table(manager, SeasonHistory::table_name(), |t| {
            t.id(None);
            t.integer(SeasonHistory::col_name_for_field_id())
                .set_is_nullable(false)
                .references_with_cascade_delete(Field::table_name(), "id");
            t.integer(SeasonHistory::col_name_for_season_year())
                .set_is_nullable(false);
            t.string(SeasonHistory::col_name_for_crop())
                .set_is_nullable(false);
            t.string(SeasonHistory::col_name_for_sown_date());
            t.integer(SeasonHistory::col_name_for_harvested())
                .default_is_zero();
            t.string(SeasonHistory::col_name_for_chemicals())
                .default_is_empty_string();
            t.text(SeasonHistory::col_name_for_notes())
                .default_is_empty_string();
            t.string(SeasonHistory::col_name_for_created_at())
                .set_is_nullable(false);
            t.string(SeasonHistory::col_name_for_updated_at())
                .set_is_nullable(false);
            t.index(&[
                SeasonHistory::col_name_for_field_id(),
                SeasonHistory::col_name_for_season_year(),
            ]);
        })
        .await?;

        Ok(())
    }

    /// Children before parents, the reverse of `all_tables`, so the foreign keys
    /// that `up` declared do not block the drop.
    async fn down(&self, manager: &Manager, _: &Context) -> Result<(), anyhow::Error> {
        for table in crate::models::all_tables().into_iter().rev() {
            if manager.has_table(table).await? {
                manager.drop_table(table).await?;
            }
        }
        Ok(())
    }
}

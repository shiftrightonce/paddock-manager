//! Paddock Manager.
//!
//! The application is a single dirtybase extension: `setup` loads the reference
//! data the UI needs at boot, `migrations` owns the schema, and `register_routes`
//! wires the JSON API under `/api` plus the SPA and photo routes that dirtybase
//! does not provide itself.

mod cleanup;
mod db;
mod domain;
mod http;
mod migrations;
mod models;
mod nutrient;
mod pdf;
mod reference;
mod uploads;

use dirtybase_contract::db_contract::base::manager::Manager;
use dirtybase_contract::db_contract::migration::Migration;
use dirtybase_contract::prelude::{Context, ExtensionMigrations, ExtensionSetup, RouterManager};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let app_service = dirtybase_app::setup().await?;
    app_service.register(PaddockApp).await;

    // A subcommand is dirtybase's own CLI (`migrate`, `seed`, `encrypt`, `cron`,
    // `serve`). Without one the app just serves, which is what the previous
    // backend did with `node server/index.js`.
    match std::env::args().nth(1).as_deref() {
        Some(_) => dirtybase_app::run(app_service).await,
        None => {
            app_service.init().await;
            dirtybase_app::run_http(app_service).await
        }
    }
}

struct PaddockApp;

#[dirtybase_contract::async_trait]
impl ExtensionSetup for PaddockApp {
    async fn setup(&mut self, _context: &Context) {
        tracing::info!("paddock manager setting up");
        // The data directory is created up front so uploads work on a fresh
        // database.
        match uploads::ensure_upload_dir() {
            Ok(()) => tracing::info!(
                "uploads directory ready at {}",
                uploads::upload_dir().display()
            ),
            Err(e) => tracing::error!("could not prepare the uploads directory: {e}"),
        }
    }

    async fn migrations(&self, _context: &Context) -> Option<ExtensionMigrations> {
        Some(vec![Box::new(migrations::PaddockMigration)])
    }

    /// The schema is ensured on every boot rather than behind a CLI command, so
    /// a missing table is created before the first request arrives.
    ///
    /// `PaddockMigration::up` guards every table with `has_table`, so this is a
    /// no-op on an existing database rather than an error.
    async fn run(&mut self, context: &Context) {
        let manager = match context.get::<Manager>().await {
            Ok(manager) => manager,
            Err(e) => {
                tracing::error!("could not reach the database: {e:#}");
                return;
            }
        };

        if let Err(e) = migrations::PaddockMigration.up(&manager, context).await {
            tracing::error!("could not prepare the schema: {e:#}");
            std::process::exit(1);
        }
        tracing::info!("schema is ready");
    }

    fn register_routes(&self, manager: &mut RouterManager) {
        manager.api(None, |router| {
            router
                // ---------------------------------------------------------- core
                .get_x("/health", http::photos::health)
                .get_x("/dashboard", http::dashboard::dashboard)
                // farms
                .get_x("/farms", http::farms::list)
                .post_x("/farms", http::farms::create)
                .get_x("/farms/{id}", http::farms::detail)
                .put_x("/farms/{id}", http::farms::update)
                .delete_x("/farms/{id}", http::farms::destroy)
                // fields
                .get_x("/fields", http::farms::list_fields)
                .post_x("/fields", http::farms::create_field)
                .get_x("/fields/{id}", http::farms::field_detail)
                .put_x("/fields/{id}", http::farms::update_field)
                .delete_x("/fields/{id}", http::farms::destroy_field)
                .get_x("/fields/{id}/water", http::farms::field_water)
                .get_x(
                    "/fields/{id}/last-residual",
                    http::farms::field_last_residual,
                )
                .get_x("/fields/{id}/moa-history", http::farms::field_moa_history)
                .get_x(
                    "/fields/{id}/nutrient-budget",
                    http::farms::field_nutrient_budget,
                )
                .get_x(
                    "/fields/{id}/resistance",
                    http::farms::field_resistance_list,
                )
                .post_x(
                    "/fields/{id}/resistance",
                    http::farms::field_resistance_create,
                )
                .post_x(
                    "/fields/{id}/resistance/check",
                    http::farms::field_resistance_check,
                )
                .get_x("/fields/{id}/seasons", http::farms::field_seasons)
                .post_x("/fields/{id}/seasons", http::farms::field_seasons_create)
                .put_x("/resistance/{id}", http::tasks::update_resistance)
                .delete_x("/resistance/{id}", http::tasks::destroy_resistance)
                .put_x("/seasons/{id}", http::tasks::update_season)
                .delete_x("/seasons/{id}", http::tasks::destroy_season)
                // entries
                .get_x("/entries", http::entries::list)
                .post_x("/entries", http::entries::create)
                .get_x("/entries/{id}", http::entries::detail)
                .put_x("/entries/{id}", http::entries::update)
                .delete_x("/entries/{id}", http::entries::destroy)
                .post_x("/entries/{id}/save-mix", http::entries::save_mix)
                .post_x("/entries/{id}/photos", http::photos::upload_photos)
                .post_x(
                    "/entries/{id}/photos-json",
                    http::photos::upload_photos_json,
                )
                .delete_x("/photos/{id}", http::photos::destroy_photo)
                // tasks
                .get_x("/tasks", http::tasks::list)
                .post_x("/tasks", http::tasks::create)
                .put_x("/tasks/{id}", http::tasks::update)
                .delete_x("/tasks/{id}", http::tasks::destroy)
                .post_x("/tasks/{id}/complete", http::tasks::complete)
                // mixes and compatibility
                .get_x("/mixes", http::mixes::list_mixes)
                .post_x("/mixes", http::mixes::create_mix)
                .put_x("/mixes/{id}", http::mixes::update_mix)
                .delete_x("/mixes/{id}", http::mixes::destroy_mix)
                .get_x("/compatibility", http::mixes::list_flags)
                .post_x("/compatibility", http::mixes::create_flag)
                .delete_x("/compatibility/{id}", http::mixes::destroy_flag)
                .post_x("/compatibility/check", http::mixes::check_compatibility)
                // reference data and external links
                .get_x("/reference/chemicals", http::photos::reference_chemicals)
                .get_x("/reference/crops", http::photos::reference_crops)
                .get_x(
                    "/reference/fertilisers",
                    http::photos::reference_fertilisers,
                )
                .get_x("/reference/lookup", http::photos::reference_lookup)
                .get_x("/reference/options", http::photos::reference_options)
                .get_x("/links/pubcris", http::photos::pubcris_link)
                .get_x("/links/bom", http::photos::bom_link)
                // export
                .get_x(
                    "/export/{kind}/{id}/preview",
                    http::dashboard::export_preview,
                )
                .get_x("/export-pdf/{kind}/{id}", http::dashboard::export_pdf);
        });

        manager.general(None, |router| {
            // Photo files, then the built SPA for every remaining path.
            router
                .get_x("/uploads/{filename}", http::photos::serve_upload)
                .get_x("/", http::spa::index)
                .get_x("/{*path}", http::spa::fallback);
        });
    }

    async fn shutdown(&mut self, _context: &Context) {
        tracing::info!("paddock manager shutting down");
    }
}

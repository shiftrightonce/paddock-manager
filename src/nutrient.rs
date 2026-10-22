//! Nutrient budget assembly: loads a field's soil tests and planting
//! fertiliser, then defers to the pure calculation in `domain`.

use crate::db::*;
use crate::domain::{self, FertiliserNutrients};
use crate::models::*;
use serde_json::json;

/// Assemble the budget for a field.
///
/// Fertiliser is summed across every planting on the field regardless of date,
/// matching how the budget has always treated it as applied for the current
/// season.
pub async fn budget_for_field(
    db: &Db,
    field_id: i64,
    crop: &str,
    target_yield: Option<f64>,
) -> domain::NutrientBudget {
    let mut tests: Vec<domain::SoilTest> = Vec::new();
    let mut ferts: Vec<domain::PlantedFertiliser> = Vec::new();

    for e in entries_of_field(db, field_id).await {
        let entry_type = col(&e, "type");
        let data = as_value(&e, "data");

        if entry_type == "soil_test" {
            tests.push(domain::SoilTest {
                id: as_i64(&e, "id"),
                date: col(&e, "entry_date"),
                lab: domain::entries::str_of(&data, "lab"),
                ph: domain::entries::num_of(&data, "ph"),
                organic_matter: domain::entries::num_of(&data, "organicMatter"),
                n: domain::entries::num_of(&data, "n"),
                p: domain::entries::num_of(&data, "p"),
                k: domain::entries::num_of(&data, "k"),
                cec: domain::entries::num_of(&data, "cec"),
            });
        }

        if entry_type == "planting" {
            if let Some(list) = data["fertilisers"].as_array() {
                for f in list {
                    ferts.push(domain::PlantedFertiliser {
                        product: domain::entries::str_of(f, "product"),
                        rate: domain::entries::num_of(f, "rate"),
                        rate_unit: {
                            let u = domain::entries::str_of(f, "rateUnit");
                            if u.is_empty() { "kg/ha".to_string() } else { u }
                        },
                        n: domain::entries::num_of(f, "n"),
                        p: domain::entries::num_of(f, "p"),
                        k: domain::entries::num_of(f, "k"),
                    });
                }
            }
        }
    }

    domain::nutrient_budget(crop, target_yield, &tests, &ferts)
}

/// N/P/K contributed by all of a field's planting fertiliser.
pub async fn field_fertiliser_totals(
    db: &Db,
    field_id: i64,
) -> (FertiliserNutrients, Vec<serde_json::Value>) {
    let mut total = FertiliserNutrients::default();
    let mut lines = Vec::new();

    for e in entries_of_field(db, field_id).await {
        if col(&e, "type") != "planting" {
            continue;
        }
        let Some(list) = as_value(&e, "data")["fertilisers"].as_array().cloned() else {
            continue;
        };
        for f in &list {
            let contribution = domain::fertiliser_nutrients(f);
            if contribution.known {
                total.known = true;
                total.n += contribution.n;
                total.p += contribution.p;
                total.k += contribution.k;
            }
            lines.push(json!({
                "entryId": as_i64(&e, "id"),
                "date": col(&e, "entry_date"),
                "product": domain::entries::str_of(f, "product"),
                "rate": domain::entries::num_of(f, "rate"),
                "rateUnit": domain::entries::str_of(f, "rateUnit"),
                "known": contribution.known,
                "n": contribution.n,
                "p": contribution.p,
                "k": contribution.k,
            }));
        }
    }

    (total, lines)
}

/// Whether a field has any planting entries at all.
pub async fn has_planting(db: &Db, field_id: i64) -> bool {
    entries_of_field(db, field_id)
        .await
        .iter()
        .any(|e| col(e, "type") == "planting")
}

/// Table name reference so the module stays tied to the models.
pub fn _unused(_: Option<&Farm>) {}

//! Field status, resistance conflicts, water input, nutrient budget and the
//! cross-farm attention list. All of these are pure functions over already
//! loaded rows so they can be unit tested without a database.

pub mod dates;
pub mod entries;

use chrono::NaiveDate;
use serde_json::{Value, json};

use self::entries::{num_of, str_of};
use crate::reference;

/// How long a residual herbicide keeps a field amber.
pub const RESIDUAL_WINDOW_DAYS: i64 = 90;

/// One product from a chemical application or a saved mix.
#[derive(Clone, Debug)]
pub struct SprayProduct {
    pub product: String,
    pub active: String,
    pub moa: String,
    pub residual: bool,
}

impl SprayProduct {
    pub fn from_value(p: &Value) -> Self {
        Self {
            product: str_of(p, "product"),
            active: str_of(p, "active"),
            moa: str_of(p, "moa"),
            residual: entries::truthy(p.get("residual")),
        }
    }

    /// Tolerates the bare-string form the compatibility check endpoint accepts.
    pub fn from_input(p: &Value) -> Self {
        if let Some(s) = p.as_str() {
            return Self {
                product: s.trim().to_string(),
                active: String::new(),
                moa: String::new(),
                residual: false,
            };
        }
        Self::from_value(p)
    }
}

#[derive(Clone, Debug)]
pub struct ResistanceRow {
    pub id: i64,
    pub weed_species: String,
    pub product: String,
    pub active: String,
    pub moa_group: String,
    pub source: String,
    pub confirmed_on: Option<String>,
}

impl ResistanceRow {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "weed_species": self.weed_species,
            "product": self.product,
            "active": self.active,
            "moa_group": self.moa_group,
            "source": self.source,
            "confirmed_on": self.confirmed_on,
        })
    }
}

/// A chemical application reduced to what the domain rules need.
#[derive(Clone, Debug)]
pub struct SprayEvent {
    pub id: i64,
    pub date: String,
    pub products: Vec<SprayProduct>,
}

impl SprayEvent {
    pub fn residual_products(&self) -> Vec<&SprayProduct> {
        self.products.iter().filter(|p| p.residual).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Red,
    Amber,
    Green,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Red => "red",
            Level::Amber => "amber",
            Level::Green => "green",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Reason {
    pub level: Level,
    pub kind: String,
    pub text: String,
}

impl Reason {
    pub fn to_json(&self) -> Value {
        json!({ "level": self.level.as_str(), "kind": self.kind, "text": self.text })
    }
}

#[derive(Clone, Debug)]
pub struct LastResidual {
    pub entry_id: i64,
    pub date: String,
    pub days_since: Option<i64>,
    pub products: Vec<String>,
    pub residuals: Vec<String>,
}

impl LastResidual {
    pub fn active(&self) -> bool {
        self.days_since
            .map(|d| d >= 0 && d <= RESIDUAL_WINDOW_DAYS)
            .unwrap_or(false)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "entryId": self.entry_id,
            "date": self.date,
            "daysSince": self.days_since,
            "products": self.products,
            "residuals": self.residuals,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FieldStatus {
    pub status: Level,
    pub reasons: Vec<Reason>,
    pub resistance_count: usize,
    pub last_residual: Option<LastResidual>,
    pub active_residual_days: Option<i64>,
}

impl FieldStatus {
    pub fn to_json(&self) -> Value {
        json!({
            "status": self.status.as_str(),
            "statusReasons": self.reasons.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
            "resistanceCount": self.resistance_count,
            "lastResidual": self.last_residual.as_ref().map(|r| r.to_json()),
            "activeResidualDays": self.active_residual_days,
        })
    }
}

/// The most recent spray containing at least one residual-flagged product.
///
/// Sprays dated after `ref` are ignored so a future-dated entry cannot make a
/// field look amber before the product was applied.
pub fn last_residual(sprays: &[SprayEvent], ref_date: NaiveDate) -> Option<LastResidual> {
    sprays
        .iter()
        .filter_map(|s| {
            let d = dates::parse(&s.date)?;
            if d > ref_date {
                return None;
            }
            let residuals = s.residual_products();
            if residuals.is_empty() {
                return None;
            }
            Some(LastResidual {
                entry_id: s.id,
                date: s.date.clone(),
                days_since: Some(dates::days_between(d, ref_date)),
                products: s.products.iter().map(|p| p.product.clone()).collect(),
                residuals: residuals.iter().map(|p| p.product.clone()).collect(),
            })
        })
        .max_by_key(|r| r.date.clone())
}

/// Field status, with red taking priority over amber and amber over green.
///
/// Any resistance record at all makes the field red; a residual inside the
/// window raises it to amber; otherwise it is green.
pub fn field_status(
    resistance: &[ResistanceRow],
    sprays: &[SprayEvent],
    ref_date: NaiveDate,
) -> FieldStatus {
    let mut reasons: Vec<Reason> = Vec::new();
    let mut status = Level::Green;

    if !resistance.is_empty() {
        status = Level::Red;
        let n = resistance.len();
        reasons.push(Reason {
            level: Level::Red,
            kind: "resistance".into(),
            text: format!(
                "{n} weed resistance record{}",
                if n == 1 { "" } else { "s" }
            ),
        });
    }

    let last = last_residual(sprays, ref_date);
    let mut active_residual_days = None;
    if let Some(lr) = &last {
        if lr.active() {
            if status == Level::Green {
                status = Level::Amber;
            }
            let days = lr.days_since.unwrap_or(0);
            active_residual_days = Some(days);
            reasons.push(Reason {
                level: Level::Amber,
                kind: "residual".into(),
                text: format!("Residual {} applied {days}d ago", lr.residuals.join(", ")),
            });
        }
    }

    if reasons.is_empty() {
        reasons.push(Reason {
            level: Level::Green,
            kind: "clear".into(),
            text: "No flags".into(),
        });
    }

    FieldStatus {
        status,
        reasons,
        resistance_count: resistance.len(),
        last_residual: last,
        active_residual_days,
    }
}

/// Split a MOA string into comparable tokens.
///
/// Drops "group"/"moa" words and single letters so "Group 27" and "27" match.
fn moa_tokens(moa: &str) -> Vec<String> {
    // Group codes are single digits as often as they are two ("9", "27"), so
    // every alphanumeric token is kept and only the filler words are dropped.
    moa.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && *t != "group" && *t != "moa")
        .map(|t| t.to_string())
        .collect()
}

/// Split an active-ingredient string, keeping digits (group numbers live here).
fn active_tokens(active: &str) -> Vec<String> {
    active
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// Compare two active-ingredient tokens.
///
/// Equal, or one is a prefix of the other provided the shorter is at least 5
/// characters. The length floor is what keeps "amine" from matching
/// "glufosinate" and "4" from matching "24".
fn actives_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 5 && long.starts_with(short)
}

#[derive(Clone, Debug)]
pub struct Match {
    pub record: ResistanceRow,
    pub matched_on: Vec<String>,
}

impl Match {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.record.id,
            "weedSpecies": self.record.weed_species,
            "moaGroup": self.record.moa_group,
            "active": self.record.active,
            "product": self.record.product,
            "source": self.record.source,
            "confirmedOn": self.record.confirmed_on,
            "matchedOn": self.matched_on,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ProductConflicts {
    pub product: String,
    pub moa: String,
    pub active: String,
    pub conflict: bool,
    pub matches: Vec<Match>,
}

impl ProductConflicts {
    pub fn to_json(&self) -> Value {
        json!({
            "product": self.product,
            "moa": self.moa,
            "active": self.active,
            "conflict": self.conflict,
            "matches": self.matches.iter().map(|m| m.to_json()).collect::<Vec<_>>(),
        })
    }
}

/// Advisory resistance check for a set of products against a field's records.
///
/// A match can come from any of three axes and one record may match on several;
/// `matchedOn` lists them in MOA, active, product order. Never blocks a save.
pub fn check_conflicts(
    products: &[SprayProduct],
    records: &[ResistanceRow],
) -> Vec<ProductConflicts> {
    products
        .iter()
        .map(|p| {
            let p_moa = moa_tokens(&p.moa);
            let p_active = active_tokens(&p.active);

            let mut matches = Vec::new();
            for record in records {
                let mut matched_on = Vec::new();

                let r_moa = moa_tokens(&record.moa_group);
                if !p_moa.is_empty() && !r_moa.is_empty() && p_moa.iter().any(|t| r_moa.contains(t))
                {
                    matched_on.push("moa".to_string());
                }

                let r_active = active_tokens(&record.active);
                if p_active
                    .iter()
                    .any(|pa| r_active.iter().any(|ra| actives_match(pa, ra)))
                {
                    matched_on.push("active".to_string());
                }

                // Product-name matching only applies when the operator gave no
                // active ingredient, otherwise the active match is broader.
                if matched_on.is_empty()
                    && p_active.is_empty()
                    && !p.product.is_empty()
                    && p.product.to_lowercase() == record.product.to_lowercase()
                {
                    matched_on.push("product".to_string());
                }

                if !matched_on.is_empty() {
                    matches.push(Match {
                        record: record.clone(),
                        matched_on: matched_on,
                    });
                }
            }

            ProductConflicts {
                product: p.product.clone(),
                moa: p.moa.clone(),
                active: p.active.clone(),
                conflict: !matches.is_empty(),
                matches,
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct WaterEvent {
    pub id: i64,
    pub date: String,
    pub rainfall_mm: f64,
    pub irrigation_mm: f64,
    pub method: String,
}

#[derive(Clone, Debug)]
pub struct WaterWindow {
    pub from: String,
    pub to: String,
    pub days: i64,
    pub rainfall_mm: f64,
    pub irrigation_mm: f64,
    pub total_mm: f64,
    pub events: Vec<WaterEvent>,
}

impl WaterWindow {
    pub fn to_json(&self) -> Value {
        json!({
            "from": self.from,
            "to": self.to,
            "days": self.days,
            "rainfallMm": round1(self.rainfall_mm),
            "irrigationMm": round1(self.irrigation_mm),
            "totalMm": round1(self.total_mm),
            "events": self.events.iter().map(|e| json!({
                "id": e.id,
                "date": e.date,
                "rainfallMm": round1(e.rainfall_mm),
                "irrigationMm": round1(e.irrigation_mm),
                "method": e.method,
            })).collect::<Vec<_>>(),
        })
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Total water input over an inclusive `days`-day window ending on `ref`.
pub fn water_input(events: &[WaterEvent], days: i64, ref_date: NaiveDate) -> WaterWindow {
    let from = dates::window_start(ref_date, days);
    let mut rainfall = 0.0;
    let mut irrigation = 0.0;
    let mut kept = Vec::new();

    for e in events {
        let Some(d) = dates::parse(&e.date) else {
            continue;
        };
        if d < from || d > ref_date {
            continue;
        }
        rainfall += e.rainfall_mm;
        irrigation += e.irrigation_mm;
        kept.push(e.clone());
    }
    kept.sort_by(|a, b| b.date.cmp(&a.date));

    WaterWindow {
        from: from.format(dates::ISO).to_string(),
        to: ref_date.format(dates::ISO).to_string(),
        days,
        rainfall_mm: rainfall,
        irrigation_mm: irrigation,
        total_mm: rainfall + irrigation,
        events: kept,
    }
}

#[derive(Clone, Debug)]
pub struct MoaUsage {
    pub group: String,
    pub applications: i64,
    pub products: Vec<String>,
    pub last_used: Option<String>,
    pub resistance: bool,
}

impl MoaUsage {
    pub fn to_json(&self) -> Value {
        json!({
            "group": self.group,
            "applications": self.applications,
            "products": self.products,
            "lastUsed": self.last_used,
            "resistance": self.resistance,
        })
    }
}

/// MOA group usage over the trailing `months` window.
///
/// `applications` counts once per spray per group, so a tank mix containing two
/// products from the same group is a single application. Groups that appear in
/// a resistance record on the field are flagged so the rotation planner can
/// highlight them.
pub fn moa_history(
    sprays: &[SprayEvent],
    records: &[ResistanceRow],
    months: i64,
    ref_date: NaiveDate,
) -> Vec<MoaUsage> {
    let from = dates::months_ago(ref_date, months);

    let resistant_groups: Vec<String> = records
        .iter()
        .flat_map(|r| moa_tokens(&r.moa_group))
        .collect();

    let mut order: Vec<String> = Vec::new();
    let mut applications: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let mut products: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut last_used: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for spray in sprays {
        let Some(d) = dates::parse(&spray.date) else {
            continue;
        };
        if d < from || d > ref_date {
            continue;
        }
        let mut seen: Vec<String> = Vec::new();
        for p in &spray.products {
            for token in moa_tokens(&p.moa) {
                if !applications.contains_key(&token) {
                    order.push(token.clone());
                    applications.insert(token.clone(), 0);
                    products.insert(token.clone(), Vec::new());
                }
                if !seen.contains(&token) {
                    seen.push(token.clone());
                    *applications.get_mut(&token).unwrap() += 1;
                }
                let list = products.get_mut(&token).unwrap();
                if !p.product.is_empty() && !list.contains(&p.product) {
                    list.push(p.product.clone());
                }
                let entry = last_used.entry(token.clone()).or_default();
                if spray.date > *entry {
                    *entry = spray.date.clone();
                }
            }
        }
    }

    let mut out: Vec<MoaUsage> = order
        .into_iter()
        .map(|group| MoaUsage {
            applications: applications[&group],
            resistance: resistant_groups.contains(&group),
            products: products.remove(&group).unwrap_or_default(),
            last_used: last_used.get(&group).filter(|s| !s.is_empty()).cloned(),
            group,
        })
        .collect();

    // Most used first, then alphabetical so equal counts are stable.
    out.sort_by(|a, b| {
        b.applications
            .cmp(&a.applications)
            .then_with(|| a.group.cmp(&b.group))
    });
    out
}

/// N/P/K contributed by one fertiliser line.
///
/// Prefers the operator's inline analysis; falls back to the built-in label
/// table. Returns `known: false` when neither identifies the product, so the
/// budget can tell "contributed nothing" from "unknown contribution".
#[derive(Clone, Debug, Default)]
pub struct FertiliserNutrients {
    pub n: f64,
    pub p: f64,
    pub k: f64,
    pub known: bool,
    pub analysis: Option<String>,
}

pub fn fertiliser_nutrients(f: &Value) -> FertiliserNutrients {
    let product = str_of(f, "product");
    let rate = num_of(f, "rate").unwrap_or(0.0);
    let unit = {
        let u = str_of(f, "rateUnit");
        if u.is_empty() { "kg/ha".to_string() } else { u }
    };

    let n_in = num_of(f, "n");
    let p_in = num_of(f, "p");
    let k_in = num_of(f, "k");

    // Inline analysis present when N is given and at least one of P/K is.
    let inline = n_in.is_some() && (p_in.is_some() || k_in.is_some());

    let (n_pct, p_pct, k_pct): (Option<f64>, Option<f64>, Option<f64>) = if inline {
        (
            n_in,
            // The UI labels these P and K but sends P2O5 and K2O percentages.
            p_in.map(|p| p / reference::P2O5_TO_P),
            k_in.map(|k| k / reference::K2O_TO_K),
        )
    } else {
        match reference::find_fertiliser(&product) {
            Some(fer) => (Some(fer.n), Some(fer.p2o5), Some(fer.k2o)),
            None => return FertiliserNutrients::default(),
        }
    };

    // Litres are treated as kilograms; tonnes scale by 1000.
    let kg = match unit.as_str() {
        "t/ha" => rate * 1000.0,
        "kg/ha" | "L/ha" => rate,
        _ => rate,
    };

    let pct = |v: Option<f64>| v.unwrap_or(0.0) / 100.0;
    FertiliserNutrients {
        n: pct(n_pct) * kg,
        p: pct(p_pct) * reference::P2O5_TO_P * kg,
        k: pct(k_pct) * reference::K2O_TO_K * kg,
        known: true,
        analysis: (!product.is_empty()).then_some(product),
    }
}

#[derive(Clone, Debug)]
pub struct SoilTest {
    pub id: i64,
    pub date: String,
    pub lab: String,
    pub ph: Option<f64>,
    pub organic_matter: Option<f64>,
    pub n: Option<f64>,
    pub p: Option<f64>,
    pub k: Option<f64>,
    pub cec: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct PlantedFertiliser {
    pub product: String,
    pub rate: Option<f64>,
    pub rate_unit: String,
    pub n: Option<f64>,
    pub p: Option<f64>,
    pub k: Option<f64>,
}

impl PlantedFertiliser {
    pub fn to_value(&self) -> Value {
        json!({
            "product": self.product,
            "rate": self.rate,
            "rateUnit": self.rate_unit,
            "n": self.n,
            "p": self.p,
            "k": self.k,
        })
    }
}

#[derive(Clone, Debug)]
pub struct NutrientBudget {
    pub crop: String,
    pub crop_known: bool,
    pub target_yield: Option<f64>,
    pub soil_test: Option<SoilTest>,
    pub soil_history: Vec<SoilTest>,
    pub fertiliser_n: f64,
    pub fertiliser_p: f64,
    pub fertiliser_k: f64,
    pub fertiliser_known: bool,
    pub lines: Vec<Value>,
}

impl NutrientBudget {
    pub fn to_json(&self) -> Value {
        let crop = reference::find_crop(&self.crop);
        let required = |uptake: f64| self.target_yield.map(|y| uptake * y);

        let build = |uptake: f64, available_soil: Option<f64>, applied: f64| {
            let required = required(uptake);
            let available = available_soil.unwrap_or(0.0) + applied;
            json!({
                "uptakePerTonne": uptake,
                "required": required.map(round1),
                "available": round1(available),
                "balance": required.map(|r| round1(r - available)),
            })
        };

        let (n_uptake, p_uptake, k_uptake) = match crop {
            Some(c) => (c.n, c.p, c.k),
            None => (0.0, 0.0, 0.0),
        };

        json!({
            "crop": self.crop,
            "cropKnown": self.crop_known,
            "targetYield": self.target_yield.map(round1),
            "soilTest": self.soil_test.as_ref().map(|t| soil_json(t)),
            "soilTestHistory": self.soil_history.iter().map(soil_json).collect::<Vec<_>>(),
            "fertiliser": {
                "n": round1(self.fertiliser_n),
                "p": round1(self.fertiliser_p),
                "k": round1(self.fertiliser_k),
                "known": self.fertiliser_known,
                "lines": self.lines,
            },
            "nutrients": {
                "n": build(n_uptake, self.soil_test.as_ref().and_then(|t| t.n), self.fertiliser_n),
                "p": build(p_uptake, self.soil_test.as_ref().and_then(|t| t.p), self.fertiliser_p),
                "k": build(k_uptake, self.soil_test.as_ref().and_then(|t| t.k), self.fertiliser_k),
            },
        })
    }
}

fn soil_json(t: &SoilTest) -> Value {
    json!({
        "id": t.id,
        "date": t.date,
        "lab": t.lab,
        "ph": t.ph,
        "organicMatter": t.organic_matter,
        "n": t.n,
        "p": t.p,
        "k": t.k,
        "cec": t.cec,
    })
}

/// Compare the latest soil test plus logged planting fertiliser against crop
/// uptake for the target yield.
///
/// Required is uptake per tonne times target yield; the balance is what is
/// still short once soil supply and applied fertiliser are counted.
pub fn nutrient_budget(
    crop: &str,
    target_yield: Option<f64>,
    soil_tests: &[SoilTest],
    planting_fertilisers: &[PlantedFertiliser],
) -> NutrientBudget {
    let crop_ref = reference::find_crop(crop);
    let target_yield = target_yield
        .or_else(|| crop_ref.map(|c| c.typical_yield))
        .filter(|v| v.is_finite() && *v > 0.0);

    let mut sorted = soil_tests.to_vec();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));

    let mut n = 0.0;
    let mut p = 0.0;
    let mut k = 0.0;
    let mut known = false;
    let mut lines = Vec::new();

    for f in planting_fertilisers {
        let value = f.to_value();
        let contribution = fertiliser_nutrients(&value);
        if contribution.known {
            known = true;
            n += contribution.n;
            p += contribution.p;
            k += contribution.k;
        }
        lines.push(json!({
            "product": f.product,
            "rate": f.rate,
            "rateUnit": f.rate_unit,
            "known": contribution.known,
            "n": round1(contribution.n),
            "p": round1(contribution.p),
            "k": round1(contribution.k),
        }));
    }

    NutrientBudget {
        crop: crop.to_string(),
        crop_known: crop_ref.is_some(),
        target_yield,
        soil_test: sorted.first().cloned(),
        soil_history: sorted,
        fertiliser_n: n,
        fertiliser_p: p,
        fertiliser_k: k,
        fertiliser_known: known,
        lines,
    }
}

/// An item on the needs-attention list.
#[derive(Clone, Debug)]
pub struct Attention {
    pub level: Level,
    pub kind: String,
    pub farm_id: Option<i64>,
    pub farm_name: String,
    pub field_id: Option<i64>,
    pub field_name: String,
    pub date: String,
    pub text: String,
    pub task_id: Option<i64>,
}

impl Attention {
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "level": self.level.as_str(),
            "kind": self.kind,
            "farmId": self.farm_id,
            "farmName": self.farm_name,
            "fieldId": self.field_id,
            "fieldName": self.field_name,
            "date": self.date,
            "text": self.text,
        });
        if let Some(t) = self.task_id {
            v["taskId"] = json!(t);
        }
        v
    }
}

/// One attention item per resistance record, not per field, so a field with
/// three records makes three visible items.
pub fn attention_item_for_record(
    farm_id: i64,
    farm_name: &str,
    field_id: i64,
    field_name: &str,
    record: &ResistanceRow,
) -> Attention {
    let mut detail: Vec<String> = Vec::new();
    if !record.product.is_empty() {
        detail.push(record.product.clone());
    }
    if !record.moa_group.is_empty() {
        detail.push(format!("Group {}", record.moa_group));
    }
    if !record.active.is_empty() {
        detail.push(record.active.clone());
    }
    // Wording is part of the contract: the dashboard shows this text verbatim.
    let species = if record.weed_species.is_empty() {
        "Weed"
    } else {
        record.weed_species.as_str()
    };
    let text = format!("{species} resistance to {}", detail.join(" / "));

    Attention {
        level: Level::Red,
        kind: "resistance".into(),
        farm_id: Some(farm_id),
        farm_name: farm_name.to_string(),
        field_id: Some(field_id),
        field_name: field_name.to_string(),
        date: record.confirmed_on.clone().unwrap_or_default(),
        text,
        task_id: None,
    }
}

pub fn attention_item_for_residual(
    farm_id: i64,
    farm_name: &str,
    field_id: i64,
    field_name: &str,
    last: &LastResidual,
) -> Attention {
    Attention {
        level: Level::Amber,
        kind: "residual".into(),
        farm_id: Some(farm_id),
        farm_name: farm_name.to_string(),
        field_id: Some(field_id),
        field_name: field_name.to_string(),
        date: last.date.clone(),
        text: format!(
            "Residual {} applied {}d ago ({RESIDUAL_WINDOW_DAYS}d window)",
            last.residuals.join(", "),
            last.days_since.unwrap_or(0)
        ),
        task_id: None,
    }
}

/// Overdue tasks are red, tasks due today are amber.
pub fn attention_item_for_task(
    farm_id: Option<i64>,
    farm_name: &str,
    field_id: Option<i64>,
    field_name: &str,
    task_id: i64,
    title: &str,
    due_date: &str,
    ref_date: NaiveDate,
) -> Attention {
    let due = dates::parse(due_date).unwrap_or(ref_date);
    let overdue = due < ref_date;
    Attention {
        level: if overdue { Level::Red } else { Level::Amber },
        kind: "task".into(),
        farm_id,
        farm_name: farm_name.to_string(),
        field_id,
        field_name: field_name.to_string(),
        date: due_date.to_string(),
        text: if overdue {
            format!("{title} - OVERDUE")
        } else {
            format!("{title} - due today")
        },
        task_id: Some(task_id),
    }
}

/// Sort red before amber before green, then chronologically by date.
pub fn sort_attention(items: &mut [Attention]) {
    items.sort_by(|a, b| a.level.cmp(&b.level).then_with(|| a.date.cmp(&b.date)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn res(id: i64, species: &str, moa: &str, active: &str, product: &str) -> ResistanceRow {
        ResistanceRow {
            id,
            weed_species: species.into(),
            product: product.into(),
            active: active.into(),
            moa_group: moa.into(),
            source: "test".into(),
            confirmed_on: None,
        }
    }

    fn spray(id: i64, date: &str, products: Vec<SprayProduct>) -> SprayEvent {
        SprayEvent {
            id,
            date: date.into(),
            products,
        }
    }

    fn prod(name: &str, moa: &str, active: &str, residual: bool) -> SprayProduct {
        SprayProduct {
            product: name.into(),
            active: active.into(),
            moa: moa.into(),
            residual,
        }
    }

    fn d(s: &str) -> NaiveDate {
        dates::parse(s).unwrap()
    }

    #[test]
    fn no_resistance_and_recent_activity_is_green() {
        let s = field_status(
            &[],
            &[spray(
                1,
                "2026-03-01",
                vec![prod("Roundup", "9", "glyphosate", false)],
            )],
            d("2026-03-07"),
        );
        assert_eq!(s.status, Level::Green);
        assert_eq!(s.reasons[0].kind, "clear");
    }

    #[test]
    fn any_resistance_record_is_red_and_pluralises() {
        let one = field_status(
            &[res(1, "Ryegrass", "9", "glyphosate", "Roundup")],
            &[],
            d("2026-03-07"),
        );
        assert_eq!(one.status, Level::Red);
        assert_eq!(one.reasons[0].text, "1 weed resistance record");
        assert_eq!(one.resistance_count, 1);

        let two = field_status(
            &[res(1, "a", "", "", ""), res(2, "b", "", "", "")],
            &[],
            d("2026-03-07"),
        );
        assert_eq!(two.reasons[0].text, "2 weed resistance records");
    }

    #[test]
    fn red_outranks_amber() {
        let sprays = vec![spray(
            1,
            "2026-03-01",
            vec![prod("Atrazine", "5", "atrazine", true)],
        )];
        let both = field_status(&[res(1, "a", "5", "", "")], &sprays, d("2026-03-07"));
        assert_eq!(both.status, Level::Red, "red wins over amber");
        assert!(both.reasons.iter().any(|r| r.level == Level::Amber));
    }

    #[test]
    fn residual_is_amber_inside_the_90_day_window() {
        let sprays = vec![spray(
            1,
            "2026-03-01",
            vec![prod("Atrazine", "5", "atrazine", true)],
        )];

        let inside = field_status(&[], &sprays, d("2026-04-15"));
        assert_eq!(inside.status, Level::Amber);
        assert_eq!(inside.active_residual_days, Some(45));

        let edge = field_status(&[], &sprays, d("2026-05-30"));
        assert_eq!(
            edge.status,
            Level::Amber,
            "day 90 is still inside the window"
        );

        let outside = field_status(&[], &sprays, d("2026-06-01"));
        assert_eq!(outside.status, Level::Green, "day 91 has cleared");
        assert!(
            outside.last_residual.is_some(),
            "still reported, just inactive"
        );
    }

    #[test]
    fn future_dated_sprays_do_not_go_amber() {
        let sprays = vec![spray(
            1,
            "2026-12-01",
            vec![prod("Atrazine", "5", "atrazine", true)],
        )];
        let s = field_status(&[], &sprays, d("2026-03-07"));
        assert_eq!(s.status, Level::Green);
        assert!(s.last_residual.is_none());
    }

    #[test]
    fn conflict_matches_moa_ignoring_group_words() {
        let records = vec![res(1, "Ryegrass", "27", "", "")];
        let out = check_conflicts(&[prod("Callisto", "Group 27", "", false)], &records);
        assert!(out[0].conflict);
        assert_eq!(out[0].matches[0].matched_on, vec!["moa"]);
    }

    #[test]
    fn conflict_requires_token_length_floor_on_actives() {
        let records = vec![res(1, "Ryegrass", "", "2,4-D amine", "")];
        // "amine" is 5 chars so it matches the recorded active.
        let hit = check_conflicts(&[prod("Amicide", "4", "2,4-D amine", false)], &records);
        assert!(hit[0].conflict);
        assert_eq!(hit[0].matches[0].matched_on, vec!["active"]);

        // "amine" vs "glufosinate" must not match.
        let miss = check_conflicts(&[prod("Basta", "10", "glufosinate", false)], &records);
        assert!(!miss[0].conflict);
    }

    #[test]
    fn product_name_only_matches_when_no_active_is_given() {
        let records = vec![res(1, "Ryegrass", "", "", "Roundup")];
        let bare = check_conflicts(&[prod("roundup", "", "", false)], &records);
        assert!(bare[0].conflict);
        assert_eq!(bare[0].matches[0].matched_on, vec!["product"]);

        let with_active = check_conflicts(&[prod("Roundup", "", "glyphosate", false)], &records);
        assert!(
            !with_active[0].conflict,
            "an explicit active ingredient replaces product-name matching"
        );
    }

    #[test]
    fn one_record_can_match_on_several_axes() {
        let records = vec![res(1, "Ryegrass", "9", "glyphosate", "Roundup")];
        let out = check_conflicts(&[prod("Roundup", "9", "glyphosate", false)], &records);
        assert_eq!(out[0].matches[0].matched_on, vec!["moa", "active"]);
    }

    #[test]
    fn water_window_is_inclusive_of_both_ends() {
        let events = vec![
            WaterEvent {
                id: 1,
                date: "2026-03-01".into(),
                rainfall_mm: 10.0,
                irrigation_mm: 0.0,
                method: "none".into(),
            },
            WaterEvent {
                id: 2,
                date: "2026-03-07".into(),
                rainfall_mm: 5.0,
                irrigation_mm: 2.0,
                method: "pivot".into(),
            },
            WaterEvent {
                id: 3,
                date: "2026-03-08".into(),
                rainfall_mm: 99.0,
                irrigation_mm: 0.0,
                method: "none".into(),
            },
            WaterEvent {
                id: 4,
                date: "2026-02-28".into(),
                rainfall_mm: 50.0,
                irrigation_mm: 0.0,
                method: "none".into(),
            },
        ];
        let w = water_input(&events, 7, d("2026-03-07"));
        assert_eq!(w.rainfall_mm, 15.0);
        assert_eq!(w.irrigation_mm, 2.0);
        assert_eq!(w.total_mm, 17.0);
        assert_eq!(w.events.len(), 2, "outside the window is excluded");
    }

    #[test]
    fn moa_history_counts_one_application_per_spray_per_group() {
        let sprays = vec![
            spray(
                1,
                "2026-03-01",
                vec![
                    prod("A", "9", "glyphosate", false),
                    prod("B", "9", "other", false),
                ],
            ),
            spray(2, "2026-02-01", vec![prod("C", "5", "atrazine", false)]),
        ];
        let hist = moa_history(&sprays, &[], 12, d("2026-03-07"));
        // Equal application counts fall back to a lexicographic group sort, the
        // same as the `localeCompare` the previous backend used.
        assert_eq!(hist[0].group, "5");
        assert_eq!(hist[1].group, "9");
        assert_eq!(
            hist[1].applications, 1,
            "two products in one group, one spray"
        );
        assert_eq!(hist[1].products.len(), 2, "both products still listed");
        assert_eq!(hist[0].applications, 1);
        assert_eq!(hist[0].products, vec!["C".to_string()]);
    }

    #[test]
    fn moa_history_flags_resistant_groups() {
        let sprays = vec![spray(1, "2026-03-01", vec![prod("A", "27", "", false)])];
        let hist = moa_history(
            &sprays,
            &[res(1, "Ryegrass", "27", "", "")],
            12,
            d("2026-03-07"),
        );
        assert!(hist[0].resistance);
    }

    #[test]
    fn moa_history_respects_the_month_window() {
        let sprays = vec![spray(1, "2025-01-01", vec![prod("A", "9", "", false)])];
        assert!(moa_history(&sprays, &[], 12, d("2026-03-07")).is_empty());
    }

    #[test]
    fn fertiliser_converts_p2o5_and_k2o_to_elemental() {
        // 100 kg/ha Urea at 46% N.
        let urea =
            fertiliser_nutrients(&json!({ "product": "Urea", "rate": 100, "rateUnit": "kg/ha" }));
        assert!((urea.n - 46.0).abs() < 0.001);
        assert!(urea.known);

        // 100 kg/ha muriate of potash: 60% K2O -> elemental K.
        let mop = fertiliser_nutrients(&json!({ "product": "Muriate of potash", "rate": 100 }));
        assert!((mop.k - 60.0 * reference::K2O_TO_K).abs() < 0.001);

        // t/ha scales by 1000.
        let lime =
            fertiliser_nutrients(&json!({ "product": "Lime", "rate": 2, "rateUnit": "t/ha" }));
        assert_eq!(lime.n, 0.0);
        assert!(lime.known);
    }

    #[test]
    fn unknown_fertiliser_is_reported_as_unknown_not_zero() {
        let out = fertiliser_nutrients(&json!({ "product": "Mystery Powder", "rate": 50 }));
        assert!(!out.known);
        assert_eq!(out.n, 0.0);
    }

    #[test]
    fn nutrient_budget_compares_uptake_against_soil_plus_fertiliser() {
        let soil = vec![SoilTest {
            id: 1,
            date: "2026-01-01".into(),
            lab: "Lab".into(),
            ph: Some(6.5),
            organic_matter: Some(2.0),
            n: Some(20.0),
            p: Some(5.0),
            k: Some(30.0),
            cec: None,
        }];
        let ferts = vec![PlantedFertiliser {
            product: "Urea".into(),
            rate: Some(50.0),
            rate_unit: "kg/ha".into(),
            n: None,
            p: None,
            k: None,
        }];

        let json = nutrient_budget("Wheat", Some(3.0), &soil, &ferts).to_json();
        // 20 kg N/t * 3 t = 60 required; 20 from soil + 23 from urea = 43 available.
        let n = &json["nutrients"]["n"];
        assert_eq!(n["required"], 60.0);
        assert_eq!(n["available"], 43.0);
        assert_eq!(n["balance"], 17.0);
        assert!(json["fertiliser"]["known"].as_bool().unwrap());
    }

    #[test]
    fn nutrient_budget_targets_default_to_the_crop_typical_yield() {
        let json = nutrient_budget("Canola", None, &[], &[]).to_json();
        assert_eq!(json["targetYield"], 2.5);
        assert_eq!(json["nutrients"]["n"]["required"], 100.0, "40 kg/t * 2.5 t");
    }

    #[test]
    fn nutrient_budget_without_a_crop_leaves_requirement_unset() {
        let json = nutrient_budget("", None, &[], &[]).to_json();
        assert_eq!(json["cropKnown"], false);
        assert_eq!(json["nutrients"]["n"]["required"], Value::Null);
        assert_eq!(json["nutrients"]["n"]["balance"], Value::Null);
    }

    #[test]
    fn attention_sorts_red_before_amber_then_by_date() {
        let mut items = vec![
            attention_item_for_task(
                None,
                "F",
                None,
                "",
                1,
                "Late",
                "2026-03-05",
                d("2026-03-07"),
            ),
            attention_item_for_residual(
                1,
                "F",
                1,
                "N",
                &LastResidual {
                    entry_id: 1,
                    date: "2026-03-01".into(),
                    days_since: Some(6),
                    products: vec![],
                    residuals: vec!["Atrazine".into()],
                },
            ),
            attention_item_for_task(
                None,
                "F",
                None,
                "",
                2,
                "Today",
                "2026-03-07",
                d("2026-03-07"),
            ),
            attention_item_for_record(1, "F", 1, "N", &res(1, "Ryegrass", "9", "", "Roundup")),
        ];
        sort_attention(&mut items);
        assert_eq!(items[0].level, Level::Red);
        assert_eq!(items[0].kind, "resistance");
        assert_eq!(items[1].level, Level::Red);
        assert_eq!(items[1].kind, "task", "overdue task is red");
        assert_eq!(items[2].level, Level::Amber);
        assert_eq!(items[3].level, Level::Amber);
    }

    #[test]
    fn attention_text_names_the_group_and_active() {
        let item = attention_item_for_record(
            1,
            "Farm",
            1,
            "North",
            &res(1, "Ryegrass", "27", "mesotrione", "Callisto"),
        );
        assert_eq!(item.text, "Ryegrass resistance to Callisto / Group 27 / mesotrione");
    }
}

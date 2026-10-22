//! Cross-farm dashboard and export report assembly.

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use dirtybase_contract::app_contract::CtxExt;
use serde_json::{Value, json};

use crate::db::*;
use crate::domain;
use crate::http::{ApiError, ApiResult, Query, attachment};
use crate::models::*;

/// Filename-safe form of a farm or field name.
pub fn safe_name(input: &str) -> String {
    let lowered = input.to_lowercase();
    let mut out = String::new();
    let mut pending_dash = false;
    for c in lowered.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        "report".to_string()
    } else {
        out
    }
}

/// `GET /api/dashboard?farmId=`
///
/// Everything the dashboard needs in one request: the farm list, the selected
/// farm roll-up, the cross-farm attention list and the badge count.
pub async fn dashboard(CtxExt(db): CtxExt<Db>, q: Query) -> ApiResult<Response> {
    let ref_date = domain::dates::today();

    let farm_rows = farms(&db).await;
    let mut farm_list = Vec::with_capacity(farm_rows.len());

    let mut attention: Vec<domain::Attention> = Vec::new();

    for f in &farm_rows {
        let id = as_i64(f, "id");
        let name = col(f, "name");
        farm_list.push(farm_json(f));
        attention.extend(crate::http::farms::farm_attention(&db, id, &name, ref_date).await);
    }

    // Tasks are farm-wide and may be linked to a field, which resolves the farm.
    for t in tasks_with_names(&db, None, None, false, None).await {
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

    let current_farm = match q.i64("farmId") {
        Some(fid) => crate::http::farms::farm_rollup(&db, fid, ref_date).await,
        None => None,
    };

    let upcoming: Vec<Value> = tasks_with_names(&db, None, None, false, Some(25))
        .await
        .iter()
        .map(|t| t.to_json(ref_date))
        .collect();

    Ok(axum::Json(json!({
        "today": ref_date.format(domain::dates::ISO).to_string(),
        "farms": farm_list,
        "currentFarm": current_farm,
        "attention": attention.iter().map(|a| a.to_json()).collect::<Vec<_>>(),
        "badge": attention.len(),
        "upcoming": upcoming,
    }))
    .into_response())
}

/// Report kind parsed from the export path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Farm,
    Field,
}

/// `GET /api/export/{kind}/{id}/preview`
///
/// The preview payload is the same structure the PDF renders from, so what the
/// operator sees on screen matches the document.
pub async fn export_preview(
    CtxExt(db): CtxExt<Db>,
    Path((kind, id)): Path<(String, i64)>,
) -> ApiResult<Response> {
    let ref_date = domain::dates::today();
    let report = build_report(&db, &kind, id, ref_date)
        .await
        .ok_or_else(|| ApiError::not_found("Not found"))?;
    Ok(axum::Json(report.to_json()).into_response())
}

/// `GET /api/export/{kind}/{id}.pdf`
///
/// axum cannot route `/x/{id}.pdf` and `/x/{id}` to different handlers, so the
/// `.pdf` form is matched by this catch-all and dispatched on the extension.
pub async fn export_pdf(
    CtxExt(db): CtxExt<Db>,
    Path((kind, id)): Path<(String, String)>,
) -> ApiResult<Response> {
    let Some(raw) = id.strip_suffix(".pdf") else {
        return Err(ApiError::not_found("Not found"));
    };
    let id: i64 = raw.parse().map_err(|_| ApiError::not_found("Not found"))?;

    let ref_date = domain::dates::today();
    let report = build_report(&db, &kind, id, ref_date)
        .await
        .ok_or_else(|| ApiError::not_found("Not found"))?;

    let bytes = crate::pdf::render(&report).map_err(|e| ApiError::internal(e.to_string()))?;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        attachment(&report.filename()),
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/pdf"),
    );

    Ok((headers, bytes).into_response())
}

/// A report ready to be shown as JSON or rendered to PDF.
#[derive(Clone)]
pub struct Report {
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub generated_at: String,
    pub today: String,
    pub facts: Vec<(String, String)>,
    pub attention: Vec<Value>,
    pub upcoming: Vec<Value>,
    /// (status, name, detail)
    pub fields: Vec<(String, String, String)>,
    pub water: Option<Value>,
    pub residual: Option<Value>,
    pub resistance: Vec<Value>,
    /// (date, title, detail lines, conflicted)
    pub timeline: Vec<(String, String, Vec<String>, bool)>,
    pub filename_stem: String,
}

impl Report {
    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "title": self.title,
            "subtitle": self.subtitle,
            "generatedAt": self.generated_at,
            "today": self.today,
            "facts": self.facts.iter().map(|(k, v)| json!({ "label": k, "value": v }))
                .collect::<Vec<_>>(),
            "attention": self.attention,
            "upcoming": self.upcoming,
            "fields": self.fields.iter().map(|(s, n, d)| json!({
                "status": s, "name": n, "detail": d
            })).collect::<Vec<_>>(),
            "water": self.water,
            "lastResidual": self.residual,
            "resistance": self.resistance,
            "timeline": self.timeline.iter().map(|(d, t, lines, c)| json!({
                "date": d, "title": t, "detail": lines, "conflicted": c
            })).collect::<Vec<_>>(),
        })
    }

    pub fn filename(&self) -> String {
        format!("{}-{}.pdf", self.filename_stem, self.today)
    }
}

async fn build_report(db: &Db, kind: &str, id: i64, ref_date: chrono::NaiveDate) -> Option<Report> {
    match kind {
        "field" => build_field_report(db, id, ref_date).await,
        // Anything unrecognised falls back to the farm shape, matching the
        // original behaviour.
        _ => build_farm_report(db, id, ref_date).await,
    }
}

async fn build_farm_report(db: &Db, farm_id: i64, ref_date: chrono::NaiveDate) -> Option<Report> {
    let rollup = crate::http::farms::farm_rollup(db, farm_id, ref_date).await?;
    let farm = &rollup["farm"];
    let name = farm["name"].as_str().unwrap_or_default().to_string();
    let counts = &rollup["counts"];

    let mut facts = Vec::new();
    if let Some(loc) = farm["location"].as_str().filter(|s| !s.is_empty()) {
        facts.push(("Location".into(), loc.into()));
    }
    if let Some(station) = farm["weather_station"].as_str().filter(|s| !s.is_empty()) {
        facts.push(("Weather station".into(), station.into()));
    }
    facts.push((
        "Fields".into(),
        format!(
            "{} total, {} flagged, {} residual",
            counts["fields"].as_i64().unwrap_or(0),
            counts["red"].as_i64().unwrap_or(0),
            counts["amber"].as_i64().unwrap_or(0),
        ),
    ));

    let fields = rollup["fields"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|f| {
            let status = f["status"].as_str().unwrap_or("green").to_string();
            let field_name = f["name"].as_str().unwrap_or_default().to_string();
            let mut detail = Vec::new();
            if let Some(area) = f["area_ha"].as_f64() {
                detail.push(format!("{area} ha"));
            }
            if let Some(soil) = f["soil_type"].as_str().filter(|s| !s.is_empty()) {
                detail.push(soil.to_string());
            }
            let reasons = f["statusReasons"]
                .as_array()
                .map(|r| {
                    r.iter()
                        .filter_map(|x| x["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            if !reasons.is_empty() {
                detail.push(reasons);
            }
            (status, field_name, detail.join(" - "))
        })
        .collect();

    Some(Report {
        kind: "farm".into(),
        title: name.clone(),
        subtitle: "Farm summary".into(),
        generated_at: chrono::Local::now().to_rfc3339(),
        today: ref_date.format(domain::dates::ISO).to_string(),
        facts,
        attention: rollup["attention"].as_array().cloned().unwrap_or_default(),
        upcoming: rollup["upcoming"].as_array().cloned().unwrap_or_default(),
        fields,
        water: None,
        residual: None,
        resistance: Vec::new(),
        timeline: Vec::new(),
        filename_stem: format!("farm-{}", safe_name(&name)),
    })
}

async fn build_field_report(db: &Db, field_id: i64, ref_date: chrono::NaiveDate) -> Option<Report> {
    let field = find_by_id(db, Field::table_name(), field_id).await?;
    let farm = find_by_id(db, Farm::table_name(), as_i64(&field, "farm_id")).await;

    let field_name = col(&field, "name");
    let farm_name = farm.as_ref().map(|f| col(f, "name")).unwrap_or_default();

    let status = crate::http::farms::field_status_json(db, field_id, ref_date).await;

    let mut facts = Vec::new();
    if let Some(loc) = opt_col(&field, "location").filter(|s| !s.is_empty()) {
        facts.push(("Location".into(), loc));
    }
    if let Some(area) = as_opt_f64(&field, "area_ha") {
        facts.push(("Area".into(), format!("{area} ha")));
    }
    if let Some(soil) = opt_col(&field, "soil_type").filter(|s| !s.is_empty()) {
        facts.push(("Soil type".into(), soil));
    }
    let status_text = status["statusReasons"]
        .as_array()
        .map(|r| {
            r.iter()
                .filter_map(|x| x["text"].as_str())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    if !status_text.is_empty() {
        facts.push(("Status".into(), status_text));
    }

    // Water over the last 7 days.
    let water = domain::water_input(
        &crate::http::entries::load_water_events(db, field_id).await,
        7,
        ref_date,
    )
    .to_json();

    let resistance = resistance_of_field(db, field_id)
        .await
        .iter()
        .map(resistance_json)
        .collect::<Vec<_>>();

    let records = crate::http::entries::load_resistance(db, field_id).await;

    let mut timeline = Vec::new();
    for e in entries_of_field(db, field_id).await {
        let entry_type = col(&e, "type");
        let date = col(&e, "entry_date");
        if let Some(d) = domain::dates::parse(&date) {
            if d > ref_date {
                continue;
            }
        }

        let data = as_value(&e, "data");
        let mut conflicted = false;
        let mut lines = Vec::new();

        match entry_type.as_str() {
            "chemical_application" => {
                let products: Vec<domain::SprayProduct> = data["products"]
                    .as_array()
                    .map(|a| a.iter().map(domain::SprayProduct::from_value).collect())
                    .unwrap_or_default();
                let checks = domain::check_conflicts(&products, &records);
                for (p, c) in products.iter().zip(checks.iter()) {
                    let mut line = format!(
                        "{}",
                        if p.product.is_empty() {
                            "(unnamed)".into()
                        } else {
                            p.product.clone()
                        }
                    );
                    if !p.moa.is_empty() {
                        line.push_str(&format!("  Group {}", p.moa));
                    }
                    if p.residual {
                        line.push_str("  residual");
                    }
                    if c.conflict {
                        conflicted = true;
                        line.push_str("  RESISTANCE CONFLICT");
                    }
                    lines.push(line);
                }
            }
            "planting" => {
                if let Some(crop) = data["crop"].as_str().filter(|s| !s.is_empty()) {
                    lines.push(format!("Crop: {crop}"));
                }
                if let Some(v) = data["variety"].as_str().filter(|s| !s.is_empty()) {
                    lines.push(format!("Variety: {v}"));
                }
                if let Some(r) = data["rateKgHa"].as_f64() {
                    lines.push(format!("Rate: {r} kg/ha"));
                }
                for f in data["fertilisers"].as_array().into_iter().flatten() {
                    lines.push(format!(
                        "Fertiliser: {} {}",
                        domain::entries::str_of(f, "product"),
                        domain::entries::str_of(f, "rateUnit")
                    ));
                }
            }
            "soil_test" => {
                for (key, label) in [
                    ("ph", "pH"),
                    ("organicMatter", "Organic matter %"),
                    ("n", "N"),
                    ("p", "P"),
                    ("k", "K"),
                    ("cec", "CEC"),
                ] {
                    if let Some(v) = domain::entries::num_of(&data, key) {
                        lines.push(format!("{label}: {v}"));
                    }
                }
                if let Some(lab) = data["lab"].as_str().filter(|s| !s.is_empty()) {
                    lines.push(format!("Lab: {lab}"));
                }
            }
            "soil_moisture" => {
                lines.push(format!(
                    "Reading: {} {}",
                    domain::entries::str_of(&data, "reading"),
                    domain::entries::str_of(&data, "unit")
                ));
            }
            "weed_scouting" => {
                lines.push(format!(
                    "{} - {} {}",
                    domain::entries::str_of(&data, "species"),
                    domain::entries::str_of(&data, "density"),
                    domain::entries::str_of(&data, "densityUnit")
                ));
                if let Some(loc) = data["location"].as_str().filter(|s| !s.is_empty()) {
                    lines.push(format!("Location: {loc}"));
                }
            }
            "harvest" => {
                lines.push(format!(
                    "Yield: {} {}",
                    domain::entries::str_of(&data, "yieldValue"),
                    domain::entries::str_of(&data, "yieldUnit")
                ));
                if let Some(m) = domain::entries::num_of(&data, "moisture") {
                    lines.push(format!("Moisture: {m}"));
                }
            }
            "water" => {
                // Printed even at zero, because a logged zero is meaningful.
                lines.push(format!(
                    "Rainfall: {} mm",
                    domain::entries::num_or_zero(&data, "rainfallMm")
                ));
                lines.push(format!(
                    "Irrigation: {} mm",
                    domain::entries::num_or_zero(&data, "irrigationMm")
                ));
                let method = domain::entries::str_of(&data, "method");
                if !method.is_empty() {
                    lines.push(format!("Method: {method}"));
                }
            }
            "inspection" => {
                if let Some(o) = data["outcome"].as_str().filter(|s| !s.is_empty()) {
                    lines.push(format!("Outcome: {o}"));
                }
            }
            _ => {}
        }

        let notes = col(&e, "notes");
        if !notes.is_empty() {
            lines.push(format!("Notes: {notes}"));
        }

        timeline.push((date, col(&e, "title"), lines, conflicted));
    }

    Some(Report {
        kind: "field".into(),
        title: format!("{farm_name} / {field_name}"),
        subtitle: "Field summary".into(),
        generated_at: chrono::Local::now().to_rfc3339(),
        today: ref_date.format(domain::dates::ISO).to_string(),
        facts,
        attention: Vec::new(),
        upcoming: Vec::new(),
        fields: Vec::new(),
        water: Some(water),
        residual: Some(status.get("lastResidual").cloned().unwrap_or(Value::Null)),
        resistance,
        timeline,
        filename_stem: format!("field-{}-{}", safe_name(&farm_name), safe_name(&field_name)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_name_lowercases_and_dashes_runs() {
        assert_eq!(safe_name("North Paddock"), "north-paddock");
        assert_eq!(safe_name("Block 12/A"), "block-12-a");
        assert_eq!(safe_name("--Weird--"), "weird");
        assert_eq!(safe_name(""), "report");
    }
}

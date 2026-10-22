//! Entry payload handling.
//!
//! An entry's `data` column holds a JSON object whose shape depends on `type`.
//! `sanitise` is the single place that decides what a stored payload looks
//! like: every field is coerced to its expected type and missing keys are
//! filled with defaults, so downstream code never has to re-check types.

use serde_json::{Map, Value, json};

pub fn str_of(data: &Value, key: &str) -> String {
    data.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn num_of(data: &Value, key: &str) -> Option<f64> {
    match data.get(key) {
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

pub fn num_or_zero(data: &Value, key: &str) -> f64 {
    num_of(data, key).unwrap_or(0.0)
}

fn put_str(obj: &mut Map<String, Value>, key: &str, value: impl Into<String>) {
    obj.insert(key.to_string(), Value::String(value.into()));
}

fn put_num(obj: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    match value {
        Some(v) if v.is_finite() => {
            obj.insert(key.to_string(), json!(v));
        }
        _ => {
            obj.insert(key.to_string(), Value::Null);
        }
    }
}

/// Read a string preferring `key`, falling back to `alt` (camel vs snake).
fn pick_str(data: &Value, key: &str, alt: &str) -> String {
    let a = str_of(data, key);
    if a.is_empty() { str_of(data, alt) } else { a }
}

fn pick_num(data: &Value, key: &str, alt: &str) -> Option<f64> {
    num_of(data, key).or_else(|| num_of(data, alt))
}

fn pick_list<'a>(data: &'a Value, key: &str, alt: &str) -> Vec<Value> {
    data.get(key)
        .or_else(|| data.get(alt))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Normalise the `data` payload for an entry type, filling defaults.
pub fn sanitise(entry_type: &str, data: &Value) -> Value {
    let input = if data.is_object() {
        data.clone()
    } else {
        Value::Object(Map::new())
    };
    let mut out = Map::new();

    match entry_type {
        "planting" => {
            let seeds = num_of(&input, "seedsPerKg");
            let rate_kg = pick_num(&input, "rateKgHa", "rate_kg_ha");
            let rate_seeds = pick_num(&input, "rateSeedsM2", "rate_seeds_m2");
            // Both rates are kept as entered. The browser shows the live
            // conversion, so the server does not second-guess the pair.
            let mode = {
                let m = pick_str(&input, "rateMode", "rate_mode");
                if m == "seeds_m2" { "seeds_m2" } else { "kg_ha" }
            };
            put_str(&mut out, "crop", pick_str(&input, "crop", "crop"));
            put_str(&mut out, "variety", pick_str(&input, "variety", "variety"));
            put_num(&mut out, "seedsPerKg", seeds);
            put_str(&mut out, "rateMode", mode);
            put_num(&mut out, "rateKgHa", rate_kg);
            put_num(&mut out, "rateSeedsM2", rate_seeds);

            let ferts = pick_list(&input, "fertilisers", "fertilizers")
                .iter()
                .filter_map(|f| sanitise_fertiliser(f))
                .collect::<Vec<_>>();
            out.insert("fertilisers".into(), Value::Array(ferts));
        }

        "soil_test" => {
            for (k, alt) in [("lab", "lab"), ("texture", "texture")] {
                put_str(&mut out, k, pick_str(&input, k, alt));
            }
            for key in [
                "ph",
                "organicMatter",
                "n",
                "p",
                "k",
                "ca",
                "mg",
                "s",
                "zn",
                "cu",
                "b",
                "fe",
                "cec",
            ] {
                put_num(&mut out, key, num_of(&input, key));
            }
        }

        "soil_moisture" => {
            put_num(&mut out, "reading", num_of(&input, "reading"));
            let unit = pick_str(&input, "unit", "unit");
            put_str(
                &mut out,
                "unit",
                if unit.is_empty() {
                    "probe reading".to_string()
                } else {
                    unit
                },
            );
            put_num(&mut out, "depthCm", pick_num(&input, "depthCm", "depth_cm"));
            put_str(&mut out, "method", pick_str(&input, "method", "method"));
        }

        "weed_scouting" => {
            put_str(&mut out, "species", pick_str(&input, "species", "species"));
            // Density is free text: operators record counts or qualitative bands.
            put_str(&mut out, "density", str_of(&input, "density"));
            let unit = pick_str(&input, "densityUnit", "density_unit");
            put_str(
                &mut out,
                "densityUnit",
                if unit.is_empty() {
                    "plants/m2".to_string()
                } else {
                    unit
                },
            );
            put_str(
                &mut out,
                "location",
                pick_str(&input, "location", "location"),
            );
        }

        "chemical_application" => {
            for (k, alt) in [("target", "target"), ("conditions", "conditions")] {
                put_str(&mut out, k, pick_str(&input, k, alt));
            }
            put_num(&mut out, "windKph", pick_num(&input, "windKph", "wind_kph"));
            put_num(
                &mut out,
                "temperatureC",
                pick_num(&input, "temperatureC", "temperature_c"),
            );
            put_num(
                &mut out,
                "humidityPct",
                pick_num(&input, "humidityPct", "humidity_pct"),
            );

            let products = pick_list(&input, "products", "products")
                .iter()
                .filter_map(|p| sanitise_product(p))
                .collect::<Vec<_>>();
            out.insert("products".into(), Value::Array(products));
        }

        "harvest" => {
            put_num(
                &mut out,
                "yieldValue",
                pick_num(&input, "yieldValue", "yield_value"),
            );
            let unit = pick_str(&input, "yieldUnit", "yield_unit");
            put_str(
                &mut out,
                "yieldUnit",
                if unit.is_empty() {
                    "t/ha".to_string()
                } else {
                    unit
                },
            );
            put_num(&mut out, "moisture", num_of(&input, "moisture"));
        }

        "water" => {
            put_num(
                &mut out,
                "rainfallMm",
                num_of(&input, "rainfallMm").or(Some(0.0)),
            );
            put_num(
                &mut out,
                "irrigationMm",
                num_of(&input, "irrigationMm").or(Some(0.0)),
            );
            let method = pick_str(&input, "method", "method");
            put_str(
                &mut out,
                "method",
                if method.is_empty() {
                    "none".to_string()
                } else {
                    method
                },
            );
        }

        "inspection" => {
            let task_type = pick_str(&input, "taskType", "task_type");
            put_str(
                &mut out,
                "taskType",
                if task_type.is_empty() {
                    "crop inspection".to_string()
                } else {
                    task_type
                },
            );
            if let Some(v) = input.get("taskId").and_then(|v| v.as_i64()) {
                out.insert("taskId".into(), json!(v));
            }
            put_str(&mut out, "outcome", pick_str(&input, "outcome", "outcome"));
        }

        _ => {}
    }

    Value::Object(out)
}

fn sanitise_product(p: &Value) -> Option<Value> {
    let name = str_of(p, "product");
    if name.is_empty() {
        return None;
    }
    let strength_unit = pick_str(p, "strengthUnit", "strength_unit");
    let rate_unit = pick_str(p, "rateUnit", "rate_unit");
    let mut out = Map::new();
    put_str(&mut out, "product", name);
    put_str(&mut out, "active", pick_str(p, "active", "active"));
    put_str(&mut out, "moa", pick_str(p, "moa", "moa"));
    put_num(&mut out, "strength", num_of(p, "strength"));
    put_str(
        &mut out,
        "strengthUnit",
        if strength_unit.is_empty() {
            "g/L".to_string()
        } else {
            strength_unit
        },
    );
    put_num(&mut out, "rate", num_of(p, "rate"));
    put_str(
        &mut out,
        "rateUnit",
        if rate_unit.is_empty() {
            "L/ha".to_string()
        } else {
            rate_unit
        },
    );
    // Stored as 0/1 so it survives the JSON round trip unchanged.
    put_num(
        &mut out,
        "residual",
        Some(if truthy(p.get("residual")) { 1.0 } else { 0.0 }),
    );
    Some(Value::Object(out))
}

fn sanitise_fertiliser(f: &Value) -> Option<Value> {
    let product = str_of(f, "product");
    if product.is_empty() {
        return None;
    }
    let rate_unit = pick_str(f, "rateUnit", "rate_unit");
    let mut out = Map::new();
    put_str(&mut out, "product", product);
    put_num(&mut out, "rate", num_of(f, "rate"));
    put_str(
        &mut out,
        "rateUnit",
        if rate_unit.is_empty() {
            "kg/ha".to_string()
        } else {
            rate_unit
        },
    );
    put_num(&mut out, "n", num_of(f, "n"));
    put_num(&mut out, "p", num_of(f, "p"));
    put_num(&mut out, "k", num_of(f, "k"));
    Some(Value::Object(out))
}

/// Frontend checkboxes/flags arrive as booleans, 0/1 or the strings "true"/"1".
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => {
            let t = s.trim().to_lowercase();
            t == "true" || t == "1" || t == "yes"
        }
        _ => false,
    }
}

/// Default entry title derived from the payload, used when the operator does
/// not type one.
pub fn auto_title(entry_type: &str, data: &Value) -> String {
    let s = |k: &str| str_of(data, k);
    match entry_type {
        "planting" => {
            let crop = s("crop");
            let variety = s("variety");
            if crop.is_empty() && variety.is_empty() {
                "Planting".into()
            } else if variety.is_empty() {
                crop
            } else if crop.is_empty() {
                variety
            } else {
                format!("{crop} - {variety}")
            }
        }
        "soil_test" => {
            let lab = s("lab");
            if lab.is_empty() {
                "Soil test".into()
            } else {
                format!("Soil test - {lab}")
            }
        }
        "soil_moisture" => {
            let reading = s("reading");
            let unit = s("unit");
            if reading.is_empty() {
                "Soil moisture".into()
            } else if unit.is_empty() {
                format!("Soil moisture - {reading}")
            } else {
                format!("Soil moisture - {reading} {unit}")
            }
        }
        "weed_scouting" => {
            let sp = s("species");
            let density = s("density");
            if sp.is_empty() && density.is_empty() {
                "Weed scouting".into()
            } else if density.is_empty() {
                sp
            } else if sp.is_empty() {
                format!("Weed scouting - {density}")
            } else {
                format!("{sp} - {density}")
            }
        }
        "chemical_application" => {
            let target = s("target");
            if target.is_empty() {
                "Chemical application".into()
            } else {
                format!("Spray - {target}")
            }
        }
        "harvest" => {
            let v = s("yieldValue");
            let u = s("yieldUnit");
            match (v.is_empty(), u.is_empty()) {
                (true, _) => "Harvest".into(),
                (false, true) => format!("Harvest - {v}"),
                (false, false) => format!("Harvest - {v} {u}"),
            }
        }
        "water" => {
            let rain = num_or_zero(data, "rainfallMm");
            let irr = num_or_zero(data, "irrigationMm");
            let method = s("method");
            if rain == 0.0 && irr == 0.0 {
                "Rainfall and irrigation".into()
            } else if method.is_empty() || method == "none" {
                format!("{rain} mm rain + {irr} mm")
            } else {
                format!("{rain} mm rain + {irr} mm {method}")
            }
        }
        "inspection" => {
            let outcome = s("outcome");
            if outcome.is_empty() {
                "Inspection".into()
            } else {
                outcome
            }
        }
        _ => "Entry".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_fills_defaults_and_drops_unnamed_products() {
        let out = sanitise(
            "chemical_application",
            &json!({ "target": "Ryegrass", "products": [
                { "product": "Roundup", "residual": true },
                { "product": "   " },
                { "product": "Atrazine", "residual": "1" }
            ]}),
        );
        let products = out["products"].as_array().unwrap();
        assert_eq!(products.len(), 2, "blank product names are dropped");
        assert_eq!(products[0]["strengthUnit"], "g/L");
        assert_eq!(products[0]["rateUnit"], "L/ha");
        assert_eq!(products[0]["residual"], 1.0);
        assert_eq!(products[1]["residual"], 1.0);
        assert_eq!(out["windKph"], Value::Null);
    }

    #[test]
    fn sanitise_rejects_non_object_payloads() {
        assert_eq!(
            sanitise("water", &json!("nonsense")),
            json!({
                "rainfallMm": 0.0, "irrigationMm": 0.0, "method": "none"
            })
        );
    }

    #[test]
    fn numbers_accept_strings_and_blank_strings_are_null() {
        let d = json!({ "ph": "6.2", "organicMatter": "" });
        let out = sanitise("soil_test", &d);
        assert_eq!(out["ph"], 6.2);
        assert_eq!(out["organicMatter"], Value::Null);
    }

    #[test]
    fn auto_title_falls_back_when_fields_are_blank() {
        assert_eq!(
            auto_title("chemical_application", &json!({})),
            "Chemical application"
        );
        assert_eq!(
            auto_title("chemical_application", &json!({ "target": "Ryegrass" })),
            "Spray - Ryegrass"
        );
        assert_eq!(auto_title("water", &json!({})), "Rainfall and irrigation");
        assert_eq!(
            auto_title(
                "water",
                &json!({ "rainfallMm": 12, "irrigationMm": 0, "method": "pivot" })
            ),
            "12 mm rain + 0 mm pivot"
        );
    }

    #[test]
    fn truthy_accepts_the_shapes_a_checkbox_can_send() {
        assert!(truthy(Some(&json!(true))));
        assert!(truthy(Some(&json!(1))));
        assert!(truthy(Some(&json!("true"))));
        assert!(!truthy(Some(&json!(0))));
        assert!(!truthy(Some(&json!("no"))));
        assert!(!truthy(None));
    }
}

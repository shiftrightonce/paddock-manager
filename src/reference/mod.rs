//! Static reference data.
//!
//! These tables are the app's built-in starting points, not authoritative
//! sources. Product MOA groups and strengths are a convenience for autocomplete;
//! crop uptake figures are indicative averages. Both are meant to be checked
//! against the label and local department guides.

/// Elemental conversion factors used by the nutrient budget.
pub const P2O5_TO_P: f64 = 0.4364;
pub const K2O_TO_K: f64 = 0.8301;

/// MOA group -> plain language description.
///
/// Keyed by the numeric group. Products that span two groups (`"15 + 27"`) are
/// looked up on the first group only.
pub const MOA_DESCRIPTIONS: &[(&str, &str)] = &[
    ("0", "Unknown / multiple"),
    ("1", "ACCase inhibitor"),
    ("2", "ALS inhibitor"),
    ("3", "Microtubule assembly inhibitor"),
    ("4", "Synthetic auxin"),
    ("5", "Photosystem II inhibitor"),
    ("6", "Photosystem II inhibitor (nitrile)"),
    ("9", "EPSPS inhibitor"),
    ("10", "Glutamine synthetase inhibitor"),
    ("13", "Carotenoid biosynthesis inhibitor"),
    ("14", "PPO inhibitor"),
    ("15", "VLCFA inhibitor"),
    ("18", "DHP synthase inhibitor"),
    ("22", "Photosystem I inhibitor"),
    ("27", "HPPD inhibitor"),
];

pub fn moa_description(moa: &str) -> String {
    let key = moa.trim();
    let first = key.split_whitespace().next().unwrap_or("");
    MOA_DESCRIPTIONS
        .iter()
        .find(|(k, _)| *k == first || k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.to_string())
        .unwrap_or_default()
}

/// Product, active, MOA, strength, whether it leaves a soil residual.
pub struct Chemical {
    pub product: &'static str,
    pub active: &'static str,
    pub moa: &'static str,
    pub strength: f64,
    pub strength_unit: &'static str,
    pub residual: bool,
    pub note: &'static str,
}

macro_rules! chem {
    ($p:expr, $a:expr, $m:expr, $s:expr, $r:expr, $n:expr) => {
        Chemical {
            product: $p,
            active: $a,
            moa: $m,
            strength: $s,
            strength_unit: "g/L",
            residual: $r,
            note: $n,
        }
    };
}

pub const CHEMICALS: &[Chemical] = &[
    chem!(
        "Roundup",
        "glyphosate",
        "9",
        480.0,
        false,
        " glyphosate isopropylamine 480 g/L"
    ),
    chem!(
        "Glyphosate 540",
        "glyphosate",
        "9",
        540.0,
        false,
        "commonly used for in-crop and stubble control"
    ),
    chem!(
        "Roundup Powermax",
        "glyphosate",
        "9",
        660.0,
        false,
        "granular formulation"
    ),
    chem!("Kendo", "glyphosate", "9", 450.0, false, ""),
    Chemical {
        strength_unit: "g/L",
        ..chem!(
            "Select",
            "clethodim",
            "1",
            240.0,
            false,
            "DIM, post-emergent grass selective"
        )
    },
    chem!("Select 240 EC", "clethodim", "1", 240.0, false, ""),
    chem!(
        "Targa",
        "quizalofop-P-ethyl",
        "1",
        102.0,
        false,
        "FOP, grass selective"
    ),
    chem!("Fusilade", "fluazifop-P-butyl", "1", 212.0, false, "FOP"),
    chem!("Verdict", "haloxyfop", "1", 510.0, false, "FOP"),
    chem!("Achieve", "tralkoxydim", "1", 300.0, false, "DIM"),
    chem!(
        "Axial",
        "pinoxaden",
        "1",
        45.0,
        false,
        "DEN, packaged with cloquintocet-mexyl safener"
    ),
    chem!("Topik", "clodinafop-propargyl", "1", 240.0, false, "FOP"),
    chem!(
        "Amicide",
        "2,4-D amine",
        "4",
        700.0,
        false,
        "synthetic auxin, drift sensitive"
    ),
    chem!("2,4-D amine", "2,4-D amine", "4", 700.0, false, ""),
    chem!(
        "Ester 40",
        "2,4-D ester",
        "4",
        680.0,
        false,
        "volatile ester formulation"
    ),
    chem!("Dicamba", "dicamba", "4", 500.0, false, "synthetic auxin"),
    chem!("MCPA", "MCPA", "4", 500.0, false, ""),
    chem!("Starane", "fluroxypyr", "4", 333.0, false, ""),
    chem!(
        "Grazon",
        "triclopyr",
        "4",
        480.0,
        false,
        "dicamba + triclopyr"
    ),
    chem!(
        "Lontar",
        "clopyralid",
        "4",
        300.0,
        false,
        "dicamba + clopyralid"
    ),
    chem!("Stinger", "clopyralid", "4", 300.0, false, ""),
    chem!(
        "Gallex",
        "aminopyralid",
        "4",
        33.0,
        true,
        "persistent in soil, grazeable with restrictions"
    ),
    chem!(
        "Atrazine",
        "atrazine",
        "5",
        500.0,
        true,
        "soil residual, long plantback on some crops"
    ),
    chem!(
        "Simazine",
        "simazine",
        "5",
        500.0,
        true,
        "long soil residual"
    ),
    chem!("Diuron", "diuron", "5", 500.0, true, ""),
    chem!(
        "Tribunic",
        "metribuzin",
        "5",
        250.0,
        true,
        "triazolinone, post-emergent and residual"
    ),
    chem!(
        "Buctril",
        "bromoxynil",
        "6",
        400.0,
        false,
        "nitrile, contact"
    ),
    chem!("Buctril 4-T", "bromoxynil", "6", 400.0, false, ""),
    chem!(
        "Command",
        "clomazone",
        "13",
        500.0,
        true,
        "bleaching herbicide, soil residual, carryover sensitive"
    ),
    chem!(
        "Balance",
        "isoxaflutole",
        "13",
        200.0,
        true,
        "requires crop safety seed, soil residual"
    ),
    chem!(
        "Spotlight",
        "carfentrazone-ethyl",
        "14",
        240.0,
        false,
        "PPO inhibitor, very low rate"
    ),
    chem!(
        "Ametron",
        "sulfentrazone",
        "14",
        100.0,
        true,
        "PPO inhibitor, soil residual"
    ),
    chem!(
        "Fomesafen",
        "fomesafen",
        "14",
        480.0,
        true,
        "PPO inhibitor, post-emergent with residual"
    ),
    chem!(
        "Tirexor",
        "pyroxasulfone + bicyclopyrone",
        "15 + 27",
        850.0,
        true,
        "pre-emergent, VLCFA + HPPD, long plantback"
    ),
    chem!(
        "Butisan",
        "methiozolin",
        "0",
        500.0,
        false,
        "residual light, check label group"
    ),
    chem!(
        "Devrinol",
        "napropamide",
        "0",
        500.0,
        true,
        "pre-emergent, unknown group on some labels"
    ),
    chem!(
        "Trifluralin",
        "trifluralin",
        "3",
        500.0,
        true,
        "microtubule assembly inhibitor"
    ),
    chem!(
        "Propyzamide",
        "propyzamide",
        "3",
        500.0,
        true,
        "microtubule assembly inhibitor"
    ),
    chem!(
        "Gramoxone",
        "paraquat",
        "22",
        275.0,
        false,
        "PSI electron diversion, contact"
    ),
    chem!("Paraquat", "paraquat", "22", 275.0, false, ""),
    chem!("Reglone", "diquat", "22", 150.0, false, "contact desiccant"),
    chem!("Basta", "glufosinate", "10", 200.0, false, "GS inhibitor"),
    chem!("Glysosperse", "glufosinate", "10", 480.0, false, ""),
    Chemical {
        strength_unit: "g/kg",
        ..chem!(
            "Ally",
            "metsulfuron-methyl",
            "2",
            600.0,
            false,
            "ALS inhibitor (SU)"
        )
    },
    Chemical {
        strength_unit: "g/kg",
        ..chem!(
            "Stinger Metsulfuron",
            "metsulfuron-methyl",
            "2",
            600.0,
            false,
            ""
        )
    },
    Chemical {
        strength_unit: "g/kg",
        ..chem!(
            "Glean",
            "chlorsulfuron",
            "2",
            750.0,
            true,
            "ALS inhibitor, carries to the next crop"
        )
    },
    Chemical {
        strength_unit: "g/kg",
        ..chem!("Eagle", "rimsulfuron", "2", 300.0, false, "")
    },
    Chemical {
        strength_unit: "g/kg",
        ..chem!(
            "Atlantis",
            "mesosulfuron",
            "2",
            300.0,
            false,
            "mixed with propranolol (Group 1 safener partner)"
        )
    },
    Chemical {
        strength_unit: "g/kg",
        ..chem!("Precept", "prosulfuron", "2", 300.0, false, "")
    },
    Chemical {
        strength_unit: "g/kg",
        ..chem!(
            "Conqueror",
            "prosulfuron + trifloxysulfuron",
            "2",
            240.0,
            false,
            ""
        )
    },
    chem!(
        "Bindi",
        "flumetsulam",
        "2",
        500.0,
        true,
        "ALS inhibitor, triazolopyrimidine, carries over"
    ),
    chem!(
        "Crusader",
        "pyroxsulam",
        "2",
        100.0,
        true,
        "ALS inhibitor, triazolopyrimidine"
    ),
    chem!(
        "Callisto",
        "mesotrione",
        "27",
        480.0,
        false,
        "HPPD inhibitor, bleaching"
    ),
    chem!("BroadStar", "mesotrione", "27", 480.0, false, ""),
    chem!(
        "Serrate",
        "pyrasulfotole",
        "27",
        300.0,
        false,
        "HPPD inhibitor"
    ),
    chem!(
        "Asulam",
        "asulam",
        "18",
        700.0,
        false,
        "DHP synthase inhibitor"
    ),
    chem!(
        "Ravene",
        "pyraflufen-ethyl",
        "14",
        240.0,
        false,
        "PPO inhibitor"
    ),
];

/// Nutrient uptake in kg per tonne of produce, plus a typical yield.
pub struct Crop {
    pub name: &'static str,
    pub n: f64,
    pub p: f64,
    pub k: f64,
    /// Legumes fix nitrogen, so the N figure is not a soil requirement.
    pub fixed_n: bool,
    pub typical_yield: f64,
}

pub const CROPS: &[Crop] = &[
    Crop {
        name: "Wheat",
        n: 20.0,
        p: 3.5,
        k: 5.0,
        fixed_n: false,
        typical_yield: 3.0,
    },
    Crop {
        name: "Barley",
        n: 18.0,
        p: 3.3,
        k: 5.0,
        fixed_n: false,
        typical_yield: 3.0,
    },
    Crop {
        name: "Oats",
        n: 20.0,
        p: 3.5,
        k: 5.0,
        fixed_n: false,
        typical_yield: 3.0,
    },
    Crop {
        name: "Canola",
        n: 40.0,
        p: 4.0,
        k: 10.0,
        fixed_n: false,
        typical_yield: 2.5,
    },
    Crop {
        name: "Chickpea",
        n: 15.0,
        p: 3.4,
        k: 6.0,
        fixed_n: true,
        typical_yield: 2.0,
    },
    Crop {
        name: "Field pea",
        n: 15.0,
        p: 3.2,
        k: 6.0,
        fixed_n: true,
        typical_yield: 2.5,
    },
    Crop {
        name: "Lentil",
        n: 20.0,
        p: 3.5,
        k: 7.0,
        fixed_n: true,
        typical_yield: 1.5,
    },
    Crop {
        name: "Faba bean",
        n: 0.0,
        p: 3.5,
        k: 8.0,
        fixed_n: true,
        typical_yield: 2.0,
    },
    Crop {
        name: "Lupin",
        n: 0.0,
        p: 4.0,
        k: 6.0,
        fixed_n: true,
        typical_yield: 2.0,
    },
    Crop {
        name: "Soybean",
        n: 0.0,
        p: 3.5,
        k: 6.0,
        fixed_n: true,
        typical_yield: 2.5,
    },
    Crop {
        name: "Cotton",
        n: 30.0,
        p: 5.0,
        k: 15.0,
        fixed_n: false,
        typical_yield: 2.0,
    },
    Crop {
        name: "Sorghum",
        n: 18.0,
        p: 3.0,
        k: 5.0,
        fixed_n: false,
        typical_yield: 3.5,
    },
    Crop {
        name: "Maize",
        n: 20.0,
        p: 3.5,
        k: 5.0,
        fixed_n: false,
        typical_yield: 8.0,
    },
    Crop {
        name: "Rice",
        n: 20.0,
        p: 3.0,
        k: 6.0,
        fixed_n: false,
        typical_yield: 8.0,
    },
    Crop {
        name: "Sunflower",
        n: 15.0,
        p: 3.0,
        k: 6.0,
        fixed_n: false,
        typical_yield: 1.5,
    },
    Crop {
        name: "Sunflower (oilseed)",
        n: 35.0,
        p: 4.5,
        k: 8.0,
        fixed_n: false,
        typical_yield: 1.5,
    },
    Crop {
        name: "Poppy",
        n: 40.0,
        p: 4.0,
        k: 15.0,
        fixed_n: false,
        typical_yield: 2.0,
    },
    Crop {
        name: "Lucerne (hay)",
        n: 25.0,
        p: 3.0,
        k: 20.0,
        fixed_n: true,
        typical_yield: 12.0,
    },
    Crop {
        name: "Pasture (DM basis)",
        n: 20.0,
        p: 3.0,
        k: 20.0,
        fixed_n: true,
        typical_yield: 10.0,
    },
    Crop {
        name: "Clover (DM basis)",
        n: 0.0,
        p: 3.0,
        k: 20.0,
        fixed_n: true,
        typical_yield: 8.0,
    },
    Crop {
        name: "Canola forage",
        n: 40.0,
        p: 4.0,
        k: 10.0,
        fixed_n: false,
        typical_yield: 6.0,
    },
    Crop {
        name: "Cereal hay (oaten)",
        n: 20.0,
        p: 3.0,
        k: 20.0,
        fixed_n: false,
        typical_yield: 8.0,
    },
];

/// Fertiliser analysis as percent of product, as stated on the label.
pub struct Fertiliser {
    pub product: &'static str,
    pub n: f64,
    pub p2o5: f64,
    pub k2o: f64,
    pub rate_unit: &'static str,
}

pub const FERTILISERS: &[Fertiliser] = &[
    Fertiliser {
        product: "Urea",
        n: 46.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Urea (prilled)",
        n: 46.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Sulfate of ammonia",
        n: 20.6,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Ammonium sulfate",
        n: 20.6,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "UAN",
        n: 32.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "L/ha",
    },
    Fertiliser {
        product: "UAN 25",
        n: 25.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "L/ha",
    },
    Fertiliser {
        product: "DAP",
        n: 18.4,
        p2o5: 20.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Di-ammonium phosphate",
        n: 18.4,
        p2o5: 20.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "MAP",
        n: 11.0,
        p2o5: 10.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Mono-ammonium phosphate",
        n: 11.0,
        p2o5: 10.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "MKP",
        n: 0.0,
        p2o5: 22.0,
        k2o: 28.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Muriate of potash",
        n: 0.0,
        p2o5: 0.0,
        k2o: 60.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Potassium chloride",
        n: 0.0,
        p2o5: 0.0,
        k2o: 60.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Sulfate of potash",
        n: 0.0,
        p2o5: 0.0,
        k2o: 41.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Potassium sulfate",
        n: 0.0,
        p2o5: 0.0,
        k2o: 41.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Potassium nitrate",
        n: 13.0,
        p2o5: 0.0,
        k2o: 38.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Nitrate of soda",
        n: 16.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Single superphosphate",
        n: 0.0,
        p2o5: 9.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "SSP",
        n: 0.0,
        p2o5: 9.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Triple superphosphate",
        n: 0.0,
        p2o5: 20.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "TSP",
        n: 0.0,
        p2o5: 20.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "MAP + Urea starter",
        n: 24.0,
        p2o5: 9.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Lime",
        n: 0.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "t/ha",
    },
    Fertiliser {
        product: "Gypsum",
        n: 0.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "t/ha",
    },
    Fertiliser {
        product: "Zinc sulfate",
        n: 0.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Trace element mix",
        n: 0.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
    Fertiliser {
        product: "Potassium thiosulfate",
        n: 0.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "L/ha",
    },
    Fertiliser {
        product: "Urea ammonium sulfate",
        n: 32.0,
        p2o5: 0.0,
        k2o: 0.0,
        rate_unit: "kg/ha",
    },
];

pub const SOIL_TYPES: &[&str] = &[
    "Sand",
    "Loamy sand",
    "Sandy loam",
    "Sandy clay loam",
    "Loam",
    "Clay loam",
    "Silt loam",
    "Clay",
    "Red clay",
    "Red-brown earth",
    "Black earth",
    "Krasnozem",
    "Chocolate",
    "Podzol",
    "Peaty",
    "Sandplain",
];

pub const IRRIGATION_METHODS: &[&str] = &["pivot", "flood", "drip", "spray", "furrow", "none"];

pub const WEED_DENSITY_UNITS: &[&str] = &[
    "plants/m2",
    "plants/ha",
    "plants/100 m2",
    "plants/paddock",
    "low",
    "medium",
    "high",
];

pub const SOIL_MOISTURE_UNITS: &[&str] = &["% VWC", "mm", "probe reading", "gravimetric %"];

pub const HARVEST_UNITS: &[&str] = &["t/ha", "kg/ha", "bales/ha", "tonnes total"];

pub const ENTRY_TYPES: &[&str] = &[
    "planting",
    "soil_test",
    "soil_moisture",
    "weed_scouting",
    "chemical_application",
    "harvest",
    "water",
    "inspection",
];

pub const RATE_UNITS: &[&str] = &[
    "L/ha",
    "kg/ha",
    "g/ha",
    "mL/ha",
    "L/100 L",
    "kg/100 L",
    "g/100 L",
    "L/ha (a.i.)",
];

pub const STRENGTH_UNITS: &[&str] = &["g/L", "g/kg", "g/100 g", "%", "g/100 mL"];

pub const TASK_TYPES: &[&str] = &["insect_scouting", "crop_inspection", "custom"];

pub const REPEAT_INTERVALS: &[&str] = &["none", "weekly", "fortnightly", "monthly"];

pub fn find_crop(name: &str) -> Option<&'static Crop> {
    let n = name.trim().to_lowercase();
    CROPS.iter().find(|c| c.name.to_lowercase() == n)
}

pub fn find_fertiliser(name: &str) -> Option<&'static Fertiliser> {
    let n = name.trim().to_lowercase();
    FERTILISERS.iter().find(|f| f.product.to_lowercase() == n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_lists_are_non_empty() {
        assert!(CHEMICALS.len() > 25, "expected a broadacre herbicide list");
        assert!(!CROPS.is_empty());
        assert!(!FERTILISERS.is_empty());
    }

    #[test]
    fn multi_group_product_resolves_first_group() {
        // "15 + 27" splits on whitespace, so VLCFA is what is found.
        assert_eq!(moa_description("15 + 27"), "VLCFA inhibitor");
        assert_eq!(moa_description("9"), "EPSPS inhibitor");
        assert_eq!(moa_description("nonsense"), "");
    }

    #[test]
    fn crop_lookup_is_case_insensitive() {
        assert!(find_crop("wheat").is_some());
        assert!(find_crop("  WHEAT ").is_some());
        assert!(find_crop("nonexistent").is_none());
    }

    #[test]
    fn legumes_carry_no_nitrogen_requirement() {
        let pea = find_crop("Chickpea").unwrap();
        assert!(pea.fixed_n);
        let wheat = find_crop("Wheat").unwrap();
        assert!(!wheat.fixed_n && wheat.n > 0.0);
    }
}

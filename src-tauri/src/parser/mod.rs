pub mod gcode;
pub mod three_mf;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use three_mf::parse_3mf;

pub(crate) fn preset_base(value: &str) -> &str {
    value
        .split_once(" @")
        .map_or(value, |(base, _)| base)
        .trim()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedPrintFile {
    pub filaments: Vec<FilamentProfile>,
    pub gcode: gcode::GcodeReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilamentProfile {
    pub tool: u8,
    pub preset_id: String,
    pub brand: String,
    pub material: String,
    pub series: String,
    pub color_hex: String,
    pub diameter_mm: f64,
    pub density_g_cm3: f64,
    pub unknown_fields: BTreeMap<String, serde_json::Value>,
}

impl FilamentProfile {
    pub fn grams_for_length_mm(&self, length_mm: f64) -> f64 {
        length_mm * std::f64::consts::PI * (self.diameter_mm / 2.0).powi(2) / 1000.0
            * self.density_g_cm3
    }
}

#[cfg(test)]
mod tests {
    use super::preset_base;

    #[test]
    fn removes_only_the_machine_suffix_from_a_preset() {
        assert_eq!(preset_base("Bambu PLA Basic @BBL A1"), "Bambu PLA Basic");
        assert_eq!(preset_base("Bambu PLA Basic"), "Bambu PLA Basic");
    }
}

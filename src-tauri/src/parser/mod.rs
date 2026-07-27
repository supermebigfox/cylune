pub mod gcode;
pub mod three_mf;

use std::collections::BTreeMap;

pub use three_mf::parse_3mf;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPrintFile {
    pub filaments: Vec<FilamentProfile>,
    pub gcode: gcode::GcodeReport,
}

#[derive(Debug, Clone, PartialEq)]
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

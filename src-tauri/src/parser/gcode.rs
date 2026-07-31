use std::collections::BTreeMap;
use std::io::BufRead;

use crate::domain::Confidence;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerUsage {
    pub layer: u32,
    pub cumulative_mm: BTreeMap<u8, f64>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcodeReport {
    pub layers: Vec<LayerUsage>,
    pub totals_mm: BTreeMap<u8, f64>,
    pub max_layer: u32,
    #[serde(default)]
    pub declared_estimated_seconds: Option<u32>,
    #[serde(default)]
    pub declared_total_layers: Option<u32>,
}

impl GcodeReport {
    pub fn display_layer_count(&self) -> u32 {
        self.declared_total_layers
            .filter(|layers| *layers > 0)
            .unwrap_or(self.max_layer)
    }
}

pub fn parse_gcode<R: BufRead>(reader: R) -> Result<GcodeReport> {
    let mut current_tool = 0;
    let mut absolute_extrusion = true;
    let mut positions = BTreeMap::new();
    let mut totals_mm = BTreeMap::new();
    let mut layers = Vec::new();
    let mut current_layer = None;
    let mut saw_extrusion_command = false;
    let mut declared_estimated_seconds = None;
    let mut declared_total_layers = None;

    for line in reader.lines() {
        let line = line?;
        if let Some(comment) = line.split_once(';').map(|(_, comment)| comment.trim()) {
            if let Some(value) = comment.strip_prefix("total estimated time:") {
                declared_estimated_seconds = parse_estimated_seconds(value);
            }
            if let Some(value) = comment.strip_prefix("total layer number:") {
                declared_total_layers = value.trim().parse().ok();
            }
        }
        if let Some(layer) = layer_marker(&line, layers.len() as u32) {
            add_layers_through(&mut layers, layer, &totals_mm);
            current_layer = Some(layer as usize);
        }

        let command = line.split(';').next().unwrap_or("").trim();
        let mut words = command.split_ascii_whitespace();
        let Some(instruction) = words.next() else {
            continue;
        };

        if instruction == "M82" {
            absolute_extrusion = true;
            continue;
        }

        if instruction == "M83" {
            absolute_extrusion = false;
            continue;
        }

        if let Some(tool) = instruction
            .strip_prefix('T')
            .and_then(|value| value.parse().ok())
        {
            current_tool = tool;
            continue;
        }

        if instruction == "M1020" {
            if let Some(tool) = words
                .filter_map(|word| word.strip_prefix('S'))
                .find_map(|value| value.parse().ok())
            {
                current_tool = tool;
            }
            continue;
        }

        if instruction == "G92" {
            if let Some(position) = extrusion_value(words) {
                positions.insert(current_tool, position);
            }
            continue;
        }

        if instruction != "G0" && instruction != "G1" {
            continue;
        }

        let Some(position) = extrusion_value(words) else {
            continue;
        };
        saw_extrusion_command = true;

        let extruded = if absolute_extrusion {
            let previous = positions.insert(current_tool, position).unwrap_or(0.0);
            position - previous
        } else {
            *positions.entry(current_tool).or_insert(0.0) += position;
            position
        };
        if extruded > 0.0 {
            *totals_mm.entry(current_tool).or_insert(0.0) += extruded;
            if let Some(layer) = current_layer {
                *layers[layer]
                    .cumulative_mm
                    .entry(current_tool)
                    .or_insert(0.0) += extruded;
            }
        }
    }

    if !saw_extrusion_command {
        return Err(AppError::UnknownGcode);
    }

    Ok(GcodeReport {
        max_layer: layers.len() as u32,
        layers,
        totals_mm,
        declared_estimated_seconds,
        declared_total_layers,
    })
}

fn parse_estimated_seconds(value: &str) -> Option<u32> {
    let mut seconds = 0_u32;
    let mut saw_component = false;

    for component in value.split_ascii_whitespace() {
        let (amount, unit) = component.split_at(component.len().checked_sub(1)?);
        let multiplier = match unit {
            "h" => 3_600,
            "m" => 60,
            "s" => 1,
            _ => return None,
        };
        seconds = seconds.checked_add(amount.parse::<u32>().ok()?.checked_mul(multiplier)?)?;
        saw_component = true;
    }

    saw_component.then_some(seconds)
}

fn layer_marker(line: &str, next_sequential_layer: u32) -> Option<u32> {
    let (_, comment) = line.split_once(';')?;
    let comment = comment.trim();
    if comment == "CHANGE_LAYER" {
        return Some(next_sequential_layer);
    }
    if let Some(value) = comment.strip_prefix("LAYER:") {
        return value.split_ascii_whitespace().next()?.parse().ok();
    }
    let progress = comment.strip_prefix("layer num/total_layer_count:")?;
    progress
        .trim()
        .split('/')
        .next()?
        .trim()
        .parse::<u32>()
        .ok()?
        .checked_sub(1)
}

fn add_layers_through(layers: &mut Vec<LayerUsage>, layer: u32, totals_mm: &BTreeMap<u8, f64>) {
    while layers.len() <= layer as usize {
        layers.push(LayerUsage {
            layer: layers.len() as u32,
            cumulative_mm: totals_mm.clone(),
            confidence: Confidence::Exact,
        });
    }
}

fn extrusion_value<'a>(words: impl Iterator<Item = &'a str>) -> Option<f64> {
    words
        .filter_map(|word| word.strip_prefix('E'))
        .find_map(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::parse_gcode;
    use crate::error::AppError;
    use std::cell::Cell;
    use std::io::{self, BufRead, Read};
    use std::rc::Rc;

    #[test]
    fn separates_usage_by_tool_and_ignores_retraction() {
        let src = b"M82\nT0\nG1 E10\nG1 E8\nG1 E15\nT1\nG92 E0\nG1 E4\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.totals_mm[&0], 17.0);
        assert_eq!(report.totals_mm[&1], 4.0);
    }

    #[test]
    fn records_cumulative_usage_for_each_comment_layer() {
        let report =
            parse_gcode(&include_bytes!("../../tests/fixtures/single_color.gcode")[..]).unwrap();

        assert_eq!(report.layers[9].cumulative_mm[&0], 42.5);
        assert_eq!(report.max_layer, 10);
    }

    #[test]
    fn counts_positive_relative_extrusion_and_ignores_retraction() {
        let report =
            parse_gcode(&include_bytes!("../../tests/fixtures/tool_changes.gcode")[..]).unwrap();

        assert_eq!(report.totals_mm[&0], 2.8);
        assert_eq!(report.totals_mm[&1], 3.5);
    }

    #[test]
    fn switches_from_relative_to_absolute_extrusion() {
        let src = b"M83\nG1 E2\nG1 E3\nM82\nG92 E10\nG1 E12\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.totals_mm[&0], 7.0);
    }

    #[test]
    fn tracks_bambu_2_8_m1020_filament_switches() {
        let src = b"M83\nM1020 S0 H0\nG1 E2\nM1020 S2 H0\nG1 E3\nM1020 S1 H0\nG1 E4\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.totals_mm[&0], 2.0);
        assert_eq!(report.totals_mm[&1], 4.0);
        assert_eq!(report.totals_mm[&2], 3.0);
    }

    #[test]
    fn advances_the_absolute_position_during_relative_extrusion() {
        let src = b"M82\nG1 E10\nM83\nG1 E3\nM82\nG1 E15\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.totals_mm[&0], 15.0);
    }

    #[test]
    fn recognizes_crlf_layer_comments_and_skips_unknown_commands() {
        let src = b"; LAYER:0\r\nM83\r\nM900 K0.05\r\nG1 E1.25\r\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.layers[0].cumulative_mm[&0], 1.25);
        assert_eq!(report.max_layer, 1);
    }

    #[test]
    fn recognizes_bambu_one_based_layer_progress_comments() {
        let src = b"M83\n; layer num/total_layer_count: 1/3\nG1 E2\n; layer num/total_layer_count: 2/3\nG1 E3\n; layer num/total_layer_count: 3/3\nG1 E4\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.max_layer, 3);
        assert_eq!(report.layers[0].cumulative_mm[&0], 2.0);
        assert_eq!(report.layers[1].cumulative_mm[&0], 5.0);
        assert_eq!(report.layers[2].cumulative_mm[&0], 9.0);
    }

    #[test]
    fn recognizes_bambu_x2d_sequential_change_layer_comments() {
        let src = b"M83\n; CHANGE_LAYER\n; Z_HEIGHT: 0.2\nG1 E2\n; CHANGE_LAYER\n; Z_HEIGHT: 0.4\nG1 E3\n; CHANGE_LAYER\n; Z_HEIGHT: 0.6\nG1 E4\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.max_layer, 3);
        assert_eq!(report.layers[0].cumulative_mm[&0], 2.0);
        assert_eq!(report.layers[1].cumulative_mm[&0], 5.0);
        assert_eq!(report.layers[2].cumulative_mm[&0], 9.0);
    }

    #[test]
    fn records_declared_time_and_total_layers_without_changing_extrusion_totals() {
        let src = b"; total estimated time: 5h 5m 7s\n; total layer number: 14\nM83\n; LAYER:0\nG1 E2\nG1 E-1\n";

        let report = parse_gcode(&src[..]).unwrap();

        assert_eq!(report.declared_estimated_seconds, Some(18_307));
        assert_eq!(report.declared_total_layers, Some(14));
        assert_eq!(report.max_layer, 1);
        assert_eq!(report.totals_mm[&0], 2.0);
    }

    #[test]
    fn declared_total_is_the_display_count_without_layer_markers() {
        let report = parse_gcode(&b"; total layer number: 14\nM83\nG1 E2\n"[..]).unwrap();

        assert_eq!(report.max_layer, 0);
        assert_eq!(report.layers.len(), 0);
        assert_eq!(report.display_layer_count(), 14);
    }

    #[test]
    fn returns_unknown_gcode_without_an_extrusion_command() {
        let error = parse_gcode(&b"M82\nT0\nG1 X10 Y10\n"[..]).unwrap_err();

        assert!(matches!(error, AppError::UnknownGcode));
    }

    #[test]
    fn parses_large_stream_with_bounded_buffer() {
        let max_buffer = Rc::new(Cell::new(0));
        let reader = GeneratedGcode::new(1_000_000, Rc::clone(&max_buffer));

        let report = parse_gcode(reader).unwrap();

        assert_eq!(report.totals_mm[&0], 1_000_000.0);
        assert!(max_buffer.get() < 1024 * 1024);
    }

    struct GeneratedGcode {
        remaining_extrusions: usize,
        starts_with_mode: bool,
        current: &'static [u8],
        offset: usize,
        max_buffer: Rc<Cell<usize>>,
    }

    impl GeneratedGcode {
        fn new(remaining_extrusions: usize, max_buffer: Rc<Cell<usize>>) -> Self {
            Self {
                remaining_extrusions,
                starts_with_mode: true,
                current: b"",
                offset: 0,
                max_buffer,
            }
        }
    }

    impl Read for GeneratedGcode {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let count = {
                let input = self.fill_buf()?;
                let count = output.len().min(input.len());
                output[..count].copy_from_slice(&input[..count]);
                count
            };
            self.consume(count);
            Ok(count)
        }
    }

    impl BufRead for GeneratedGcode {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if self.offset == self.current.len() {
                if self.starts_with_mode {
                    self.starts_with_mode = false;
                    self.current = b"M83\n";
                    self.offset = 0;
                } else if self.remaining_extrusions > 0 {
                    self.remaining_extrusions -= 1;
                    self.current = b"G1 E1\n";
                    self.offset = 0;
                }
            }

            let buffer = &self.current[self.offset..];
            self.max_buffer.set(self.max_buffer.get().max(buffer.len()));
            Ok(buffer)
        }

        fn consume(&mut self, amount: usize) {
            self.offset = (self.offset + amount).min(self.current.len());
        }
    }
}

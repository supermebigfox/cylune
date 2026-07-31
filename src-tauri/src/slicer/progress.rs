#[derive(Debug, Default)]
pub(crate) struct BambuProgressParser {
    plate_count: usize,
    plate_index: usize,
    last_percent: f64,
}

impl BambuProgressParser {
    pub(crate) fn observe(&mut self, line: &str) -> Option<f64> {
        if let Some(plate_count) = parse_plate_count(line) {
            self.plate_count = plate_count;
            return None;
        }
        if let Some(plate_index) = parse_plate_index(line) {
            self.plate_index = plate_index;
            return None;
        }
        if line.contains("will export 3mf") {
            return self.advance_to(97.0);
        }

        let plate_percent = parse_status_callback(line)?;
        let total = if self.plate_count <= 1 {
            3.0 + 0.9 * plate_percent
        } else {
            3.0 + ((self.plate_index.saturating_sub(1) as f64) * 90.0) / self.plate_count as f64
                + (plate_percent * 0.9) / self.plate_count as f64
        };
        self.advance_to(total.clamp(0.0, 97.0))
    }

    fn advance_to(&mut self, percent: f64) -> Option<f64> {
        if percent <= self.last_percent {
            return None;
        }
        self.last_percent = percent;
        Some(percent)
    }
}

fn parse_plate_count(line: &str) -> Option<usize> {
    let (_, count) = line.split_once("total plate count ")?;
    let (count, _) = count.split_once(" partplates!")?;
    count.parse().ok().filter(|count: &usize| *count > 0)
}

fn parse_plate_index(line: &str) -> Option<usize> {
    let (_, index) = line.split_once("start Print::process for partplate ")?;
    index.trim().parse().ok()
}

fn parse_status_callback(line: &str) -> Option<f64> {
    let (_, callback) = line.split_once("default_status_callback: ")?;
    let (percent, callback) = callback.split_once(", warning_step=")?;
    let percent = percent.strip_prefix("percent=")?.parse::<f64>().ok()?;
    if !percent.is_finite() {
        return None;
    }
    let (warning_step, _) = callback.split_once(',')?;
    (warning_step == "-1").then_some(percent)
}

#[cfg(test)]
mod tests {
    use super::BambuProgressParser;

    #[test]
    fn maps_bambu_callback_to_its_total_progress_formula() {
        let mut parser = BambuProgressParser::default();
        parser.observe("Need to slice for plate 0, total plate count 2 partplates!");
        parser.observe("start Print::process for partplate 1");
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0"
            ),
            Some(25.5)
        );
        parser.observe("start Print::process for partplate 2");
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0"
            ),
            Some(70.5)
        );
    }

    #[test]
    fn ignores_malformed_warning_repeated_and_decreasing_callbacks() {
        let mut parser = BambuProgressParser::default();
        assert_eq!(parser.observe("not a Bambu callback"), None);
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=50, warning_step=3, message=warning, message_type=0"
            ),
            None
        );
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0"
            ),
            Some(48.0)
        );
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0"
            ),
            None
        );
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=40, warning_step=-1, message=Generating infill, message_type=0"
            ),
            None
        );
    }

    #[test]
    fn clamps_callback_progress_and_advances_export_to_97() {
        let mut parser = BambuProgressParser::default();
        assert_eq!(
            parser.observe(
                "default_status_callback: percent=200, warning_step=-1, message=Generating infill, message_type=0"
            ),
            Some(97.0)
        );

        let mut parser = BambuProgressParser::default();
        assert_eq!(parser.observe("will export 3mf"), Some(97.0));
    }
}

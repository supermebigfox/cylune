const SAFE_INSET: f64 = 16.0;
const LENS_EXPANSION_FACTOR: f64 = 1.24;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetPoint {
    pub x: f64,
    pub y: f64,
}

impl PetPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayRect {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

impl DisplayRect {
    pub const fn new(id: u64, x: f64, y: f64, width: f64, height: f64, scale: f64) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureRect {
    pub display_id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub logical_pet_size: f64,
}

pub fn display_for_pet(origin: PetPoint, size: f64, displays: &[DisplayRect]) -> DisplayRect {
    let mut selected = *displays
        .first()
        .expect("display_for_pet requires at least one display");
    let mut greatest_area = intersection_area(origin, size, selected);
    for display in displays.iter().copied().skip(1) {
        let area = intersection_area(origin, size, display);
        if area > greatest_area {
            selected = display;
            greatest_area = area;
        }
    }
    selected
}

pub fn safe_origin(origin: PetPoint, size: f64, displays: &[DisplayRect]) -> PetPoint {
    let Some(_) = displays.first() else {
        return origin;
    };
    let display = display_for_pet(origin, size, displays);
    PetPoint::new(
        clamp_axis(
            origin.x,
            display.x + SAFE_INSET,
            display.x + display.width - size - SAFE_INSET,
        ),
        clamp_axis(
            origin.y,
            display.y + SAFE_INSET,
            display.y + display.height - size - SAFE_INSET,
        ),
    )
}

pub fn capture_rect(origin: PetPoint, size: f64, display: DisplayRect) -> CaptureRect {
    let requested_side = (size * LENS_EXPANSION_FACTOR).floor();
    let margin = (requested_side - size) / 2.0;
    let requested_x = origin.x - margin;
    let requested_y = origin.y - margin;
    let display_right = display.x + display.width;
    let display_top = display.y + display.height;
    let x = requested_x.clamp(display.x, display_right);
    let y = requested_y.clamp(display.y, display_top);
    let right = (requested_x + requested_side).clamp(display.x, display_right);
    let top = (requested_y + requested_side).clamp(display.y, display_top);
    let width = (right - x).max(0.0);
    let height = (top - y).max(0.0);

    CaptureRect {
        display_id: display.id,
        x,
        y,
        width,
        height,
        pixel_width: backing_pixels(width, display.scale),
        pixel_height: backing_pixels(height, display.scale),
        logical_pet_size: size,
    }
}

fn intersection_area(origin: PetPoint, size: f64, display: DisplayRect) -> f64 {
    let overlap_width =
        ((origin.x + size).min(display.x + display.width) - origin.x.max(display.x)).max(0.0);
    let overlap_height =
        ((origin.y + size).min(display.y + display.height) - origin.y.max(display.y)).max(0.0);
    overlap_width * overlap_height
}

fn clamp_axis(value: f64, minimum: f64, maximum: f64) -> f64 {
    if minimum <= maximum {
        value.clamp(minimum, maximum)
    } else {
        (minimum + maximum) / 2.0
    }
}

fn backing_pixels(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().clamp(0.0, u32::MAX as f64) as u32
}

#[cfg(test)]
mod tests {
    use super::{capture_rect, display_for_pet, safe_origin, DisplayRect, PetPoint};

    #[test]
    fn disconnected_display_clamps_pet_into_primary_safe_area() {
        let primary = DisplayRect::new(7, 0.0, 0.0, 1512.0, 982.0, 2.0);
        assert_eq!(
            safe_origin(PetPoint::new(5000.0, -80.0), 220.0, &[primary]),
            PetPoint::new(1276.0, 16.0)
        );
    }

    #[test]
    fn greatest_panel_intersection_selects_the_new_display() {
        let screens = [
            DisplayRect::new(1, 0.0, 0.0, 1512.0, 982.0, 2.0),
            DisplayRect::new(2, 1512.0, 0.0, 1920.0, 1080.0, 1.0),
        ];

        assert_eq!(
            display_for_pet(PetPoint::new(1450.0, 100.0), 220.0, &screens).id,
            2
        );
    }

    #[test]
    fn disconnected_pet_uses_the_first_display_as_primary() {
        let screens = [
            DisplayRect::new(1, 0.0, 0.0, 1512.0, 982.0, 2.0),
            DisplayRect::new(2, 1512.0, 0.0, 1920.0, 1080.0, 1.0),
        ];

        assert_eq!(
            display_for_pet(PetPoint::new(5000.0, 5000.0), 220.0, &screens).id,
            1
        );
    }

    #[test]
    fn retina_capture_expands_by_the_lens_margin_in_backing_pixels() {
        let screen = DisplayRect::new(3, 0.0, 0.0, 1512.0, 982.0, 2.0);
        let rect = capture_rect(PetPoint::new(200.0, 100.0), 220.0, screen);

        assert_eq!(rect.pixel_width, 544);
        assert_eq!(rect.pixel_height, 544);
        assert_eq!(rect.logical_pet_size, 220.0);
    }

    #[test]
    fn capture_rectangle_is_clipped_to_the_selected_display() {
        let screen = DisplayRect::new(3, 0.0, 0.0, 300.0, 300.0, 1.0);
        let rect = capture_rect(PetPoint::new(100.0, 100.0), 220.0, screen);

        assert_eq!(rect.x, 74.0);
        assert_eq!(rect.y, 74.0);
        assert_eq!(rect.width, 226.0);
        assert_eq!(rect.height, 226.0);
        assert_eq!(rect.pixel_width, 226);
        assert_eq!(rect.pixel_height, 226);
    }

    #[test]
    fn wholly_disconnected_capture_has_an_empty_rect_at_the_display_edge() {
        let screen = DisplayRect::new(3, 0.0, 0.0, 300.0, 300.0, 1.0);
        let rect = capture_rect(PetPoint::new(5000.0, 5000.0), 220.0, screen);

        assert_eq!(rect.x, 300.0);
        assert_eq!(rect.y, 300.0);
        assert_eq!(rect.width, 0.0);
        assert_eq!(rect.height, 0.0);
    }
}

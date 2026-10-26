use kurbo::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Red,
    Yellow,
    Green,
    Blue,
    Black,
}

impl Color {
    #[cfg_attr(not(test), allow(dead_code))] // only the tests iterate the palette
    pub const ALL: [Color; 5] = [Color::Red, Color::Yellow, Color::Green, Color::Blue, Color::Black];

    pub fn rgb(self) -> (f64, f64, f64) {
        let hex = |r: u8, g: u8, b: u8| (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        match self {
            Color::Red => hex(0xe0, 0x1b, 0x24),
            Color::Yellow => hex(0xf6, 0xd3, 0x2d),
            Color::Green => hex(0x33, 0xd1, 0x7a),
            Color::Blue => hex(0x35, 0x84, 0xe4),
            Color::Black => hex(0, 0, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Width {
    Thin,
    #[default]
    Medium,
    Thick,
}

impl Width {
    pub fn px(self) -> f64 {
        match self {
            Width::Thin => 2.0,
            Width::Medium => 4.0,
            Width::Thick => 8.0,
        }
    }

    /// Radius of a numbered marker drawn at this width.
    pub fn marker_radius(self) -> f64 {
        match self {
            Width::Thin => 12.0,
            Width::Medium => 16.0,
            Width::Thick => 22.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Color,
    pub width: Width,
}

pub type ItemId = u64;

/// `Rect` values are always normalized (x0 < x1, y0 < y1).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect { rect: Rect, style: Style },
    Arrow { start: Point, end: Point, style: Style },
    Text { origin: Point, text: String, color: Color },
    Redact { rect: Rect, color: Color },
    /// Highlighter-like tint, drawn under all other annotations.
    Highlight { rect: Rect, color: Color },
    /// Numbered circle. Its number is not stored: it is the marker's position
    /// among the markers in the item list (see `render::draw_items`).
    Marker { center: Point, style: Style },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub shape: Shape,
}

pub const TEXT_FONT_PX: f64 = 16.0;
pub const TEXT_PADDING: f64 = 6.0;
pub const TEXT_BORDER: f64 = 2.0;
pub const MIN_SIZE: f64 = 3.0;

pub trait TextMeasure {
    /// Size of the laid-out text, without padding.
    fn text_size(&self, text: &str) -> Size;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_and_widths() {
        assert_eq!(Color::default(), Color::Red);
        assert_eq!(Color::ALL.len(), 5);
        let (r, g, b) = Color::Red.rgb();
        assert_eq!((r * 255.0).round(), 224.0);
        assert_eq!((g * 255.0).round(), 27.0);
        assert_eq!((b * 255.0).round(), 36.0);
        assert_eq!(Color::Black.rgb(), (0.0, 0.0, 0.0));
        assert_eq!(
            [Width::Thin.px(), Width::Medium.px(), Width::Thick.px()],
            [2.0, 4.0, 8.0]
        );
        assert_eq!(Width::default(), Width::Medium);
    }

    #[test]
    fn marker_radius_per_width() {
        assert_eq!(
            [Width::Thin.marker_radius(), Width::Medium.marker_radius(), Width::Thick.marker_radius()],
            [12.0, 16.0, 22.0]
        );
    }
}

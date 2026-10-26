use crate::geometry::{bounds, handles};
use crate::model::{
    Item, ItemId, Shape, TextMeasure, TEXT_BORDER, TEXT_FONT_PX, TEXT_PADDING,
};
use gtk::cairo;
use gtk::pango;
use kurbo::{Point, Size};

pub struct PangoMeasure {
    cr: cairo::Context,
}

impl PangoMeasure {
    pub fn new() -> Self {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1)
            .expect("create 1x1 surface");
        let cr = cairo::Context::new(&surface).expect("create cairo context");
        Self { cr }
    }
}

impl TextMeasure for PangoMeasure {
    fn text_size(&self, text: &str) -> Size {
        let layout = make_layout(&self.cr, text);
        let (w, h) = layout.size();
        Size::new(w as f64 / pango::SCALE as f64, h as f64 / pango::SCALE as f64)
    }
}

fn make_layout(cr: &cairo::Context, text: &str) -> pango::Layout {
    let layout = pangocairo::functions::create_layout(cr);
    // Unhinted metrics scale linearly, so text measured on an identity context
    // fits the same box when drawn under the canvas view scale.
    if let Ok(mut opts) = cairo::FontOptions::new() {
        opts.set_hint_metrics(cairo::HintMetrics::Off);
        opts.set_hint_style(cairo::HintStyle::None);
        pangocairo::functions::context_set_font_options(&layout.context(), Some(&opts));
        layout.context_changed();
    }
    let mut font = pango::FontDescription::from_string("Sans Bold");
    font.set_absolute_size(TEXT_FONT_PX * pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_text(text);
    layout
}

fn set_color(cr: &cairo::Context, color: crate::model::Color) {
    let (r, g, b) = color.rgb();
    cr.set_source_rgb(r, g, b);
}

fn rect_path(cr: &cairo::Context, r: kurbo::Rect) {
    cr.rectangle(r.x0, r.y0, r.width(), r.height());
}

pub fn draw_shape(cr: &cairo::Context, shape: &Shape, m: &PangoMeasure) {
    let _ = cr.save();
    cr.set_line_join(cairo::LineJoin::Round);
    cr.set_line_cap(cairo::LineCap::Round);
    match shape {
        Shape::Rect { rect, style } => {
            set_color(cr, style.color);
            cr.set_line_width(style.width.px());
            rect_path(cr, *rect);
            let _ = cr.stroke();
        }
        Shape::Redact { rect, color } => {
            set_color(cr, *color);
            rect_path(cr, *rect);
            let _ = cr.fill();
        }
        Shape::Arrow { start, end, style } => {
            let w = style.width.px();
            let d = *end - *start;
            let len = d.hypot();
            if len > 0.0 {
                let dir = d / len;
                let head = (4.0 * w).max(12.0);
                let half = std::f64::consts::FRAC_PI_6;
                let rot = |a: f64| kurbo::Vec2::new(dir.x * a.cos() - dir.y * a.sin(), dir.x * a.sin() + dir.y * a.cos());
                let c1 = *end - rot(half) * head;
                let c2 = *end - rot(-half) * head;
                let base: Point = *end - dir * (head * half.cos());
                set_color(cr, style.color);
                cr.set_line_width(w);
                // Shaft stops at the head's base (clamped to the start for tiny arrows).
                let shaft_end = if (base - *start).dot(dir) > 0.0 { base } else { *start };
                cr.move_to(start.x, start.y);
                cr.line_to(shaft_end.x, shaft_end.y);
                let _ = cr.stroke();
                cr.move_to(end.x, end.y);
                cr.line_to(c1.x, c1.y);
                cr.line_to(c2.x, c2.y);
                cr.close_path();
                let _ = cr.fill();
            }
        }
        Shape::Text { origin, text, color } => {
            let b = bounds(shape, m);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            rect_path(cr, b);
            let _ = cr.fill();
            set_color(cr, *color);
            cr.set_line_width(TEXT_BORDER);
            rect_path(cr, b.inflate(-TEXT_BORDER / 2.0, -TEXT_BORDER / 2.0));
            let _ = cr.stroke();
            cr.move_to(origin.x + TEXT_PADDING, origin.y + TEXT_PADDING);
            pangocairo::functions::show_layout(cr, &make_layout(cr, text));
        }
    }
    let _ = cr.restore();
}

pub fn draw_items(cr: &cairo::Context, items: &[Item], skip: Option<ItemId>, m: &PangoMeasure) {
    for item in items.iter().filter(|i| Some(i.id) != skip) {
        draw_shape(cr, &item.shape, m);
    }
}

pub fn draw_selection(cr: &cairo::Context, shape: &Shape, scale: f64, m: &PangoMeasure) {
    let _ = cr.save();
    let px = 1.0 / scale;
    cr.set_line_width(px);
    cr.set_dash(&[4.0 * px, 4.0 * px], 0.0);
    cr.set_source_rgb(0.2, 0.2, 0.2);
    rect_path(cr, bounds(shape, m));
    let _ = cr.stroke();
    cr.set_dash(&[], 0.0);
    let size = 8.0 * px;
    for (_, p) in handles(shape) {
        cr.rectangle(p.x - size / 2.0, p.y - size / 2.0, size, size);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.fill_preserve();
        cr.set_source_rgb(0.2, 0.2, 0.2);
        let _ = cr.stroke();
    }
    let _ = cr.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Style, Width};
    use kurbo::{Point, Rect};

    fn white_surface() -> cairo::ImageSurface {
        let s = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();
        let cr = cairo::Context::new(&s).unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().unwrap();
        s
    }

    fn pixel(s: &mut cairo::ImageSurface, x: usize, y: usize) -> (u8, u8, u8) {
        s.flush();
        let stride = s.stride() as usize;
        let d = s.data().unwrap();
        let i = y * stride + x * 4;
        (d[i + 2], d[i + 1], d[i])
    }

    fn near(a: (u8, u8, u8), b: (u8, u8, u8)) -> bool {
        let f = |x: u8, y: u8| (x as i32 - y as i32).abs() <= 6;
        f(a.0, b.0) && f(a.1, b.1) && f(a.2, b.2)
    }

    const RED: (u8, u8, u8) = (0xe0, 0x1b, 0x24);
    const WHITE: (u8, u8, u8) = (255, 255, 255);

    fn render(shape: Shape) -> cairo::ImageSurface {
        let s = white_surface();
        {
            let cr = cairo::Context::new(&s).unwrap();
            draw_shape(&cr, &shape, &PangoMeasure::new());
        }
        s.flush();
        s
    }

    #[test]
    fn redact_fills_opaquely() {
        let mut s = render(Shape::Redact { rect: Rect::new(20., 20., 80., 80.), color: Color::Black });
        assert_eq!(pixel(&mut s, 50, 50), (0, 0, 0));
    }

    #[test]
    fn rect_is_outline_only() {
        let mut s = render(Shape::Rect {
            rect: Rect::new(20., 20., 180., 180.),
            style: Style { color: Color::Red, width: Width::Medium },
        });
        assert!(near(pixel(&mut s, 20, 100), RED));
        assert_eq!(pixel(&mut s, 100, 100), WHITE);
    }

    #[test]
    fn arrow_head_at_end() {
        let mut s = render(Shape::Arrow {
            start: Point::new(20., 100.),
            end: Point::new(180., 100.),
            style: Style { color: Color::Red, width: Width::Thick },
        });
        assert!(near(pixel(&mut s, 175, 100), RED));
        assert!(near(pixel(&mut s, 164, 106), RED));
        assert!(near(pixel(&mut s, 60, 100), RED));
        assert_eq!(pixel(&mut s, 25, 108), WHITE);
    }

    #[test]
    fn arrow_shaft_does_not_poke_past_tip() {
        let mut s = render(Shape::Arrow {
            start: Point::new(20., 100.),
            end: Point::new(180., 100.),
            style: Style { color: Color::Red, width: Width::Thick },
        });
        assert_eq!(pixel(&mut s, 184, 100), WHITE);
        // beside the head tip, shaft width would be visible here if it reached the tip
        assert_eq!(pixel(&mut s, 178, 97), WHITE);
    }

    #[test]
    fn text_label_has_white_box_and_colored_border() {
        let mut s = render(Shape::Text {
            origin: Point::new(10., 10.),
            text: "Hi".into(),
            color: Color::Blue,
        });
        assert!(near(pixel(&mut s, 11, 11), (0x35, 0x84, 0xe4)));
        assert_eq!(pixel(&mut s, 14, 14), WHITE);
    }

    #[test]
    fn skip_hides_item() {
        let mut s = white_surface();
        {
            let cr = cairo::Context::new(&s).unwrap();
            let items = [Item {
                id: 7,
                shape: Shape::Redact { rect: Rect::new(20., 20., 80., 80.), color: Color::Black },
            }];
            draw_items(&cr, &items, Some(7), &PangoMeasure::new());
        }
        assert_eq!(pixel(&mut s, 50, 50), WHITE);
    }

    #[test]
    fn items_draw_in_order() {
        let mut s = white_surface();
        {
            let cr = cairo::Context::new(&s).unwrap();
            let items = [
                Item { id: 1, shape: Shape::Redact { rect: Rect::new(20., 20., 80., 80.), color: Color::Black } },
                Item { id: 2, shape: Shape::Redact { rect: Rect::new(40., 40., 100., 100.), color: Color::Red } },
            ];
            draw_items(&cr, &items, None, &PangoMeasure::new());
        }
        assert!(near(pixel(&mut s, 50, 50), RED));
        assert_eq!(pixel(&mut s, 30, 30), (0, 0, 0));
    }

    #[test]
    fn selection_draws_handles() {
        let shape = Shape::Redact { rect: Rect::new(50., 50., 150., 150.), color: Color::Red };
        let mut s = white_surface();
        {
            let cr = cairo::Context::new(&s).unwrap();
            draw_selection(&cr, &shape, 1.0, &PangoMeasure::new());
        }
        // handle interior at the top-left corner is white, border is dark
        assert_eq!(pixel(&mut s, 52, 52), WHITE);
        let b = pixel(&mut s, 46, 52);
        assert!(b.0 < 200 && b.1 < 200 && b.2 < 200, "{b:?}");
    }

    #[test]
    fn text_size_is_independent_of_view_scale() {
        let m = PangoMeasure::new();
        let text = "Remove the arrow\nand this WWW label";
        let expected = m.text_size(text);
        for scale in [0.25, 0.37, 0.5, 0.71] {
            let s = cairo::ImageSurface::create(cairo::Format::ARgb32, 10, 10).unwrap();
            let cr = cairo::Context::new(&s).unwrap();
            cr.scale(scale, scale);
            let (w, h) = make_layout(&cr, text).size();
            let w = w as f64 / pango::SCALE as f64;
            let h = h as f64 / pango::SCALE as f64;
            assert!(w <= expected.width + 0.5, "scale {scale}: {w} > {}", expected.width);
            assert!(h <= expected.height + 0.5, "scale {scale}: {h} > {}", expected.height);
        }
    }

    #[test]
    fn pango_measure_grows_with_text() {
        let m = PangoMeasure::new();
        assert!(m.text_size("Hello world").width > m.text_size("Hi").width);
        assert!(m.text_size("a\nb").height > m.text_size("a").height);
    }
}

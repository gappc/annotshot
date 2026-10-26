use crate::geometry::{bounds, handles};
use crate::model::{
    Color, Item, ItemId, Shape, TextMeasure, TEXT_BORDER, TEXT_FONT_PX, TEXT_PADDING,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
    sized_layout(cr, text, TEXT_FONT_PX)
}

/// Layout for a marker's number; the font size equals the circle's radius.
fn marker_layout(cr: &cairo::Context, number: u32, radius: f64) -> pango::Layout {
    sized_layout(cr, &number.to_string(), radius)
}

fn sized_layout(cr: &cairo::Context, text: &str, font_px: f64) -> pango::Layout {
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
    font.set_absolute_size(font_px * pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_text(text);
    layout
}

fn set_color(cr: &cairo::Context, color: crate::model::Color) {
    let (r, g, b) = color.rgb();
    cr.set_source_rgb(r, g, b);
}

/// Highlight tint: yellow as is, other colors halfway to white (black → grey),
/// so text under a multiplied highlight stays readable.
fn highlight_rgb(color: Color) -> (f64, f64, f64) {
    let (r, g, b) = color.rgb();
    match color {
        Color::Yellow => (r, g, b),
        _ => ((1.0 + r) / 2.0, (1.0 + g) / 2.0, (1.0 + b) / 2.0),
    }
}

fn rect_path(cr: &cairo::Context, r: kurbo::Rect) {
    cr.rectangle(r.x0, r.y0, r.width(), r.height());
}

/// Draw one shape. `number` is the label of a marker and ignored otherwise.
/// Pixelations need the screenshot and are drawn by `draw_items`.
fn draw_shape(cr: &cairo::Context, shape: &Shape, number: u32, m: &PangoMeasure) {
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
        Shape::Pixelate { .. } => {}
        Shape::Highlight { rect, color } => {
            let (r, g, b) = highlight_rgb(*color);
            cr.set_source_rgb(r, g, b);
            cr.set_operator(cairo::Operator::Multiply);
            rect_path(cr, *rect);
            let _ = cr.fill();
        }
        Shape::Marker { center, style } => {
            let r = style.width.marker_radius();
            set_color(cr, style.color);
            cr.arc(center.x, center.y, r, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
            // Dark digits on yellow, white on the other colors.
            match style.color {
                Color::Yellow => cr.set_source_rgb(0.0, 0.0, 0.0),
                _ => cr.set_source_rgb(1.0, 1.0, 1.0),
            }
            let layout = marker_layout(cr, number, r);
            let (ink, _) = layout.extents();
            let s = pango::SCALE as f64;
            let (x, y) = (ink.x() as f64 / s, ink.y() as f64 / s);
            let (w, h) = (ink.width() as f64 / s, ink.height() as f64 / s);
            cr.move_to(center.x - x - w / 2.0, center.y - y - h / 2.0);
            pangocairo::functions::show_layout(cr, &layout);
        }
    }
    let _ = cr.restore();
}

/// Average colors of the whole cells of a `block` grid over one screenshot.
struct CellGrid {
    cols: usize,
    avg: Vec<[u8; 3]>,
}

impl CellGrid {
    fn build(base: &cairo::ImageSurface, block: usize) -> CellGrid {
        let cols = base.width() as usize / block;
        let rows = base.height() as usize / block;
        let stride = base.stride() as usize;
        let n = (block * block) as u32;
        let mut avg = Vec::with_capacity(cols * rows);
        let _ = base.with_data(|src| {
            let mut sums = vec![[0u32; 3]; cols];
            for row in 0..rows {
                sums.fill([0; 3]);
                for y in row * block..(row + 1) * block {
                    let line = &src[y * stride..y * stride + cols * block * 4];
                    for (sum, cell) in sums.iter_mut().zip(line.chunks_exact(block * 4)) {
                        for px in cell.as_chunks::<4>().0 {
                            sum[0] += px[0] as u32;
                            sum[1] += px[1] as u32;
                            sum[2] += px[2] as u32;
                        }
                    }
                }
                avg.extend(sums.iter().map(|s| s.map(|v| ((v + n / 2) / n) as u8)));
            }
        });
        CellGrid { cols, avg }
    }
}

/// Block colors of the current screenshot, computed once per block size, so
/// redrawing a pixelation (even while it is dragged) only has to work out the
/// partial cells along its edges. Resets itself when the screenshot changes.
#[derive(Default)]
pub struct PixelCache {
    inner: RefCell<Option<CachedScreenshot>>,
}

struct CachedScreenshot {
    surface: cairo::ImageSurface,
    grids: HashMap<usize, Rc<CellGrid>>,
}

impl PixelCache {
    fn grid(&self, base: &cairo::ImageSurface, block: usize) -> Rc<CellGrid> {
        let mut inner = self.inner.borrow_mut();
        // Holding a reference to the old surface keeps its address from being
        // reused by a new one, so the pointer comparison is safe.
        if inner.as_ref().is_none_or(|c| c.surface.to_raw_none() != base.to_raw_none()) {
            *inner = Some(CachedScreenshot { surface: base.clone(), grids: HashMap::new() });
        }
        let grids = &mut inner.as_mut().expect("just set").grids;
        grids.entry(block).or_insert_with(|| Rc::new(CellGrid::build(base, block))).clone()
    }
}

/// Replace `rect` with blocks of `base` averaged over a `block`-sized grid
/// anchored at the image origin (so blocks don't shift while the rect moves).
/// Averages only use pixels inside the rect. With a `cache`, whole cells come
/// from it and only cells cut by the rect's edges are summed here.
fn draw_pixelate(
    cr: &cairo::Context,
    rect: kurbo::Rect,
    block: usize,
    base: &cairo::ImageSurface,
    cache: Option<&PixelCache>,
) {
    let clamp = |v: f64, max: i32| (v.round() as i64).clamp(0, max as i64) as usize;
    let (x0, x1) = (clamp(rect.x0, base.width()), clamp(rect.x1, base.width()));
    let (y0, y1) = (clamp(rect.y0, base.height()), clamp(rect.y1, base.height()));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let (gx, gy) = (x0 / block * block, y0 / block * block);
    let (cols, rows) = ((x1 - gx).div_ceil(block), (y1 - gy).div_ceil(block));
    let Ok(mut grid) = cairo::ImageSurface::create(cairo::Format::Rgb24, cols as i32, rows as i32) else {
        return;
    };
    let cells = cache.map(|c| c.grid(base, block));
    let src_stride = base.stride() as usize;
    let dst_stride = grid.stride() as usize;
    {
        let Ok(mut dst) = grid.data() else { return };
        let _ = base.with_data(|src| {
            for row in 0..rows {
                let (cy0, cy1) = ((gy + row * block).max(y0), (gy + (row + 1) * block).min(y1));
                for col in 0..cols {
                    let (cx0, cx1) = ((gx + col * block).max(x0), (gx + (col + 1) * block).min(x1));
                    let whole = cx1 - cx0 == block && cy1 - cy0 == block;
                    let color = match &cells {
                        Some(g) if whole => g.avg[(cy0 / block) * g.cols + cx0 / block],
                        _ => {
                            let mut sum = [0usize; 3];
                            for y in cy0..cy1 {
                                for x in cx0..cx1 {
                                    let i = y * src_stride + x * 4;
                                    for (c, s) in sum.iter_mut().enumerate() {
                                        *s += src[i + c] as usize;
                                    }
                                }
                            }
                            let n = (cy1 - cy0) * (cx1 - cx0);
                            sum.map(|v| ((v + n / 2) / n) as u8)
                        }
                    };
                    let o = row * dst_stride + col * 4;
                    dst[o..o + 3].copy_from_slice(&color);
                }
            }
        });
    }
    let _ = cr.save();
    cr.rectangle(x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64);
    cr.clip();
    cr.translate(gx as f64, gy as f64);
    cr.scale(block as f64, block as f64);
    if cr.set_source_surface(&grid, 0.0, 0.0).is_ok() {
        cr.source().set_filter(cairo::Filter::Nearest);
        let _ = cr.paint();
    }
    let _ = cr.restore();
}

/// Draw all items over the screenshot `base` (already painted). Layers:
/// pixelations (made from `base` alone), then highlights, then everything
/// else in list order. `skip` hides one item (a label being edited).
/// `cache` speeds up repeated pixelation of the same screenshot.
pub fn draw_items(
    cr: &cairo::Context,
    items: &[Item],
    skip: Option<ItemId>,
    base: &cairo::ImageSurface,
    cache: Option<&PixelCache>,
    m: &PangoMeasure,
) {
    let drawn = |i: &&Item| Some(i.id) != skip;
    for item in items.iter().filter(drawn) {
        if let Shape::Pixelate { rect, width } = item.shape {
            draw_pixelate(cr, rect, width.pixel_block(), base, cache);
        }
    }
    for item in items.iter().filter(drawn) {
        if let Shape::Highlight { .. } = item.shape {
            draw_shape(cr, &item.shape, 0, m);
        }
    }
    let mut markers = 0;
    for item in items.iter().filter(|i| !i.shape.is_underlay()) {
        if let Shape::Marker { .. } = item.shape {
            markers += 1;
        }
        if drawn(&item) {
            draw_shape(cr, &item.shape, markers, m);
        }
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
            draw_shape(&cr, &shape, 1, &PangoMeasure::new());
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
            draw_items(&cr, &items, Some(7), &white_surface(), None, &PangoMeasure::new());
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
            draw_items(&cr, &items, None, &white_surface(), None, &PangoMeasure::new());
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

    fn count(s: &mut cairo::ImageSurface, x0: usize, y0: usize, x1: usize, y1: usize, f: impl Fn((u8, u8, u8)) -> bool) -> usize {
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                if f(pixel(s, x, y)) {
                    n += 1;
                }
            }
        }
        n
    }

    fn marker(x: f64, y: f64, color: Color, width: Width) -> Shape {
        Shape::Marker { center: Point::new(x, y), style: Style { color, width } }
    }

    #[test]
    fn marker_fills_circle_with_white_number() {
        let mut s = render(marker(100., 100., Color::Red, Width::Thick));
        assert!(near(pixel(&mut s, 100, 81), RED)); // inside, above the digit
        assert!(near(pixel(&mut s, 81, 100), RED)); // inside, left of the digit
        assert_eq!(pixel(&mut s, 125, 100), WHITE); // outside
        assert_eq!(pixel(&mut s, 117, 117), WHITE); // bounds corner, outside the circle
        let digit = count(&mut s, 92, 90, 108, 110, |p| p.0 > 240 && p.1 > 240 && p.2 > 240);
        assert!(digit > 10, "white digit pixels: {digit}");
    }

    #[test]
    fn yellow_marker_uses_dark_number() {
        let mut s = render(marker(100., 100., Color::Yellow, Width::Thick));
        let dark = count(&mut s, 92, 90, 108, 110, |p| p.0 < 80 && p.1 < 80 && p.2 < 80);
        assert!(dark > 10, "dark digit pixels: {dark}");
    }

    #[test]
    fn two_digit_numbers_fit_the_circle() {
        let s = cairo::ImageSurface::create(cairo::Format::ARgb32, 10, 10).unwrap();
        let cr = cairo::Context::new(&s).unwrap();
        for w in [Width::Thin, Width::Medium, Width::Thick] {
            let r = w.marker_radius();
            let (tw, th) = marker_layout(&cr, 88, r).pixel_size();
            assert!((tw as f64) < 2.0 * r * 0.85, "{w:?}: text {tw} wide, radius {r}");
            assert!((th as f64) < 2.0 * r, "{w:?}: text {th} high, radius {r}");
        }
    }

    fn region(s: &mut cairo::ImageSurface, x0: usize, y0: usize, x1: usize, y1: usize) -> Vec<(u8, u8, u8)> {
        (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| pixel(s, x, y)).collect()
    }

    #[test]
    fn draw_items_numbers_markers_in_order() {
        let red_box = Shape::Redact { rect: Rect::new(0., 0., 10., 10.), color: Color::Red };
        let items = [
            Item { id: 1, shape: marker(40., 40., Color::Red, Width::Thick) },
            Item { id: 2, shape: red_box },
            Item { id: 3, shape: marker(140., 140., Color::Red, Width::Thick) },
        ];
        let mut listed = white_surface();
        {
            let cr = cairo::Context::new(&listed).unwrap();
            draw_items(&cr, &items, None, &white_surface(), None, &PangoMeasure::new());
        }
        let expect = |n: u32| {
            let mut s = white_surface();
            {
                let cr = cairo::Context::new(&s).unwrap();
                draw_shape(&cr, &items[2].shape, n, &PangoMeasure::new());
            }
            region(&mut s, 120, 120, 160, 160)
        };
        let got = region(&mut listed, 120, 120, 160, 160);
        assert_eq!(got, expect(2));
        assert_ne!(got, expect(3));
    }

    fn highlight(color: Color) -> Shape {
        Shape::Highlight { rect: Rect::new(20., 20., 180., 180.), color }
    }

    fn render_on_black_stripe(shape: &Shape) -> cairo::ImageSurface {
        let mut s = white_surface();
        {
            let cr = cairo::Context::new(&s).unwrap();
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.rectangle(0., 90., 200., 20.);
            cr.fill().unwrap();
            draw_shape(&cr, shape, 1, &PangoMeasure::new());
        }
        s.flush();
        let _ = pixel(&mut s, 0, 0);
        s
    }

    #[test]
    fn highlight_multiplies() {
        let mut s = render_on_black_stripe(&highlight(Color::Yellow));
        assert!(near(pixel(&mut s, 50, 50), (0xf6, 0xd3, 0x2d))); // white → yellow
        assert_eq!(pixel(&mut s, 50, 100), (0, 0, 0)); // dark text stays dark
        assert_eq!(pixel(&mut s, 10, 10), WHITE); // outside
    }

    #[test]
    fn non_yellow_highlight_is_lighter() {
        let mut s = render_on_black_stripe(&highlight(Color::Red));
        assert!(near(pixel(&mut s, 50, 50), (0xf0, 0x8d, 0x92)), "{:?}", pixel(&mut s, 50, 50));
        let mut s = render_on_black_stripe(&highlight(Color::Black));
        assert!(near(pixel(&mut s, 50, 50), (128, 128, 128)), "{:?}", pixel(&mut s, 50, 50));
    }

    #[test]
    fn highlights_draw_under_other_items() {
        let items = [
            Item { id: 1, shape: Shape::Redact { rect: Rect::new(40., 40., 100., 100.), color: Color::Blue } },
            Item { id: 2, shape: highlight(Color::Yellow) },
        ];
        let mut s = white_surface();
        {
            let cr = cairo::Context::new(&s).unwrap();
            draw_items(&cr, &items, None, &white_surface(), None, &PangoMeasure::new());
        }
        assert!(near(pixel(&mut s, 70, 70), (0x35, 0x84, 0xe4))); // redaction untinted
        assert!(near(pixel(&mut s, 150, 150), (0xf6, 0xd3, 0x2d)));
    }

    /// 40x40 screenshot: black for x < 15, white from x = 15.
    fn split_base() -> cairo::ImageSurface {
        let s = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
        let cr = cairo::Context::new(&s).unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().unwrap();
        cr.set_source_rgb(0.0, 0.0, 0.0);
        cr.rectangle(0., 0., 15., 40.);
        cr.fill().unwrap();
        drop(cr);
        s
    }

    /// Paint `base`, then draw `items` on it, like the export does.
    fn flatten(base: &cairo::ImageSurface, items: &[Item]) -> cairo::ImageSurface {
        let out = cairo::ImageSurface::create(cairo::Format::ARgb32, base.width(), base.height()).unwrap();
        {
            let cr = cairo::Context::new(&out).unwrap();
            cr.set_source_surface(base, 0., 0.).unwrap();
            cr.paint().unwrap();
            draw_items(&cr, items, None, base, None, &PangoMeasure::new());
        }
        out
    }

    fn pixelate(rect: Rect) -> Item {
        Item { id: 1, shape: Shape::Pixelate { rect, width: Width::Medium } }
    }

    #[test]
    fn pixelate_averages_blocks_on_image_grid() {
        let base = split_base();
        let mut s = flatten(&base, &[pixelate(Rect::new(3., 3., 40., 40.))]);
        // Blocks sit on a 10 px grid from the image origin, not from the rect.
        assert_eq!(pixel(&mut s, 5, 5), (0, 0, 0));
        let grey = pixel(&mut s, 12, 12); // block 10..20: half black, half white
        assert!(near(grey, (128, 128, 128)), "{grey:?}");
        assert_eq!(pixel(&mut s, 19, 30), grey);
        assert_eq!(pixel(&mut s, 25, 25), WHITE);
        // Outside the rect the screenshot is untouched.
        assert_eq!(pixel(&mut s, 14, 1), (0, 0, 0));
        assert_eq!(pixel(&mut s, 16, 1), WHITE);
    }

    #[test]
    fn pixelate_reads_screenshot_not_annotations() {
        let base = split_base();
        let blue = Item { id: 2, shape: Shape::Redact { rect: Rect::new(0., 0., 40., 10.), color: Color::Blue } };
        let mut s = flatten(&base, &[blue, pixelate(Rect::new(0., 0., 40., 40.))]);
        assert!(near(pixel(&mut s, 12, 5), (0x35, 0x84, 0xe4))); // redaction intact on top
        assert!(near(pixel(&mut s, 12, 15), (128, 128, 128))); // blue not mixed in
    }

    #[test]
    fn pixelate_clips_to_image_and_ignores_empty_rects() {
        let base = split_base();
        let mut s = flatten(&base, &[pixelate(Rect::new(-20., -20., 1000., 15.)), pixelate(Rect::new(5., 30., 5., 30.))]);
        assert!(near(pixel(&mut s, 12, 5), (128, 128, 128)));
        assert_eq!(pixel(&mut s, 12, 30), (0, 0, 0)); // below the rect, empty rect drew nothing
    }

    #[test]
    #[ignore] // timing probe: cargo test --release -- --ignored --nocapture pixelate_4k
    fn pixelate_4k_timing() {
        let base = cairo::ImageSurface::create(cairo::Format::ARgb32, 3840, 2160).unwrap();
        let out = cairo::ImageSurface::create(cairo::Format::ARgb32, 3840, 2160).unwrap();
        let cr = cairo::Context::new(&out).unwrap();
        let cache = PixelCache::default();
        for w in [Width::Thin, Width::Thick] {
            let t = std::time::Instant::now();
            draw_pixelate(&cr, Rect::new(0., 0., 1., 1.), w.pixel_block(), &base, Some(&cache));
            eprintln!("{w:?} cache build: {:.1} ms (once per paste and block size)", t.elapsed().as_secs_f64() * 1000.0);
            for (name, c) in [("direct", None), ("cached", Some(&cache))] {
                let t = std::time::Instant::now();
                for i in 0..10 {
                    // A slightly different rect each time, like a drag.
                    let r = Rect::new(i as f64, i as f64, 3840. - i as f64, 2160.);
                    draw_pixelate(&cr, r, w.pixel_block(), &base, c);
                }
                eprintln!("{w:?} {name}: {:.1} ms per full-screen pixelation", t.elapsed().as_secs_f64() * 100.0);
            }
        }
    }

    /// Screenshot with fine, irregular detail so every block average differs.
    fn noisy_base(w: i32, h: i32, seed: usize) -> cairo::ImageSurface {
        let mut s = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).unwrap();
        let stride = s.stride() as usize;
        {
            let mut d = s.data().unwrap();
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let i = y * stride + x * 4;
                    let v = (x * 31 + y * 17 + seed) * 2654435761 % 251;
                    d[i..i + 4].copy_from_slice(&[v as u8, (v * 3 % 256) as u8, (255 - v) as u8, 255]);
                }
            }
        }
        s
    }

    fn pixelated(base: &cairo::ImageSurface, rect: Rect, block: usize, cache: Option<&PixelCache>) -> Vec<u8> {
        let mut out = cairo::ImageSurface::create(cairo::Format::ARgb32, base.width(), base.height()).unwrap();
        {
            let cr = cairo::Context::new(&out).unwrap();
            draw_pixelate(&cr, rect, block, base, cache);
        }
        out.flush();
        out.data().unwrap().to_vec()
    }

    #[test]
    fn cached_pixelation_matches_direct() {
        let base = noisy_base(97, 61, 0);
        let cache = PixelCache::default();
        let rects = [
            Rect::new(0., 0., 97., 61.),
            Rect::new(3.4, 7.6, 55., 41.),
            Rect::new(-10., 20., 200., 25.),
            Rect::new(12., 12., 18., 18.), // inside one cell
        ];
        for block in [6, 10, 16] {
            for r in rects {
                for _ in 0..2 {
                    assert_eq!(pixelated(&base, r, block, Some(&cache)), pixelated(&base, r, block, None), "{block} {r:?}");
                }
            }
        }
    }

    #[test]
    fn pixel_cache_follows_a_new_screenshot() {
        let cache = PixelCache::default();
        let r = Rect::new(0., 0., 60., 40.);
        let a = noisy_base(60, 40, 0);
        let _ = pixelated(&a, r, 10, Some(&cache));
        let b = noisy_base(60, 40, 5);
        assert_eq!(pixelated(&b, r, 10, Some(&cache)), pixelated(&b, r, 10, None));
    }
}

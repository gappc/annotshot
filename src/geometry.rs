use crate::model::{Shape, TextMeasure, TEXT_PADDING};
use kurbo::{Line, ParamCurveNearest, Point, Rect, Vec2};

pub fn bounds(shape: &Shape, m: &dyn TextMeasure) -> Rect {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } => *rect,
        Shape::Arrow { start, end, .. } => Rect::from_points(*start, *end),
        Shape::Marker { center, style } => {
            let r = style.width.marker_radius();
            Rect::new(center.x - r, center.y - r, center.x + r, center.y + r)
        }
        Shape::Text { origin, text, .. } => {
            let s = m.text_size(text);
            Rect::from_origin_size(
                *origin,
                (s.width + 2.0 * TEXT_PADDING, s.height + 2.0 * TEXT_PADDING),
            )
        }
    }
}

pub fn hit_test(shape: &Shape, p: Point, tol: f64, m: &dyn TextMeasure) -> bool {
    match shape {
        Shape::Rect { rect, style } => {
            let reach = tol + style.width.px() / 2.0;
            let outer = rect.inflate(reach, reach);
            let inner = rect.inflate(-reach, -reach);
            outer.contains(p) && !(inner.width() > 0.0 && inner.height() > 0.0 && inner.contains(p))
        }
        Shape::Arrow { start, end, style } => {
            let d = Line::new(*start, *end).nearest(p, 1e-6).distance_sq.sqrt();
            d <= tol + style.width.px() / 2.0
        }
        Shape::Marker { center, style } => center.distance(p) <= style.width.marker_radius() + tol,
        Shape::Text { .. } | Shape::Redact { .. } | Shape::Highlight { .. } => {
            bounds(shape, m).inflate(tol, tol).contains(p)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Start,
    End,
    // Sides of boxes and redactions: grabbed anywhere along the line.
    Top,
    Bottom,
    Left,
    Right,
    /// Inside a marker: moves it.
    Center,
}

pub fn handles(shape: &Shape) -> Vec<(Handle, Point)> {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } => vec![
            (Handle::TopLeft, Point::new(rect.x0, rect.y0)),
            (Handle::TopRight, Point::new(rect.x1, rect.y0)),
            (Handle::BottomLeft, Point::new(rect.x0, rect.y1)),
            (Handle::BottomRight, Point::new(rect.x1, rect.y1)),
        ],
        Shape::Arrow { start, end, .. } => vec![(Handle::Start, *start), (Handle::End, *end)],
        Shape::Text { .. } | Shape::Marker { .. } => vec![],
    }
}

pub fn handle_at(shape: &Shape, p: Point, size: f64) -> Option<Handle> {
    let half = size / 2.0;
    handles(shape)
        .into_iter()
        .find(|(_, hp)| (p.x - hp.x).abs() <= half && (p.y - hp.y).abs() <= half)
        .map(|(h, _)| h)
}

/// What part of `shape` can be dragged at `p`: a handle (corner or arrow end,
/// within a `handle_size` square) first, else a side of a box or redaction
/// (within `tol` of the line, plus half the stroke for boxes).
pub fn grab_at(shape: &Shape, p: Point, handle_size: f64, tol: f64) -> Option<Handle> {
    if let Some(h) = handle_at(shape, p, handle_size) {
        return Some(h);
    }
    let (rect, reach) = match shape {
        Shape::Rect { rect, style } => (rect, tol + style.width.px() / 2.0),
        Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } => (rect, tol),
        Shape::Marker { center, style } => {
            return (center.distance(p) <= style.width.marker_radius()).then_some(Handle::Center);
        }
        _ => return None,
    };
    let along_x = p.x >= rect.x0 - reach && p.x <= rect.x1 + reach;
    let along_y = p.y >= rect.y0 - reach && p.y <= rect.y1 + reach;
    let near = |a: f64, b: f64| (a - b).abs() <= reach;
    if along_x && near(p.y, rect.y0) {
        Some(Handle::Top)
    } else if along_x && near(p.y, rect.y1) {
        Some(Handle::Bottom)
    } else if along_y && near(p.x, rect.x0) {
        Some(Handle::Left)
    } else if along_y && near(p.x, rect.x1) {
        Some(Handle::Right)
    } else {
        None
    }
}

pub fn translate(shape: &mut Shape, d: Vec2) {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } => *rect = *rect + d,
        Shape::Arrow { start, end, .. } => {
            *start += d;
            *end += d;
        }
        Shape::Text { origin, .. } => *origin += d,
        Shape::Marker { center, .. } => *center += d,
    }
}

pub fn drag_handle(shape: &mut Shape, h: Handle, p: Point) {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } => {
            let mut r = *rect;
            match h {
                Handle::TopLeft => (r.x0, r.y0) = (p.x, p.y),
                Handle::TopRight => (r.x1, r.y0) = (p.x, p.y),
                Handle::BottomLeft => (r.x0, r.y1) = (p.x, p.y),
                Handle::BottomRight => (r.x1, r.y1) = (p.x, p.y),
                Handle::Top => r.y0 = p.y,
                Handle::Bottom => r.y1 = p.y,
                Handle::Left => r.x0 = p.x,
                Handle::Right => r.x1 = p.x,
                Handle::Start | Handle::End | Handle::Center => {}
            }
            *rect = r.abs();
        }
        Shape::Arrow { start, end, .. } => match h {
            Handle::Start => *start = p,
            Handle::End => *end = p,
            _ => {}
        },
        Shape::Text { .. } | Shape::Marker { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Style};
    use kurbo::Size;

    struct FixedMeasure(Size);
    impl TextMeasure for FixedMeasure {
        fn text_size(&self, _: &str) -> Size {
            self.0
        }
    }
    const M: FixedMeasure = FixedMeasure(Size::new(20., 16.));

    fn text_shape() -> Shape {
        Shape::Text { origin: Point::new(5., 5.), text: "hi".into(), color: Color::Red }
    }

    fn marker() -> Shape {
        Shape::Marker { center: Point::new(50., 50.), style: Style::default() }
    }

    #[test]
    fn marker_bounds_and_hit() {
        // Medium marker: radius 16.
        assert_eq!(bounds(&marker(), &M), Rect::new(34., 34., 66., 66.));
        assert!(hit_test(&marker(), Point::new(50., 50.), 0., &M));
        assert!(hit_test(&marker(), Point::new(70., 50.), 6., &M));
        assert!(!hit_test(&marker(), Point::new(64., 64.), 0., &M)); // bounds corner, outside circle
    }

    #[test]
    fn marker_grab_is_center_inside_circle() {
        assert!(handles(&marker()).is_empty());
        assert_eq!(grab_at(&marker(), Point::new(60., 55.), 8., 6.), Some(Handle::Center));
        assert_eq!(grab_at(&marker(), Point::new(90., 50.), 8., 6.), None);
        let mut m = marker();
        drag_handle(&mut m, Handle::Center, Point::new(0., 0.)); // moving is the editor's job
        assert_eq!(m, marker());
        translate(&mut m, Vec2::new(3., 4.));
        assert!(matches!(m, Shape::Marker { center, .. } if center == Point::new(53., 54.)));
    }

    #[test]
    fn highlight_behaves_like_redact() {
        let h = Shape::Highlight { rect: Rect::new(0., 0., 50., 50.), color: Color::Yellow };
        assert_eq!(bounds(&h, &M), Rect::new(0., 0., 50., 50.));
        assert!(hit_test(&h, Point::new(25., 25.), 0., &M));
        assert_eq!(grab_at(&h, Point::new(25., 49.), 8., 6.), Some(Handle::Bottom));
        assert_eq!(handles(&h).len(), 4);
        let mut m = h.clone();
        drag_handle(&mut m, Handle::BottomRight, Point::new(80., 70.));
        translate(&mut m, Vec2::new(1., 2.));
        assert_eq!(m, Shape::Highlight { rect: Rect::new(1., 2., 81., 72.), color: Color::Yellow });
    }

    #[test]
    fn grab_prefers_corners_then_sides() {
        let s = Shape::Rect { rect: Rect::new(10., 10., 110., 60.), style: Style::default() };
        assert_eq!(grab_at(&s, Point::new(11., 9.), 8., 6.), Some(Handle::TopLeft));
        assert_eq!(grab_at(&s, Point::new(60., 12.), 8., 6.), Some(Handle::Top));
        assert_eq!(grab_at(&s, Point::new(60., 57.), 8., 6.), Some(Handle::Bottom));
        assert_eq!(grab_at(&s, Point::new(8., 35.), 8., 6.), Some(Handle::Left));
        assert_eq!(grab_at(&s, Point::new(113., 35.), 8., 6.), Some(Handle::Right));
        assert_eq!(grab_at(&s, Point::new(60., 35.), 8., 6.), None); // interior
        assert_eq!(grab_at(&s, Point::new(60., 80.), 8., 6.), None); // far below
    }

    #[test]
    fn redact_sides_grabbable_arrow_and_text_only_handles() {
        let r = Shape::Redact { rect: Rect::new(0., 0., 50., 50.), color: Color::Black };
        assert_eq!(grab_at(&r, Point::new(25., 49.), 8., 6.), Some(Handle::Bottom));
        let a = Shape::Arrow { start: Point::new(0., 0.), end: Point::new(100., 0.), style: Style::default() };
        assert_eq!(grab_at(&a, Point::new(99., 2.), 8., 6.), Some(Handle::End));
        assert_eq!(grab_at(&a, Point::new(50., 0.), 8., 6.), None);
        assert_eq!(grab_at(&text_shape(), Point::new(5., 5.), 8., 6.), None);
    }

    #[test]
    fn dragging_a_side_moves_only_that_side() {
        let mut s = Shape::Rect { rect: Rect::new(10., 10., 50., 50.), style: Style::default() };
        drag_handle(&mut s, Handle::Right, Point::new(80., 999.));
        drag_handle(&mut s, Handle::Top, Point::new(-5., 0.));
        assert_eq!(s, Shape::Rect { rect: Rect::new(10., 0., 80., 50.), style: Style::default() });
        drag_handle(&mut s, Handle::Left, Point::new(100., 0.)); // past the right side
        assert_eq!(s, Shape::Rect { rect: Rect::new(80., 0., 100., 50.), style: Style::default() });
    }

    #[test]
    fn rect_outline_hit_but_not_interior() {
        let s = Shape::Rect { rect: Rect::new(10., 10., 110., 60.), style: Style::default() };
        assert!(hit_test(&s, Point::new(10., 30.), 6., &M));
        assert!(hit_test(&s, Point::new(60., 13.), 6., &M));
        assert!(!hit_test(&s, Point::new(60., 35.), 6., &M));
    }

    #[test]
    fn arrow_hit_near_segment_only() {
        let s = Shape::Arrow { start: Point::new(0., 0.), end: Point::new(100., 0.), style: Style::default() };
        assert!(hit_test(&s, Point::new(50., 7.), 6., &M));
        assert!(!hit_test(&s, Point::new(50., 9.), 6., &M));
        assert!(!hit_test(&s, Point::new(120., 0.), 6., &M));
    }

    #[test]
    fn text_bounds_include_padding() {
        assert_eq!(bounds(&text_shape(), &M), Rect::new(5., 5., 37., 33.));
    }

    #[test]
    fn text_hit_inside_bounds() {
        assert!(hit_test(&text_shape(), Point::new(20., 20.), 0., &M));
        assert!(!hit_test(&text_shape(), Point::new(100., 100.), 6., &M));
    }

    #[test]
    fn redact_interior_is_hit() {
        let s = Shape::Redact { rect: Rect::new(0., 0., 100., 100.), color: Color::Black };
        assert!(hit_test(&s, Point::new(50., 50.), 6., &M));
        assert!(!hit_test(&s, Point::new(120., 50.), 6., &M));
    }

    #[test]
    fn drag_corner_past_opposite_renormalizes() {
        let mut s = Shape::Rect { rect: Rect::new(10., 10., 50., 50.), style: Style::default() };
        drag_handle(&mut s, Handle::TopLeft, Point::new(80., 70.));
        assert_eq!(s, Shape::Rect { rect: Rect::new(50., 50., 80., 70.), style: Style::default() });
    }

    #[test]
    fn arrow_end_handle_moves_end() {
        let mut s = Shape::Arrow { start: Point::new(1., 2.), end: Point::new(100., 0.), style: Style::default() };
        drag_handle(&mut s, Handle::End, Point::new(5., 5.));
        match s {
            Shape::Arrow { start, end, .. } => {
                assert_eq!(end, Point::new(5., 5.));
                assert_eq!(start, Point::new(1., 2.));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn text_has_no_handles() {
        assert!(handles(&text_shape()).is_empty());
    }

    #[test]
    fn rect_handles_and_hit() {
        let s = Shape::Redact { rect: Rect::new(10., 10., 50., 40.), color: Color::Red };
        assert_eq!(handles(&s).len(), 4);
        assert_eq!(handle_at(&s, Point::new(51., 41.), 8.), Some(Handle::BottomRight));
        assert_eq!(handle_at(&s, Point::new(30., 25.), 8.), None);
    }

    #[test]
    fn translate_moves_all_points() {
        let mut s = Shape::Arrow { start: Point::new(0., 0.), end: Point::new(10., 10.), style: Style::default() };
        translate(&mut s, Vec2::new(3., 4.));
        assert_eq!(
            s,
            Shape::Arrow { start: Point::new(3., 4.), end: Point::new(13., 14.), style: Style::default() }
        );
        let mut t = text_shape();
        translate(&mut t, Vec2::new(3., 4.));
        assert!(matches!(t, Shape::Text { origin, .. } if origin == Point::new(8., 9.)));
    }
}

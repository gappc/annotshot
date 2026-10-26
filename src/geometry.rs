use crate::model::{Shape, TextMeasure, TEXT_PADDING};
use kurbo::{Line, ParamCurveNearest, Point, Rect, Vec2};

pub fn bounds(shape: &Shape, m: &dyn TextMeasure) -> Rect {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } => *rect,
        Shape::Arrow { start, end, .. } => Rect::from_points(*start, *end),
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
        Shape::Text { .. } | Shape::Redact { .. } => bounds(shape, m).inflate(tol, tol).contains(p),
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
}

pub fn handles(shape: &Shape) -> Vec<(Handle, Point)> {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } => vec![
            (Handle::TopLeft, Point::new(rect.x0, rect.y0)),
            (Handle::TopRight, Point::new(rect.x1, rect.y0)),
            (Handle::BottomLeft, Point::new(rect.x0, rect.y1)),
            (Handle::BottomRight, Point::new(rect.x1, rect.y1)),
        ],
        Shape::Arrow { start, end, .. } => vec![(Handle::Start, *start), (Handle::End, *end)],
        Shape::Text { .. } => vec![],
    }
}

pub fn handle_at(shape: &Shape, p: Point, size: f64) -> Option<Handle> {
    let half = size / 2.0;
    handles(shape)
        .into_iter()
        .find(|(_, hp)| (p.x - hp.x).abs() <= half && (p.y - hp.y).abs() <= half)
        .map(|(h, _)| h)
}

pub fn translate(shape: &mut Shape, d: Vec2) {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } => *rect = *rect + d,
        Shape::Arrow { start, end, .. } => {
            *start += d;
            *end += d;
        }
        Shape::Text { origin, .. } => *origin += d,
    }
}

pub fn drag_handle(shape: &mut Shape, h: Handle, p: Point) {
    match shape {
        Shape::Rect { rect, .. } | Shape::Redact { rect, .. } => {
            let mut r = *rect;
            match h {
                Handle::TopLeft => (r.x0, r.y0) = (p.x, p.y),
                Handle::TopRight => (r.x1, r.y0) = (p.x, p.y),
                Handle::BottomLeft => (r.x0, r.y1) = (p.x, p.y),
                Handle::BottomRight => (r.x1, r.y1) = (p.x, p.y),
                Handle::Start | Handle::End => {}
            }
            *rect = r.abs();
        }
        Shape::Arrow { start, end, .. } => match h {
            Handle::Start => *start = p,
            Handle::End => *end = p,
            _ => {}
        },
        Shape::Text { .. } => {}
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

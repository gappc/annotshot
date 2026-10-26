use kurbo::{Point, Size, Vec2};

/// Maps image pixel coordinates to widget coordinates: `widget = image * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pub scale: f64,
    pub offset: Vec2,
}

impl ViewTransform {
    /// Fit `image` inside `widget` without upscaling, centred.
    pub fn fit(image: Size, widget: Size) -> Self {
        let mut scale = 1.0_f64;
        if image.width > 0.0 && image.height > 0.0 {
            scale = scale.min(widget.width / image.width).min(widget.height / image.height);
        }
        if scale.is_nan() || scale <= 0.0 {
            // Degenerate widget size (e.g. before allocation): keep the transform invertible.
            scale = 1.0;
        }
        // Whole-pixel offset keeps an unscaled image crisp.
        let offset = Vec2::new(
            ((widget.width - image.width * scale) / 2.0).round(),
            ((widget.height - image.height * scale) / 2.0).round(),
        );
        Self { scale, offset }
    }

    pub fn to_image(self, p: Point) -> Point {
        ((p.to_vec2() - self.offset) / self.scale).to_point()
    }

    #[allow(dead_code)] // places the in-place text editor (Task 7)
    pub fn to_widget(self, p: Point) -> Point {
        (p.to_vec2() * self.scale + self.offset).to_point()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_image_is_not_upscaled_and_is_centred() {
        let v = ViewTransform::fit(Size::new(400., 300.), Size::new(800., 600.));
        assert_eq!(v.scale, 1.0);
        assert_eq!(v.to_widget(Point::ZERO), Point::new(200., 150.));
    }

    #[test]
    fn large_image_scales_down_and_round_trips() {
        let v = ViewTransform::fit(Size::new(3840., 2160.), Size::new(960., 600.));
        assert_eq!(v.scale, 0.25);
        let p = Point::new(1234., 567.);
        let q = v.to_image(v.to_widget(p));
        assert!((p - q).hypot() < 1e-9);
    }

    #[test]
    fn unallocated_widget_keeps_transform_invertible() {
        let v = ViewTransform::fit(Size::new(400., 300.), Size::ZERO);
        assert_eq!(v.scale, 1.0);
        let p = Point::new(10., 20.);
        assert_eq!(v.to_image(v.to_widget(p)), p);
    }
}

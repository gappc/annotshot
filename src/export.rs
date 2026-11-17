use std::path::{Path, PathBuf};

use gtk::cairo::{self, Format, ImageSurface};
use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;

use kurbo::Rect;

use crate::model::Item;
use crate::render::{PangoMeasure, draw_items};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

pub fn format_for_path(path: &Path) -> (PathBuf, ImageFormat) {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match ext.as_deref() {
        Some("png") => (path.to_path_buf(), ImageFormat::Png),
        Some("jpg" | "jpeg") => (path.to_path_buf(), ImageFormat::Jpeg),
        _ => {
            let mut name = path.as_os_str().to_owned();
            name.push(".jpg");
            (PathBuf::from(name), ImageFormat::Jpeg)
        }
    }
}

/// New surface of the base's exact size with the base painted and all items drawn.
pub fn flatten(base: &ImageSurface, items: &[Item], m: &PangoMeasure) -> ImageSurface {
    let out = ImageSurface::create(Format::ARgb32, base.width(), base.height())
        .expect("create export surface");
    {
        let cr = cairo::Context::new(&out).expect("create cairo context");
        cr.set_source_surface(base, 0.0, 0.0).expect("paint base");
        cr.paint().expect("paint base");
        // One-off drawing: the canvas's pixel cache would not pay off here.
        draw_items(&cr, items, None, base, None, m);
    }
    out.flush();
    out
}

/// `flatten`, cut to `area` when there is one (what Copy puts on the clipboard).
pub fn flatten_area(base: &ImageSurface, items: &[Item], area: Option<Rect>, m: &PangoMeasure) -> ImageSurface {
    let flat = flatten(base, items, m);
    match area {
        Some(a) => crop(&flat, a),
        None => flat,
    }
}

/// Direction of a quarter turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// Counter-clockwise.
    Left,
    /// Clockwise.
    Right,
}

/// New surface with `src` turned by 90°: width and height swap, pixel values
/// are copied unchanged.
pub fn rotate(src: &ImageSurface, turn: Turn) -> ImageSurface {
    let (w, h) = (src.width(), src.height());
    let out = ImageSurface::create(Format::ARgb32, h, w).expect("create rotated surface");
    {
        let cr = cairo::Context::new(&out).expect("create cairo context");
        match turn {
            Turn::Right => {
                cr.translate(h as f64, 0.0);
                cr.rotate(std::f64::consts::FRAC_PI_2);
            }
            Turn::Left => {
                cr.translate(0.0, w as f64);
                cr.rotate(-std::f64::consts::FRAC_PI_2);
            }
        }
        cr.set_source_surface(src, 0.0, 0.0).expect("paint source");
        cr.source().set_filter(cairo::Filter::Nearest);
        cr.paint().expect("paint source");
    }
    out.flush();
    out
}

/// New surface holding the pixels of `area` (whole-pixel edges inside `src`), unchanged.
pub fn crop(src: &ImageSurface, area: Rect) -> ImageSurface {
    let out = ImageSurface::create(Format::ARgb32, area.width() as i32, area.height() as i32)
        .expect("create cropped surface");
    {
        let cr = cairo::Context::new(&out).expect("create cairo context");
        cr.set_source_surface(src, -area.x0, -area.y0).expect("paint source");
        cr.source().set_filter(cairo::Filter::Nearest);
        cr.paint().expect("paint source");
    }
    out.flush();
    out
}

pub fn save(surface: &ImageSurface, path: &Path, fmt: ImageFormat) -> Result<(), String> {
    // The cairo `png` feature is not enabled, so both formats go through gdk-pixbuf.
    let (w, h) = (surface.width() as usize, surface.height() as usize);
    surface.flush();
    let stride = surface.stride() as usize;
    let mut rgb = Vec::new();
    surface
        .with_data(|data| rgb = argb32_to_rgb(data, w, h, stride))
        .map_err(|e| format!("Cannot read image: {e}"))?;
    let pixbuf = Pixbuf::from_bytes(
        &glib::Bytes::from_owned(rgb),
        gtk::gdk_pixbuf::Colorspace::Rgb,
        false,
        8,
        w as i32,
        h as i32,
        (w * 3) as i32,
    );
    match fmt {
        ImageFormat::Png => pixbuf.savev(path, "png", &[]),
        ImageFormat::Jpeg => pixbuf.savev(path, "jpeg", &[("quality", "90")]),
    }
    .map_err(|e| format!("Cannot save image: {}", e.message()))
}

/// Little-endian Cairo ARGB32 (bytes B,G,R,A) to packed RGB; alpha is ignored.
pub fn argb32_to_rgb(data: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        let row = &data[y * stride..y * stride + width * 4];
        for px in row.as_chunks::<4>().0 {
            out.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    out
}

pub fn surface_from_texture(t: &gdk::Texture) -> ImageSurface {
    let (w, h) = (t.width(), t.height());
    let mut surface = ImageSurface::create(Format::ARgb32, w, h).expect("create surface");
    let stride = surface.stride() as usize;
    {
        let mut dl = gdk::TextureDownloader::new(t);
        dl.set_format(gdk::MemoryFormat::B8g8r8a8Premultiplied);
        let (bytes, src_stride) = dl.download_bytes();
        let row = w as usize * 4;
        let mut data = surface.data().expect("surface data");
        for y in 0..h as usize {
            data[y * stride..y * stride + row]
                .copy_from_slice(&bytes[y * src_stride..y * src_stride + row]);
        }
    }
    surface.mark_dirty();
    surface
}

pub fn texture_from_surface(s: &ImageSurface) -> gdk::Texture {
    s.flush();
    let stride = s.stride() as usize;
    let mut bytes = None;
    s.with_data(|d| bytes = Some(glib::Bytes::from(d)))
        .expect("surface data");
    let bytes = bytes.expect("surface bytes");
    gdk::MemoryTexture::new(
        s.width(),
        s.height(),
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &bytes,
        stride,
    )
    .upcast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Shape};
    use gtk::cairo::Context;
    use kurbo::Rect;

    fn white(w: i32, h: i32) -> ImageSurface {
        let s = ImageSurface::create(Format::ARgb32, w, h).unwrap();
        let cr = Context::new(&s).unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().unwrap();
        s
    }

    #[test]
    fn format_for_path_rules() {
        assert_eq!(format_for_path(Path::new("/x/a.PNG")).1, ImageFormat::Png);
        assert_eq!(
            format_for_path(Path::new("/x/a.jpeg")),
            (PathBuf::from("/x/a.jpeg"), ImageFormat::Jpeg)
        );
        assert_eq!(
            format_for_path(Path::new("/x/shot")),
            (PathBuf::from("/x/shot.jpg"), ImageFormat::Jpeg)
        );
    }

    #[test]
    fn argb_to_rgb_swaps_channels_and_respects_stride() {
        let data = [0x10, 0x20, 0x30, 0xff, 0, 0, 0, 0];
        assert_eq!(argb32_to_rgb(&data, 1, 1, 8), vec![0x30, 0x20, 0x10]);
    }

    #[test]
    fn flatten_keeps_original_size() {
        let base = ImageSurface::create(Format::ARgb32, 3840, 2160).unwrap();
        let out = flatten(&base, &[], &PangoMeasure::new());
        assert_eq!((out.width(), out.height()), (3840, 2160));
    }

    #[test]
    fn flatten_area_keeps_annotations_in_the_area() {
        let base = white(64, 48);
        let items = [Item {
            id: 1,
            shape: Shape::Redact { rect: Rect::new(10.0, 10.0, 30.0, 30.0), color: Color::Black },
        }];
        let m = PangoMeasure::new();
        let mut out = flatten_area(&base, &items, Some(Rect::new(10., 10., 40., 40.)), &m);
        assert_eq!((out.width(), out.height()), (30, 30));
        assert_eq!(rgb_at(&mut out, 5, 5), (0, 0, 0));
        assert_eq!(rgb_at(&mut out, 25, 25), (255, 255, 255));
        let whole = flatten_area(&base, &items, None, &m);
        assert_eq!((whole.width(), whole.height()), (64, 48));
    }

    #[test]
    fn saved_jpeg_and_png_reload_with_same_size_and_colors() {
        let base = white(64, 48);
        let items = [Item {
            id: 1,
            shape: Shape::Redact { rect: Rect::new(0.0, 0.0, 32.0, 48.0), color: Color::Black },
        }];
        let flat = flatten(&base, &items, &PangoMeasure::new());
        for (name, fmt) in [("a.jpg", ImageFormat::Jpeg), ("a.png", ImageFormat::Png)] {
            let path = std::env::temp_dir()
                .join(format!("annotshot-export-{}-{}", std::process::id(), name));
            save(&flat, &path, fmt).unwrap();
            let pb = Pixbuf::from_file(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            assert_eq!((pb.width(), pb.height()), (64, 48));
            let px = pb.read_pixel_bytes();
            let at = |x: usize, y: usize| {
                let i = y * pb.rowstride() as usize + x * pb.n_channels() as usize;
                [px[i] as i32, px[i + 1] as i32, px[i + 2] as i32]
            };
            assert!(at(10, 10).iter().all(|&c| c <= 8), "{name} black: {:?}", at(10, 10));
            assert!(at(50, 10).iter().all(|&c| c >= 247), "{name} white: {:?}", at(50, 10));
        }
    }

    #[test]
    fn save_into_missing_dir_returns_err() {
        let s = white(4, 4);
        assert!(save(&s, Path::new("/nonexistent/dir/a.jpg"), ImageFormat::Jpeg).is_err());
        assert!(save(&s, Path::new("/nonexistent/dir/a.png"), ImageFormat::Png).is_err());
    }

    #[test]
    fn texture_surface_round_trip() {
        let s = white(5, 3);
        let t = texture_from_surface(&s);
        assert_eq!((t.width(), t.height()), (5, 3));
        let back = surface_from_texture(&t);
        assert_eq!((back.width(), back.height()), (5, 3));
        let mut back = back;
        let d = back.data().unwrap();
        assert_eq!(&d[0..4], &[255, 255, 255, 255]);
    }

    const RED: (u8, u8, u8) = (255, 0, 0);
    const GREEN: (u8, u8, u8) = (0, 255, 0);
    const BLUE: (u8, u8, u8) = (0, 0, 255);

    #[test]
    fn crop_cuts_exact_pixels() {
        let mut out = crop(&sample_3x2(), Rect::new(1., 0., 3., 1.));
        assert_eq!((out.width(), out.height()), (2, 1));
        assert_eq!(rgb_at(&mut out, 0, 0), (255, 255, 255));
        assert_eq!(rgb_at(&mut out, 1, 0), GREEN);
        let mut out = crop(&sample_3x2(), Rect::new(0., 1., 1., 2.));
        assert_eq!(rgb_at(&mut out, 0, 0), BLUE);
    }

    /// 3×2 white image with red top-left, green top-right, blue bottom-left.
    fn sample_3x2() -> ImageSurface {
        let s = white(3, 2);
        {
            let cr = Context::new(&s).unwrap();
            for (x, y, (r, g, b)) in [(0, 0, RED), (2, 0, GREEN), (0, 1, BLUE)] {
                cr.set_source_rgb(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
                cr.rectangle(x as f64, y as f64, 1.0, 1.0);
                cr.fill().unwrap();
            }
        }
        s
    }

    fn rgb_at(s: &mut ImageSurface, x: usize, y: usize) -> (u8, u8, u8) {
        s.flush();
        let stride = s.stride() as usize;
        let d = s.data().unwrap();
        let i = y * stride + x * 4;
        (d[i + 2], d[i + 1], d[i])
    }

    #[test]
    fn rotate_right_turns_clockwise() {
        let mut out = rotate(&sample_3x2(), Turn::Right);
        assert_eq!((out.width(), out.height()), (2, 3));
        assert_eq!(rgb_at(&mut out, 1, 0), RED); // top-left → top-right
        assert_eq!(rgb_at(&mut out, 1, 2), GREEN); // top-right → bottom-right
        assert_eq!(rgb_at(&mut out, 0, 0), BLUE); // bottom-left → top-left
    }

    #[test]
    fn rotate_left_turns_counter_clockwise() {
        let mut out = rotate(&sample_3x2(), Turn::Left);
        assert_eq!((out.width(), out.height()), (2, 3));
        assert_eq!(rgb_at(&mut out, 0, 2), RED); // top-left → bottom-left
        assert_eq!(rgb_at(&mut out, 0, 0), GREEN); // top-right → top-left
        assert_eq!(rgb_at(&mut out, 1, 2), BLUE); // bottom-left → bottom-right
    }

    #[test]
    fn left_then_right_restores_every_pixel() {
        let mut src = sample_3x2();
        let mut back = rotate(&rotate(&src, Turn::Left), Turn::Right);
        assert_eq!((back.width(), back.height()), (3, 2));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(rgb_at(&mut back, x, y), rgb_at(&mut src, x, y), "pixel {x},{y}");
            }
        }
    }
}

//! In-place text editing: a `gtk::TextView` laid over the canvas that looks
//! like the label it will become, committed into the editor on close.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use kurbo::Point;

use crate::canvas::SharedCanvas;
use crate::editor::Editor;
use crate::model::{Color, ItemId, TextMeasure, TEXT_BORDER, TEXT_FONT_PX, TEXT_PADDING};
use crate::view::ViewTransform;

const CSS_CLASS: &str = "annotshot-text-editor";

/// What a commit writes to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A new label with this image-space origin.
    New(Point),
    /// An existing label being re-edited.
    Existing(ItemId),
}

/// Write the typed text into the editor. Empty text discards a new label and
/// deletes an existing one (handled by `add_text` / `set_text`).
pub fn apply_commit(editor: &mut Editor, target: Target, text: &str) {
    match target {
        Target::New(origin) => editor.add_text(origin, text),
        Target::Existing(id) => editor.set_text(id, text),
    }
}

/// Widget-space geometry of the editor box for a label at `origin_widget`
/// (the label's top-left corner) drawn at view `scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Overlay margins of the box's outer (border) edge.
    pub margin_start: i32,
    pub margin_top: i32,
    /// Border width in widget px.
    pub border: f64,
    /// Space between the border and the text (TextView margins), in widget px.
    pub inner: i32,
    pub font_px: f64,
}

/// The label border is drawn inside its padding, so the outer box edge is the
/// label origin and the text starts `TEXT_PADDING` further in, as in `render`.
pub fn placement(origin_widget: Point, scale: f64) -> Placement {
    Placement {
        margin_start: origin_widget.x.round().max(0.0) as i32,
        margin_top: origin_widget.y.round().max(0.0) as i32,
        border: TEXT_BORDER * scale,
        inner: ((TEXT_PADDING - TEXT_BORDER) * scale).round() as i32,
        font_px: TEXT_FONT_PX * scale,
    }
}

/// Widget size of the editor box for laid-out text of `text` image px at view
/// `scale`: the same box the committed label gets (text + padding). GtkTextView
/// doesn't request a width for its content outside a scrolled window, so the
/// editor is sized explicitly. Empty text keeps room for the caret.
pub fn editor_size(text: kurbo::Size, scale: f64) -> (i32, i32) {
    let w = text.width.max(TEXT_FONT_PX / 2.0) + 2.0 * TEXT_PADDING;
    let h = text.height + 2.0 * TEXT_PADDING;
    ((w * scale).ceil() as i32, (h * scale).ceil() as i32)
}

fn hex(color: Color) -> String {
    let (r, g, b) = color.rgb();
    let c = |v: f64| (v * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
}

/// Stylesheet that makes the TextView look like a label: Sans Bold in the item
/// color on white, with a border in the item color.
pub fn editor_css(scale: f64, color: Color) -> String {
    let p = placement(Point::ZERO, scale);
    let c = hex(color);
    format!(
        "textview.{CSS_CLASS} {{ border: {b:.3}px solid {c}; color: {c}; padding: 0; margin: 0; \
         background-color: #ffffff; border-radius: 0; box-shadow: none; outline: none; \
         font-family: Sans; font-weight: bold; font-size: {f:.3}px; }}\n\
         textview.{CSS_CLASS} text {{ background-color: #ffffff; color: {c}; caret-color: {c}; }}\n",
        b = p.border,
        f = p.font_px,
    )
}

pub struct TextEditor {
    view: gtk::TextView,
    css: gtk::CssProvider,
    state: SharedCanvas,
    target: Cell<Option<Target>>,
    scale: Rc<Cell<f64>>,
}

impl TextEditor {
    /// `on_escape` is called when Escape is pressed in the editor; the app
    /// answers by committing (`AppMsg::TextCommit`).
    pub fn new(state: SharedCanvas, on_escape: impl Fn() + 'static) -> Self {
        let view = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::None)
            .accepts_tab(false)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .visible(false)
            .css_classes([CSS_CLASS])
            .build();
        let css = gtk::CssProvider::new();
        // Real values are loaded on open; loading now surfaces CSS errors at startup.
        css.load_from_string(&editor_css(1.0, Color::default()));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                on_escape();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        view.add_controller(keys);
        let scale = Rc::new(Cell::new(1.0));
        // Grow/shrink the box with its text, measured like the committed label.
        {
            let state = state.clone();
            let scale = scale.clone();
            let view_weak = view.downgrade();
            view.buffer().connect_changed(move |buffer| {
                let Some(view) = view_weak.upgrade() else { return };
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                let size = state.borrow().measure.text_size(&text);
                let (w, h) = editor_size(size, scale.get());
                view.set_size_request(w, h);
            });
        }
        Self {
            view,
            css,
            state,
            target: Cell::new(None),
            scale,
        }
    }

    /// The widget to add to the canvas overlay.
    pub fn widget(&self) -> &gtk::TextView {
        &self.view
    }

    pub fn is_open(&self) -> bool {
        self.target.get().is_some()
    }

    /// Show the editor with its box's top-left at `origin_widget`, focused.
    pub fn open(&self, origin_widget: Point, scale: f64, color: Color, initial: &str) {
        let p = placement(origin_widget, scale);
        self.scale.set(scale);
        self.restyle(color);
        self.view.set_margin_start(p.margin_start);
        self.view.set_margin_top(p.margin_top);
        self.view.set_left_margin(p.inner);
        self.view.set_right_margin(p.inner);
        self.view.set_top_margin(p.inner);
        self.view.set_bottom_margin(p.inner);
        let buffer = self.view.buffer();
        buffer.set_text(initial);
        buffer.place_cursor(&buffer.end_iter());
        self.view.set_visible(true);
        self.view.grab_focus();
    }

    /// Hide the editor and return its text. Focus goes back to the window so
    /// single-key shortcuts work again.
    pub fn close(&self) -> String {
        let buffer = self.view.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        buffer.set_text("");
        if let Some(root) = self.view.root() {
            root.set_focus(None::<&gtk::Widget>);
        }
        self.view.set_visible(false);
        text
    }

    fn restyle(&self, color: Color) {
        self.css
            .load_from_string(&editor_css(self.scale.get(), color));
    }

    /// Restyle the open editor after a color swatch click and give it back the
    /// focus the swatch button took.
    pub fn set_color(&self, color: Color) {
        self.restyle(color);
        self.view.grab_focus();
    }

    /// Open an empty editor for a new label at image point `origin`.
    pub fn start_new(&self, origin: Point, view: ViewTransform) {
        let color = {
            let mut st = self.state.borrow_mut();
            // A new label starts unselected so color clicks while typing don't
            // recolor some other item.
            st.editor.selected = None;
            st.editing = None;
            st.text_open = true;
            st.editor.style.color
        };
        self.target.set(Some(Target::New(origin)));
        self.open(view.to_widget(origin), view.scale, color, "");
    }

    /// Open the editor on existing label `id`, hiding the label meanwhile.
    pub fn start_edit(&self, id: ItemId, view: ViewTransform) {
        let Some((origin, text, color)) = ({
            let mut st = self.state.borrow_mut();
            let found = st.editor.text_of(id);
            if let Some((_, _, color)) = &found {
                // Select it so the toolbar shows its color and swatches recolor it.
                st.editor.selected = Some(id);
                st.editor.style.color = *color;
                st.editing = Some(id);
                st.text_open = true;
            }
            found
        }) else {
            return;
        };
        self.target.set(Some(Target::Existing(id)));
        self.open(view.to_widget(origin), view.scale, color, &text);
    }

    /// Commit the open editor into the editor model. Returns false if nothing was open.
    pub fn commit(&self) -> bool {
        let Some(target) = self.target.take() else {
            return false;
        };
        let text = self.close();
        let mut st = self.state.borrow_mut();
        st.editing = None;
        st.text_open = false;
        apply_commit(&mut st.editor, target, &text);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Shape;

    #[test]
    fn placement_scales_with_view() {
        let p = placement(Point::new(100.4, 50.6), 1.0);
        assert_eq!((p.margin_start, p.margin_top), (100, 51));
        assert_eq!(p.border, TEXT_BORDER);
        assert_eq!(p.inner, (TEXT_PADDING - TEXT_BORDER) as i32);
        assert_eq!(p.font_px, TEXT_FONT_PX);

        let half = placement(Point::new(10., 10.), 0.5);
        assert_eq!(half.border, 1.0);
        assert_eq!(half.inner, 2);
        assert_eq!(half.font_px, 8.0);
        // Border + inner margin = padding: the text starts where `render` draws it.
        assert_eq!(half.border + half.inner as f64, TEXT_PADDING * 0.5);
    }

    #[test]
    fn editor_size_matches_label_box() {
        // Text 50×19 image px → label box (50+12)×(19+12), at scale 1 and 0.5.
        assert_eq!(editor_size(kurbo::Size::new(50., 19.), 1.0), (62, 31));
        assert_eq!(editor_size(kurbo::Size::new(50., 19.), 0.5), (31, 16));
        // Empty text still leaves room for the caret.
        let (w, _) = editor_size(kurbo::Size::new(0., 19.), 1.0);
        assert!(w as f64 >= 2.0 * TEXT_PADDING + TEXT_FONT_PX / 2.0);
    }

    #[test]
    fn placement_never_negative() {
        let p = placement(Point::new(-3., -0.2), 1.0);
        assert_eq!((p.margin_start, p.margin_top), (0, 0));
    }

    #[test]
    fn css_uses_color_and_scaled_font() {
        let css = editor_css(2.0, Color::Blue);
        assert!(css.contains("#3584e4"));
        assert!(css.contains("font-size: 32.000px"));
        assert!(css.contains("border: 4.000px solid #3584e4"));
        assert!(css.contains("font-weight: bold"));
        assert!(editor_css(1.0, Color::Black).contains("color: #000000"));
        // libadwaita ignores `color` on the inner `text` node; it must be on the
        // textview node itself (first rule).
        let first_rule = css.lines().next().unwrap();
        assert!(first_rule.contains("; color: #3584e4;"), "{first_rule}");
    }

    #[test]
    fn commit_new_and_existing() {
        let mut e = Editor::new();
        let o = Point::new(5., 6.);
        apply_commit(&mut e, Target::New(o), "  ");
        assert!(e.items.is_empty());
        apply_commit(&mut e, Target::New(o), "Hi\nthere");
        let id = e.items[0].id;
        assert_eq!(e.text_of(id).map(|t| t.1), Some("Hi\nthere".into()));
        apply_commit(&mut e, Target::Existing(id), "Bye");
        assert!(matches!(&e.items[0].shape, Shape::Text { text, .. } if text == "Bye"));
        e.undo();
        assert_eq!(e.text_of(id).map(|t| t.1), Some("Hi\nthere".into()));
        e.redo();
        apply_commit(&mut e, Target::Existing(id), "");
        assert!(e.items.is_empty());
    }
}

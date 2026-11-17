//! In-place text editing: a `gtk::TextView` laid over the canvas that looks
//! like the label it will become, committed into the editor on close.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use kurbo::Point;

use crate::canvas::SharedCanvas;
use crate::editor::Editor;
use crate::model::{Color, ItemId, Style, TextMeasure, Width, TEXT_BORDER};
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
/// (the label's top-left corner) drawn at view `scale`, for a label of `width`.
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
/// label origin and the text starts `width.text_padding()` further in, as in
/// `render` (to the nearest widget px: TextView margins are whole pixels).
pub fn placement(origin_widget: Point, scale: f64, width: Width) -> Placement {
    Placement {
        margin_start: origin_widget.x.round().max(0.0) as i32,
        margin_top: origin_widget.y.round().max(0.0) as i32,
        border: TEXT_BORDER * scale,
        inner: ((width.text_padding() - TEXT_BORDER) * scale).round() as i32,
        font_px: width.font_px() * scale,
    }
}

/// Widget size of the editor box for laid-out text of `text` image px at view
/// `scale` for a label of `width`: the same box the committed label gets
/// (text + padding). GtkTextView
/// doesn't request a width for its content outside a scrolled window, so the
/// editor is sized explicitly. Empty text keeps room for the caret.
pub fn editor_size(text: kurbo::Size, scale: f64, width: Width) -> (i32, i32) {
    let pad = width.text_padding();
    let w = text.width.max(width.font_px() / 2.0) + 2.0 * pad;
    let h = text.height + 2.0 * pad;
    ((w * scale).ceil() as i32, (h * scale).ceil() as i32)
}

fn hex(color: Color) -> String {
    let (r, g, b) = color.rgb();
    let c = |v: f64| (v * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
}

/// Stylesheet that makes the TextView look like a label: Sans Bold in the item
/// color and size on its label box, with the label border (see `Color::label_box`).
pub fn editor_css(scale: f64, style: Style) -> String {
    let color = style.color;
    let p = placement(Point::ZERO, scale, style.width);
    let c = hex(color);
    let bg = hex(color.label_box());
    let border = hex(color.label_border());
    format!(
        "textview.{CSS_CLASS} {{ border: {b:.3}px solid {border}; color: {c}; padding: 0; margin: 0; \
         background-color: {bg}; border-radius: 0; box-shadow: none; outline: none; \
         font-family: Sans; font-weight: bold; font-size: {f:.3}px; }}\n\
         textview.{CSS_CLASS} text {{ background-color: {bg}; color: {c}; caret-color: {c}; }}\n",
        b = p.border,
        f = p.font_px,
    )
}

/// Size `view` to its current text, measured like the committed label.
fn fit_size(view: &gtk::TextView, state: &SharedCanvas, scale: f64, width: Width) {
    let buffer = view.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    let size = state.borrow().measure.text_size(&text, width.font_px());
    let (w, h) = editor_size(size, scale, width);
    view.set_size_request(w, h);
}

/// Set the same text again, keeping the cursor and selection, without an
/// undo step.
fn relayout_text(view: &gtk::TextView) {
    let buffer = view.buffer();
    let offset = |mark: &gtk::TextMark| buffer.iter_at_mark(mark).offset();
    let insert = offset(&buffer.get_insert());
    let bound = offset(&buffer.selection_bound());
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    buffer.begin_irreversible_action();
    buffer.set_text(&text);
    buffer.end_irreversible_action();
    buffer.select_range(&buffer.iter_at_offset(insert), &buffer.iter_at_offset(bound));
}

pub struct TextEditor {
    view: gtk::TextView,
    css: gtk::CssProvider,
    state: SharedCanvas,
    target: Cell<Option<Target>>,
    scale: Rc<Cell<f64>>,
    /// Width (font size) of the label being edited.
    width: Rc<Cell<Width>>,
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
        css.load_from_string(&editor_css(1.0, Style::default()));
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
        let width = Rc::new(Cell::new(Width::default()));
        // Grow/shrink the box with its text, measured like the committed label.
        {
            let state = state.clone();
            let scale = scale.clone();
            let width = width.clone();
            let view_weak = view.downgrade();
            view.buffer().connect_changed(move |_| {
                let Some(view) = view_weak.upgrade() else { return };
                fit_size(&view, &state, scale.get(), width.get());
            });
        }
        Self {
            view,
            css,
            state,
            target: Cell::new(None),
            scale,
            width,
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
    pub fn open(&self, origin_widget: Point, scale: f64, style: Style, initial: &str) {
        let p = placement(origin_widget, scale, style.width);
        self.scale.set(scale);
        self.restyle(style);
        self.view.set_margin_start(p.margin_start);
        self.view.set_margin_top(p.margin_top);
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

    /// Apply color and size: stylesheet, inner margins and box size.
    fn restyle(&self, style: Style) {
        let width_changed = self.width.replace(style.width) != style.width;
        self.css
            .load_from_string(&editor_css(self.scale.get(), style));
        let inner = placement(Point::ZERO, self.scale.get(), style.width).inner;
        self.view.set_left_margin(inner);
        self.view.set_right_margin(inner);
        self.view.set_top_margin(inner);
        self.view.set_bottom_margin(inner);
        fit_size(&self.view, &self.state, self.scale.get(), style.width);
        // GtkTextView keeps its old line heights after a CSS font change, so a
        // smaller font would leave the box too tall. Only a size change needs
        // this: it also clears the field's own undo history. The new style is applied
        // in the next frame's layout phase; after that frame (tick callback,
        // then idle), setting the same text again relays the lines and the
        // `changed` handler resizes the box.
        if !width_changed {
            return;
        }
        self.view.add_tick_callback(|view, _| {
            let view = view.downgrade();
            glib::idle_add_local_once(move || {
                if let Some(view) = view.upgrade() {
                    relayout_text(&view);
                }
            });
            glib::ControlFlow::Break
        });
    }

    /// Restyle the open editor after a color swatch or width button click and
    /// give it back the focus the button took.
    pub fn set_style(&self, style: Style) {
        self.restyle(style);
        self.view.grab_focus();
    }

    /// Open an empty editor for a new label at image point `origin`.
    pub fn start_new(&self, origin: Point, view: ViewTransform) {
        let style = {
            let mut st = self.state.borrow_mut();
            // A new label starts unselected so color and width clicks while
            // typing don't change some other item.
            st.editor.selected = None;
            st.editing = None;
            st.text_open = true;
            st.editor.style
        };
        self.target.set(Some(Target::New(origin)));
        self.open(view.to_widget(origin), view.scale, style, "");
    }

    /// Open the editor on existing label `id`, hiding the label meanwhile.
    pub fn start_edit(&self, id: ItemId, view: ViewTransform) {
        let Some((origin, text, style)) = ({
            let mut st = self.state.borrow_mut();
            let found = st.editor.text_of(id);
            if let Some((_, _, style)) = &found {
                // Select it so the toolbar shows its color and size, and
                // swatches and width buttons change it.
                st.editor.selected = Some(id);
                st.editor.style = *style;
                st.editing = Some(id);
                st.text_open = true;
            }
            found
        }) else {
            return;
        };
        self.target.set(Some(Target::Existing(id)));
        self.open(view.to_widget(origin), view.scale, style, &text);
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
    use crate::model::{Shape, Style, Width};

    const M_PAD: f64 = 6.0; // Width::Medium.text_padding()
    const M_FONT: f64 = 16.0;

    #[test]
    fn placement_scales_with_view() {
        let p = placement(Point::new(100.4, 50.6), 1.0, Width::Medium);
        assert_eq!((p.margin_start, p.margin_top), (100, 51));
        assert_eq!(p.border, TEXT_BORDER);
        assert_eq!(p.inner, (M_PAD - TEXT_BORDER) as i32);
        assert_eq!(p.font_px, M_FONT);

        let half = placement(Point::new(10., 10.), 0.5, Width::Medium);
        assert_eq!(half.border, 1.0);
        assert_eq!(half.inner, 2);
        assert_eq!(half.font_px, 8.0);
        // Border + inner margin = padding: the text starts where `render` draws it.
        assert_eq!(half.border + half.inner as f64, M_PAD * 0.5);
    }

    #[test]
    fn editor_size_matches_label_box() {
        // Text 50×19 image px → label box (50+12)×(19+12), at scale 1 and 0.5.
        assert_eq!(editor_size(kurbo::Size::new(50., 19.), 1.0, Width::Medium), (62, 31));
        assert_eq!(editor_size(kurbo::Size::new(50., 19.), 0.5, Width::Medium), (31, 16));
        // Empty text still leaves room for the caret.
        let (w, _) = editor_size(kurbo::Size::new(0., 19.), 1.0, Width::Medium);
        assert!(w as f64 >= 2.0 * M_PAD + M_FONT / 2.0);
    }

    #[test]
    fn placement_follows_width() {
        let p = placement(Point::new(10., 10.), 1.0, Width::Thick);
        assert_eq!(p.font_px, 28.0);
        assert!((p.border + p.inner as f64 - Width::Thick.text_padding()).abs() <= 0.5, "{p:?}");
        let half = placement(Point::new(10., 10.), 0.5, Width::Thick);
        assert_eq!(half.font_px, 14.0);
        assert!((half.border + half.inner as f64 - Width::Thick.text_padding() * 0.5).abs() <= 0.5, "{half:?}");
    }

    #[test]
    fn editor_size_follows_width() {
        // Text 50×19 at Thick: padding 10.5 on each side → 71×40.
        assert_eq!(editor_size(kurbo::Size::new(50., 19.), 1.0, Width::Thick), (71, 40));
    }

    #[test]
    fn css_uses_width_font_size() {
        let css = editor_css(1.0, Style { color: Color::Blue, width: Width::Thin });
        assert!(css.contains("font-size: 12.000px"), "{css}");
    }

    #[test]
    fn placement_never_negative() {
        let p = placement(Point::new(-3., -0.2), 1.0, Width::Medium);
        assert_eq!((p.margin_start, p.margin_top), (0, 0));
    }

    #[test]
    fn css_uses_color_and_scaled_font() {
        let css = editor_css(2.0, Style { color: Color::Blue, width: Width::Medium });
        assert!(css.contains("#3584e4"));
        assert!(css.contains("font-size: 32.000px"));
        assert!(css.contains("border: 4.000px solid #3584e4"));
        assert!(css.contains("font-weight: bold"));
        assert!(editor_css(1.0, Style { color: Color::Black, width: Width::Medium }).contains("color: #000000"));
        // libadwaita ignores `color` on the inner `text` node; it must be on the
        // textview node itself (first rule).
        let first_rule = css.lines().next().unwrap();
        assert!(first_rule.contains("; color: #3584e4;"), "{first_rule}");
    }

    #[test]
    fn css_white_text_on_black_box() {
        let css = editor_css(1.0, Style { color: Color::White, width: Width::Medium });
        assert!(css.contains("border: 2.000px solid #000000"), "{css}");
        assert!(css.contains("background-color: #000000"), "{css}");
        assert!(css.contains("; color: #ffffff;"), "{css}");
        assert!(!css.contains("background-color: #ffffff"), "{css}");
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

//! Canvas wiring: shared state, the DrawingArea draw func and pointer gestures.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::cairo;
use gtk::prelude::*;
use kurbo::{Point, Size};

use crate::editor::{Editor, Hover, PressOutcome, Tool};
use crate::geometry::Handle;
use crate::model::ItemId;
use crate::render::{PangoMeasure, draw_items, draw_selection, draw_shape};
use crate::text_edit::TextEditor;
use crate::view::{ViewTransform, inside_image};

/// Hit tolerance in screen pixels.
const HIT_TOLERANCE: f64 = 6.0;

pub struct CanvasState {
    pub editor: Editor,
    pub image: Option<cairo::ImageSurface>,
    pub measure: PangoMeasure,
    /// Text item hidden from the canvas while it is being edited in place.
    pub editing: Option<ItemId>,
    /// True while an in-place text editor is open; window shortcuts must let keys through.
    pub text_open: bool,
}

pub type SharedCanvas = Rc<RefCell<CanvasState>>;

impl CanvasState {
    pub fn new() -> Self {
        Self {
            editor: Editor::new(),
            image: None,
            measure: PangoMeasure::new(),
            editing: None,
            text_open: false,
        }
    }

    /// View transform for a widget of the given size, or `None` without an image.
    pub fn view(&self, widget_w: f64, widget_h: f64) -> Option<ViewTransform> {
        let img = self.image.as_ref()?;
        Some(ViewTransform::fit(
            Size::new(img.width() as f64, img.height() as f64),
            Size::new(widget_w, widget_h),
        ))
    }
}

pub fn draw(st: &CanvasState, cr: &cairo::Context, width: i32, height: i32) {
    // Without an image the window background stays visible behind the paste hint.
    let Some((img, v)) = st.image.as_ref().zip(st.view(width as f64, height as f64)) else {
        return;
    };
    cr.set_source_rgb(0.5, 0.5, 0.5);
    let _ = cr.paint();

    cr.translate(v.offset.x, v.offset.y);
    cr.scale(v.scale, v.scale);

    // Clip annotations to the image so the canvas matches the export.
    let _ = cr.save();
    cr.rectangle(0.0, 0.0, img.width() as f64, img.height() as f64);
    cr.clip();
    if cr.set_source_surface(img, 0.0, 0.0).is_ok() {
        cr.source().set_filter(cairo::Filter::Good);
        let _ = cr.paint();
    }
    let ed = &st.editor;
    draw_items(cr, &ed.items, st.editing, &st.measure);
    if let Some(shape) = ed.provisional() {
        draw_shape(cr, shape, ed.next_marker_number(), &st.measure);
    }
    let _ = cr.restore();

    if let Some(item) = ed
        .selected
        .filter(|id| Some(*id) != st.editing)
        .and_then(|id| ed.items.iter().find(|i| i.id == id))
    {
        draw_selection(cr, &item.shape, v.scale, &st.measure);
    }
}

/// An in-progress button-1 drag, in image coordinates.
#[derive(Clone, Copy)]
struct ActiveDrag {
    start: Point,
    last: Point,
    view: ViewTransform,
}

/// Commit an unfinished drag at its last point so a move keeps its history step.
fn finish_drag(state: &SharedCanvas, drag: &Cell<Option<ActiveDrag>>) -> bool {
    match drag.take() {
        Some(d) => {
            state.borrow_mut().editor.release(d.last);
            true
        }
        None => false,
    }
}

/// Cursor name for what a press would do: resize arrows over a selected item's
/// corners and sides, a move cursor over arrow ends and over items the Select
/// tool would move. `drawable`: the pointer is over the image (no image → false).
pub fn cursor_for(hover: Hover, tool: Tool, drawable: bool) -> &'static str {
    match hover {
        Hover::Handle(Handle::TopLeft | Handle::BottomRight) => "nwse-resize",
        Hover::Handle(Handle::TopRight | Handle::BottomLeft) => "nesw-resize",
        Hover::Handle(Handle::Top | Handle::Bottom) => "ns-resize",
        Hover::Handle(Handle::Left | Handle::Right) => "ew-resize",
        Hover::Handle(Handle::Start | Handle::End | Handle::Center) | Hover::Item => "move",
        Hover::Nothing => match tool {
            _ if !drawable => "default",
            Tool::Select => "default",
            Tool::Text => "text",
            Tool::Rect | Tool::Arrow | Tool::Redact | Tool::Marker | Tool::Highlight => "crosshair",
        },
    }
}

/// Install the draw func and pointer gestures. `changed` is called after any
/// editor change that may affect the toolbar (selection, history). Presses
/// commit an open text editor first and open it for the Text tool.
pub fn attach(
    area: &gtk::DrawingArea,
    state: SharedCanvas,
    text: Rc<TextEditor>,
    changed: impl Fn() + Clone + 'static,
) {
    {
        let state = state.clone();
        area.set_draw_func(move |_, cr, w, h| draw(&state.borrow(), cr, w, h));
    }

    let drag = Rc::new(Cell::new(None::<ActiveDrag>));
    let gesture = gtk::GestureDrag::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);

    gesture.connect_drag_begin({
        let (state, drag, area, text, changed) =
            (state.clone(), drag.clone(), area.clone(), text.clone(), changed.clone());
        move |_, x, y| {
            let finished = finish_drag(&state, &drag);
            // A press outside the open editor commits it, then is handled normally.
            let committed = text.commit();
            let pressed = {
                let mut st = state.borrow_mut();
                let st = &mut *st;
                st.view(area.width() as f64, area.height() as f64).and_then(|view| {
                    let p = view.to_image(Point::new(x, y));
                    // Drawing tools only start on the image: items begun in
                    // the letterbox would be clipped away yet still exist.
                    // (A drag may still end outside.) Select keeps working
                    // there so handles on the edge stay reachable.
                    let img = st.image.as_ref()?;
                    let size = Size::new(img.width() as f64, img.height() as f64);
                    let tol = HIT_TOLERANCE / view.scale;
                    let grabs = matches!(st.editor.hover(p, tol, &st.measure), Hover::Handle(_));
                    if st.editor.tool != Tool::Select && !grabs && !inside_image(size, p) {
                        return None;
                    }
                    let outcome = st.editor.press(p, tol, &st.measure);
                    drag.set(Some(ActiveDrag { start: Point::new(x, y), last: p, view }));
                    Some((outcome, view))
                })
            };
            match pressed {
                Some((PressOutcome::StartText(p), view)) => text.start_new(p, view),
                Some((PressOutcome::EditText(id), view)) => text.start_edit(id, view),
                _ => {}
            }
            if finished || committed || pressed.is_some() {
                area.queue_draw();
                changed();
            }
        }
    });

    gesture.connect_drag_update({
        let (state, drag, area) = (state.clone(), drag.clone(), area.clone());
        move |_, dx, dy| {
            let Some(mut d) = drag.get() else { return };
            d.last = d.view.to_image(d.start + (dx, dy));
            drag.set(Some(d));
            state.borrow_mut().editor.drag_to(d.last);
            area.queue_draw();
        }
    });

    gesture.connect_drag_end({
        let (state, drag, area, changed) = (state.clone(), drag.clone(), area.clone(), changed.clone());
        move |_, dx, dy| {
            if let Some(mut d) = drag.get() {
                d.last = d.view.to_image(d.start + (dx, dy));
                drag.set(Some(d));
            }
            if finish_drag(&state, &drag) {
                area.queue_draw();
                changed();
            }
        }
    });

    gesture.connect_cancel({
        let (state, drag, area, changed) = (state.clone(), drag.clone(), area.clone(), changed.clone());
        move |_, _| {
            if finish_drag(&state, &drag) {
                area.queue_draw();
                changed();
            }
        }
    });
    area.add_controller(gesture);

    // Cursor feedback; kept unchanged while a drag is in progress.
    let motion = gtk::EventControllerMotion::new();
    motion.connect_motion({
        let (state, drag, area) = (state.clone(), drag.clone(), area.clone());
        move |_, x, y| {
            if drag.get().is_some() {
                return;
            }
            let st = state.borrow();
            let name = match (st.view(area.width() as f64, area.height() as f64), st.image.as_ref()) {
                (Some(view), Some(img)) => {
                    let p = view.to_image(Point::new(x, y));
                    let size = Size::new(img.width() as f64, img.height() as f64);
                    let hover = st.editor.hover(p, HIT_TOLERANCE / view.scale, &st.measure);
                    cursor_for(hover, st.editor.tool, inside_image(size, p))
                }
                _ => "default",
            };
            area.set_cursor_from_name(Some(name));
        }
    });
    area.add_controller(motion);

    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    click.connect_pressed({
        let (state, drag, area, text, changed) =
            (state.clone(), drag.clone(), area.clone(), text.clone(), changed.clone());
        move |_, n_press, x, y| {
            if n_press != 2 || text.is_open() {
                return;
            }
            let hit = {
                let mut st = state.borrow_mut();
                let st = &mut *st;
                if st.editor.tool != Tool::Select {
                    return;
                }
                st.view(area.width() as f64, area.height() as f64).and_then(|view| {
                    let p = view.to_image(Point::new(x, y));
                    st.editor
                        .double_click(p, HIT_TOLERANCE / view.scale, &st.measure)
                        .map(|id| (id, view))
                })
            };
            let Some((id, view)) = hit else { return };
            // Open after this press is fully dispatched, so the drag gesture's
            // handling of the same press (which commits any open editor and
            // starts a move) cannot close the new editor, whatever the order.
            let (state, drag, area, text, changed) =
                (state.clone(), drag.clone(), area.clone(), text.clone(), changed.clone());
            gtk::glib::idle_add_local_once(move || {
                // End the move drag the second press started so the label
                // stays put while it is edited.
                finish_drag(&state, &drag);
                text.start_edit(id, view);
                area.queue_draw();
                changed();
            });
        }
    });
    area.add_controller(click);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;

    #[test]
    fn cursor_shows_what_a_press_would_do() {
        use crate::editor::Hover;
        use crate::geometry::Handle;
        let h = |x| cursor_for(Hover::Handle(x), Tool::Rect, true);
        assert_eq!(h(Handle::TopLeft), "nwse-resize");
        assert_eq!(h(Handle::BottomRight), "nwse-resize");
        assert_eq!(h(Handle::TopRight), "nesw-resize");
        assert_eq!(h(Handle::Left), "ew-resize");
        assert_eq!(h(Handle::Bottom), "ns-resize");
        assert_eq!(h(Handle::End), "move");
        assert_eq!(h(Handle::Center), "move");
        assert_eq!(cursor_for(Hover::Nothing, Tool::Marker, true), "crosshair");
        assert_eq!(cursor_for(Hover::Item, Tool::Select, true), "move");
        assert_eq!(cursor_for(Hover::Nothing, Tool::Rect, true), "crosshair");
        assert_eq!(cursor_for(Hover::Nothing, Tool::Rect, false), "default");
        assert_eq!(cursor_for(Hover::Nothing, Tool::Text, true), "text");
        assert_eq!(cursor_for(Hover::Nothing, Tool::Select, true), "default");
    }

    fn pixel(s: &mut cairo::ImageSurface, x: usize, y: usize) -> (u8, u8, u8) {
        s.flush();
        let stride = s.stride() as usize;
        let d = s.data().unwrap();
        let i = y * stride + x * 4;
        (d[i + 2], d[i + 1], d[i])
    }

    #[test]
    fn draws_scaled_image_with_items_and_letterbox() {
        let img = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 200).unwrap();
        {
            let cr = cairo::Context::new(&img).unwrap();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.paint().unwrap();
        }
        let mut st = CanvasState::new();
        st.image = Some(img);
        st.editor.tool = Tool::Redact;
        st.editor.set_color(Color::Black);
        st.editor.press(Point::new(100., 50.), 1.0, &st.measure);
        st.editor.release(Point::new(300., 150.));

        // 200x200 widget: scale 0.5, image occupies y 50..150.
        let mut out = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();
        {
            let cr = cairo::Context::new(&out).unwrap();
            draw(&st, &cr, 200, 200);
        }
        assert_eq!(pixel(&mut out, 100, 20), (128, 128, 128)); // letterbox
        assert_eq!(pixel(&mut out, 10, 60), (255, 255, 255)); // image
        assert_eq!(pixel(&mut out, 100, 100), (0, 0, 0)); // redaction at image (200,100)
    }
}

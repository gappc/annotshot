use crate::geometry::{drag_handle, grab_at, hit_test, translate, Handle};
use crate::history::History;
use crate::model::{Color, Item, ItemId, Shape, Style, TextMeasure, Width, MIN_SIZE};
use kurbo::{Point, Rect, Vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    Select,
    #[default]
    Rect,
    Arrow,
    Text,
    Redact,
    Marker,
    Highlight,
    Pixelate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PressOutcome {
    Nothing,
    StartText(Point),
    EditText(ItemId),
}

/// What a press at a point would grab; drives the mouse cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    Nothing,
    /// A handle or side of the selected item (any tool except Text).
    Handle(Handle),
    /// An item that the Select tool would pick up and move.
    Item,
}

enum Drag {
    None,
    Draw { start: Point, shape: Shape },
    Move { id: ItemId, last: Point, before: Vec<Item> },
    Handle { id: ItemId, handle: Handle, before: Vec<Item> },
}

pub struct Editor {
    pub items: Vec<Item>,
    pub selected: Option<ItemId>,
    pub tool: Tool,
    pub style: Style,
    history: History,
    dirty: bool,
    drag: Drag,
    next_id: ItemId,
    /// Current color of the Highlight tool while another tool is active.
    highlight_color: Color,
    /// Current color of the other tools while Highlight is active.
    other_color: Color,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            items: Vec::new(),
            selected: None,
            tool: Tool::default(),
            style: Style::default(),
            history: History::default(),
            dirty: false,
            drag: Drag::None,
            next_id: 1,
            highlight_color: Color::Yellow,
            other_color: Color::default(),
        }
    }

    pub fn reset(&mut self) {
        self.items.clear();
        self.selected = None;
        self.history.clear();
        self.dirty = false;
        self.drag = Drag::None;
    }

    /// Switch tools. Highlight keeps its own current color (yellow at first),
    /// so entering or leaving it swaps colors; a selection of the other kind
    /// is dropped so the toolbar color always belongs to what it would change.
    pub fn set_tool(&mut self, tool: Tool) {
        let was = self.tool == Tool::Highlight;
        let is = tool == Tool::Highlight;
        self.tool = tool;
        if was == is {
            return;
        }
        if is {
            self.other_color = self.style.color;
            self.style.color = self.highlight_color;
        } else {
            self.highlight_color = self.style.color;
            self.style.color = self.other_color;
        }
        let selected_highlight = self
            .selected
            .and_then(|id| self.items.iter().find(|i| i.id == id))
            .is_some_and(|i| matches!(i.shape, Shape::Highlight { .. }));
        if selected_highlight != is {
            self.selected = None;
        }
    }

    fn alloc_id(&mut self) -> ItemId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Record the current state as an undo step and mark dirty.
    fn commit_from(&mut self, before: Vec<Item>) {
        self.history.record(before);
        self.dirty = true;
    }

    fn push_item(&mut self, shape: Shape) -> ItemId {
        let before = self.items.clone();
        let id = self.alloc_id();
        self.items.push(Item { id, shape });
        self.selected = Some(id);
        self.adopt_selected_style();
        self.commit_from(before);
        id
    }

    /// Make the current style match the selected item, so the toolbar always
    /// shows the style the next shape will use. Text/Redact have no width.
    fn adopt_selected_style(&mut self) {
        let Some(item) = self.selected.and_then(|id| self.items.iter().find(|i| i.id == id)) else {
            return;
        };
        match &item.shape {
            Shape::Rect { style, .. } | Shape::Arrow { style, .. } | Shape::Marker { style, .. } => {
                self.style = *style
            }
            Shape::Text { color, .. } | Shape::Redact { color, .. } | Shape::Highlight { color, .. } => {
                self.style.color = *color
            }
            Shape::Pixelate { width, .. } => self.style.width = *width,
        }
    }

    fn index_of(&self, id: ItemId) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    /// The handle or side of the selected item under `p`, if the current tool
    /// may resize it (every tool but Text).
    fn grab_selected(&self, p: Point, tol: f64) -> Option<(ItemId, Handle)> {
        if self.tool == Tool::Text {
            return None;
        }
        let it = self.selected.and_then(|id| self.items.iter().find(|i| i.id == id))?;
        grab_at(&it.shape, p, tol * 8.0 / 6.0, tol).map(|h| (it.id, h))
    }

    /// The item the Select tool picks at `p`: the selected item anywhere inside
    /// its bounds (so a selected box moves by its inside, while its sides resize
    /// it), otherwise the topmost item hit.
    fn select_target(&self, p: Point, tol: f64, m: &dyn TextMeasure) -> Option<ItemId> {
        if let Some(it) = self.selected.and_then(|id| self.items.iter().find(|i| i.id == id))
            && crate::geometry::bounds(&it.shape, m).contains(p)
        {
            return Some(it.id);
        }
        // Highlights and pixelations are drawn under everything else, so they come last.
        let hit = |underlay: bool| {
            self.items
                .iter()
                .rev()
                .filter(|i| i.shape.is_underlay() == underlay)
                .find(|i| hit_test(&i.shape, p, tol, m))
                .map(|i| i.id)
        };
        hit(false).or_else(|| hit(true))
    }

    pub fn hover(&self, p: Point, tol: f64, m: &dyn TextMeasure) -> Hover {
        if let Some((_, h)) = self.grab_selected(p, tol) {
            return Hover::Handle(h);
        }
        if self.tool == Tool::Select && self.select_target(p, tol, m).is_some() {
            return Hover::Item;
        }
        Hover::Nothing
    }

    pub fn press(&mut self, p: Point, tol: f64, m: &dyn TextMeasure) -> PressOutcome {
        self.drag = Drag::None;
        if let Some((id, handle)) = self.grab_selected(p, tol) {
            let before = self.items.clone();
            self.drag = match handle {
                Handle::Center => Drag::Move { id, last: p, before },
                _ => Drag::Handle { id, handle, before },
            };
            return PressOutcome::Nothing;
        }
        match self.tool {
            Tool::Select => {
                let hit = self.select_target(p, tol, m);
                self.selected = hit;
                self.adopt_selected_style();
                if let Some(id) = hit {
                    self.drag = Drag::Move { id, last: p, before: self.items.clone() };
                }
            }
            Tool::Rect | Tool::Arrow | Tool::Redact | Tool::Marker | Tool::Highlight | Tool::Pixelate => {
                let shape = match self.tool {
                    Tool::Rect => Shape::Rect { rect: Rect::from_points(p, p), style: self.style },
                    Tool::Arrow => Shape::Arrow { start: p, end: p, style: self.style },
                    Tool::Marker => Shape::Marker { center: p, style: self.style },
                    Tool::Highlight => Shape::Highlight { rect: Rect::from_points(p, p), color: self.style.color },
                    Tool::Pixelate => Shape::Pixelate { rect: Rect::from_points(p, p), width: self.style.width },
                    _ => Shape::Redact { rect: Rect::from_points(p, p), color: self.style.color },
                };
                self.drag = Drag::Draw { start: p, shape };
            }
            Tool::Text => {
                let hit = self
                    .items
                    .iter()
                    .rev()
                    .find(|i| matches!(i.shape, Shape::Text { .. }) && hit_test(&i.shape, p, tol, m));
                return match hit {
                    Some(i) => PressOutcome::EditText(i.id),
                    None => PressOutcome::StartText(p),
                };
            }
        }
        PressOutcome::Nothing
    }

    pub fn drag_to(&mut self, p: Point) {
        match &mut self.drag {
            Drag::None => {}
            Drag::Draw { start, shape } => match shape {
                Shape::Rect { rect, .. } | Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } | Shape::Pixelate { rect, .. } => {
                    *rect = Rect::from_points(*start, p)
                }
                Shape::Arrow { end, .. } => *end = p,
                Shape::Marker { center, .. } => *center = p,
                Shape::Text { .. } => {}
            },
            Drag::Move { id, last, .. } => {
                let d: Vec2 = p - *last;
                *last = p;
                let id = *id;
                if let Some(it) = self.items.iter_mut().find(|i| i.id == id) {
                    translate(&mut it.shape, d);
                }
            }
            Drag::Handle { id, handle, .. } => {
                let (id, handle) = (*id, *handle);
                if let Some(it) = self.items.iter_mut().find(|i| i.id == id) {
                    drag_handle(&mut it.shape, handle, p);
                }
            }
        }
    }

    pub fn release(&mut self, p: Point) {
        self.drag_to(p);
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => {}
            Drag::Draw { shape, .. } => {
                let big_enough = match &shape {
                    Shape::Rect { rect, .. } | Shape::Redact { rect, .. } | Shape::Highlight { rect, .. } | Shape::Pixelate { rect, .. } => {
                        rect.width() >= MIN_SIZE || rect.height() >= MIN_SIZE
                    }
                    Shape::Arrow { start, end, .. } => start.distance(*end) >= MIN_SIZE,
                    Shape::Marker { .. } => true,
                    Shape::Text { .. } => false,
                };
                if big_enough {
                    self.push_item(shape);
                }
            }
            Drag::Move { before, .. } | Drag::Handle { before, .. } => {
                if before != self.items {
                    self.commit_from(before);
                }
            }
        }
    }

    pub fn double_click(&mut self, p: Point, tol: f64, m: &dyn TextMeasure) -> Option<ItemId> {
        self.items
            .iter()
            .rev()
            .find(|i| matches!(i.shape, Shape::Text { .. }) && hit_test(&i.shape, p, tol, m))
            .map(|i| i.id)
    }

    pub fn add_text(&mut self, origin: Point, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let shape = Shape::Text { origin, text: text.to_string(), color: self.style.color };
        self.push_item(shape);
    }

    pub fn set_text(&mut self, id: ItemId, text: &str) {
        let Some(idx) = self.index_of(id) else { return };
        let before = self.items.clone();
        if text.trim().is_empty() {
            self.items.remove(idx);
            if self.selected == Some(id) {
                self.selected = None;
            }
        } else if let Shape::Text { text: t, .. } = &mut self.items[idx].shape {
            if t == text {
                return;
            }
            *t = text.to_string();
        } else {
            return;
        }
        self.commit_from(before);
    }

    pub fn text_of(&self, id: ItemId) -> Option<(Point, String, Color)> {
        self.items.iter().find(|i| i.id == id).and_then(|i| match &i.shape {
            Shape::Text { origin, text, color } => Some((*origin, text.clone(), *color)),
            _ => None,
        })
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(idx) = self.selected.and_then(|id| self.index_of(id)) else { return false };
        let before = self.items.clone();
        self.items.remove(idx);
        self.selected = None;
        self.commit_from(before);
        true
    }

    fn modify_selected(&mut self, f: impl FnOnce(&mut Shape)) {
        let Some(idx) = self.selected.and_then(|id| self.index_of(id)) else { return };
        let before = self.items.clone();
        f(&mut self.items[idx].shape);
        if before != self.items {
            self.commit_from(before);
        }
    }

    pub fn set_color(&mut self, c: Color) {
        self.style.color = c;
        self.modify_selected(|s| match s {
            Shape::Rect { style, .. } | Shape::Arrow { style, .. } | Shape::Marker { style, .. } => {
                style.color = c
            }
            Shape::Text { color, .. } | Shape::Redact { color, .. } | Shape::Highlight { color, .. } => *color = c,
            Shape::Pixelate { .. } => {}
        });
    }

    pub fn set_width(&mut self, w: Width) {
        self.style.width = w;
        self.modify_selected(|s| match s {
            Shape::Rect { style, .. } | Shape::Arrow { style, .. } | Shape::Marker { style, .. } => {
                style.width = w
            }
            Shape::Pixelate { width, .. } => *width = w,
            Shape::Text { .. } | Shape::Redact { .. } | Shape::Highlight { .. } => {}
        });
    }

    fn apply(&mut self, items: Vec<Item>) {
        self.items = items;
        if self.selected.is_some_and(|id| self.index_of(id).is_none()) {
            self.selected = None;
        }
        self.adopt_selected_style();
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        match self.history.undo(self.items.clone()) {
            Some(items) => {
                self.apply(items);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.history.redo(self.items.clone()) {
            Some(items) => {
                self.apply(items);
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn provisional(&self) -> Option<&Shape> {
        match &self.drag {
            Drag::Draw { shape, .. } => Some(shape),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Size;

    struct FixedMeasure(Size);
    impl TextMeasure for FixedMeasure {
        fn text_size(&self, _: &str) -> Size {
            self.0
        }
    }
    const M: FixedMeasure = FixedMeasure(Size::new(20., 16.));

    #[test]
    fn drawing_tool_resizes_selected_box_via_corner_and_side() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.)); // selected after drawing
        draw(&mut e, pt(110., 60.), pt(150., 90.)); // corner drag with Box tool
        assert_eq!(e.items.len(), 1);
        assert_eq!(rect_of(&e, 0), Rect::new(10., 10., 150., 90.));
        draw(&mut e, pt(80., 10.), pt(80., 0.)); // top side
        assert_eq!(rect_of(&e, 0), Rect::new(10., 0., 150., 90.));
        assert_eq!(e.items.len(), 1);
        e.undo();
        assert_eq!(rect_of(&e, 0), Rect::new(10., 10., 150., 90.));
    }

    #[test]
    fn drawing_tool_moves_selected_arrow_end() {
        let mut e = Editor::new();
        e.tool = Tool::Arrow;
        draw(&mut e, pt(0., 0.), pt(100., 0.));
        draw(&mut e, pt(0., 0.), pt(20., 30.)); // start point, with Arrow tool
        assert_eq!(e.items.len(), 1);
        assert!(matches!(e.items[0].shape, Shape::Arrow { start, end, .. }
            if start == pt(20., 30.) && end == pt(100., 0.)));
    }

    #[test]
    fn drawing_tool_off_handles_still_draws_new_shape() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        draw(&mut e, pt(200., 200.), pt(250., 250.));
        assert_eq!(e.items.len(), 2);
        // Unselected box: its corner doesn't resize, a new box is drawn.
        e.selected = None;
        draw(&mut e, pt(110., 60.), pt(130., 80.));
        assert_eq!(e.items.len(), 3);
    }

    #[test]
    fn hover_reports_grab_targets() {
        let mut e = Editor::new();
        assert_eq!(e.hover(pt(10., 10.), 6., &M), Hover::Nothing);
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        assert_eq!(e.hover(pt(110., 60.), 6., &M), Hover::Handle(Handle::BottomRight));
        assert_eq!(e.hover(pt(10., 35.), 6., &M), Hover::Handle(Handle::Left));
        assert_eq!(e.hover(pt(60., 35.), 6., &M), Hover::Nothing); // Box tool, interior
        e.tool = Tool::Select;
        e.selected = None;
        assert_eq!(e.hover(pt(10., 35.), 6., &M), Hover::Item); // unselected item under Select
        assert_eq!(e.hover(pt(300., 300.), 6., &M), Hover::Nothing);
    }

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn draw(e: &mut Editor, a: Point, b: Point) {
        e.press(a, 6., &M);
        e.drag_to(b);
        e.release(b);
    }

    fn rect_of(e: &Editor, i: usize) -> Rect {
        match &e.items[i].shape {
            Shape::Rect { rect, .. } | Shape::Redact { rect, .. } => *rect,
            s => panic!("not a rect: {s:?}"),
        }
    }

    #[test]
    fn drawing_a_box_adds_selects_and_dirties() {
        let mut e = Editor::new();
        e.tool = Tool::Rect;
        e.press(pt(10., 10.), 6., &M);
        assert!(e.provisional().is_some());
        e.drag_to(pt(60., 40.));
        e.release(pt(60., 40.));
        assert!(e.provisional().is_none());
        assert_eq!(e.items.len(), 1);
        assert!(e.is_dirty());
        assert_eq!(e.selected, Some(e.items[0].id));
        assert_eq!(e.tool, Tool::Rect);
        assert_eq!(rect_of(&e, 0), Rect::new(10., 10., 60., 40.));
    }

    #[test]
    fn click_without_drag_creates_nothing() {
        for t in [Tool::Rect, Tool::Arrow, Tool::Redact] {
            let mut e = Editor::new();
            e.tool = t;
            e.press(pt(10., 10.), 6., &M);
            e.drag_to(pt(11., 11.));
            e.release(pt(11., 11.));
            assert!(e.items.is_empty());
            assert!(!e.is_dirty());
            assert!(!e.can_undo());
        }
    }

    #[test]
    fn thin_box_with_one_long_side_is_kept() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(80., 10.));
        assert_eq!(e.items.len(), 1);
    }

    #[test]
    fn arrow_and_redact_are_created() {
        let mut e = Editor::new();
        e.tool = Tool::Arrow;
        draw(&mut e, pt(0., 0.), pt(30., 40.));
        e.tool = Tool::Redact;
        e.style.color = Color::Black;
        draw(&mut e, pt(50., 50.), pt(90., 70.));
        assert_eq!(e.items.len(), 2);
        assert!(matches!(e.items[0].shape, Shape::Arrow { end, .. } if end == pt(30., 40.)));
        assert!(matches!(e.items[1].shape, Shape::Redact { color: Color::Black, .. }));
        assert_ne!(e.items[0].id, e.items[1].id);
    }

    #[test]
    fn move_is_one_undo_step() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        e.tool = Tool::Select;
        // The box is selected after drawing: its inside moves it (its sides resize).
        e.press(pt(60., 30.), 6., &M);
        e.drag_to(pt(70., 35.));
        e.drag_to(pt(80., 40.));
        e.drag_to(pt(90., 50.));
        e.release(pt(90., 50.));
        assert_eq!(rect_of(&e, 0), Rect::new(40., 30., 140., 80.));
        assert!(e.undo());
        assert_eq!(rect_of(&e, 0), Rect::new(10., 10., 110., 60.));
        assert!(e.undo());
        assert!(e.items.is_empty());
        assert!(!e.undo());
    }

    #[test]
    fn handle_drag_resizes_in_one_step() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        e.tool = Tool::Select;
        e.press(pt(110., 60.), 6., &M);
        e.drag_to(pt(150., 80.));
        e.release(pt(160., 90.));
        assert_eq!(rect_of(&e, 0), Rect::new(10., 10., 160., 90.));
        e.undo();
        assert_eq!(rect_of(&e, 0), Rect::new(10., 10., 110., 60.));
    }

    #[test]
    fn press_release_on_item_selects_without_history() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        e.mark_saved();
        e.selected = None;
        e.tool = Tool::Select;
        e.press(pt(10., 30.), 6., &M);
        e.release(pt(10., 30.));
        assert_eq!(e.selected, Some(e.items[0].id));
        assert!(!e.is_dirty());
        assert!(e.undo()); // only the creation step exists
        assert!(!e.can_undo());
    }

    #[test]
    fn select_on_empty_space_deselects() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        assert!(e.selected.is_some());
        e.tool = Tool::Select;
        e.press(pt(300., 300.), 6., &M);
        e.release(pt(300., 300.));
        assert_eq!(e.selected, None);
    }

    #[test]
    fn select_picks_topmost_item() {
        let mut e = Editor::new();
        e.tool = Tool::Redact;
        draw(&mut e, pt(0., 0.), pt(100., 100.));
        draw(&mut e, pt(50., 50.), pt(150., 150.));
        e.tool = Tool::Select;
        e.selected = None;
        e.press(pt(75., 75.), 6., &M);
        e.release(pt(75., 75.));
        assert_eq!(e.selected, Some(e.items[1].id));
    }

    #[test]
    fn redo_after_undo_and_new_change_clears_redo() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(60., 40.));
        e.undo();
        assert!(e.can_redo());
        draw(&mut e, pt(20., 20.), pt(80., 80.));
        assert!(!e.can_redo());
    }

    #[test]
    fn undo_redo_restore_items_and_clear_stale_selection() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(60., 40.));
        e.undo();
        assert_eq!(e.selected, None);
        assert!(e.redo());
        assert_eq!(e.items.len(), 1);
        assert!(!e.redo());
    }

    #[test]
    fn set_color_changes_selected_item_and_style() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(60., 40.));
        e.set_color(Color::Blue);
        assert_eq!(e.style.color, Color::Blue);
        assert!(matches!(e.items[0].shape, Shape::Rect { style, .. } if style.color == Color::Blue));
        e.undo();
        assert!(matches!(e.items[0].shape, Shape::Rect { style, .. } if style.color == Color::Red));
        e.undo();
        assert!(e.items.is_empty());
    }

    #[test]
    fn set_width_without_selection_only_updates_style() {
        let mut e = Editor::new();
        e.set_width(Width::Thick);
        assert_eq!(e.style.width, Width::Thick);
        assert!(!e.can_undo() && !e.is_dirty());
    }

    #[test]
    fn set_width_ignored_by_redact() {
        let mut e = Editor::new();
        e.tool = Tool::Redact;
        draw(&mut e, pt(0., 0.), pt(50., 50.));
        e.set_width(Width::Thin);
        e.undo();
        assert!(e.items.is_empty()); // width change added no step
    }

    #[test]
    fn set_text_empty_removes_item() {
        let mut e = Editor::new();
        e.add_text(pt(5., 5.), "hello");
        let id = e.items[0].id;
        e.set_text(id, "  ");
        assert!(e.items.is_empty());
        assert_eq!(e.selected, None);
        e.undo();
        assert_eq!(e.items.len(), 1);
    }

    #[test]
    fn set_text_unchanged_records_nothing_and_change_records_one() {
        let mut e = Editor::new();
        e.add_text(pt(5., 5.), "hello");
        let id = e.items[0].id;
        e.set_text(id, "hello");
        e.set_text(id, "bye");
        assert_eq!(e.text_of(id), Some((pt(5., 5.), "bye".into(), Color::Red)));
        e.undo();
        assert_eq!(e.text_of(id).unwrap().1, "hello");
        e.undo();
        assert!(e.items.is_empty());
    }

    #[test]
    fn add_text_blank_is_ignored() {
        let mut e = Editor::new();
        e.add_text(pt(5., 5.), "");
        e.add_text(pt(5., 5.), " \n ");
        assert!(e.items.is_empty());
        assert!(!e.is_dirty() && !e.can_undo());
    }

    #[test]
    fn add_text_uses_current_color_and_selects() {
        let mut e = Editor::new();
        e.style.color = Color::Green;
        e.add_text(pt(5., 5.), "x");
        assert_eq!(e.selected, Some(e.items[0].id));
        assert_eq!(e.text_of(e.items[0].id).unwrap().2, Color::Green);
    }

    #[test]
    fn text_tool_on_existing_label_edits_it() {
        let mut e = Editor::new();
        e.add_text(pt(5., 5.), "hello");
        let id = e.items[0].id;
        e.tool = Tool::Text;
        assert_eq!(e.press(pt(10., 10.), 6., &M), PressOutcome::EditText(id));
        assert_eq!(e.press(pt(300., 300.), 6., &M), PressOutcome::StartText(pt(300., 300.)));
        assert_eq!(e.double_click(pt(10., 10.), 6., &M), Some(id));
        assert_eq!(e.double_click(pt(300., 300.), 6., &M), None);
    }

    #[test]
    fn delete_selected_removes_and_is_undoable() {
        let mut e = Editor::new();
        assert!(!e.delete_selected());
        draw(&mut e, pt(10., 10.), pt(60., 40.));
        assert!(e.delete_selected());
        assert!(e.items.is_empty() && e.selected.is_none());
        e.undo();
        assert_eq!(e.items.len(), 1);
    }

    #[test]
    fn mark_saved_clears_dirty_and_next_change_sets_it() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(60., 40.));
        assert!(e.is_dirty());
        e.mark_saved();
        assert!(!e.is_dirty());
        e.set_color(Color::Yellow);
        assert!(e.is_dirty());
    }

    #[test]
    fn reset_clears_everything_but_keeps_tool_and_style() {
        let mut e = Editor::new();
        e.tool = Tool::Arrow;
        e.style.color = Color::Blue;
        draw(&mut e, pt(10., 10.), pt(60., 40.));
        e.reset();
        assert!(e.items.is_empty() && e.selected.is_none());
        assert!(!e.is_dirty() && !e.can_undo() && !e.can_redo());
        assert_eq!(e.tool, Tool::Arrow);
        assert_eq!(e.style.color, Color::Blue);
    }

    #[test]
    fn selecting_an_item_adopts_its_style() {
        let mut e = Editor::new();
        e.set_color(Color::Blue);
        e.set_width(Width::Thick);
        draw(&mut e, pt(10., 10.), pt(100., 100.));
        e.selected = None;
        e.set_color(Color::Red);
        e.set_width(Width::Thin);
        e.tool = Tool::Select;
        e.press(pt(10., 50.), 2.0, &M);
        e.release(pt(10., 50.));
        assert_eq!(e.style, Style { color: Color::Blue, width: Width::Thick });
    }

    #[test]
    fn selecting_redact_adopts_color_but_keeps_width() {
        let mut e = Editor::new();
        e.tool = Tool::Redact;
        e.set_color(Color::Black);
        draw(&mut e, pt(10., 10.), pt(100., 100.));
        e.selected = None;
        e.set_color(Color::Green);
        e.set_width(Width::Thin);
        e.tool = Tool::Select;
        e.press(pt(50., 50.), 2.0, &M);
        e.release(pt(50., 50.));
        assert_eq!(e.style, Style { color: Color::Black, width: Width::Thin });
    }

    #[test]
    fn undo_restoring_selected_item_adopts_its_style() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(100., 100.)); // red, medium, selected
        e.set_color(Color::Blue);
        assert!(e.undo());
        assert!(e.selected.is_some());
        assert_eq!(e.style.color, Color::Red);
        assert!(e.redo());
        assert_eq!(e.style.color, Color::Blue);
    }

    fn center_of(e: &Editor, i: usize) -> Point {
        match e.items[i].shape {
            Shape::Marker { center, .. } => center,
            ref s => panic!("not a marker: {s:?}"),
        }
    }

    /// Number shown in marker `id` (same rule as `render::draw_items`).
    fn marker_number(items: &[Item], id: ItemId) -> Option<u32> {
        let mut n = 0;
        for item in items {
            if let Shape::Marker { .. } = item.shape {
                n += 1;
                if item.id == id {
                    return Some(n);
                }
            } else if item.id == id {
                return None;
            }
        }
        None
    }

    fn numbers(e: &Editor) -> Vec<Option<u32>> {
        e.items.iter().map(|i| marker_number(&e.items, i.id)).collect()
    }

    #[test]
    fn marker_tool_places_on_click() {
        let mut e = Editor::new();
        e.tool = Tool::Marker;
        e.press(pt(50., 50.), 6., &M);
        assert!(matches!(e.provisional(), Some(Shape::Marker { .. })));
        e.release(pt(50., 50.));
        assert_eq!(e.items.len(), 1);
        assert_eq!(center_of(&e, 0), pt(50., 50.));
        assert_eq!(e.selected, Some(e.items[0].id));
        assert!(e.is_dirty());
    }

    #[test]
    fn marker_press_then_drag_positions_it() {
        let mut e = Editor::new();
        e.tool = Tool::Marker;
        draw(&mut e, pt(50., 50.), pt(80., 90.));
        assert_eq!(center_of(&e, 0), pt(80., 90.));
    }

    #[test]
    fn numbers_follow_order_and_close_gaps() {
        let mut e = Editor::new();
        e.tool = Tool::Marker;
        draw(&mut e, pt(100., 100.), pt(100., 100.));
        e.tool = Tool::Rect;
        draw(&mut e, pt(300., 300.), pt(350., 350.));
        e.tool = Tool::Marker;
        draw(&mut e, pt(200., 100.), pt(200., 100.));
        draw(&mut e, pt(300., 100.), pt(300., 100.));
        assert_eq!(numbers(&e), vec![Some(1), None, Some(2), Some(3)]);
        e.selected = Some(e.items[2].id);
        assert!(e.delete_selected());
        assert_eq!(numbers(&e), vec![Some(1), None, Some(2)]);
        assert_eq!(center_of(&e, 2), pt(300., 100.));
        assert!(e.undo());
        assert_eq!(numbers(&e), vec![Some(1), None, Some(2), Some(3)]);
        assert_eq!(center_of(&e, 2), pt(200., 100.));
    }

    #[test]
    fn marker_tool_on_selected_marker_moves_it() {
        let mut e = Editor::new();
        e.tool = Tool::Marker;
        draw(&mut e, pt(100., 100.), pt(100., 100.));
        assert_eq!(e.hover(pt(105., 100.), 6., &M), Hover::Handle(Handle::Center));
        draw(&mut e, pt(105., 100.), pt(155., 120.)); // grab off-center: no jump
        assert_eq!(e.items.len(), 1);
        assert_eq!(center_of(&e, 0), pt(150., 120.));
        assert!(e.undo());
        assert_eq!(center_of(&e, 0), pt(100., 100.));
        // Text tool does not grab it.
        e.tool = Tool::Text;
        assert_eq!(e.hover(pt(100., 100.), 6., &M), Hover::Nothing);
    }

    #[test]
    fn style_buttons_change_selected_marker() {
        let mut e = Editor::new();
        e.tool = Tool::Marker;
        draw(&mut e, pt(100., 100.), pt(100., 100.));
        e.set_width(Width::Thick);
        e.set_color(Color::Blue);
        assert!(matches!(e.items[0].shape, Shape::Marker { style, .. }
            if style == Style { color: Color::Blue, width: Width::Thick }));
        assert!(e.undo());
        assert!(matches!(e.items[0].shape, Shape::Marker { style, .. } if style.width == Width::Thick && style.color == Color::Red));
        e.selected = None;
        e.style = Style::default();
        e.tool = Tool::Select;
        e.press(pt(100., 100.), 6., &M);
        e.release(pt(100., 100.));
        assert_eq!(e.style.width, Width::Thick); // adopted from the marker
    }

    fn highlight_color(e: &Editor, i: usize) -> Color {
        match e.items[i].shape {
            Shape::Highlight { color, .. } => color,
            ref s => panic!("not a highlight: {s:?}"),
        }
    }

    #[test]
    fn highlight_tool_draws_yellow_and_restores_color() {
        let mut e = Editor::new();
        e.set_tool(Tool::Rect);
        e.set_color(Color::Blue);
        e.set_tool(Tool::Highlight);
        assert_eq!(e.style.color, Color::Yellow);
        draw(&mut e, pt(10., 10.), pt(110., 40.));
        assert_eq!(highlight_color(&e, 0), Color::Yellow);
        e.set_color(Color::Green); // recolors the selected highlight
        assert_eq!(highlight_color(&e, 0), Color::Green);
        e.set_tool(Tool::Rect);
        assert_eq!(e.style.color, Color::Blue);
        e.set_tool(Tool::Highlight);
        assert_eq!(e.style.color, Color::Green); // remembered
        assert!(e.undo());
        assert_eq!(highlight_color(&e, 0), Color::Yellow);
    }

    #[test]
    fn set_tool_clears_mismatched_selection() {
        let mut e = Editor::new();
        draw(&mut e, pt(10., 10.), pt(110., 60.)); // red box, selected
        e.set_tool(Tool::Highlight);
        assert_eq!(e.selected, None);
        assert_eq!(e.style.color, Color::Yellow);
        draw(&mut e, pt(200., 200.), pt(250., 250.));
        assert!(e.selected.is_some());
        e.set_tool(Tool::Select);
        assert_eq!(e.selected, None);
        assert_eq!(e.style.color, Color::Red);
        // Switching between other tools keeps the selection.
        e.press(pt(10., 30.), 6., &M);
        e.release(pt(10., 30.));
        let sel = e.selected;
        assert!(sel.is_some());
        e.set_tool(Tool::Arrow);
        assert_eq!(e.selected, sel);
    }

    #[test]
    fn select_prefers_items_over_highlights() {
        let mut e = Editor::new();
        e.set_tool(Tool::Arrow);
        draw(&mut e, pt(0., 50.), pt(200., 50.));
        e.set_tool(Tool::Highlight);
        draw(&mut e, pt(50., 30.), pt(150., 70.)); // drawn later, yet shown underneath
        e.set_tool(Tool::Select);
        e.press(pt(100., 50.), 6., &M);
        e.release(pt(100., 50.));
        assert_eq!(e.selected, Some(e.items[0].id));
        e.press(pt(100., 65.), 6., &M); // highlight only
        e.release(pt(100., 65.));
        assert_eq!(e.selected, Some(e.items[1].id));
    }

    #[test]
    fn pixelate_tool_uses_width_and_keeps_color() {
        let mut e = Editor::new();
        e.set_width(Width::Thick);
        e.set_tool(Tool::Pixelate);
        assert_eq!(e.style.color, Color::Red); // no color swap, unlike Highlight
        draw(&mut e, pt(10., 10.), pt(110., 60.));
        assert_eq!(e.items[0].shape, Shape::Pixelate { rect: Rect::new(10., 10., 110., 60.), width: Width::Thick });
        e.set_color(Color::Blue); // no effect on a pixelation, no undo step
        e.set_width(Width::Thin);
        assert_eq!(e.items[0].shape, Shape::Pixelate { rect: Rect::new(10., 10., 110., 60.), width: Width::Thin });
        assert!(e.undo());
        assert!(matches!(e.items[0].shape, Shape::Pixelate { width: Width::Thick, .. }));
        assert!(e.undo());
        assert!(e.items.is_empty());
    }

    #[test]
    fn select_prefers_items_over_pixelations() {
        let mut e = Editor::new();
        e.set_tool(Tool::Arrow);
        draw(&mut e, pt(0., 50.), pt(200., 50.));
        e.set_tool(Tool::Pixelate);
        draw(&mut e, pt(50., 30.), pt(150., 70.));
        e.set_tool(Tool::Select);
        e.selected = None;
        e.press(pt(100., 50.), 6., &M);
        e.release(pt(100., 50.));
        assert_eq!(e.selected, Some(e.items[0].id));
    }
}

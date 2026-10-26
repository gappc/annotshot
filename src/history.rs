use crate::model::Item;

/// Snapshot-based undo/redo: each step stores the full item list.
#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Item>>,
    redo: Vec<Vec<Item>>,
}

impl History {
    /// Remember `before` as an undo step; invalidates redo.
    pub fn record(&mut self, before: Vec<Item>) {
        self.undo.push(before);
        self.redo.clear();
    }

    pub fn undo(&mut self, current: Vec<Item>) -> Option<Vec<Item>> {
        let prev = self.undo.pop()?;
        self.redo.push(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: Vec<Item>) -> Option<Vec<Item>> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Shape};
    use kurbo::Point;

    fn items(n: usize) -> Vec<Item> {
        (0..n as u64)
            .map(|id| Item {
                id,
                shape: Shape::Text { origin: Point::ZERO, text: "x".into(), color: Color::Red },
            })
            .collect()
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut h = History::default();
        assert!(!h.can_undo() && !h.can_redo());
        assert!(h.undo(items(1)).is_none());
        h.record(items(0));
        assert!(h.can_undo());
        assert_eq!(h.undo(items(1)), Some(items(0)));
        assert!(h.can_redo() && !h.can_undo());
        assert_eq!(h.redo(items(0)), Some(items(1)));
        assert!(h.can_undo() && !h.can_redo());
        assert!(h.redo(items(1)).is_none());
    }

    #[test]
    fn record_clears_redo_and_clear_empties() {
        let mut h = History::default();
        h.record(items(0));
        h.undo(items(1));
        h.record(items(2));
        assert!(!h.can_redo());
        h.clear();
        assert!(!h.can_undo());
    }
}

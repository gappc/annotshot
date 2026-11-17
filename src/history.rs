/// Snapshot-based undo/redo: each step stores a full snapshot `T`.
pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        History { undo: Vec::new(), redo: Vec::new() }
    }
}

impl<T> History<T> {
    /// Remember `before` as an undo step; invalidates redo.
    pub fn record(&mut self, before: T) {
        self.undo.push(before);
        self.redo.clear();
    }

    /// Pop the last step. `current` builds the snapshot to keep for redo; it
    /// sees the popped step, so a step can say what its counterpart holds.
    pub fn undo(&mut self, current: impl FnOnce(&T) -> T) -> Option<T> {
        let prev = self.undo.pop()?;
        self.redo.push(current(&prev));
        Some(prev)
    }

    pub fn redo(&mut self, current: impl FnOnce(&T) -> T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current(&next));
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
    use crate::model::{Color, Item, Shape, Width};
    use kurbo::Point;

    fn items(n: usize) -> Vec<Item> {
        (0..n as u64)
            .map(|id| Item {
                id,
                shape: Shape::Text { origin: Point::ZERO, text: "x".into(), color: Color::Red, width: Width::Medium },
            })
            .collect()
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut h = History::<Vec<Item>>::default();
        assert!(!h.can_undo() && !h.can_redo());
        assert!(h.undo(|_| items(1)).is_none());
        h.record(items(0));
        assert!(h.can_undo());
        assert_eq!(h.undo(|_| items(1)), Some(items(0)));
        assert!(h.can_redo() && !h.can_undo());
        assert_eq!(h.redo(|_| items(0)), Some(items(1)));
        assert!(h.can_undo() && !h.can_redo());
        assert!(h.redo(|_| items(1)).is_none());
    }

    #[test]
    fn record_clears_redo_and_clear_empties() {
        let mut h = History::<Vec<Item>>::default();
        h.record(items(0));
        h.undo(|_| items(1));
        h.record(items(2));
        assert!(!h.can_redo());
        h.clear();
        assert!(!h.can_undo());
    }

    #[test]
    fn counterpart_sees_the_popped_step() {
        let mut h = History::<usize>::default();
        h.record(3);
        assert_eq!(h.undo(|popped| popped * 10), Some(3));
        assert_eq!(h.redo(|popped| popped + 1), Some(30));
        assert_eq!(h.undo(|_| 0), Some(31));
    }
}

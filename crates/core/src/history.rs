/// Snapshot-based undo/redo with a bounded number of undo steps.
#[derive(Debug, Clone)]
pub struct History<T: Clone + PartialEq> {
    states: Vec<T>,
    index: usize,
    saved: T,
    capacity: usize,
}

impl<T: Clone + PartialEq> History<T> {
    pub fn new(initial: T, capacity: usize) -> Self {
        Self {
            states: vec![initial.clone()],
            index: 0,
            saved: initial,
            capacity,
        }
    }

    pub fn current(&self) -> &T {
        &self.states[self.index]
    }

    pub fn commit(&mut self, next: T) {
        if *self.current() == next {
            return;
        }
        self.states.truncate(self.index + 1);
        self.states.push(next);
        if self.states.len() > self.capacity + 1 {
            self.states.remove(0);
        }
        self.index = self.states.len() - 1;
    }

    pub fn undo(&mut self) -> bool {
        if self.index == 0 {
            return false;
        }
        self.index -= 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.index + 1 >= self.states.len() {
            return false;
        }
        self.index += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.states.len()
    }

    pub fn is_dirty(&self) -> bool {
        *self.current() != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.current().clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut h = History::new(0u32, 100);
        h.commit(1);
        h.commit(2);
        assert!(h.is_dirty());
        assert!(h.undo());
        assert_eq!(*h.current(), 1);
        assert!(h.undo());
        assert_eq!(*h.current(), 0);
        assert!(!h.undo());
        assert!(h.redo());
        assert_eq!(*h.current(), 1);
    }

    #[test]
    fn capacity_drops_oldest() {
        let mut h = History::new(0u32, 2);
        h.commit(1);
        h.commit(2);
        h.commit(3);
        assert_eq!(*h.current(), 3);
        assert!(h.undo());
        assert!(h.undo());
        assert!(!h.undo());
        assert_eq!(*h.current(), 1);
    }

    #[test]
    fn mark_saved_clears_dirty_and_is_idempotent() {
        let mut h = History::new(0u32, 10);
        h.commit(1);
        h.mark_saved();
        assert!(!h.is_dirty());
        h.mark_saved();
        assert!(!h.is_dirty());
    }
}

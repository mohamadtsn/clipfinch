//! The Paste Queue: collect several copies, then paste them one by one.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub clip_id: i64,
    pub label: String,
}

#[derive(Debug, Default)]
pub struct PasteQueue {
    items: Vec<QueueItem>,
    reversed: bool,
    active: bool,
}

impl PasteQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, on: bool) {
        self.active = on;
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub fn toggle_reverse(&mut self) {
        self.reversed = !self.reversed;
    }

    pub fn push(&mut self, clip_id: i64, label: String) {
        self.items.push(QueueItem { clip_id, label });
    }

    /// Removes and returns the next item to paste
    /// (oldest first, or newest first when reversed).
    pub fn next(&mut self) -> Option<QueueItem> {
        if self.items.is_empty() {
            None
        } else if self.reversed {
            self.items.pop()
        } else {
            Some(self.items.remove(0))
        }
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_and_reverse() {
        let mut q = PasteQueue::new();
        q.push(1, "a".into());
        q.push(2, "b".into());
        q.push(3, "c".into());
        assert_eq!(q.next().unwrap().clip_id, 1);
        q.toggle_reverse();
        assert_eq!(q.next().unwrap().clip_id, 3);
        assert_eq!(q.next().unwrap().clip_id, 2);
        assert!(q.next().is_none());
    }
}

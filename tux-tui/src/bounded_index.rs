/// A wrapping index for bounded collections.
///
/// Provides `next`/`prev` with modular wrapping and `clamp_to` for when
/// the backing collection shrinks.  The collection length is passed per-call
/// rather than stored, because the backing data can change independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoundedIndex(usize);

impl BoundedIndex {
    /// Create a new bounded index with the given initial value.
    #[allow(dead_code)]
    pub fn new(val: usize) -> Self {
        Self(val)
    }

    /// Get the current index value.
    pub fn get(self) -> usize {
        self.0
    }

    /// Set the index directly.
    pub fn set(&mut self, val: usize) {
        self.0 = val;
    }

    /// Advance to the next position, wrapping around.
    /// No-op when `len == 0`.
    pub fn next(&mut self, len: usize) {
        if len > 0 {
            self.0 = (self.0 + 1) % len;
        }
    }

    /// Move to the previous position, wrapping around.
    /// No-op when `len == 0`.
    pub fn prev(&mut self, len: usize) {
        if len > 0 {
            self.0 = (self.0 + len - 1) % len;
        }
    }

    /// Clamp the index so it stays valid after the collection shrinks.
    /// If `len == 0`, resets to 0.
    pub fn clamp_to(&mut self, len: usize) {
        if len == 0 {
            self.0 = 0;
        } else if self.0 >= len {
            self.0 = len - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_wraps_around() {
        let mut idx = BoundedIndex::new(2);
        idx.next(3);
        assert_eq!(idx.get(), 0);
    }

    #[test]
    fn prev_wraps_around() {
        let mut idx = BoundedIndex::new(0);
        idx.prev(3);
        assert_eq!(idx.get(), 2);
    }

    #[test]
    fn next_increments() {
        let mut idx = BoundedIndex::new(0);
        idx.next(5);
        assert_eq!(idx.get(), 1);
    }

    #[test]
    fn prev_decrements() {
        let mut idx = BoundedIndex::new(3);
        idx.prev(5);
        assert_eq!(idx.get(), 2);
    }

    #[test]
    fn next_noop_on_empty() {
        let mut idx = BoundedIndex::new(0);
        idx.next(0);
        assert_eq!(idx.get(), 0);
    }

    #[test]
    fn prev_noop_on_empty() {
        let mut idx = BoundedIndex::new(0);
        idx.prev(0);
        assert_eq!(idx.get(), 0);
    }

    #[test]
    fn clamp_shrinks() {
        let mut idx = BoundedIndex::new(5);
        idx.clamp_to(3);
        assert_eq!(idx.get(), 2);
    }

    #[test]
    fn clamp_noop_when_valid() {
        let mut idx = BoundedIndex::new(1);
        idx.clamp_to(5);
        assert_eq!(idx.get(), 1);
    }

    #[test]
    fn clamp_to_zero_resets() {
        let mut idx = BoundedIndex::new(3);
        idx.clamp_to(0);
        assert_eq!(idx.get(), 0);
    }

    #[test]
    fn default_is_zero() {
        let idx = BoundedIndex::default();
        assert_eq!(idx.get(), 0);
    }
}

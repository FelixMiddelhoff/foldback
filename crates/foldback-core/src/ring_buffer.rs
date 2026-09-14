// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded retention ring buffer — holds the last N entries, evicting the
//! oldest as new ones arrive past capacity. Used for the tick-hash
//! retention window and the snapshot ring buffer.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> RingBuffer<T> {
    /// `capacity == 0` is valid — every push is immediately evicted, so
    /// the buffer always reports empty. Not an error case to special-case.
    pub fn new(capacity: usize) -> Self {
        RingBuffer {
            capacity,
            items: VecDeque::with_capacity(capacity.min(1024)),
        }
    }

    pub fn push(&mut self, item: T) {
        if self.capacity == 0 {
            return;
        }
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub fn back(&self) -> Option<&T> {
        self.items.back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_zero_stays_empty() {
        let mut rb = RingBuffer::new(0);
        for i in 0..10 {
            rb.push(i);
        }
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn capacity_one_keeps_only_latest() {
        let mut rb = RingBuffer::new(1);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.len(), 1);
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn wraparound_evicts_oldest_first() {
        let mut rb = RingBuffer::new(3);
        for i in 1..=5 {
            rb.push(i);
        }
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn under_capacity_keeps_everything() {
        let mut rb = RingBuffer::new(10);
        rb.push(1);
        rb.push(2);
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
    }

    proptest::proptest! {
        #[test]
        fn retention_invariant_never_violated(capacity in 0usize..50, pushes in proptest::collection::vec(0i32..1000, 0..200)) {
            let mut rb = RingBuffer::new(capacity);
            for p in pushes {
                rb.push(p);
                proptest::prop_assert!(rb.len() <= capacity);
            }
        }
    }
}

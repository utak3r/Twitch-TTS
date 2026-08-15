use super::models::{MessageStatus, SpokenItem};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct OverflowQueue {
    inner: Arc<Mutex<InnerQueue>>,
}

#[derive(Debug)]
struct InnerQueue {
    items: VecDeque<SpokenItem>,
    max_size: usize,
}

impl OverflowQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerQueue {
                items: VecDeque::new(),
                max_size: max_size.max(1),
            })),
        }
    }

    /// Pushes an item to the queue. If queue is full, drops the oldest item and returns it
    /// with status updated to `MessageStatus::DroppedOverflow`.
    pub fn push(&self, item: SpokenItem) -> Option<SpokenItem> {
        let mut queue = self.inner.lock().unwrap();
        let mut dropped = None;

        while queue.items.len() >= queue.max_size {
            if let Some(mut oldest) = queue.items.pop_front() {
                oldest.status = MessageStatus::DroppedOverflow;
                dropped = Some(oldest);
            }
        }

        queue.items.push_back(item);
        dropped
    }

    pub fn pop(&self) -> Option<SpokenItem> {
        let mut queue = self.inner.lock().unwrap();
        queue.items.pop_front()
    }

    pub fn clear(&self) -> Vec<SpokenItem> {
        let mut queue = self.inner.lock().unwrap();
        let mut cleared: Vec<SpokenItem> = queue.items.drain(..).collect();
        for item in &mut cleared {
            item.status = MessageStatus::DroppedOverflow;
        }
        cleared
    }

    pub fn len(&self) -> usize {
        let queue = self.inner.lock().unwrap();
        queue.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set_max_size(&self, max_size: usize) {
        let mut queue = self.inner.lock().unwrap();
        queue.max_size = max_size.max(1);
    }
}

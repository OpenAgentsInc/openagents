use std::sync::{Arc, atomic::Ordering};

use super::invalidator::{InvalidationState, LAYOUT_BIT, PAINT_BIT, PREPAINT_BIT};

#[derive(Clone)]
pub struct WindowHandle {
    pub(crate) state: Arc<InvalidationState>,
}

impl WindowHandle {
    pub fn request_layout(&self) {
        self.state
            .flags
            .fetch_or(LAYOUT_BIT | PREPAINT_BIT | PAINT_BIT, Ordering::SeqCst);
    }

    pub fn request_prepaint(&self) {
        self.state
            .flags
            .fetch_or(PREPAINT_BIT | PAINT_BIT, Ordering::SeqCst);
    }

    pub fn request_paint(&self) {
        self.state.flags.fetch_or(PAINT_BIT, Ordering::SeqCst);
    }

    pub fn is_dirty(&self) -> bool {
        self.state.flags.load(Ordering::SeqCst) != 0
    }
}

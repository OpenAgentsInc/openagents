use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use super::window_handle::WindowHandle;

pub(crate) const LAYOUT_BIT: u8 = 0b0000_0001;
pub(crate) const PREPAINT_BIT: u8 = 0b0000_0010;
pub(crate) const PAINT_BIT: u8 = 0b0000_0100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvalidationFlags {
    pub layout: bool,
    pub prepaint: bool,
    pub paint: bool,
}

impl InvalidationFlags {
    fn from_bits(bits: u8) -> Self {
        Self {
            layout: bits & LAYOUT_BIT != 0,
            prepaint: bits & PREPAINT_BIT != 0,
            paint: bits & PAINT_BIT != 0,
        }
    }

    pub fn any(&self) -> bool {
        self.layout || self.prepaint || self.paint
    }
}

pub(crate) struct InvalidationState {
    pub(crate) flags: AtomicU8,
}

#[derive(Clone)]
pub struct Invalidator {
    pub(crate) state: Arc<InvalidationState>,
}

impl Invalidator {
    pub fn new() -> Self {
        Self {
            state: Arc::new(InvalidationState {
                flags: AtomicU8::new(0),
            }),
        }
    }

    pub fn handle(&self) -> WindowHandle {
        WindowHandle {
            state: self.state.clone(),
        }
    }

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

    pub fn take(&self) -> InvalidationFlags {
        let bits = self.state.flags.swap(0, Ordering::SeqCst);
        InvalidationFlags::from_bits(bits)
    }

    pub fn is_dirty(&self) -> bool {
        self.state.flags.load(Ordering::SeqCst) != 0
    }
}

impl Default for Invalidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalidator_flags() {
        let invalidator = Invalidator::new();
        let handle = invalidator.handle();

        assert!(!invalidator.is_dirty());

        handle.request_paint();
        let flags = invalidator.take();
        assert_eq!(
            flags,
            InvalidationFlags {
                layout: false,
                prepaint: false,
                paint: true,
            }
        );

        handle.request_layout();
        let flags = invalidator.take();
        assert_eq!(
            flags,
            InvalidationFlags {
                layout: true,
                prepaint: true,
                paint: true,
            }
        );
        assert!(!invalidator.is_dirty());
    }
}

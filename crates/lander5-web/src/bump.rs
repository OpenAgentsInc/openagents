//! A bump allocator for the runner, in place of the standard library's
//! dlmalloc (about 7 KB of the module before compression).
//!
//! The runner allocates only while it starts (the two shader sources, the
//! context options, the frame closure) and never frees in a frame, so
//! memory that is never reused costs a few kilobytes once. Each allocation
//! takes the next aligned bytes past the end of what is in use, growing
//! memory by whole pages when it runs out; freeing does nothing. A wasm32
//! module here has one thread.

use std::alloc::{GlobalAlloc, Layout};
use std::arch::wasm32::{memory_grow, memory_size};
use std::cell::Cell;

const PAGE: usize = 65_536;

struct Bump {
    /// The next free address, or 0 before the first allocation.
    next: Cell<usize>,
}

// SAFETY: a wasm32 module without the atomics feature runs on one thread.
unsafe impl Sync for Bump {}

#[global_allocator]
static ALLOCATOR: Bump = Bump { next: Cell::new(0) };

// SAFETY: every block returned is aligned as asked, lies inside linear
// memory, and is never handed out twice, since `next` only grows.
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut next = self.next.get();
        if next == 0 {
            // Start past everything the module placed in memory at load:
            // its data, its stack, and what memory it began with.
            next = memory_size(0) * PAGE;
        }
        let start = (next + layout.align() - 1) & !(layout.align() - 1);
        let Some(end) = start.checked_add(layout.size()) else {
            return std::ptr::null_mut();
        };
        let have = memory_size(0) * PAGE;
        if end > have && memory_grow(0, (end - have).div_ceil(PAGE)) == usize::MAX {
            return std::ptr::null_mut();
        }
        self.next.set(end);
        start as *mut u8
    }

    unsafe fn dealloc(&self, _block: *mut u8, _layout: Layout) {}
}

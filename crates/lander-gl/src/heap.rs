//! The module's allocator and panic handler, in place of std's.
//!
//! Blocks come in power-of-two sizes from 16 bytes up. A freed block goes
//! on its size's list and the next allocation of that size reuses it; new
//! blocks are cut from the end of linear memory, which grows as needed.
//! The module allocates a handful of objects that live as long as the page
//! and one coverage buffer a word each time the window changes size, so
//! this is all it needs, at a fraction of dlmalloc's size.

use core::alloc::{GlobalAlloc, Layout};
use core::arch::wasm32;
use core::cell::UnsafeCell;

const PAGE: usize = 65536;

struct Heap {
    free: UnsafeCell<[usize; usize::BITS as usize]>,
    next: UnsafeCell<usize>,
    end: UnsafeCell<usize>,
}

// SAFETY: the page's module runs on one thread.
unsafe impl Sync for Heap {}

#[global_allocator]
static HEAP: Heap = Heap {
    free: UnsafeCell::new([0; usize::BITS as usize]),
    next: UnsafeCell::new(0),
    end: UnsafeCell::new(0),
};

fn class(layout: Layout) -> usize {
    layout
        .size()
        .max(layout.align())
        .max(16)
        .next_power_of_two()
        .trailing_zeros() as usize
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let k = class(layout);
        // SAFETY: one thread, and no reference outlives this call.
        let (free, next, end) = unsafe {
            (
                &mut *self.free.get(),
                &mut *self.next.get(),
                &mut *self.end.get(),
            )
        };
        if free[k] != 0 {
            let block = free[k];
            // SAFETY: a freed block holds the next free block's address.
            free[k] = unsafe { *(block as *const usize) };
            return block as *mut u8;
        }
        if *end == 0 {
            *end = wasm32::memory_size(0) * PAGE;
            *next = *end;
        }
        let align = layout.align().max(16);
        let start = (*next + align - 1) & !(align - 1);
        let stop = start + (1usize << k);
        if stop > *end {
            let pages = (stop - *end).div_ceil(PAGE);
            if wasm32::memory_grow(0, pages) == usize::MAX {
                return core::ptr::null_mut();
            }
            *end += pages * PAGE;
        }
        *next = stop;
        start as *mut u8
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        let k = class(layout);
        // SAFETY: one thread; the block is at least 16 bytes and aligned.
        unsafe {
            let free = &mut *self.free.get();
            *(block as *mut usize) = free[k];
            free[k] = block as usize;
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    wasm32::unreachable()
}

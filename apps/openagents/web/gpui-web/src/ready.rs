//! An app's "ready" frame: the first frame drawn after the app says its
//! content has arrived (`announce_ready`), not just its first frame.
//!
//! A page that shows a snapshot until the app can replace it without a
//! visible change waits for `gpui-ready` (an event that bubbles from the
//! canvas, and a performance mark), not `gpui-first-frame`, when the app's
//! first frames are still loading. With the `first-frame` feature, that
//! frame is also recorded as an `openagents-first-frame` scene
//! ([`recorded_frame`]), which the page can keep for the next visit.

use std::cell::{Cell, RefCell};

thread_local! {
    /// 0: not asked, 1: asked, the next frame is the ready one, 2: done.
    static READY: Cell<u8> = const { Cell::new(0) };
    static RECORDED: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Says the app's content has arrived: the next frame the window draws is
/// its ready frame. Only the first call counts.
pub fn announce_ready() {
    READY.with(|ready| {
        if ready.get() == 0 {
            ready.set(1);
        }
    });
}

/// Whether this frame is the ready one; true once.
pub(crate) fn take_ready() -> bool {
    READY.with(|ready| {
        if ready.get() == 1 {
            ready.set(2);
            true
        } else {
            false
        }
    })
}

#[cfg(feature = "first-frame")]
pub(crate) fn keep(scene: Vec<u8>) {
    RECORDED.with(|recorded| *recorded.borrow_mut() = Some(scene));
}

/// The ready frame as an encoded `openagents-first-frame` scene, once,
/// after `gpui-ready`; `None` when it had something the scene format can't
/// hold.
#[cfg(feature = "first-frame")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = gpuiRecordedFrame)]
pub fn recorded_frame() -> Option<Vec<u8>> {
    RECORDED.with(|recorded| recorded.borrow_mut().take())
}

#[cfg(not(feature = "first-frame"))]
#[allow(dead_code)]
fn unused() -> Option<Vec<u8>> {
    RECORDED.with(|recorded| recorded.borrow_mut().take())
}

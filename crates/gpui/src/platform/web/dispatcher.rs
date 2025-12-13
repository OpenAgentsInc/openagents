//! Web dispatcher implementation

use crate::platform::{PlatformDispatcher, RunnableVariant, TaskLabel, TaskTiming, ThreadTaskTimings};
use std::time::{Duration, Instant};

/// Web dispatcher using browser APIs
pub struct WebDispatcher;

impl WebDispatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformDispatcher for WebDispatcher {
    fn get_all_timings(&self) -> Vec<ThreadTaskTimings> {
        Vec::new()
    }

    fn get_current_thread_timings(&self) -> Vec<TaskTiming> {
        Vec::new()
    }

    fn is_main_thread(&self) -> bool {
        // In WASM, everything runs on the main thread
        true
    }

    fn dispatch(&self, runnable: RunnableVariant, _label: Option<TaskLabel>) {
        // In WASM, we can't spawn threads, so we just run it immediately
        // or schedule it via queueMicrotask
        match runnable {
            RunnableVariant::Meta(r) => r.run(),
            RunnableVariant::Compat(r) => r.run(),
        }
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant) {
        // Everything is already on the main thread in WASM
        self.dispatch(runnable, None);
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        use wasm_bindgen::prelude::*;

        let millis = duration.as_millis() as i32;

        // Use setTimeout to schedule the runnable
        let closure = Closure::once(move || {
            match runnable {
                RunnableVariant::Meta(r) => r.run(),
                RunnableVariant::Compat(r) => r.run(),
            }
        });

        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                millis,
            );
        }

        // Prevent the closure from being dropped
        closure.forget();
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

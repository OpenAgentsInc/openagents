//! A host that reaps child processes survives the signals it handles.
//!
//! On macOS, Wasmtime's default Mach-port trap thread aborted the whole
//! process when a handled signal such as SIGCHLD interrupted its receive.
//! A host that spawns children installs exactly such a handler. This test
//! runs without the libtest harness so that every thread but the ones
//! Wasmtime starts blocks the signal, which leaves the kernel no other
//! thread to deliver it to.

//!
//! Windows has no Unix signals, so there the test is empty.

#![cfg_attr(not(unix), allow(unused_imports))]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use plugin::{Limits, Profile, Snapshot, invoke};
use serde_json::json;

#[cfg(not(unix))]
fn main() {}

#[cfg(unix)]
extern "C" fn ignore(_: libc::c_int) {}

#[cfg(unix)]
fn main() {
    // SAFETY: installs a no-op handler, as a host that reaps children
    // does, and blocks it on this thread; no Rust-managed state changes.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = ignore as extern "C" fn(libc::c_int) as usize;
        libc::sigemptyset(&mut action.sa_mask);
        assert_eq!(
            libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()),
            0
        );
    }

    // Starting an invocation initializes the process's trap handler; the
    // module itself need not compile. Threads it starts from here inherit
    // this thread's mask, which leaves SIGCHLD deliverable.
    std::thread::spawn(|| {
        let result = invoke(plugin::Call {
            wasm: b"\0asm",
            profile: Profile::Pure,
            invocation: "inv-signals",
            operation: "noop",
            input: &json!({}),
            snapshot: &Snapshot::default(),
            handles: &BTreeMap::new(),
            limits: Limits::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
            required: true,
        });
        assert!(result.is_err(), "the module is not valid");
    })
    .join()
    .unwrap();

    // SAFETY: changes only this thread's signal mask and signals this
    // process.
    unsafe {
        let mut blocked: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut blocked);
        libc::sigaddset(&mut blocked, libc::SIGCHLD);
        assert_eq!(
            libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, std::ptr::null_mut()),
            0
        );
        for _ in 0..100 {
            libc::kill(libc::getpid(), libc::SIGCHLD);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    println!("handled signals after a guest runs do not abort the host ... ok");
}

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    openagents_lib::run()
}
//! Minimal binary that forwards to the shared `openagents_lib::run()` entry.
//!
//! This keeps the application bootstrap in `lib.rs` so it can be unit‑tested
//! and referenced from different binary targets if needed.

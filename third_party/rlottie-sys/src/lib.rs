#![no_std]
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

//! Unsafe Rust bindings to rlottie.
//!
//! # Features
//!
//!  - `vendor-samsung` (enabled by default):
//!    If rlottie cannot be found on the system, download Samsung's version of rlottie
//!    and compile it.
//!  - `vendor-telegram`:
//!    If rlottie cannot be found on the system, download Telegram's version of rlottie
//!    and compile it.
//!
//! You can force the use of vendored code by setting the `RLOTTIE_NO_PKG_CONFIG`
//! environment variable at compile time.
//!
//! If both the `vendor-samsung` and `vendor-telegram` features are enabled, the Samsung
//! feature has priority.

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

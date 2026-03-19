# rlottie-sys ![License: MIT](https://img.shields.io/badge/license-MIT-blue) [![rlottie-sys on crates.io](https://img.shields.io/crates/v/rlottie-sys)](https://crates.io/crates/rlottie-sys) [![rlottie-sys on docs.rs](https://docs.rs/rlottie-sys/badge.svg)](https://docs.rs/rlottie-sys) [![Source Code Repository](https://img.shields.io/badge/Code-On%20Codeberg-blue?logo=Codeberg)](https://codeberg.org/msrd0/rlottie-rs) ![Rust Version: 1.70.0](https://img.shields.io/badge/rustc-1.70.0-orange.svg)

Unsafe Rust bindings to rlottie.

## Features

* `vendor-samsung` (enabled by default):
  If rlottie cannot be found on the system, download Samsung’s version of rlottie
  and compile it.
* `vendor-telegram`:
  If rlottie cannot be found on the system, download Telegram’s version of rlottie
  and compile it.

You can force the use of vendored code by setting the `RLOTTIE_NO_PKG_CONFIG`
environment variable at compile time.

If both the `vendor-samsung` and `vendor-telegram` features are enabled, the Samsung
feature has priority.

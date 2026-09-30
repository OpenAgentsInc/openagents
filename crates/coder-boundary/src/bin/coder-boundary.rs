//! The launcher a Windows boundary wraps its command in: `coder-boundary
//! run --sid SID [--network] -- PROGRAM ARGS…` starts the program in that
//! AppContainer and exits with its code. See `coder_boundary::windows`.

#[cfg(windows)]
fn main() {
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    std::process::exit(coder_boundary::windows::launch::main(&arguments));
}

/// macOS and Linux wrap the command in a system program instead.
#[cfg(not(windows))]
fn main() {
    eprintln!("coder-boundary: the launcher is for Windows; this platform uses its own sandbox");
    std::process::exit(2);
}

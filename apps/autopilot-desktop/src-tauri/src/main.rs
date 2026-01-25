// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn configure_linux_display_backend() {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let display = std::env::var("DISPLAY").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();

    if session_type.as_deref() == Some("wayland")
        && display.as_deref().map(|value| !value.is_empty()).unwrap_or(false)
        && wayland_display
            .as_deref()
            .map(|value| !value.is_empty())
            .unwrap_or(false)
        && std::env::var_os("AUTOPILOT_FORCE_WAYLAND").is_none()
    {
        std::env::set_var("GDK_BACKEND", "x11");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_display_backend();

    autopilot_desktop_lib::run()
}

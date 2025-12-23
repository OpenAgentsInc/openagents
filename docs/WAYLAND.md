# Wayland Support for OpenAgents Desktop

This document describes the Wayland support implementation for the OpenAgents desktop GUI and known issues with webkit2gtk.

## Overview

The OpenAgents desktop app uses `wry` (WebView library) with GTK to render the UI. On Linux, wry uses `webkit2gtk` as the webview engine. This works on both X11 and Wayland, but requires specific configuration for Wayland compatibility.

## Architecture

```
┌─────────────────────────────────────────┐
│           OpenAgents Desktop            │
├─────────────────────────────────────────┤
│  GTK Window (gtk::Window)               │
│  ├── gtk::Box (container)               │
│  │   └── WebView (wry + webkit2gtk)     │
│  └── Event handling (gtk::main)         │
├─────────────────────────────────────────┤
│  Actix Web Server (background thread)   │
│  └── Serves HTML/HTMX UI on localhost   │
└─────────────────────────────────────────┘
```

## Why GTK Instead of tao

The original implementation used `tao` (cross-platform windowing library) with raw window handles:

```rust
// OLD - X11 only on Linux
let event_loop = EventLoop::new();
let window = WindowBuilder::new().build(&event_loop)?;
let webview = WebViewBuilder::new().with_url(&url).build(&window)?;
```

This approach fails on Wayland with the error:
```
Error: the window handle kind is not supported
```

The `build(&window)` method uses `raw-window-handle` which only supports X11 on Linux. For Wayland support, we must use GTK directly:

```rust
// NEW - Works on both X11 and Wayland
gtk::init()?;
let window = gtk::Window::new(gtk::WindowType::Toplevel);
let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
window.add(&vbox);
window.show_all();

let webview = WebViewBuilder::new()
    .with_url(&url)
    .build_gtk(&vbox)?;  // Uses WebViewBuilderExtUnix trait

gtk::main();
```

The `build_gtk()` method from the `WebViewBuilderExtUnix` trait properly handles both X11 and Wayland display servers.

## webkit2gtk DMABUF Issue

### The Problem

Even with GTK-based windowing, webkit2gtk 2.42+ has a bug with its DMABUF renderer on Wayland that causes:

```
Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display.
```

This is a known issue tracked at [WebKit Bug 262607](https://bugs.webkit.org/show_bug.cgi?id=262607).

### The Workaround

We disable the DMABUF renderer on Wayland by setting an environment variable before GTK initialization:

```rust
if std::env::var("WAYLAND_DISPLAY").is_ok() {
    unsafe {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}
gtk::init()?;
```

This falls back to the WPE renderer which still provides GPU acceleration, just without DMABUF buffer sharing.

### Why `unsafe`?

In Rust Edition 2024, `std::env::set_var` is unsafe because modifying environment variables can cause race conditions if other threads are reading them. We mark it as safe here because:

1. It's called before any other threads are spawned
2. It's called before GTK initialization (which reads these variables)
3. The variable is only read by webkit2gtk during its initialization

### Alternative Workarounds

If the DMABUF workaround doesn't work, there are other options:

| Environment Variable | Effect |
|---------------------|--------|
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | Disables DMABUF, uses WPE renderer (recommended) |
| `WEBKIT_DISABLE_COMPOSITING_MODE=1` | Disables GPU compositing entirely |
| `GDK_BACKEND=x11` | Forces X11/XWayland instead of native Wayland |

Users can set these manually if needed:
```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 openagents
```

## Dependencies

The following dependencies are required for Wayland support:

```toml
[dependencies]
gtk = { version = "0.18", features = ["v3_24"] }
wry = "0.50"
```

Note: `glib` is re-exported through `gtk::glib`, so a separate dependency isn't needed.

### System Dependencies (Arch Linux)

```bash
pacman -S webkit2gtk-4.1 gtk3
```

### System Dependencies (Ubuntu/Debian)

```bash
apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

## Code Location

- **Main GUI code**: `src/gui/app.rs`
- **Server code**: `src/gui/server.rs`
- **Routes**: `src/gui/routes/`

## Testing

### Native Wayland
```bash
cargo run --bin openagents
```

### Force X11 (XWayland)
```bash
GDK_BACKEND=x11 cargo run --bin openagents
```

### Debug webkit2gtk issues
```bash
WEBKIT_DEBUG=all cargo run --bin openagents 2>&1 | grep -i webkit
```

## Known Issues

1. **NVIDIA drivers**: Some NVIDIA driver versions have additional issues with webkit2gtk on Wayland. If you see blank windows or rendering artifacts, try:
   ```bash
   WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 openagents
   ```

2. **Fractional scaling**: webkit2gtk may have rendering issues with fractional scaling on some Wayland compositors. This is a upstream webkit2gtk issue.

3. **Window decorations**: GTK windows use client-side decorations on Wayland. The appearance may differ from X11.

## References

- [wry WebViewBuilderExtUnix](https://docs.rs/wry/latest/wry/trait.WebViewBuilderExtUnix.html)
- [WebKit Bug 262607 - DMABUF renderer issues](https://bugs.webkit.org/show_bug.cgi?id=262607)
- [WebKitGTK DMA-BUF Rendering Blog Post](https://blogs.igalia.com/carlosgc/2023/04/03/webkitgtk-accelerated-compositing-rendering/)
- [Tauri Wayland Issues Discussion](https://github.com/tauri-apps/tauri/issues/9394)

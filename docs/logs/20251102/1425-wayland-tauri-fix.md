## Tauri on Linux/Wayland: fixes applied

Date: 2025-11-02 14:25

### Summary
- Tauri `cargo metadata` workspace mismatch fixed.
- Resolved "Failed to parse version `2` for crate `tauri`".
- Avoided Wayland protocol crash by falling back to X11 under Wayland.

### Changes
- Workspace membership: added `tauri/src-tauri` to root workspace.
  - File: `Cargo.toml`
- Pinned Tauri crate versions to concrete semver to satisfy tooling that expects exact versions.
  - File: `tauri/src-tauri/Cargo.toml`
  - `tauri-build = "2.5.1"`, `tauri = "2.9.2"`
- Wayland workaround: when `WAYLAND_DISPLAY` is present, force X11/XWayland and disable WebKit DMABUF renderer to prevent Wayland protocol error.
  - File: `tauri/src-tauri/src/lib.rs`
  - Sets env vars before building the app:
    - `WINIT_UNIX_BACKEND=x11`
    - `GDK_BACKEND=x11`
    - `WEBKIT_DISABLE_DMABUF_RENDERER=1`

### Rationale
- Cargo workspace error: Tauri crate lived under the repo’s Cargo workspace root but wasn’t included in `workspace.members`, causing `cargo metadata` to fail.
- Version parse: some tooling reads dependency versions as strict semver; bare `"2"` triggered a parse warning/failure.
- Wayland crash: WebKitGTK/winit can hit Wayland protocol errors on some setups; XWayland fallback is a stable workaround.

### How to run
- From `tauri/`: `bun run tauri dev`
- Dev server: http://localhost:1420

### Notes
- If true Wayland is preferred, we can gate the fallback behind an env flag (e.g., only set X11 vars when `FORCE_X11=1`).

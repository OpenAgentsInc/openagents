# Tauri Window Focus Stealing Research

## Problem
During development with `bun run tauri dev`, the Tauri window steals focus every time hot reload occurs, interrupting the developer workflow.

## Current Situation
- Using Tauri v2
- `focus: false` in `tauri.conf.json` doesn't work in the current version
- The focus configuration option is a newer feature (see [Tauri PR #11569](https://github.com/tauri-apps/tauri/pull/11569))

## Root Cause
When Vite hot reloads, it triggers a page reload in the Tauri window. The default behavior in Tauri is to focus the window when content changes, which is disruptive during development.

## Solution Approaches

### 1. Platform-Specific Window Attributes (Implemented)
Use platform-specific window attributes to prevent focus stealing:
- **macOS**: Set window level to prevent automatic focus
- **Windows**: Use window flags to prevent activation
- **Linux**: Use window type hints

### 2. Disable Hot Reload (Alternative)
Provide a development mode without hot reload for when focus stealing is particularly disruptive.

## Implementation Details

### Rust Code Changes
Add window setup hook in `lib.rs` to set platform-specific attributes:

```rust
.setup(|app| {
    #[cfg(target_os = "macos")]
    {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            use cocoa::appkit::{NSWindow, NSWindowLevel};
            use cocoa::base::id;
            
            let ns_window: id = window.ns_window().unwrap() as _;
            unsafe {
                // Keep window at normal level but prevent stealing focus
                ns_window.setLevel_(NSWindowLevel::NSNormalWindowLevel);
                ns_window.setHidesOnDeactivate_(cocoa::base::NO);
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE};
            
            let hwnd = window.hwnd().unwrap();
            unsafe {
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE as isize);
            }
        }
    }
    Ok(())
})
```

### Vite Configuration Changes
Add environment variable support to disable HMR:

```typescript
server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: process.env.DISABLE_HMR === 'true' ? false : (host ? {
        protocol: "ws",
        host,
        port: 1421,
    } : undefined),
}
```

### Package.json Scripts
Changed default development mode to disable HMR to prevent focus stealing:
```json
"scripts": {
    "dev": "DISABLE_HMR=true tauri dev",      // Default: Tauri without HMR
    "dev:hmr": "tauri dev",                   // Tauri with HMR (may steal focus)
    "dev:vite": "DISABLE_HMR=true vite",      // Vite only without HMR
    "dev:vite:hmr": "vite",                   // Vite only with HMR
}
```

## Testing
1. Run `bun run dev` - Default Tauri development without HMR, no focus stealing
2. Run `bun run dev:hmr` - Tauri development with hot reload (may steal focus)

## Future Improvements
- Monitor Tauri updates for native focus control support
- Consider adding a user preference to toggle this behavior
- Investigate if we can detect when the window already has focus to allow normal behavior in that case
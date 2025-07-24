# Tauri window focus stealing during hot reload: Current status and solutions

When developing with Tauri, the window stealing focus during hot reload is a well-documented issue affecting developer productivity. The `focus: false` configuration setting fails to prevent this behavior during development, and this research reveals both the technical reasons and practical solutions.

## The focus control landscape in Tauri

PR #11569, merged in November 2024 and included in Tauri 2.0.6+, addressed a major focus regression where webviews weren't being focused by default despite `focus: true` configurations. However, this fix didn't resolve the hot reload focus stealing issue, which stems from a different architectural problem.

The current state reveals **three distinct focus-related issues**: the configuration setting being ignored during development (particularly on Windows), the complete application restart during hot reload that recreates windows, and platform-specific inconsistencies in focus behavior. Issue #11566 specifically confirms that `focus: false` in tauri.conf.json doesn't work during `tauri dev` mode, though programmatic focus control using Rust's `WebviewWindowBuilder::focused(false)` works more reliably.

## Why hot reload causes focus stealing

During hot reload, Tauri performs a complete application restart when Rust code changes. This process terminates the existing application, spawns a new process, and recreates all windows from configuration. The operating system's window manager typically focuses newly created windows by default, and **Tauri's window creation process doesn't properly respect the `focus: false` configuration during these development restarts**.

This fundamental limitation exists because the OS treats each restart as a new application launch. The focus configuration works correctly for initial window creation in production builds, but the development lifecycle's constant recreation defeats this setting. Platform behavior varies significantly - Windows exhibits the most problematic behavior with focus configuration frequently ignored, Linux shows inconsistent focus issues depending on the window manager, and macOS generally handles focus more reliably but still has edge cases.

## Immediate solutions for development

The most straightforward solution is **disabling the file watcher** that triggers automatic restarts:

```bash
bun run tauri dev --no-dev-watcher
```

This prevents automatic rebuilds, requiring manual restarts when you change Rust code. While less convenient, it completely eliminates focus stealing. For a more selective approach, create a `.taurignore` file in your project root to prevent unnecessary rebuilds:

```
# .taurignore
*.md
*.txt
*.log
node_modules/
dist/
assets/
```

For scenarios requiring some hot reload functionality, **separate your frontend and backend development**. Run your frontend dev server in one terminal and Tauri without auto-restart in another:

```bash
# Terminal 1: Frontend with hot reload
bun run dev

# Terminal 2: Tauri without auto-restart
bun run tauri dev --no-dev-watcher
```

## Programmatic focus control in Rust

Since configuration-based focus control fails during development, implement focus management directly in your Rust code. In `src-tauri/src/lib.rs`, replace the default window creation:

```rust
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Only apply focus prevention during development
            #[cfg(debug_assertions)]
            {
                // Create window programmatically instead of from config
                let window = tauri::WebviewWindowBuilder::new(
                    app,
                    "main",
                    tauri::WebviewUrl::App("index.html".into())
                )
                .title("Your App Title")
                .inner_size(1200.0, 800.0)
                .focused(false)  // More reliable than config
                .visible(false)  // Start invisible
                .build()?;

                // Show window after a delay without focusing
                let window_clone = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    window_clone.show().unwrap();
                });
            }

            // Production build uses normal config
            #[cfg(not(debug_assertions))]
            {
                // Let config handle window creation
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

This approach creates windows programmatically during development, applying focus prevention more reliably than configuration. The delayed visibility trick helps prevent focus stealing on some platforms.

## Platform-specific implementations

Different operating systems require tailored approaches. For Windows, which exhibits the most problematic behavior, combine multiple techniques:

```rust
#[cfg(target_os = "windows")]
fn create_unfocused_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "main",
        tauri::WebviewUrl::App("index.html".into())
    )
    .focused(false)
    .visible(false)
    .skip_taskbar(true)  // Helps prevent focus
    .build()?;

    // Delayed show without focus
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        window.show().ok();
        window.set_skip_taskbar(false).ok();
    });

    Ok(())
}
```

For development workflows heavily impacted by focus stealing, **third-party window management tools** provide additional control. On Windows, tools like DeskPins or AlwaysOnTop can pin your editor window above the Tauri application, preventing it from losing focus even when new windows appear.

## Version compatibility and future outlook

This focus stealing issue affects all current Tauri versions, with **Tauri 2.0.6+ including some focus improvements but not addressing the hot reload problem**. The issue is fundamentally architectural - the complete process restart during hot reload defeats any window-level focus settings.

Active GitHub issues tracking this problem include #11566 (config focus settings ignored during development), #12055 (WebviewWindowBuilder focus issues), and #7519 (general focus property problems). The Tauri team acknowledges these limitations, but a comprehensive fix would require significant changes to the development mode architecture.

For now, the combination of disabling auto-restart with `--no-dev-watcher` and implementing programmatic focus control provides the most reliable solution. While less convenient than seamless hot reload, it preserves developer focus and productivity by preventing constant window interruptions during development.

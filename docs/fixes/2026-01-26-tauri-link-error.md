# Tauri link error (autopilot-desktop) - 2026-01-26

## Context
- Target: `autopilot-desktop` (Tauri app)
- Build command: `cargo build -p autopilot-desktop`

## Symptoms
- Linker failed with undefined symbols like:
  - `core::ptr::drop_in_place<tauri::menu::MenuItemKind<tauri_runtime_wry::Wry<...>>>`
- Warnings during link:
  - Object files built for newer macOS version (26.2) than being linked (11.0)

## Diagnosis
The link error was caused by stale build artifacts compiled with a newer macOS SDK/target
version. Incremental artifacts in `target/` caused mismatched symbols during the final
link step.

## Fix
Clean the Tauri target artifacts and rebuild:

```bash
cargo clean -p autopilot-desktop
cargo build -p autopilot-desktop
```

## Verification
The build completed successfully after the clean rebuild (warnings only, no link errors).

## Notes
If the link error returns, repeat the clean. A longer-term option is to disable
incremental builds for this crate in dev profile if it becomes frequent.

# Stale / Deprecated Items

- `packages/tricoder/` remains for responsible deprecation. Confirm excluded from all Xcode targets and SPM packages.
- Provider JSONL readers remain only as translation inputs in DesktopWebSocketServer+Tailer. Ensure no raw provider payloads are ever surfaced to the app — continue translating to ACP updates only.
- Any references to old Rust/Tauri v0.2 paths in docs should be clearly marked as historical (migration notes only).


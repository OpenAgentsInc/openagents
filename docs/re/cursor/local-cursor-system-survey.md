# Cursor on This Mac: Local System Survey

This document summarizes what’s installed and how Cursor runs locally on this machine. Sources include the bundled app, on-disk data, logs, and the repo doc “Architecture of Cursor: An AI‑Enhanced VS Code Fork”.

## App Installation
- App bundle: `/Applications/Cursor.app`
- Bundle identifier: `com.todesktop.230313mzl4w4u92`
- App version: `2.0.43`
- Build base: VS Code fork (reports VS Code `1.99.3` in logs)
- Binary: arm64 Mach‑O (`.../Contents/MacOS/Cursor`)
- Frameworks present: `Electron Framework.framework`, `Squirrel.framework`, `Mantle.framework`, `ReactiveObjC.framework`

## Product Metadata (Resources)
- Product file: `/Applications/Cursor.app/Contents/Resources/app/product.json`
  - `applicationName`: `cursor`
  - `version`: `2.0.43`
  - `commit`: `8e4da76ad196925accaa169efcae28c45454cce0`
  - Extension marketplace: `https://marketplace.cursorapi.com/_apis/public/gallery`
- App package file: `/Applications/Cursor.app/Contents/Resources/app/package.json`
  - `name`: `Cursor`
  - `version`: `2.0.43`
  - `repository`: `https://github.com/microsoft/vscode.git` (indicates VS Code upstream)

## CLI and Utilities
- CLI symlink: `/usr/local/bin/cursor` → `.../Cursor.app/Contents/Resources/app/bin/code`
- Launcher script: `.../Contents/Resources/app/bin/cursor` (wraps Electron with `ELECTRON_RUN_AS_NODE=1` to run `out/cli.js`)
- Tunnel binary: `.../Contents/Resources/app/bin/cursor-tunnel` (native arm64). Strings indicate VS Code tunnel/remote CLI lineage; logs/strings show “Cursor Tunnel”.

## Built‑in and Bundled Extensions
- Built‑in extensions live in: `/Applications/Cursor.app/Contents/Resources/app/extensions`
- A cache of built‑ins is tracked at: `~/Library/Application Support/Cursor/CachedProfilesData/__default__profile__/extensions.builtin.cache`
- Examples (from the cache and folder listing): `vscode.bat`, `vscode.coffeescript`, `vscode.json-language-features`, `vscode.markdown-language-features`, `vscode.python`, `vscode.javascript`, `vscode.typescript`, etc. (standard VS Code built‑ins).

## User Profile, Data, and Extensions
- Primary data root(s):
  - `~/Library/Application Support/Cursor`
  - `~/Library/Application Support/cursor` (legacy/duplicate)
- Key subfolders under `~/Library/Application Support/Cursor`:
  - `User/` — settings, keybindings, snippets, global/workspace storage.
    - `User/settings.json` — user settings.
    - `User/keybindings.json` — custom keybindings.
    - `User/snippets/` — user snippets.
    - `User/globalStorage/` — global state and extension storage.
    - `User/workspaceStorage/<hash>/` — per‑workspace state.
  - `logs/<timestamp>/` — per‑session logs.
  - `CachedExtensionVSIXs/` — downloaded VSIX caches.
  - `Session Storage/` — Chromium session storage (log/ldb files).
- User‑installed extensions live in: `~/.cursor/extensions`
  - Sample installed items observed: `anthropic.claude-code-1.0.68`, `openai.chatgpt-0.4.39-universal`, `ms-python.python-2025.6.1-darwin-arm64`, `golang.go-0.50.0-universal`, `redhat.vscode-yaml-1.19.1-universal`, `tauri-apps.tauri-vscode-0.2.10-universal`, `vscodevim.vim-1.32.1-universal`, etc. (numerous others present).

## Logs and Reported Versions
- Logs directory example: `~/Library/Application Support/Cursor/logs/20251113T094530/`
  - `main.log` includes:
    - Native module: `CursorProclistService` loaded
    - Updates disabled (by user preference)
  - `sharedprocess.log` includes extension install/cleanup activity and reports product versions:
    - Product `version`: `2.0.43`
    - `vscodeVersion`: `1.99.3`
  - `ptyhost.log` and `terminal.log` show terminal host lifecycle and latency diagnostics
  - `remoteTunnelService.log` indicates tunneling not configured in `product.json`

## Local Databases (SQLite)
- VS Code‑style state databases are present:
  - `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`
  - `~/Library/Application Support/Cursor/User/workspaceStorage/<hash>/state.vscdb` (many per‑workspace)
- File type: SQLite 3
- Example schema (`globalStorage/state.vscdb`):
  - Tables: `ItemTable (key TEXT UNIQUE, value BLOB)`, `cursorDiskKV (key TEXT UNIQUE, value BLOB)`
  - `PRAGMA user_version = 1`
- Additional JSON state: `User/globalStorage/storage.json` (backup workspaces, profile associations, menu cache, telemetry IDs, etc.). Note: actual IDs are not reproduced here.

## App Process Model (Observed + From Architecture Doc)
- Electron app with:
  - Main process: `.../Contents/MacOS/Cursor`
  - Renderer and helper apps: `Cursor Helper.app` variants
  - Extension host processes (Node) and language servers as in VS Code
- Logs show a native module `cursor-proclist` used by the main process.
- The included architecture doc indicates heavy AI compute is cloud‑side; the client gathers context and applies edits locally. No large local AI model observed in app contents.

## Notable Node/Electron Dependencies (Bundled)
From `.../Resources/app/node_modules` (high‑level, not exhaustive):
- VS Code internals: `@vscode/*`, `vscode-textmate`, `vscode-oniguruma`, `vscode-regexpp`
- Terminal/PTY: `node-pty`, `@xterm/*`
- Telemetry/Observability: `@opentelemetry/*`, `@sentry/*`
- Native addon: `cursor-proclist` (uses `node-addon-api`)
- Misc/common libs: `debug`, `archiver`, `chrome-remote-interface`, etc.

## Preferences / Identifiers
- Preferences plist observed: `~/Library/Preferences/com.todesktop.230313mzl4w4u92.plist` (likely legacy ToDesktop bundle naming). Minimal keys; most app state is stored under Application Support.

## Updater
- `Squirrel.framework` is bundled. Logs show “updates are disabled by user preference”.

## What Cursor Uses Locally
- Editor core: VS Code fork (Electron), with built‑in extensions and extension host
- Storage: SQLite (`state.vscdb` global/workspace), JSON (`storage.json`), Chromium session storage
- Terminal: `node-pty` + xterm
- Native modules: `cursor-proclist` (process listing)
- CLI: `/usr/local/bin/cursor` script, `cursor-tunnel` native binary
- Extension marketplace: Cursor’s own gallery service (`marketplace.cursorapi.com`)

## Cloud vs Local (from “Architecture of Cursor” doc)
- Compute‑heavy AI runs in Cursor’s cloud; local app assembles context and applies edits
- Indexing metadata cached in `.cursor`/workspace and app support folders; no local LLM observed

## Quick Inventory (Paths)
- App: `/Applications/Cursor.app`
- Built‑ins: `/Applications/Cursor.app/Contents/Resources/app/extensions`
- Profile root: `~/Library/Application Support/Cursor`
- User settings: `~/Library/Application Support/Cursor/User/settings.json`
- Global DB: `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`
- Workspace DBs: `~/Library/Application Support/Cursor/User/workspaceStorage/*/state.vscdb`
- User extensions: `~/.cursor/extensions`
- CLI: `/usr/local/bin/cursor`
- Logs: `~/Library/Application Support/Cursor/logs/<timestamp>/`

## Notes
- This report does not include private content from databases beyond table names and schema shape.
- Some folders also exist under lowercase `.../Application Support/cursor/` and mirror the same structure.


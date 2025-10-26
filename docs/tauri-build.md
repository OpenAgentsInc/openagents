# Desktop Build (Tauri v2)

This guide explains how to produce a signed, distributable desktop build of OpenAgents using the Tauri project in `tauri/`.

The Tauri app bundles a Rust/WASM UI (built with Trunk) and launches local sidecars for the Convex backend and the Codex WebSocket bridge. For production, ensure assets and sidecars are prepared before packaging.

## Overview

- Frontend: Rust → WASM, built by `trunk` into `tauri/dist` (configured by `tauri/Trunk.toml`).
- Tauri bundle config: `tauri/src-tauri/tauri.conf.json` (icons, product name, version, resources).
- Sidecars:
  - Convex local backend binary is bundled as a resource at `tauri/src-tauri/bin/local_backend`.
  - The Codex bridge is currently spawned via `cargo run -p codex-bridge` (dev-centric). For a packaged build, run the bridge yourself before launching the app (see Caveats below).

## Prerequisites

- Rust toolchain (stable), including `wasm32-unknown-unknown` target.
  - `rustup target add wasm32-unknown-unknown`
- Trunk (WASM bundler): `cargo install trunk`.
- Bun (for Convex CLI helpers used during dev/bootstrap): https://bun.sh
- Tauri CLI: `cargo install tauri-cli@2`.
- Apple/Windows signing tooling as applicable (see Signing below).

## Prepare Versioning, Branding, and Icons

Edit `tauri/src-tauri/tauri.conf.json`:

- `productName`: public app name (menus/dock/window title)
- `version`: semantic version of the desktop app
- `identifier`: bundle identifier (macOS, Windows, Linux)
- `app.windows[0].title`: initial window title
- `bundle.icon`: ensure icons exist under `tauri/src-tauri/icons/`

Example (already set):

```
"productName": "OpenAgents",
"identifier": "com.openagents.desktop",
"bundle": { "icon": [ ... ] }
```

## Prepare Sidecars (Convex)

The build includes a local Convex backend as a resource. Ensure the binary exists at `tauri/src-tauri/bin/local_backend` before packaging:

```
bun run convex:fetch-backend
```

This runs `scripts/fetch-convex-backend.sh` and installs the latest cached Convex local backend binary to the expected path. If Bun or the Convex cache is missing, the script explains how to resolve it.

Optional runtime tuning via env vars (defaults in code):

- `OPENAGENTS_CONVEX_PORT` (default: 7788)
- `OPENAGENTS_CONVEX_INTERFACE` (default: 0.0.0.0)

## Build Commands

Build runs Trunk first (frontend), then Tauri packaging:

```
cd tauri
cargo tauri build
```

Artifacts are created under:

- macOS: `tauri/src-tauri/target/release/bundle/macos/`
- Windows: `tauri/src-tauri/target/release/bundle/msi/` (or `nsis/` if configured)
- Linux: `tauri/src-tauri/target/release/bundle/` (`deb`, `appimage`, etc.)

You can limit bundles via flags, for example:

```
cargo tauri build --bundles dmg
```

## Signing and Notarization (macOS)

For local testing, you can run the unsigned `.app` from the bundle directory. For distribution, sign and (optionally) notarize:

- Use Apple API key auth (recommended):
  - `APPLE_API_KEY`, `APPLE_API_ISSUER`, and `APPLE_CERTIFICATE`/keychain setup per Tauri docs
- Or Apple ID auth: `APPLE_ID`, `APPLE_PASSWORD` (app-specific password)
- Notarization toggles are picked up by the Tauri bundler; see the official Tauri v2 signing docs for the current env matrix.

Notes:
- The macOS bundle identifier is `com.openagents.desktop` (from `tauri.conf.json`).
- If you see Gatekeeper warnings during local testing, you can clear quarantine attributes: `xattr -cr /path/to/OpenAgents.app`.

## Windows and Linux Notes

- Windows: ensure the appropriate VC++ redistributables are present on target systems if needed. Tauri bundles MSIX/MSI or NSIS depending on configuration.
- Linux: choose the formats you want (AppImage, deb, rpm). You may need system libraries (WebKitGTK) depending on the target distro.

## Caveats (Current State)

- Bridge sidecar: the app’s backend bootstrap currently uses `cargo run -p codex-bridge` during startup (good for dev). A packaged app does not ship Cargo, so production builds won’t successfully auto‑spawn the bridge yet. Until we ship the bridge as a sidecar:
  - Start the bridge manually before launching OpenAgents:
    - `cargo run -p codex-bridge -- --bind 0.0.0.0:8787`
  - Or run an external bridge process and ensure the app connects to it.
- Convex sidecar: if `tauri/src-tauri/bin/local_backend` is missing, the app will skip embedded Convex and try to use any configured `CONVEX_URL` instead.

## Addendum: Ship Convex as a Real Sidecar

Goal: ensure the Convex local backend binary is bundled and launched from inside the packaged app without any external setup.

What’s already done
- The bundler includes `bin/local_backend` via `bundle.resources` in `tauri/src-tauri/tauri.conf.json`.
- Dev scripts fetch the binary: `bun run convex:fetch-backend`.

What to add
1) Resolve the resource path at runtime and spawn that binary.
   - Modify the sidecar launcher to look up `bin/local_backend` from the app’s Resources directory in production.
   - Example code (Tauri v2):

   ```rust
   use tauri::path::BaseDirectory;

   fn start_convex_sidecar(app: tauri::AppHandle) {
       let port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
       if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)).is_ok() {
           println!("[tauri/convex] detected local backend on 127.0.0.1:{}", port);
           return;
       }

       // Prefer packaged resource path
       let resource_bin = app
           .path()
           .resolve(
               if cfg!(windows) { "bin/local_backend.exe" } else { "bin/local_backend" },
               BaseDirectory::Resource,
           )
           .ok();

       let mut candidates = Vec::new();
       if let Some(p) = resource_bin { candidates.push(p); }
       if let Ok(p) = std::env::var("OPENAGENTS_CONVEX_BIN") { candidates.push(p.into()); }
       // Repo-local dev path fallback
       let repo = detect_repo_root(None);
       candidates.push(repo.join("tauri/src-tauri/bin").join(if cfg!(windows) { "local_backend.exe" } else { "local_backend" }));
       // Legacy user install fallback
       if let Ok(home) = std::env::var("HOME") { candidates.push(std::path::PathBuf::from(home).join(".openagents/bin/local_backend")); }

       let bin = candidates.into_iter().find(|p| p.is_file()).unwrap_or_else(|| std::path::PathBuf::from("local_backend"));
       if !bin.exists() {
           println!("[tauri/convex] local backend binary not found in resources or fallbacks");
           return;
       }

       // ... spawn as done today (db path, interface, port, inherit stdio)
   }
   ```

   - Wire it in `setup` (pass the app handle):

   ```rust
   tauri::Builder::default()
       .setup(|app| {
           if std::env::var("OPENAGENTS_SKIP_EMBEDDED_CONVEX").ok().as_deref() != Some("1") {
               let handle = app.app_handle().clone();
               tauri::async_runtime::spawn(async move { start_convex_sidecar(handle); });
           }
           Ok(())
       })
       // ...
   ```

2) Ensure the resource is present for all targets:
   - Keep `tauri/src-tauri/tauri.conf.json` → `bundle.resources: ["bin/local_backend"]` (Windows will use the `.exe` when present).
   - Verify after `cargo tauri build` that the resource exists in the bundled app:
     - macOS: `OpenAgents.app/Contents/Resources/bin/local_backend`
     - Windows: `<Bundle>/resources/bin/local_backend.exe`
     - Linux: `<bundle>/resources/bin/local_backend`

3) Fetch the correct platform binary before building:
   - Run on the target OS and execute: `bun run convex:fetch-backend`.
   - If cross-building, place the matching binary manually under `tauri/src-tauri/bin/`.

4) Test the packaged app:
   - Launch the app from the built bundle, open the sidebar, and confirm the Convex indicator shows the local sidecar URL (typically `http://127.0.0.1:7788` unless you override the port).

Notes
- You can still override with `OPENAGENTS_CONVEX_BIN` to point to an external binary if desired.
- The DB path defaults to `~/.openagents/convex/data.sqlite3`; change it in code if you want an app-scoped path.

## Release Checklist

- Bump `version` and confirm `productName`/`identifier` in `tauri/src-tauri/tauri.conf.json`.
- Ensure icons under `tauri/src-tauri/icons/` are correct.
- Run `bun run convex:fetch-backend` to embed the local backend (optional but recommended).
- Build: `cd tauri && cargo tauri build`.
- macOS only: sign/notarize if distributing externally.
- Test the packaged app on a clean machine or VM.

## Troubleshooting

- Trunk not found: `cargo install trunk`.
- WASM target missing: `rustup target add wasm32-unknown-unknown`.
- Bun not found: install from https://bun.sh.
- Missing Convex backend: run `bun run convex:fetch-backend` or set `CONVEX_URL` to an existing backend.
- Bridge not connecting: start the bridge separately (`cargo run -p codex-bridge -- --bind 0.0.0.0:8787`) until a sidecar is added.

For deep details, see:
- `tauri/src-tauri/tauri.conf.json` (bundle config)
- `tauri/Trunk.toml` (frontend build)
- `scripts/fetch-convex-backend.sh` (sidecar installer)
- `tauri/src-tauri/src/lib.rs` (bootstrapping, env vars, sidecar spawn logic)

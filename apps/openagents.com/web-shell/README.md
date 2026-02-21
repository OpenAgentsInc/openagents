# openagents-web-shell (OA-RUST-023)

Rust/WASM bootstrap entrypoint for the OpenAgents web surface.

## What this crate does

1. Boots a WGPUI web platform on `#openagents-web-shell-canvas`.
2. Starts a deterministic render loop and renders an initial shell frame.
3. Runs Rust-native auth/session lifecycle flows against control-service APIs:
   - magic-code challenge (`/api/auth/email`)
   - code verify (`/api/auth/verify`)
   - session restore (`/api/auth/session`)
   - refresh (`/api/auth/refresh`)
   - logout (`/api/auth/logout`)
4. Persists auth tokens in browser local storage (Rust-only storage path) and restores session on boot.
5. Exposes diagnostics/state helpers (`boot_diagnostics_json`, `app_state_json`, `auth_state_json`) and an explicit startup error boundary.
6. Supports forced startup error simulation with `?oa_boot_fail=1`.
7. Uses `crates/openagents-ui-core` shared tokens/primitives for shell backdrop/card rendering.
8. Uses shared command-bus planning from `crates/openagents-app-state::command_bus` for typed intent -> HTTP adapter mapping and deterministic error/retry classification.
9. Includes Khala WS client lane (Rust-only) with:
   - sync token minting (`/api/sync/token`)
   - Phoenix join/subscribe over `/sync/socket/websocket`
   - replay resume from persisted topic watermarks
   - stale-cursor reset + reconnect behavior
   - idempotent duplicate/out-of-order watermark handling.

## Build (WASM)

From repo root:

```bash
cargo build -p openagents-web-shell --target wasm32-unknown-unknown
```

Build browser-ready dist assets:

```bash
apps/openagents.com/web-shell/build-dist.sh
```

This writes:

- `apps/openagents.com/web-shell/dist/index.html`
- `apps/openagents.com/web-shell/dist/manifest.json`
- `apps/openagents.com/web-shell/dist/assets/openagents_web_shell.js`
- `apps/openagents.com/web-shell/dist/assets/openagents_web_shell_bg.wasm`
- `apps/openagents.com/web-shell/dist/assets/host-shim.js`

## JS Host Boundary

Boundary policy:

- `apps/openagents.com/web-shell/HOST_SHIM_BOUNDARY.md`

Enforcement check:

```bash
apps/openagents.com/web-shell/check-host-shim.sh
```

## Browser smoke test

```bash
python3 -m http.server 8788 --directory apps/openagents.com/web-shell/dist
```

Open:

- `http://127.0.0.1:8788/` for normal boot
- `http://127.0.0.1:8788/?oa_boot_fail=1` to verify startup error boundary

Auth helper exports available from wasm module:

- `auth_send_code(email: String)`
- `auth_verify_code(code: String)`
- `auth_restore_session()`
- `auth_refresh_session()`
- `auth_logout()`
- `khala_connect()`
- `khala_disconnect()`

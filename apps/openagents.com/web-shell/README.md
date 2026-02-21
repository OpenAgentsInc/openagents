# openagents-web-shell (OA-RUST-023)

Rust/WASM bootstrap entrypoint for the OpenAgents web surface.

## What this crate does

1. Boots a WGPUI web platform on `#openagents-web-shell-canvas`.
2. Starts a deterministic render loop and renders an initial shell frame.
3. Exposes boot diagnostics (`boot_diagnostics_json`) and an explicit startup error boundary.
4. Supports forced startup error simulation with `?oa_boot_fail=1`.
5. Uses `crates/openagents-ui-core` shared tokens/primitives for shell backdrop/card rendering.

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

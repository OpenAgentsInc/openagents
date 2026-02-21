# OA-RUST-094 WASM Boot Performance + Capability Policy

Status: Active  
Owner: `owner:web-platform` + `owner:control`  
Applies to: `apps/openagents.com/web-shell/` and `apps/openagents.com/service/`

## 1) Performance Budgets

Release gate budgets (enforced by `perf-soak-signoff.sh` / `perf-budget-gate.sh`):

1. `root.p95_ms <= 120`
2. `manifest.p95_ms <= 100`
3. `wasm_asset.p95_ms <= 250`
4. `auth_verify.p95_ms <= 250` (first interaction proxy)
5. `sync_token.p95_ms <= 250` (reconnect/auth path proxy)
6. `soak.errors == 0`
7. `rss_growth_kb <= 51200`
8. `js_asset_bytes <= 380000`
9. `wasm_asset_bytes <= 5500000`
10. `host_shim_bytes <= 12000`
11. `wasm_gzip_bytes <= 1900000`

Boot diagnostics budgets (surfaced from wasm diagnostics):

1. `dom_ready_latency_ms <= 450`
2. `gpu_init_latency_ms <= 1600`
3. `first_frame_latency_ms <= 2200`
4. `boot_total_latency_ms <= 2500`

## 2) Capability Matrix

Deterministic policy is evaluated in `host/capability-policy.js` and passed to Rust via `window.__OA_GPU_MODE__`.

Mode selection:

1. `auto`:
   - Prefer `webgpu` when available and user agent is not Linux desktop.
   - Fall back to `webgl2` for Linux desktop or WebGPU-unavailable browsers.
   - Fall back to `limited` when neither backend is available.
2. `oa_gpu_mode=webgpu`:
   - Force WebGPU when possible, then deterministic fallback to WebGL2.
3. `oa_gpu_mode=webgl2`:
   - Force WebGL2 when possible, otherwise fallback to WebGPU.
4. `oa_gpu_mode=limited`:
   - Hard-stop boot with visible error in host status.

Implementation references:

1. `apps/openagents.com/web-shell/host/capability-policy.js`
2. `apps/openagents.com/web-shell/host/capability-policy.test.mjs`
3. `crates/wgpui/src/platform.rs` (backend selection and host override)

## 3) Compression + Caching Requirements

1. Dist assets are content versioned and pinned by service worker.
2. `manifest.json` and `sw.js` must remain `no-store` / `no-cache`.
3. Hashed assets remain immutable (`max-age=31536000, immutable`).
4. Gzipped WASM size budget (`wasm_gzip_bytes`) is enforced in signoff.
5. SW pinning verification remains mandatory (`sw-policy-verify.sh`).

## 4) Operator Visibility

Telemetry and artifacts:

1. `apps/openagents.com/web-shell/perf/latest.json` (latest budget report)
2. `apps/openagents.com/web-shell/perf/signoff-*.json` (historical signoff artifacts)
3. `boot_diagnostics_json()` (runtime boot milestones + backend + budget breaches)

## 5) Required Verification

Before merge/deploy:

```bash
apps/openagents.com/web-shell/check-host-shim.sh
apps/openagents.com/web-shell/scripts/sw-policy-verify.sh
apps/openagents.com/web-shell/scripts/perf-budget-gate.sh
```

For full soak signoff:

```bash
apps/openagents.com/web-shell/scripts/perf-soak-signoff.sh
```

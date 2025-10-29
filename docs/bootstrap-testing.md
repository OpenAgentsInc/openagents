# Bootstrap & Fast‑Start Testing Guide

This guide documents how to test the bridge + Convex bootstrap changes locally on macOS and how to reproduce timing measurements, so future agents can iterate with confidence.

Applies to
- Bridge: crates/oa-bridge (Rust)
- Tricoder CLI: packages/tricoder (Node)
- Related docs: docs/audit/convex-boot-latency.md, docs/convex/cli-comparison-and-fast-start.md

## Goals
- Verify that tricoder launches the bridge via a prebuilt binary when available, otherwise falls back to cargo.
- Measure Convex backend readiness time and confirm improvements.
- Run time‑bounded tests to avoid hanging shells.

## Prerequisites
- macOS, Node 18+, git, ripgrep (`rg`) installed.
- Rust toolchain if testing cargo fallback: `cargo --version`.
- Optional: Bun if you plan to run scripts using bun, but not required for these tests.

## Key environment knobs
- Tricoder spawn defaults (set by tricoder):
  - `OPENAGENTS_CONVEX_STATE=convex` (Convex CLI layout under `~/.convex/convex-backend-state/openagents-dev/`)
  - `OPENAGENTS_CONVEX_INTERFACE=127.0.0.1` (loopback unless `TRICODER_EXPOSE_LAN=1`)
  - `OPENAGENTS_CONVEX_INSTANCE=openagents` (enables early `/instance_name` readiness checks)
  - `OPENAGENTS_CONVEX_DEBUG=1` when passing `--verbose` to tricoder
- Tricoder feature toggles:
  - `TRICODER_PREFER_BIN=0` forces cargo fallback
  - `TRICODER_EXPOSE_LAN=1` binds the bridge/Convex to `0.0.0.0`
  - `TRICODER_BRIDGE_VERSION/OWNER/REPO` allow pinning a release for prebuilt binary downloads (when assets exist)

## Test paths

### 1) Prebuilt bridge binary path (home bin)
Use a locally built binary to exercise tricoder’s prebuilt path.

- Build bridge and copy to home bin:
  - `cargo build -p oa-bridge --release`
  - `mkdir -p ~/.openagents/bin && cp target/release/oa-bridge ~/.openagents/bin/oa-bridge && chmod +x ~/.openagents/bin/oa-bridge`
- Run tricoder to start the bridge:
  - `node packages/tricoder/dist/index.js --run-bridge --verbose`
- Expect console output:
  - “Starting bridge (home-bin)…” and oa-bridge logs
  - “oa-bridge listening (route: /ws)”
  - Convex bootstrap with “convex.ensure: healthy … ready_ms=… ms”

### 2) Cargo fallback (builds from source)
- Force cargo:
  - `TRICODER_PREFER_BIN=0 node packages/tricoder/dist/index.js --run-bridge --verbose`
- Expect “Starting bridge via cargo …”, a short cargo build (debug), then the same oa-bridge logs as above.

### 3) LAN vs loopback
- Loopback (default via tricoder): no action needed.
- Expose on LAN:
  - `TRICODER_EXPOSE_LAN=1 node packages/tricoder/dist/index.js --run-bridge`
- In both cases, look for the bind address in logs and use the printed IP/port in the mobile app.

### 4) Shell wrapper
- You can also use the wrapper to prefer prebuilt or fallback to cargo:
  - `bash scripts/bridge-cli.sh --bind 127.0.0.1:8787`

## Measuring readiness time (ready_ms)
We instrumented `oa-bridge` to log elapsed millis from spawn to healthy Convex.

- Look for a line like:
  - `convex.ensure: healthy after start url=http://127.0.0.1:7788 pid=... ready_ms=33163`
- Extract with ripgrep:
  - `rg -n "convex.ensure: healthy after start" <logfile>`
- This is the key metric to track when iterating on bootstrap speed.

## Time‑bounded local tests (macOS)
To avoid long‑running processes during experiments, run oa‑bridge with a timeout and log to a temp file.

- Default state (legacy path) timing:
```
PORT=8799
LOG=$(mktemp -t oa_bridge_default.log)
BIN=target/debug/oa-bridge
cargo build -q -p oa-bridge
OPENAGENTS_CONVEX_INTERFACE=127.0.0.1 RUST_LOG=info "$BIN" --bind "127.0.0.1:$PORT" >"$LOG" 2>&1 &
PID=$!
for i in {1..120}; do sleep 0.5; rg -n "healthy after start" -q "$LOG" && break; done
kill -TERM $PID >/dev/null 2>&1 || true
rg -n "healthy after start" "$LOG"
```

- Convex state layout timing (uses `~/.convex/convex-backend-state/openagents-dev/`):
```
PORT=8798
LOG=$(mktemp -t oa_bridge_convexstate.log)
BIN=target/debug/oa-bridge
cargo build -q -p oa-bridge
lsof -i :7788 -sTCP:LISTEN -t 2>/dev/null | xargs -I{} kill -TERM {} 2>/dev/null || true
OPENAGENTS_CONVEX_STATE=convex OPENAGENTS_CONVEX_INTERFACE=127.0.0.1 OPENAGENTS_CONVEX_INSTANCE=openagents RUST_LOG=info "$BIN" --bind "127.0.0.1:$PORT" >"$LOG" 2>&1 &
PID=$!
for i in {1..120}; do sleep 0.5; rg -n "healthy after start" -q "$LOG" && break; done
kill -TERM $PID >/dev/null 2>&1 || true
rg -n "healthy after start" "$LOG"
```

Notes
- If a backend is already running on :7788, logs may show “already healthy”. Kill it (see `lsof` line above) to measure a full cold start.
- The first readiness will often be ~30–35s with a non‑empty state; subsequent attaches are instant if the backend stays running.

## Backend logs (debug)
Enable debug passthrough to see backend stdout/stderr during startup:
- `OPENAGENTS_CONVEX_DEBUG=1` (tricoder adds this automatically when you pass `--verbose`).
- You’ll see lines like “Starting a Convex backend”, “Connected to SQLite …”, “Bootstrapping indexes…”.

## Known pitfalls
- Bare `OPENAGENTS_CONVEX_DB` pointing at a random temp file is not sufficient; the backend expects a proper deployment state directory. Use `OPENAGENTS_CONVEX_STATE=convex` (or leave tricoder defaults) to get the correct layout.
- If port 7788 is already bound by another process, readiness checks may switch to “already healthy”; ensure you’re measuring a clean start when benchmarking.

## Validation checklist
- Tricoder prints your desktop IP (Tailscale if present) and port, then starts the bridge.
- Bridge logs show the bind address and Convex bootstrap status.
- You can extract `ready_ms` from the logs.
- Mobile app connects to the bridge at the configured IP:port.

## References
- docs/audit/convex-boot-latency.md (what contributes to the 30s)
- docs/convex/cli-comparison-and-fast-start.md (lessons from the Convex CLI and our plan)
- scripts/bridge-cli.sh (shell wrapper)


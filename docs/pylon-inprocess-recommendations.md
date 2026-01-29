# Pylon In-Process Recommendations (Autopilot Desktop)

Date: 2026-01-29

## Summary
Autopilot desktop now runs Pylon **in-process** (no daemon/CLI) with a single **Turn On / Turn Off** control in the Pylon pane. For this first pass we **only enable Ollama** (localhost:11434) and refuse to start without it. This removes the duplicate-daemon start behavior and simplifies lifecycle management.

Previously the app controlled Pylon by shelling into the `pylon` CLI, which starts/stops a daemon process and communicates via a Unix control socket. This led to:

- duplicate daemon starts (observed: start in `Both` mode, then another start in `Provider` mode),
- control socket errors during shutdown (`Resource temporarily unavailable`),
- noisy reconnection errors from the DVM / NIP-90 stack (h2 protocol errors).

Recommendation implemented (phase 1): **run Pylon in-process** inside the Autopilot desktop app (no daemon) with a single toggle button. Keep the CLI daemon for headless usage, but the desktop app should **not** spawn it.

## Observed behavior (from current logs)
- App start triggers a `pylon stop`, then two separate `pylon start` sequences with different modes.
- Control socket fails with EAGAIN during shutdown, then the CLI falls back to SIGTERM.
- DVM event loop logs: `h2 protocol error: error reading a body from connection` and reconnects.

## Current wiring (as implemented)

### UI actions → CLI
- `UserAction::PylonStart` → `run_pylon_cli(["pylon", "start"])`
- `UserAction::DvmProviderStart` → `run_pylon_cli(["pylon", "start", "--mode", "provider"])`
- `UserAction::PylonStop` / `DvmProviderStop` → `run_pylon_cli(["pylon", "stop"])`

### Daemon model
- `pylon start` daemonizes (double fork), writes PID, opens control socket, starts provider/host tasks.
- `pylon stop` tries control socket, then SIGTERM.

### Status queries
- Autopilot uses `pylon::daemon::ControlClient` + PID file status. No in-process state.

## Likely causes of duplicate starts
- The UI exposes both **Pylon** and **Sell Compute** panes. Starting in each pane triggers its own CLI start sequence (`Both` vs `Provider`).
- The CLI does not allow `start` while running (it returns early), but stale pid/control socket conditions can cause stop/start loops during retries.

## In-process Pylon runtime (implemented, phase 1)

### Goal
- Autopilot controls one in-process provider instance directly.
- No daemon process, no PID file, no control socket.
- Clean lifecycle: a single **Turn On / Turn Off** button in the Pylon pane.
- **Ollama-only**: start fails if `ollama` backend is not detected.

### Implemented design (phase 1)
The desktop app now owns an in-process provider runtime:

```
struct InProcessPylon {
  provider: Option<PylonProvider>,
  started_at: Option<Instant>,
  last_error: Option<String>,
}
```

### Key behaviors (phase 1)
- **Identity**: auto-created on first start (`identity.mnemonic` written to `~/.openagents/pylon`).
- **Start**: `PylonProvider::new` → `init_with_identity` → `start`.
- **Stop**: `PylonProvider::stop`.
- **Ollama only**: if the detected backends do not include `ollama`, start fails and the UI reports the error.
- **No daemon**: all CLI/daemon control calls removed from the desktop app.

### UI mapping changes (phase 1)
- Pylon pane now has **one** button: **Turn On / Turn Off**.
- `UserAction::PylonStart/Stop/Refresh` and DVM start/stop now dispatch to in-process state (no CLI).
- The existing “init identity” path now simply ensures `identity.mnemonic` exists.

### Follow-on steps
1. Add a first-class `pylon::runtime` module (host + provider + bridge).
2. Move Sell Compute / NIP-90 panes to use the same runtime handle.
3. Add graceful shutdown hooks on app exit.
4. Add provider diagnostics + backend display.

## DVM / NIP-90 observations
- The error (`h2 protocol error`) indicates a transport issue (likely gRPC/HTTP2) in the DVM client layer. This is not fixed by daemonizing; it should be handled with a **bounded retry policy** and proper backoff logging.
- Recommend: wrap DVM client `recv` with exponential backoff and record `last_error` in UI status, not spamming logs.

## Compute job handling (Ollama only, phase 1)
- Provider detection still runs via `BackendRegistry::detect`.
- Autopilot forces `backend_preference = ["ollama"]`, and the provider narrows to that backend when only one preference is given.
- Start fails if Ollama is not detected (localhost:11434).

## Why in-process is better for Autopilot
- Single control plane (no pid/socket failures).
- Consistent lifecycle (no mode conflicts).
- Easier to integrate UI state and logging.
- Less resource churn (no fork/exec during UI interactions).

## Risks / mitigations
- Long-running tasks must be cancellable (use `tokio::select!` + cancellation tokens).
- Need to ensure `PylonRuntime::shutdown` cleans up bridge sockets and stops tasks.
- Guard against double initialization (if user clicks Start twice).

## Work log
- Implemented in-process provider state (`InProcessPylon`) in `apps/autopilot-desktop-wgpu`.
- Replaced Pylon CLI/daemon calls with in-process start/stop/status.
- Added Ollama-only enforcement (start fails without Ollama backend).
- Simplified Pylon pane to a single Turn On/Turn Off button (already in UI).

# ADR 0009 — Desktop‑Managed Bridge (Tauri) with Standalone Compatibility

 - Date: 2025-11-03
 - Status: Accepted — Planned (implementation tracked in desktop PRs)

## Context

Today the Rust WebSocket bridge (`oa-bridge`) is launched and managed outside of the UI via the `tricoder` CLI (e.g., `npx tricoder`). Mobile connects to an already‑running bridge on the LAN or localhost. With the introduction of the Tauri desktop app, we want a first‑class desktop experience that:

- Can connect to an existing bridge (local or remote, e.g., Tailscale) with no changes to current workflows.
- Can also manage its own bridge lifecycle (spawn/stop/restart), so users don’t have to run a separate terminal process.
- Preserves the standalone deploy story for power users and CI (`npx tricoder` remains valid and recommended for headless/remote setups).

Key bridge details and current defaults are defined in `crates/oa-bridge/src/main.rs`:
- Binds WebSocket on `--bind` (default `0.0.0.0:8787`), route `/ws`.
- Persists to Tinyvex (in‑process SQLite) under `~/.openagents/tinyvex/data.sqlite3` by default.
- Spawns the Codex CLI (and Claude Code) with JSON output; tails stdout/stderr.
- Requires a token for `/ws` (persisted/created in `~/.openagents/bridge.json`).

The Tauri app already implements passive client behavior (auto token load, auto‑detect local port, and auto‑connect).

## Decision

Support two official bridge operation modes:

1) Standalone (unchanged)
   - Users run `npx tricoder` (or the installed `tricoder` binary). It launches `oa-bridge` with sensible defaults and prints the connection info.
   - Mobile/desktop connect to the advertised `ws://…/ws?token=…`.

2) Desktop‑managed (new)
   - The Tauri app can spawn and supervise an `oa-bridge` process as a sidecar and connect to it automatically.
   - If a compatible bridge is already reachable (local or remote IP), the app connects to it instead of spawning its own.
   - Users can switch between “Use existing bridge” and “Run a local bridge” in Settings.

This keeps the “single source of truth” for the bridge binary in the Rust crate and keeps `tricoder` as the headless channel, while enabling a streamlined desktop UX.

## Rationale

- Best of both worlds: power users and CI keep the simple `npx tricoder` workflow; desktop end‑users get a one‑click experience.
- Clear responsibility boundaries: The bridge remains a separate Rust binary with a stable CLI/contract; the desktop app supervises it when asked.
- Minimal disruption: Tauri simply calls the same binary with the same flags the CLI uses today.

## Alternatives Considered

1) Always run the bridge inside Tauri
   - Simpler UX but breaks headless/remote workflows and complicates server setups.

2) Embed the bridge as a library into Tauri
   - Tighter integration but reduces reuse and complicates release artifacts and Rust crate factoring.

3) Rely only on stand‑alone bridge
   - Keeps today’s model but sacrifices desktop usability and onboarding.

## Consequences

- UX: Desktop gains a “Local bridge” toggle with lifecycle controls (start/stop/restart) and a status view (port, token, logs).
- Packaging: The desktop app will ship `oa-bridge` as a Tauri sidecar for OS builds, or download the appropriate binary on first run. `tricoder` continues to distribute and run `oa-bridge` for terminal users.
- Security: The same token model is used in both modes (persisted in `~/.openagents/bridge.json`). The app displays and respects token changes.
- Networking: When both a local and a remote bridge are reachable, the app connects to the one the user selected (default: local highest port 8787–8798).

## Implementation Plan

1) Keep standalone flows intact
   - No changes to `tricoder`’s ability to install/run the bridge.
   - Ensure `oa-bridge` CLI flags remain stable (`--bind`, `--ws-token`, Codex/Claude flags).

2) Tauri: process supervision
   - Ship `oa-bridge` as a Tauri “sidecar” binary per‑platform (or lazy‑download then verify checksum/signature).
   - Add commands in `src-tauri`:
     - `bridge_start({ bind?: string, token?: string })` → spawns the child with `--bind` and passes token via env/flag.
     - `bridge_stop()` → terminates the child.
     - `bridge_status()` → returns pid, bind, token source, recent logs.
   - Redirect the child’s stdout/stderr to an internal ring buffer for display in the app (and to help support/troubleshooting).
   - Persist the token in `~/.openagents/bridge.json` if missing (keep parity with `oa-bridge` behavior).

3) Tauri: connection strategy
   - On launch, attempt to connect to an existing bridge (user‑selected host or local scan 8787–8798; highest wins) with the token from `~/.openagents/bridge.json`.
   - If none reachable and “Run local bridge” is enabled, start the sidecar on the first free port in 8787–8798 and auto‑connect.
   - Show status and allow switching to a remote host/IP (e.g., Tailscale `100.x.y.z`).

4) Logs and health
   - Expose minimal health checks (probe `/ws` → upgrade) and show the last N log lines from the child in a dedicated panel.
   - Provide quick actions: restart bridge, copy connection string.

5) Releases
   - `docs/bridge-release.md` remains the source of truth for publishing bridge artifacts.
   - Desktop CI attaches per‑platform sidecar binaries to app builds (or implements lazy download on first run using the same release tags).

## Acceptance

- “Standalone mode” still works: `npx tricoder` starts a bridge that the desktop app can discover and connect to without starting its own.
- “Desktop‑managed mode” works: Tauri can start/stop/restart a local bridge sidecar, shows its status, and auto‑connects.
- Token/port management is consistent between modes (app uses the same token file; highest local port preference is preserved).
- No regression for mobile or remote desktop scenarios (connecting to a remote host/IP continues to work).

## Notes / References

- Bridge entrypoint and defaults: `crates/oa-bridge/src/main.rs`
- Current desktop helpers: `tauri/src/App.tsx` auto‑token/auto‑port/auto‑connect; `src-tauri/src/lib.rs` token loader.
- Packaging: see `docs/bridge-release.md` for release channels and artifact distribution; sidecar strategy to mirror the same tags.


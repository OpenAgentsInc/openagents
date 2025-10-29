# Tricoder One‑Command Bootstrap (Design Notes)

Goal: a single command (`npx tricoder`) that installs prerequisites as needed, launches the local bridge, and optionally exposes public tunnels so the mobile app can connect — with safe defaults, clear prompts, and predictable behavior across macOS, Linux, and Windows.

This doc captures constraints, options, risks, and a pragmatic implementation plan.

## UX Goals

- Single entrypoint: `npx tricoder [flags]`
- Zero manual steps for a fresh machine; auto‑download or build required binaries
- Safe on low‑resource Linux boxes; never hard‑lock the system
- Clear, concise console output; actionable errors, no walls of logs
- Re‑runnable: subsequent runs should be fast (reuse cached artifacts)
- Respect repo policies:
  - Rust bridge (`oa-bridge`) is the only `/ws` server
  - No HTTP control; bridge control only via WebSocket messages

## Outcomes (Happy Path)

1) Local bridge running on `ws://127.0.0.1:8787/ws`
2) Optional public tunnels established:
   - Bridge WS via `oa-tunnel` → `bore.pub`
   - Convex HTTP via another `oa-tunnel` → `bore.pub`
3) Printed pair code payload (bridge + convex URLs) ready to paste into the mobile app

## Inputs and Flags (proposed)

- `--local-only`  Run without public tunnels (local app/dev only)
- `--no-install`  Do not install toolchains/binaries; fail with guidance
- `--force-build` Force local Rust build even if prebuilt binaries exist
- `--jobs <n>`    Limit parallel build jobs when compiling (default: 2 on Linux, else auto)
- `--verbose/-v`  Verbose logs (debug checks and tails)
- `--yes/-y`      Assume “yes” to prompts (CI/non‑interactive)

## Platform Matrix and Strategy

We need two Rust binaries: `oa-bridge` and `oa-tunnel`.

Order of preference per platform:

1) Prebuilt release download (preferred)
   - Pros: fastest; avoids local compile; consistent CPU/RAM usage
   - Cons: release infra required; need code signing/notarization where applicable
2) Local build via `rustup + cargo`
   - Pros: no release infra dependency; always possible (with toolchain)
   - Cons: heavy CPU/RAM; first build can be long; risky on low‑memory Linux

Target triples (minimum):
- macOS: `aarch64-apple-darwin`, `x86_64-apple-darwin`
- Linux: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`
- Windows: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` (as feasible)

Binary distribution:
- Host binaries on GitHub Releases under this repo
- Tag version matches npm package version (e.g., `tricoder@0.2.0` → `openagents@v0.2.0`)
- Assets: tar.gz/zip per target with a checksum file (SHA256)
- Verification: `npx tricoder` verifies checksum before placing in cache

Cache locations:
- `OPENAGENTS_HOME` (default: `~/.openagents`)
- Place binaries in `~/.openagents/bin/` (e.g., `oa-bridge`, `oa-tunnel`)
- Mark executable; never install system‑wide; never require `sudo`

## Boot Flow (proposed)

1) Node preflight (always lightweight)
   - Node >= 18 check
   - Disk space and memory quick check (warn when <1 GB RAM free on Linux)
   - Ports availability scan for 8787/7788; pick alternates if busy

2) Resolve binaries
   - If prebuilt found in cache and matching version → use
   - Else if allowed to download → fetch from GitHub Releases, verify checksum, cache
   - Else if allowed to build → ensure `rustup` installed (install if user consents), then `cargo build -p oa-bridge -p oa-tunnel`
     - Concurrency caps: `CARGO_BUILD_JOBS` (default 2 on Linux), and sequential builds to avoid spikes
     - Surface progress, ETA, and a cancel hint
   - If none possible → fail with clear guidance

3) Start local services
   - Start `oa-bridge` bound to `0.0.0.0:8787` (default)
   - Health probe via WS; print status

4) Optional tunnels (skip with `--local-only`)
   - Start `oa-tunnel` to `bore.pub` for the bridge (WS)
   - Start second `oa-tunnel` to `bore.pub` for Convex (HTTP)
   - Summarize tunnel events (rate‑limited), not the full bore logs

5) Pair code and tips
   - Print one base64 payload with bridge + convex URLs (no secrets)
   - In verbose mode, start tails: WS event tail, basic convex probes

6) Lifecycle
   - Keep foreground process running; exit cleanly on Ctrl‑C
   - Clean up child processes; do not daemonize by default

## Linux‑Specific Considerations (stability)

- OOM/thermal risk: concurrent `cargo run` or multi‑crate builds can freeze low‑RAM hosts
- Mitigations:
  - Default to prebuilt downloads
  - If compiling: cap jobs (2) and build sequentially; `nice` and `ionice` when available
  - Preflight memory check; warn and require `--yes` to continue when low
  - Avoid compiling the same crate twice concurrently (no parallel `cargo run`)

## Security and Policy Alignment

- Bridge is the only `/ws` server; `npx tricoder` must not stand up its own Node WS
- No HTTP control to the bridge; only WS messages
- The bridge injects permissive sandbox/approvals for development; print clear warnings about trust boundaries
- Tunnels: `bore.pub` is third‑party; provide opt‑out via `--local-only`
- Binaries are verified via checksums

## Networking and Proxies

- Download sources: GitHub Releases; fallback mirrors optional
- Respect `HTTP(S)_PROXY`/`NO_PROXY` envs
- Connectivity checks with backoff; error summaries include next steps

## Port and Firewall Handling

- Probe 8787 (bridge) and 7788 (Convex) locally; if busy, pick next available in range (e.g., 8787–8799, 7788–7799)
- Print selected ports and include them in the pair code
- For public tunnels, pick server port assigned by `bore.pub` (as today)

## Compatibility with Current Repo Rules

- Rust workspace and lockfile remain at repo root
- `npx tricoder` should work both inside the repo (dev) and outside (user install)
- When inside repo, prefer the local workspace binaries (debug/dev flow)
- When outside repo, prefer cached prebuilt binaries

## Data and Logs

- Config and cache under `~/.openagents/`
- Minimal console output by default; verbose mode reveals tails and probes
- Log rotation not required for foreground UX; long‑running daemonization is out‑of‑scope for now

## Uninstall / Cleanup

- `npx tricoder --uninstall` removes `~/.openagents/bin/{oa-bridge,oa-tunnel}` and cache entries after confirmation

## Failure Modes and Messages

- Cannot download binaries → suggest `--force-build` if Rust is acceptable, or `--local-only`
- Rust toolchain install blocked → abort with guidance link
- Ports unavailable → print fallback ports used and how to override
- Tunnels blocked by firewall → suggest `--local-only` and LAN testing

## Implementation Plan (phased)

Phase 1: Prebuilt binary bootstrap
- Add GitHub Actions to build and publish `oa-bridge` and `oa-tunnel` for the target matrix with checksums
- Update `packages/tricoder` to:
  - Detect repo vs non‑repo run
  - Resolve binaries (cache → download → build)
  - Start bridge and optional tunnels with resource‑safe defaults (no concurrent builds)
  - Add `--local-only`, `--no-install`, `--force-build`, `--jobs`, `-y`, `-v`

Phase 2: Polish and resilience
- Proxy support, improved error banners, better progress bars
- Port auto‑selection and pair code augmentation
- Basic self‑update hints if npm package version < latest

Phase 3: Optional backgrounding
- Explore platform‑native backgrounding (LaunchAgent/systemd/Windows service) as an opt‑in, with clear stop/uninstall

## Open Questions

- Signing/notarization requirements for macOS binaries?
- Windows target support level and testing bandwidth?
- Is `bore.pub` the long‑term tunnel, or should we support a configurable bore host?
- Do we want a “no‑network” mode that never does any outbound downloads, even checksums?

## Acceptance Criteria (v0)

- Fresh Linux/macOS host with Node >= 18 can run `npx tricoder` and:
  - Download or build both Rust binaries without freezing the system
  - Launch the bridge locally and print a working pair code
  - Exit cleanly on Ctrl‑C without orphan processes
  - Re‑run completes in < 3s when binaries are cached and no tunnels requested

---

Notes related to the incident: On Linux, running multiple `cargo run` processes in parallel (bridge + two tunnels) can spike CPU/RAM and trigger OOM or thermal shutdowns. The above plan avoids concurrent compiles, prefers prebuilt downloads, and caps build parallelism by default on Linux.

 
## Addendum — Implementation Notes and Recommendations

- Binary resolution order and prompts
  - Prefer prebuilt downloads per target triple into `~/.openagents/bin` with SHA256 verification.
  - If Rust toolchain is missing and a download is unavailable, prompt clearly before installing Rust (respect `--yes`).
  - If Rust is present, build sequentially with capped jobs; never spawn concurrent `cargo run` builds.

- Detection logic (repo vs non‑repo)
  - Inside repo: use workspace `cargo run -p oa-bridge` and `-p oa-tunnel` for dev ergonomics.
  - Outside repo: never require cloning; resolve to cached or downloaded binaries. Only fall back to building if the user explicitly opts in or `--force-build` is set.

- Convex bootstrap
  - Keep Convex out of the critical path for first paint: start bridge first, then background Convex setup with a concise spinner + health line.
  - Use Bun if present; otherwise fall back to `npx convex dev` automatically. Skip quietly when neither is available and print a one‑line hint.

- Port selection and pairing payload
  - Probe and select free local ports within small ranges. Include selected ports in the base64 pairing payload so the app can connect without guessing.
  - When tunnels are disabled (`--local-only`), still print a local‑only pairing payload for LAN use.

- Safety on Linux and small hosts
  - Default `--jobs 2` for local builds on Linux; consider `nice`/`ionice` when available.
  - Detect <1 GB free RAM: require `--yes` or emit a prominent warning before compiling.

- Security posture
  - Keep the Rust bridge as the single `/ws` endpoint. Tricoder must not create a Node WS server.
  - Surface a short security banner when exposing public tunnels (third‑party service, plaintext if not yet TLS, private code caution).

- Future TLS and token auth
  - Plan for `wss://` by either fronting Bore with a TLS terminator or baking TLS into our vendored tunnel. Gate `/ws` by an optional token in `OPENAGENTS_HOME` and accept via `?token=` or `Authorization: Bearer`.

- UX polish
  - Always print an environment assessment at start (platform, repo found, Rust, Bun/NPM, codex presence).
  - Keep default logs succinct; show tails only with `--verbose`.
  - Offer `--uninstall` to remove cached binaries.

These align with the repo’s policies and should make `npx tricoder` behave predictably on a fresh computer without requiring the OpenAgents repo. In the interim, we can keep the “inside‑repo” path while we stand up releases for prebuilt binaries.

## Second Addendum — Commentary and Decisions

This builds on the first addendum’s implementation notes. Key clarifications and decisions to keep `npx tricoder` robust across environments:

- Linux binary compatibility (glibc vs musl)
  - Ship two Linux flavors where feasible: `x86_64-unknown-linux-gnu` (glibc) and `x86_64-unknown-linux-musl` (static) to avoid old‑glibc issues on distros like CentOS/Ubuntu LTS. Prefer musl when available; fall back to glibc if dynamic linking is required.
  - Document minimum glibc version in the release notes when using glibc builds.

- Windows specifics
  - Distribute `.exe` binaries and ensure child process cleanup uses platform‑appropriate termination: `taskkill /PID <pid> /T` on exit in addition to Node signal handling.
  - Avoid Unix‑specific niceness (nice/ionice); instead, throttle with sequential work and progress updates.

- Corporate proxies and offline hosts
  - Respect `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY` for all downloads.
  - When building, pass through `CARGO_HTTP_PROXY`/`CARGO_HTTP_DEBUG` if set.
  - Provide `--no-install` and a clear offline mode that skips downloads and explains what’s missing.

- Integrity and release verification
  - Always publish SHA256 sums and verify on client; expose a single JSON manifest per release with filenames, sizes, and checksums.
  - Optional signature (minisign or cosign) is a stretch goal; the checksum manifest is sufficient for v0.

- Download controls and mirrors
  - Allow `OPENAGENTS_DOWNLOAD_BASE` to override the GitHub Releases origin for air‑gapped mirrors/CDNs.
  - Use ETag/If‑None‑Match to avoid redundant downloads; stream to disk with progress and low memory footprint.

- Port conflict handling
  - Probe 8787/7788; if occupied, select the next free port in a short range and print the chosen ports.
  - For “kill and restart” ergonomics, only offer a guided prompt in interactive mode; never auto‑kill without explicit consent.

- Child process lifecycle
  - On Ctrl‑C and process exit, terminate both tunnels and the bridge with a short timeout; on timeout, issue a forceful kill.
  - Ensure no orphan processes on all platforms (double‑check Windows process‑tree behavior).

- Security modes
  - Keep today’s permissive dev flags as the default, but add `--safe` to run the bridge without bypassing sandbox/approvals. Reflect the mode in the printed banner and pairing payload.

- Self‑update guidance
  - If the running npm version is not the latest, print a one‑liner: `Tip: run npx tricoder@latest for updates`.

- Telemetry
  - No telemetry. Only local console output, with an opt‑in `--verbose` tail for debugging.

- TLS and WSS (future)
  - Plan a follow‑up to enable `wss://` by placing a TLS terminator in front of Bore or shipping a TLS‑enabled tunnel. Keep today’s pairing format flexible for `wss://` URLs.

These items keep the bootstrap reliable on constrained Linux hosts, workable behind corporate proxies, and ready for future hardening without changing the basic one‑command UX.

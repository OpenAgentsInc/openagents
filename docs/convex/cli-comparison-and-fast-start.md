# Convex CLI Comparison & Fast‑Start Plan

Date: 2025-10-29
Owner: OpenAgents (oa-bridge)

## Summary

The official Convex CLI downloads a precompiled local backend binary into a versioned cache and starts a lightweight local deployment that is ready in a few seconds on a fresh DB. Our bridge currently manages its own binary path/state and often waits tens of seconds for readiness when the DB has grown or indices need warming.

This doc compares approaches and outlines concrete changes to bring our cold‑start to a few seconds by adopting proven pieces from the Convex CLI.

## What the Convex CLI does

- Cached binary per release
  - Picks platform artifact name, finds a stable GitHub release containing it, downloads the zip once into `~/.cache/convex/binaries/<version>/convex-local-backend[.exe]`, chmods, and reuses.
- Local state layout
  - Stores runtime state under `~/.convex/convex-backend-state/<deployment>/` (e.g. `convex_local_backend.sqlite3`, `convex_local_storage`). Fresh deployments start on an empty DB and are ready in a few seconds.
- Readiness handshake
  - Spawns the backend and waits for `GET /instance_name` to return the expected name before proceeding.
- Caller owns lifecycle
  - `npx convex dev` supervises the subprocess and does a quick smoke test (`--help`) to fail fast on platform issues.

References: npm‑packages under convex‑backend monorepo — download.ts, filePaths.ts, localDeployment.ts, run.ts. The Developer Hub documents the local state paths and behavior.

## Where our bootstrapper differs today

- Binary location and install
  - Expects `~/.openagents/bin/local_backend`. A best‑effort `bunx convex dev --configure ... --once --skip-push` runs if missing, but we don’t integrate with the CLI’s cache.
- State path and DB size
  - Uses `~/.openagents/convex/` for DB and storage. As we mirror more data (threads/messages/tools), this DB can grow and lengthen warm‑up.
- Readiness probe
  - Probes TCP + `GET /instance_version`/`/health_check`. Readiness might be reported later than `/instance_name` and we don’t validate instance identity.
- Bind interface
  - Default bind is `0.0.0.0` to support LAN/VPN. The CLI defaults to local dev ergonomics; loopback is typical and simple.

Result: even when the binary is present, a large/aged DB under `~/.openagents/convex/` can make the first healthy response arrive after 20–40s.

## Lessons to adopt (to get to ~2–5s)

1) Reuse the official cached binary
- Detect `~/.cache/convex/binaries/*/convex-local-backend[.exe]` and prefer the latest present version. Symlink/copy it to our expected path on first run, or exec it directly.
- If absent, call the CLI’s download path (equivalent of `ensureBackendBinaryDownloaded`) via `bunx convex dev --configure --dev-deployment local --once --skip-push` and then pick the cached binary.

2) Use the CLI’s local state layout by default
- Default our runtime state to `~/.convex/convex-backend-state/openagents-dev/` (or a namespaced deployment). This keeps the DB small for dev and aligns with tooling.
- Option: make state ephemeral for speed (`OPENAGENTS_CONVEX_EPHEMERAL=1`) by using a temp dir for DB/storage. Mirroring can still write to Convex while Codex JSONLs remain the source of truth.

3) Switch to the instance‑name handshake
- Pass `--instance-name openagents` when spawning and poll `GET /instance_name` for equality. This endpoint is designed as the earliest readiness signal the CLI uses.

4) Prefer loopback by default
- Bind `127.0.0.1` unless the user explicitly opts into LAN exposure. This mirrors the CLI’s local dev posture and reduces complexity.

5) Keep the backend warm across bridge runs
- Add a small “daemon” mode or reuse an already running process (detect by port + instance name). With a warm DB, reconnect time is near‑instant.

6) Defer heavy work until after ready
- Maintain `convex_ready` gating (already implemented) and move any optional background tasks (indexing, backfill, FS→Convex sync) after readiness with gentle backoff.

## Concrete plan (code‑level changes)

- bootstrap.rs
  - Add `find_official_cached_binary()` to locate `~/.cache/convex/binaries/*/convex-local-backend[.exe]` and pick the newest tag by lexical sort (Convex tags are stable). If found, use it. If not, run our existing best‑effort `bunx convex dev --configure ...` and retry lookup.
  - Support `OPENAGENTS_CONVEX_STATE` modes:
    - `convex` → `~/.convex/convex-backend-state/openagents-dev` (default)
    - `openagents` → `~/.openagents/convex` (legacy)
    - `ephemeral` → OS temp dir
  - Spawn flags: add `--instance-name openagents` and change readiness probe to poll `/instance_name` for `openagents`.
  - Default `OPENAGENTS_CONVEX_INTERFACE=127.0.0.1`; allow `0.0.0.0` when explicitly requested.

- main.rs
  - Keep `convex_ready` gate and background health watcher; allow attaching to an already running backend (no restart) if `/instance_name` matches and port is bound.

- scripts/convex-cli.sh
  - No change required; continues to be used for function deploys with the configured URL and admin key.

## Expected impact

- Fresh dev environment (empty DB under `~/.convex/...`): readiness in ~2–5s on typical hardware.
- Warm environment with small/moderate DB: ~1–3s readiness; subsequent `cargo bridge` runs do not restart the backend in “daemon”/reuse mode.
- Large existing DBs (legacy path): opt into `convex` or `ephemeral` state to avoid long warm‑ups; mirroring can repopulate as needed.

## Risks & mitigations

- Divergence from legacy state path
  - Provide an env flag to use the old `~/.openagents/convex` path; default new installs to the CLI layout.
- Permissions/ownership of cached binaries
  - Respect the CLI’s cache directory; avoid modifying its contents. If we need a stable path, symlink into `~/.openagents/bin/local_backend`.
- Endpoint compatibility
  - Keep `/instance_version` fallback for older builds; prefer `/instance_name` when `--instance-name` is set.

## Quick wins we can ship first

- Use official cached binary (no behavioral change, immediate reuse of proven artifacts).
- Switch readiness probe to `/instance_name` with `--instance-name`.
- Default to loopback bind and add an explicit “Expose on LAN” toggle in settings.
- Add `OPENAGENTS_CONVEX_STATE=convex|openagents|ephemeral` and default to `convex`.

## References

- Official CLI source (download, cache, run): see `download.ts`, `filePaths.ts`, `localDeployment.ts`, `run.ts` in the convex‑backend monorepo.
- OpenAgents bootstrapper: `crates/oa-bridge/src/bootstrap.rs`, `crates/oa-bridge/src/main.rs`.
- Related docs: `docs/convex/convex.md`, `docs/convex/bridge-setup.md`, `docs/convex/hardening-audit.md`.


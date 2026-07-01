# Khala Code Codex Wrapper Implementation Log

Date: 2026-07-01
Tracking epic: <https://github.com/OpenAgentsInc/openagents/issues/7780>
Audit: `docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md`

## Issue #7781: Codex Install And Auth Gate

Status: implemented

Khala Code Desktop now has a typed Codex harness readiness projection for the
main user Codex session. The probe checks:

- the Codex command source (`PATH`, `KHALA_CODE_CODEX_BINARY`,
  `KHALA_CODE_CODEX_COMMAND`, or explicit test input);
- `codex --version` availability and version text;
- the main user Codex home (`CODEX_HOME` or default `~/.codex`);
- `auth.json` presence, JSON shape, and token-field presence without exposing
  token values;
- the distinction between the main user Codex home and isolated Pylon fleet
  worker homes.

The desktop RPC now exposes `codexHarnessStatus()`, composes
`codexAccountsStatus()` with that harness gate, and marks `codingStatus()` as
blocked when Codex is missing, unsigned, or invalid. Unified Inbox projects
missing main Codex setup as a critical local blocker.

The README now states the pivot clearly: the default Khala Code harness requires
Codex, while the hosted Khala/OpenRouter runtime is legacy/fallback during the
transition.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop test`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7782: App-Server Supervisor And Typed Client

Status: implemented

Khala Code Desktop now has an idle-by-default Codex app-server host in the Bun
process. The host supervises `codex app-server --stdio`, speaks newline-delimited
JSON-RPC, and performs the required `initialize` request followed by the
`initialized` notification. It records bounded diagnostics, maps request errors,
tracks pending requests, supports timeouts, exposes notification subscriptions,
and provides start, stop, restart, status, request, and dispose methods.

The desktop RPC now exposes:

- `codexAppServerStatus()`
- `codexAppServerStart()`
- `codexAppServerStop()`
- `codexAppServerRestart()`

The desktop app constructs one host and disposes it on process exit or signal.
The host does not auto-start during ordinary readiness polling; later issues can
use it as the kernel for thread and turn lifecycle without spawning Codex from
every status check.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop test`
- `bun run --cwd clients/khala-code-desktop verify`

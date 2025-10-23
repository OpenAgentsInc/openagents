# Settings Audit (OpenAgents App)

This document explains every option on the Settings screen, how it’s persisted, how it is used in-app, what the bridge does with it, and what actually reaches the Codex CLI at runtime.

## Overview

- Source files
  - Settings UI: `expo/app/settings/index.tsx:1`
  - Settings store (persist): `expo/lib/settings-store.ts:1`
  - WS provider (connection + request wrappers): `expo/providers/ws.tsx:1`
  - Project-aware send: `expo/providers/projects.tsx:1`
  - Bridge (Rust): `crates/codex-bridge/src/main.rs:1`, history helpers `crates/codex-bridge/src/history.rs:1`

- Persistence
  - Settings are saved with Zustand persist to AsyncStorage under `@openagents/settings-v1` (see `expo/lib/settings-store.ts:1`).
  - The app normalizes the Bridge Host for connection only; the raw value you type is preserved for display (see `expo/providers/ws.tsx:1`).

## Connection

- Bridge Host (host:port)
  - Persisted key: `@openagents/settings-v1` → `bridgeHost`
  - Used by: WS connect to `ws://<host>/ws` (see `expo/providers/ws.tsx:1`).
  - Notes: Input remains exactly as you type; we normalize to host:port internally for dialing.

- Connect / Disconnect
  - Triggers WebSocket open/close (see `expo/providers/ws.tsx:1`). Auto‑retry every 3s when disconnected.

## Dev Tools

- Clear Log (button)
  - Calls `useBridge().clearLog()` which invokes a handler registered by the live Session screen to clear the on‑screen feed (see `expo/providers/ws.tsx:1`, `expo/app/session/index.tsx:266`).
  - It does NOT delete historical logs or AsyncStorage; it only resets the current live feed state.

## Preferences → What’s actually sent

The app composes a one‑line JSON “preface” sent as the first line of each prompt (see `expo/providers/projects.tsx:58`). Example keys:

```json
{ "sandbox": "danger-full-access" | "read-only", "approval": "never|on-request|on-failure", "cd": "/path", "project": { ... }, "resume": "<id|last|new>" }
```

- Filesystem (Read‑only | Write)
  - UI key: `readOnly` (persisted)
  - App effect: Sets `preface.sandbox` and alters the human preface text (see `expo/providers/projects.tsx:58`, `buildHumanPreface`).
  - Bridge/CLI reality: The bridge always spawns Codex with full access:
    - `-s danger-full-access` and `-c sandbox_mode="danger-full-access"`
    - See `crates/codex-bridge/src/main.rs:418` (in `build_bin_and_args`).
  - Net: Informational only for now; the CLI process still runs with full access regardless of toggle.

- Network (Restricted | Enabled)
  - UI key: `networkEnabled` (persisted)
  - App effect: Only changes the human preface text (developer hint to the agent).
  - Bridge/CLI reality: Not enforced by the bridge; no network sandboxing flags are passed/removed based on this toggle.

- Approvals (never | on‑request | on‑failure)
  - UI key: `approvals` (persisted)
  - App effect: Sets `preface.approval` and updates the human preface text.
  - Bridge/CLI reality: Overridden to “never” at process level via `-c approval_policy="never"` (see `crates/codex-bridge/src/main.rs:452`).
  - Net: Informational only; the effective policy at the CLI is “never”.

- Attach preface to prompts (On | Off)
  - UI key: `attachPreface` (persisted)
  - App effect: If On, prepends a human‑readable context block (capabilities, environment, active project) to the user message (see `buildHumanPreface` in `expo/providers/projects.tsx:101`). If Off, only the user text is sent.
  - Bridge/CLI reality: This one is real — it changes what the model sees for instruction/priming. No bridge override.

## Project + Resume

- Active Project (from Projects drawer)
  - When sending, includes `preface.project` and `preface.cd` (working directory) if set (see `expo/providers/projects.tsx:71`).
  - The bridge expands `~/` and uses the path for the Codex child process working directory on the next spawn (see `crates/codex-bridge/src/main.rs:165`, `expand_home`).

- Resume semantics
  - The app sets `preface.resume` to `'new'`, `'last'`, or a specific id from a historical thread.
  - The bridge resolves this into `exec resume ...` at spawn time when the Codex binary supports it (see `crates/codex-bridge/src/main.rs:393`, `cli_supports_resume`).

## What the bridge always forces

On every Codex spawn the bridge injects the following (unless the user provided them explicitly via `CODEX_ARGS`/extra args):

- Model: `-m gpt-5`
- Reasoning effort: `-c model_reasoning_effort="high"`
- Approvals bypass: `--dangerously-bypass-approvals-and-sandbox`
- Sandbox mode: `-s danger-full-access`, `-c sandbox_mode="danger-full-access"`, `-c sandbox_permissions=["disk-full-access"]`
- Approvals policy: `-c approval_policy="never"`

See `crates/codex-bridge/src/main.rs:418` for the full injection logic. These overrides mean the Filesystem/Network/Approvals toggles in Settings do not alter the actual process sandbox or approval mechanics; they only shape the preface JSON + human instructions today.

## Recommendations

- Keep “Attach preface” ON (default). It meaningfully steers agent behavior and carries the active project context.
- Consider hiding or demoting Filesystem/Network/Approvals toggles (or wiring them to real enforcement) to avoid confusion, since the bridge forces full access and never approvals.
- “Clear Log” is useful for resetting the current live feed, but it does not affect historical logs; it’s safe but limited.
- The HTTP base is unused now that the bridge operates WS‑only; Settings shows the derived WS endpoint only.

## Future work (optional)

- Honor “Read‑only” by swapping bridge flags to a read‑only preset when selected.
- Make “Network Restricted” meaningful (e.g., block shell network commands or pass a config the CLI honors).
- Allow per‑project overrides (model, flags) with safe defaults and explicit UI cues when overridden by the bridge.


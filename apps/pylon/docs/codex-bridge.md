# The Local Codex Bridge

Promise: `autopilot.codex_probe_pylon_successor.v1`. Epic: #4793
(rungs CX1 #4788, CX2 #4789, CX3 #4790, CX4 #4791, CX5 #4792).
Roadmap addendum:
`docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`
(workspace root). Reference implementation: the Claude Agent bridge
(`docs/claude-agent-bridge.md`, epic #4717) — this lane is its peer
adapter behind the same executor gate, the design #4717 explicitly
anticipated ("`local_codex` and `local_claude_agent` are peer adapters
behind one gate").

Pylon can talk to your local Codex: the Pylon worker loop hands a
coding assignment to the Codex SDK (`@openai/codex-sdk`) running on
your machine, with your credentials, in a bounded sandboxed workspace,
and returns public-safe closeout refs. This document covers the
credential-policy review, the readiness probe, capability declaration,
configuration, and the rules that bind the lane.

Not to be confused with the legacy `codex_runtime` fixture gate
(`agentKind: "codex_cli_or_fixture"`, the runtime-gate fallback in the
worker loop): that gate predates this lane and proves binary-runtime
plumbing, not Codex SDK execution. This lane is `codex_agent_task`,
`agentKind: "codex_sdk"`.

## Credential policy (the CX1 ToS review)

Reviewed 2026-06-11 against OpenAI's published Codex terms posture and
the OpenAgents no-resale law. The lane honors exactly three credential
sources, all owner-held, checked by presence only and in this order:

1. **`CODEX_API_KEY`** — the owner's own Codex API key
   (`credential.source.codex_agent.codex_api_key`).
2. **`OPENAI_API_KEY`** — the owner's own OpenAI API key
   (`credential.source.codex_agent.openai_api_key`).
3. **The owner's own Codex CLI login** — a non-empty `auth.json` under
   `$CODEX_HOME` (default `~/.codex/`), created by the owner running
   `codex login` themselves on this device
   (`credential.source.codex_agent.codex_cli_login`). The Codex
   CLI/SDK is built by OpenAI to run on the user's own machine under
   the user's own ChatGPT/Codex sign-in; using your own login for your
   own jobs on your own device is the intended use. The file is never
   read by the probe — presence and non-emptiness only.

**Never honored, by policy:** platform-supplied keys, shared or pooled
accounts, leased credentials on contributor devices, login brokering,
or any credential the device owner does not personally hold. OpenAgents
never resells, rents, proxies, or brokers provider accounts,
subscription seats, sessions, or API access — it pays for accepted work
output only.

**Scope boundary flagged for future review:** this lane is owner-jobs,
no-spend first (your machine, your credentials, your job). Serving
*other people's* jobs (Lane C, P6 #4782) on subscription-login auth
raises resale questions this review does not clear; a paid Lane C leg
on `codex_cli_login` auth requires its own ToS review before it runs.
API-key sources are the safe default for provider-mode work.

Cost honesty: you pay for your own inference, on whichever source the
probe reports.

## What ships in this surface (CX1, #4788)

- `@openai/codex-sdk` as an **optional dependency** with a **lazy
  import**: every Pylon command works when the SDK is absent. The SDK
  bundles its own platform-native `codex` binary via npm platform
  packages, so no separate CLI install is required — and a Pylon
  without the SDK simply never declares the capability.
- `probeCodexAgentReadiness()` (`src/codex-agent.ts`): reports one of
  `ready`, `sdk_missing`, `credentials_missing`, `platform_unsupported`,
  or `disabled_by_config`. The probe checks **presence only** — it
  never reads, logs, or persists credential values.
- Capability declaration: `pylon provider go-online` declares
  `capability.pylon.local_codex` when and only when the probe reports
  `ready`, and strips a stale declaration when it does not. The
  capability ref is the only public signal that a local Codex exists;
  no machine details, paths, or account identifiers leave the device.

## Configuration

Optional `codex` section in the Pylon config file
(`~/.pylon/config.json` by default):

```json
{
  "codex": {
    "enabled": true,
    "model": "gpt-5.4-codex",
    "maxTurns": 12,
    "timeoutSeconds": 600,
    "sandboxMode": "workspace-write"
  }
}
```

- `enabled: false` disables the lane regardless of SDK/credential
  presence.
- `model`, `maxTurns`, `timeoutSeconds`, `sandboxMode` are runtime
  preferences consumed by the executor gate (CX2); they cap, never
  expand, what an assignment may do. Config is preference, not
  authority.
- `sandboxMode` accepts only `read-only` or `workspace-write`.
  `danger-full-access` is never configurable and the executor never
  uses it.
- Never put credential values in the config file; the bridge reads
  credentials from the environment and the owner's own CLI login state
  only.

## Boundaries

- **Your identity, your credentials, your machine.** The lane acts only
  with the local owner's credentials; no platform keys on devices,
  ever.
- **Sandbox law (the design delta from the Claude bridge).** The Codex
  SDK has no PreToolUse hook, so the workspace boundary is enforced by
  the SDK sandbox (`workspace-write` pinned to the bounded working
  directory, no additional directories, network disabled inside the
  thread) plus post-hoc validation that every reported `file_change`
  stayed inside the workspace. Any violation produces a typed rejected
  closeout.
- **Redaction law.** Raw SDK events, prompts, file contents, provider
  payloads, and local paths never leave the device. Closeouts carry
  hashed refs only, through the existing `assertPublicProjectionSafe`
  boundary.
- **Authority unchanged.** Worker closeout is not accepted work; the
  lane grants no settlement, payout, deploy, spend, or Forum
  publication authority.
- **Copy law.** This lane is "Codex" / "your local Codex" in any
  user-facing copy; nothing may imply an OpenAI partnership, and
  nothing about this lane may be described as shipped or autonomous
  before its receipts exist.

## Executor gate (CX2, #4789)

`executeCodexAgentAssignment` (`src/codex-agent-executor.ts`) sits in
the worker loop's executor chain after the Claude Agent gate and
before the runtime-gate fallback, returning `null` for any assignment
that does not carry the `codex_sdk` work class. When it does run:

- the fixture workspace is materialized under
  `~/.pylon/cache/codex-agent-tasks/<hashed-ref>`;
- the production runner (`runWithCodexSdk`) opens one Codex SDK
  thread pinned to that directory with `approvalPolicy: "never"`,
  `skipGitRepoCheck`, network disabled, and the effective sandbox
  mode (`read-only` requested anywhere wins; default
  `workspace-write`; full access does not exist on this code path);
- the wall-clock budget is enforced through the turn `AbortSignal`;
- every `file_change` the thread reports is validated post hoc
  against the workspace — an escape aborts the thread and produces
  `blocker.assignment.codex_agent_workspace_escape_blocked`;
- the fixture's verification command runs independently in the gate;
  only a passing exit code yields an accepted closeout;
- typed refusal arms: `codex_agent_unavailable` (+ probe blockers),
  `codex_agent_execution_refused`, `codex_agent_budget_exceeded`,
  `codex_agent_workspace_escape_blocked`, `codex_agent_test_failed`.

## Work class, dispatch, and smokes (CX3, #4790)

- Wire format: `jobKind: "codex_agent_task"` with
  `codingAssignment.codex` (`schema:
  openagents.pylon.codex_agent_task.v0.3`, `agentKind: "codex_sdk"`,
  `fixtureRef`, optional `sandboxMode`/`timeoutSeconds`) — the
  structural peer of `codingAssignment.claudeAgent`, inside the same
  normalized assignment payload. `requiredCapabilityRefs` travels
  inside the codingAssignment so Pylon-side admission enforces the
  capability too, not just operator dispatch.
- Operator dispatch:
  `apps/openagents.com/workers/api/scripts/codex-task-dispatch.ts`
  (twin of `claude-agent-task-dispatch.ts`), `unpaid_smoke` only,
  ref-only payload, no instruction text on the wire.
- Smokes: `bun run smoke:codex-agent-task` (CI-safe: local harness,
  mock runner, full worker-loop lifecycle, redaction scan) and the
  `--live` leg for CX4. Runbook: `codex-agent-task-smoke.md`.

## Current status

CX1 (#4788) ships the probe, capability declaration, credential-policy
review, and config surface; CX2 (#4789) ships the bounded executor
gate in the worker loop; CX3 (#4790) ships the `codex_agent_task` work
class, operator dispatch, and the CI-safe + live smoke harness. The
live-device leg (CX4 #4791) and the API-parity path (CX5 #4792) follow
on the epic. Until CX4's receipts exist, product copy must keep saying
the Codex bridge is implemented but not proven live in production.

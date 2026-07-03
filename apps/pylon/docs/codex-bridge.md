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
the OpenAgents no-resale law. The lane honors exactly four owner-held
credential sources in this order; probes expose only public source refs, never
credential values:

1. **`CODEX_API_KEY`** — the owner's own Codex API key
   (`credential.source.codex_agent.codex_api_key`).
2. **`OPENAI_API_KEY`** — the owner's own OpenAI API key
   (`credential.source.codex_agent.openai_api_key`).
3. **OpenAgents custody re-prime for a linked owner account** — Pylon
   requests short-lived `OPENCODE_AUTH_CONTENT` from
   `/api/pylon/provider-accounts/chatgpt-codex/auth-material` using the
   registered agent bearer linked to the owner's OpenAuth account
   (`credential.source.codex_agent.opencode_auth_content`). The Worker
   stores and refreshes the long-lived OAuth refresh token in owner-scoped
   custody; the returned auth content never includes a `refresh` field.
4. **The owner's own Codex CLI login** — a non-empty `auth.json` under
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
consumer subscription seats or sessions — it pays for accepted work output
only. API-key inference resale belongs to the Provider Capacity Marketplace
Gate, not this subscription-account prohibition.

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
- The local dashboard composer uses the same SDK package for direct
  owner-supervised prompts: `src/codex-composer.ts` opens a typed SDK
  stream in the current working directory, defaults to
  `workspace-write`/`approvalPolicy: "never"`, surfaces structured
  progress events in the TUI, and reports SDK/auth blockers before any
  thread starts. This is the daily-driver path (#4839), separate from
  public assignment execution. #4840 adds an explicit local-only
  `local_supervised_danger` mode that maps to SDK
  `sandboxMode: "danger-full-access"` while keeping assignment execution
  bounded.
- `probeCodexAgentReadiness()` (`src/codex-agent.ts`): reports one of
  `ready`, `sdk_missing`, `credentials_missing`, `platform_unsupported`,
  or `disabled_by_config`. The probe checks **presence only** — it
  never reads, logs, or persists credential values.
- Capability declaration: `pylon provider go-online` declares
  `capability.pylon.local_codex` when and only when the probe reports
  `ready`, and strips a stale declaration when it does not. The
  capability ref is the only public signal that a local Codex exists;
  no machine details, paths, or account identifiers leave the device.
- Linked per-account Codex capacity is custody-backed: if a registry
  account carries `openAgentsProviderAccountRef`, Pylon can advertise the
  account from that link plus the local health/quota ledgers without requiring
  a refresh token embedded in the isolated `CODEX_HOME`.

## Configuration

Optional `codex` section in the Pylon config file
(`~/.pylon/config.json` by default):

```json
{
  "codex": {
    "enabled": true,
    "model": "gpt-5.4-codex",
    "maxTurns": 12,
    "timeoutSeconds": 2400,
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
  `danger-full-access` is never configurable through the assignment-safe
  `codex` section and the assignment executor never uses it.
- Never put credential values in the config file; the bridge reads
  credentials from the environment and the owner's own CLI login state
  only.
- `dev.accounts[].openAgentsProviderAccountRef` is an internal linkage ref
  written by `pylon auth codex` / `pylon accounts connect codex
  --openagents-link`. Keep it in the local config; never place raw provider
  auth values, bearer tokens, or `OPENCODE_AUTH_CONTENT` in the config file.

Optional local-only dev composer override:

```json
{
  "dev": {
    "codexExecutionMode": "local_supervised_danger"
  }
}
```

This is not a capability, not an assignment preference, and not public
execution evidence. It affects only the local dashboard composer (the same
mode is also available through `pylon dev --codex-danger` or
`pylon --codex-danger`), makes the TUI label the backend `Codex DANGER`, and
sets the SDK thread to `sandboxMode: "danger-full-access"` with
`approvalPolicy: "never"`. `pylon work`, `pylon assignment`, `pylon provider`,
`pylon node`, and `pylon attach` reject `--codex-danger`.

`pylon dev doctor --json` projects the same local execution mode alongside
repo, instruction, Codex, Claude/Fable, and backend readiness refs. It emits
digest refs and bounded states only; raw credential values, auth file paths,
instruction text, changed filenames, and local absolute paths are omitted.

`pylon dev check --json`, `pylon dev apply --json`, and
`pylon dev reload --json` provide the local supervised post-Codex loop.
`check` returns a typed dirty-state summary, changed file refs, command refs,
exit codes, and stdout/stderr digest refs; it requires `--allow-dirty` when
untracked files make attribution ambiguous. `apply` records the current patch
summary without committing or pushing. `reload` is an explicit action and is a
safe no-op until Pylon has a controlled restart/reattach process.

## Boundaries

- **Your identity, your credentials, your machine.** The lane acts only
  with the local owner's credentials; no platform keys on devices,
  ever.
- **Custody re-prime law.** Linked Codex registry accounts re-prime through
  OpenAgents custody before assignment execution and account-usage refresh.
  Pylon preserves the isolated `CODEX_HOME`, injects short-lived
  `OPENCODE_AUTH_CONTENT` into only the child process env, enforces a
  5-minute pre-expiry buffer, and rejects with
  `blocker.assignment.codex_agent_custody_unavailable` plus a
  `blocker.pylon.codex_custody.*` ref when custody cannot issue usable
  material.
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
- Caller-owned Codex coding delegation uses a 2400-second maximum assignment
  budget so the 30-minute sustained smoke has headroom beyond its live Codex
  session proof, dependency prep, and closeout submission.

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

## Adapter selection and the API path (CX5, #4792)

Two adapters, one deterministic rule set
(`workers/api/src/autopilot-work-adapter-selection.ts`):

1. **Requester intent wins.** An order naming an adapter (explicit
   request or an adapter capability ref in its required refs) gets
   that adapter, and placement refuses — never substitutes — when the
   Pylon cannot honor it.
2. **Owner preference is capability declaration.** Adapter-agnostic
   orders go to the placed Pylon's declared lanes; disabling a lane in
   `~/.pylon/config.json` strips its capability at go-online, which is
   how an owner expresses preference. A single-capability Pylon gets
   its one adapter.
3. **Dual-capability default is `claude_agent`** (documented, matches
   the executor chain order), with the selection reason ref carried in
   the assignment's `selectionPolicyRefs`.
4. **The closeout names the adapter.** Run, result, and summary refs
   are adapter-prefixed (`…pylon.codex_agent_task.…` vs
   `…pylon.claude_agent_task.…`), and each Pylon gate executes only
   its own work class.

The Pylon work-order CLI now exposes that requester intent directly:
`pylon work submit "<objective>" --commit <40-char-sha> --adapter
codex|claude_agent|fable`. The command preflights the public GitHub commit
before posting the work order, and `fable` is encoded as the Claude Agent lane
with `profile.claude_agent.fable` until a dedicated Fable adapter exists.

The API path is B2's (#4756) exactly: `POST /api/autopilot/work` with
a `git_checkout` task → own-Pylon placement → the synthesizer picks
the work class per the policy above → durable `codex_agent_task`
assignment whose `codingAssignment.codex` payload rides the **same**
`workspace` contract (shared validator and checkout runner owned by
the adapter-neutral `workspace-materializer` module since #4798 —
never forked) → own-Pylon pickup →
`executeCodexAgentAssignment` → independent verification → ref-only
closeout → delivered → review.

## Current status

CX1 (#4788) ships the probe, capability declaration, credential-policy
review, and config surface; CX2 (#4789) ships the bounded executor
gate in the worker loop; CX3 (#4790) ships the `codex_agent_task` work
class, operator dispatch, and the CI-safe + live smoke harness.

**The live-device leg (CX4 #4791) has run.** On 2026-06-11 a
contributor machine with the owner's own Codex CLI login
(`credential.source.codex_agent.codex_cli_login`) went online with
`capability.pylon.local_codex`, a production `codex_agent_task`
assignment (`assignment.codex_agent_task.1781191118187`,
`unpaid_smoke`) was dispatched through the controlled gate, the real
Codex SDK executed the bounded fixture task in a sandboxed workspace,
the independent verification command passed on-device, and the
closeout reached the deployed API as `accepted`
(`assignment.closeout.f264043a9f173b20514521da`) with the redaction
scan clean and the no-spend boundary intact. A green transition for
`autopilot.codex_probe_pylon_successor.v1` was proposed receipt-first;
the maintainer flips the registry.

**The API-parity leg (CX5 #4792) has also run live.** On 2026-06-11 an
API-submitted work order
(`autopilot_work_order.c63284d5-e24a-4f4a-aeab-4be45ffd8d72`,
free-slice, registered-agent auth) was placed on the requester's own
codex-only Pylon, synthesized as `codex_agent_task` by the adapter
selection policy (`adapter_selection.single_capability`), executed by
the real Codex SDK against the public fixture repo
`AtlantisPleb/openagents-b2-git-checkout-fixture-20260611144040` at
pinned commit `1745cd4b54b8a12a50922f80b5d345314c91d70d` via the
shared `git_checkout` workspace contract, verified by an independent
`bun test`, and closed out as accepted
(`assignment.closeout.b6d31228033e1009fe773326`,
`result.public.pylon.codex_agent_task.git_checkout_verified_passed`)
with the no-spend boundary and redaction scan intact. The full CX
epic (#4793) is complete.

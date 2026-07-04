# Khala Code AI SDK Harness Upgrade Audit

Date: 2026-07-04
Status: audit / upgrade recommendation. No code changed by this doc.
Scope: Khala Code Desktop's Codex and Claude Code paths, Pylon's local Codex
and Claude assignment runners, and the new AI SDK harness abstraction.

## Executive Decision

AI SDK Harnesses are a credible technology to spike, but they are not a viable
drop-in replacement for Khala Code's current Codex or Claude Code integration
yet.

The viable path is an isolated pilot:

- Use AI SDK Harnesses for a sandboxed/cloud or experimental desktop mode where
  AI SDK stream compatibility is the goal.
- Keep Codex app-server as the default Khala Code Desktop harness authority.
- Keep Pylon's local Codex and Claude assignment runners as the fleet authority
  until a harness adapter proves the exact same account isolation, token
  accounting, workspace boundary, credential scanning, raw-private-event, and
  closeout behavior.

The main reason is that AI SDK Harnesses normalize agent turns into AI SDK
`generate` / `stream` results. Khala Code currently depends on deeper runtime
contracts than a turn stream: Codex app-server thread APIs, settings and MCP
methods, local account homes, exact usage ingestion, owner-only raw event
archives, typed account health, PR publication, and assignment closeout.

## External Evidence

Official docs reviewed:

- AI SDK Harness overview:
  <https://ai-sdk.dev/docs/ai-sdk-harnesses/overview.md>
- `HarnessAgent` API:
  <https://ai-sdk.dev/docs/ai-sdk-harnesses/harness-agent.md>
- Harness adapters:
  <https://ai-sdk.dev/docs/ai-sdk-harnesses/harness-adapters.md>
- Codex adapter:
  <https://ai-sdk.dev/providers/ai-sdk-harnesses/codex.md>
- Claude Code adapter:
  <https://ai-sdk.dev/providers/ai-sdk-harnesses/claude-code.md>
- Harness tools:
  <https://ai-sdk.dev/docs/ai-sdk-harnesses/tools.md>
- Harness UI:
  <https://ai-sdk.dev/docs/ai-sdk-harnesses/ui.md>
- Workflow utilities:
  <https://ai-sdk.dev/docs/ai-sdk-harnesses/workflow-utilities.md>

Package metadata checked on 2026-07-04:

| Package | Latest checked | Relevance |
| --- | ---: | --- |
| `ai` | `7.0.15` | AI SDK stream/UI ecosystem. |
| `@ai-sdk/harness` | `1.0.18` | `HarnessAgent` core. |
| `@ai-sdk/harness-codex` | `1.0.19` | Codex bridge adapter. |
| `@ai-sdk/harness-claude-code` | `1.0.18` | Claude Code bridge adapter. |
| `@ai-sdk/sandbox-vercel` | `1.0.18` | Documented sandbox provider for bridge-backed harnesses. |
| `@openai/codex-sdk` | `0.142.5` | Current upstream package behind the Codex harness adapter. |
| `@anthropic-ai/claude-agent-sdk` | `0.3.201` | Current upstream package behind the Claude Code harness adapter. |

The repo currently depends on `@openai/codex-sdk ^0.139.0` in Pylon and
`@anthropic-ai/claude-agent-sdk ^0.3.172` in Pylon and Khala Code Desktop.
No AI SDK harness package is currently present in `package.json` or `bun.lock`.

## Current Khala Code Shape

### Desktop Codex

Current default mode is `codex_harness`, but "harness" here means the
OpenAgents Codex app-server wrapper, not AI SDK Harnesses.

Key files:

- `clients/khala-code-desktop/src/bun/codex-app-server-client.ts`
- `clients/khala-code-desktop/src/bun/codex-app-server-service.ts`
- `clients/khala-code-desktop/src/bun/codex-app-server-chat-runtime.ts`
- `docs/khala-code/2026-07-01-codex-app-server-gap-matrix.md`
- `docs/khala-code/2026-07-01-codex-parity-contract.md`

The desktop process supervises `codex app-server --stdio`, performs
`initialize` / `initialized`, and calls Codex JSON-RPC methods such as
`thread/start`, `thread/resume`, `thread/read`, `thread/list`, `turn/start`,
`turn/interrupt`, and `turn/steer`. The wrapper also subscribes to app-server
notifications, projects thread items into Khala transcript messages, persists a
small desktop session map, captures token usage updates, exposes app-server
status over RPC, and keeps a parity matrix for slash commands and settings.

This is deeper than AI SDK's `HarnessAgent.stream()` contract. The AI SDK Codex
adapter is attractive for stream compatibility, but it does not replace the
Codex app-server surface Khala uses for thread management, settings, MCP server
status, slash-command parity, background terminal controls, and exact desktop
projectors.

### Desktop Claude

Key files:

- `clients/khala-code-desktop/src/bun/claude-app-sdk-chat-runtime.ts`
- `clients/khala-code-desktop/src/bun/claude-harness-status.ts`
- `clients/khala-code-desktop/src/bun/claude-approvals.ts`
- `clients/khala-code-desktop/src/bun/claude-session-store.ts`

Desktop Claude already uses `@anthropic-ai/claude-agent-sdk` directly. It has a
session store, approval service, slash-command projection, settings projection,
thread item projector, interruption path, and token usage reporter. The AI SDK
Claude Code adapter wraps the same underlying SDK, so it is a better candidate
for an isolated experiment than Codex app-server replacement.

The risk is that the current runtime depends on local session detection,
`CLAUDE_CONFIG_DIR` / OAuth-token account routing, approval behavior, and custom
projectors. The AI SDK adapter must prove it can carry those same semantics
before becoming a default.

### Pylon Fleet Runners

Key files:

- `apps/pylon/src/agent-runner-registry.ts`
- `apps/pylon/src/agent-harness-adapter.ts`
- `apps/pylon/src/codex-agent.ts`
- `apps/pylon/src/codex-agent-executor.ts`
- `apps/pylon/src/claude-agent.ts`
- `apps/pylon/src/claude-agent-executor.ts`
- `apps/pylon/src/account-registry.ts`
- `apps/pylon/src/workspace-materializer.ts`

Pylon already has a harness-neutral internal registry, but it is OpenAgents'
runtime contract, not AI SDK Harnesses. It maps Codex and Claude assignments to
local execution policies, readiness probes, turn reporters, and workspace
boundaries.

Codex assignment behavior is intentionally local and owner-capacity specific:

- Uses `@openai/codex-sdk` directly.
- Runs with owner-local full access: `danger-full-access`, approval policy
  `never`, and network enabled.
- Uses per-account environments from the Pylon account registry.
- Streams raw Codex SDK events into private owner-scoped event chunk and
  whole-turn archives.
- Posts exact usage to `/api/pylon/codex/turns`.
- Performs post-hoc file-change boundary validation.
- Runs SCM credential scanning and PR publication after verification.
- Records typed quota, auth, and account health failures.

Claude assignment behavior differs on purpose:

- Uses `@anthropic-ai/claude-agent-sdk` directly.
- Uses `allowedTools` and `PreToolUse` hooks to deny workspace escapes before
  tool execution.
- Reads cumulative exact SDK usage from result messages and posts it to
  `/api/pylon/claude/turns`.
- Enforces the same materialized workspace, cleanup, and credential scanning
  contracts as Codex.

An AI SDK harness runner would have to reproduce all of that. A mere
`HarnessAgent.stream()` wrapper is insufficient.

## Fit Against AI SDK Harnesses

Strong fit:

- AI SDK stream compatibility. Harness output can become AI SDK UI message
  streams, which is useful for web chat, future shared stream rendering, and
  possibly a cleaner desktop transcript bridge.
- Session lifecycle primitives. `createSession`, `detach`, `stop`, and
  continuation APIs match Khala's need to resume long-running turns, especially
  for hosted or workflow-backed routes.
- Shared harness abstraction. Codex, Claude Code, OpenCode, and other adapters
  can be presented through one application-facing API.
- Sandbox bootstrap hooks. `sandboxConfig.onBootstrap` and `onSession` are a
  reasonable place to install `rg`, seed files, or write session metadata in an
  isolated sandbox.
- Host-executed tools. AI SDK tools can be supplied to `HarnessAgent` and
  executed by the host, while built-in runtime tools remain provider-executed.

Poor fit or not proven:

- Default local-first desktop. Bridge-backed Codex and Claude Code adapters
  require a network sandbox with an exposed port, with Vercel Sandbox shown as
  the documented provider. That introduces `VERCEL_OIDC_TOKEN` and external
  sandbox lifecycle requirements that are not part of Khala Code Desktop's
  local-first default contract.
- Codex app-server parity. AI SDK Codex streams turns; it does not expose the
  app-server methods Khala uses for thread catalogs, thread mutations, settings,
  MCP, slash-command parity, and background terminal controls.
- Codex built-in permissions. The AI SDK Codex adapter currently documents no
  built-in approval requests and no built-in tool filtering. That is acceptable
  only for the owner-local full-access Pylon lane, not for any untrusted or
  policy-restricted lane.
- Raw event fidelity. Pylon currently stores ordered raw Codex SDK events and
  event chunks privately. AI SDK stream parts may be a projection; we need a
  proof that no required raw event, usage field, file-change detail, or
  failure classifier is lost.
- Per-account local homes. Current Pylon isolation depends on account-specific
  `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, and OAuth token environments. The AI SDK
  adapters forward selected host auth into a sandbox; they do not yet prove the
  same multi-account local home model.
- Credential and SCM scanners. Current closeout scans the bounded workspace and
  selected isolated account home for long-lived SCM credentials. A sandboxed
  harness runner must still expose the right filesystem roots to scan without
  leaking secrets into public artifacts.
- Experimental API risk. The harness docs explicitly mark packages as
  experimental, and the packages are at early `1.0.x` harness versions while
  the underlying Codex and Claude SDKs are moving.

## Recommended Upgrade Plan

Do not replace the default Codex app-server path. Add a quarantine pilot.

1. Create an optional `ai_sdk_harness_experiment` runtime mode, disabled by
   default and excluded from product-promise green paths.
2. Implement one tiny adapter boundary that can run the public sum-repair
   fixture through `HarnessAgent` with Codex and Claude Code in a Vercel
   sandbox.
3. Treat all AI SDK harness events as untrusted adapter input. Decode them
   through Effect Schema before projecting into Khala transcript, Pylon usage,
   or closeout records.
4. Preserve current Pylon runners as the production fleet path until the pilot
   passes the parity gates below.
5. If the pilot succeeds, consider two narrow uses:
   - a hosted/sandboxed Khala Code session mode for web or cloud workrooms;
   - a shared UI stream adapter that can render AI SDK harness parts alongside
     current Codex app-server and Claude SDK projections.

## Required Gates Before Promotion

The pilot must pass all of these before it can replace any default path.

| Gate | Required proof |
| --- | --- |
| Codex desktop parity | Does not regress the app-server gap matrix, slash-command parity, thread catalog, settings, MCP status, background terminal, interrupt, or token usage tests. |
| Claude desktop parity | Preserves local session readiness, `CLAUDE_CONFIG_DIR` / OAuth-account routing, approval behavior, interruption, settings, slash-command projection, and token usage reporting. |
| Pylon account isolation | Runs one Codex and one Claude assignment from isolated account refs without reading or mutating default `~/.codex` or user `~/.claude` state. |
| Exact usage | Produces the same exact downstream usage rows expected by `/api/pylon/codex/turns` and `/api/pylon/claude/turns`; no estimated usage may move public counters. |
| Raw event archive | Preserves private raw event chunks and final ordered archives, or documents an explicit model-boundary exception and keeps the old runner for any flow needing replay. |
| Workspace boundary | Codex file changes outside the materialized workspace still become `workspace_escape_blocked`; Claude tool escapes are denied before tool execution or fail an equivalent pre-execution gate. |
| Credential scanning | Scans the bounded workspace and any selected isolated account home before verification or PR publication, with the same typed refusal behavior. |
| PR publishing | Verified Codex assignment diffs can still be committed, pushed, and opened as PRs through the existing publisher path. |
| Failure taxonomy | Quota, rate limit, revoked credentials, auth failures, timeout, and network failures update the same typed health/quota ledgers. |
| Local-first claim safety | No public Khala Code copy claims AI SDK harness support as default while the mode depends on Vercel Sandbox or experimental APIs. |

## Bottom Line

This is a promising compatibility layer, not a default-platform upgrade yet.

Best near-term use: a narrow sandboxed AI SDK harness pilot that helps Khala
render and test harness streams through standard AI SDK UI shapes.

Worst near-term use: replacing Codex app-server or Pylon's local assignment
runners before the AI SDK adapters prove the exact OpenAgents authority,
accounting, isolation, and closeout contracts.

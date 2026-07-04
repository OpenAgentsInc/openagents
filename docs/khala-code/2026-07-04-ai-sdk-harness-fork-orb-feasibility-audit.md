# AI SDK Harness Fork + Orb Feasibility Audit

Date: 2026-07-04
Status: second-pass audit. No runtime code changed by this document.
Scope: actual published AI SDK harness package code, local Pylon Codex/Claude
runners, OpenAgents sandbox/Orb plans, and whether a maintained fork or local
prototype is feasible.

## Executive Answer

Yes, we can get AI SDK Harnesses working locally with modifications, and yes,
the right long-term integration is to make an OpenAgents Orb/workroom implement
the AI SDK sandbox-provider contract.

No, we should not maintain a broad fork of AI SDK core. The viable ownership
shape is narrower:

- Keep upstream `ai` and `@ai-sdk/harness` as dependencies whenever possible.
- Build an OpenAgents sandbox provider that implements
  `HarnessV1SandboxProvider`.
- Maintain a shallow fork or wrapper of the Codex and Claude Code adapters only
  where OpenAgents needs account-home routing, raw private event capture,
  package version control, typed failure mapping, or stronger policy seams.
- Upstream every generic extension point we need so the fork can shrink.

The decisive code-level finding is that AI SDK already separates the agent
runtime from the sandbox provider. `@ai-sdk/harness` does not hard-code Vercel.
It asks for a provider that can create/resume a network sandbox session with
file I/O, `run`, `spawn`, an exposed WebSocket port, and lifecycle methods. That
is almost exactly the public-safe boundary our OpenAgents sandbox docs call
`openagents.sandbox.v1`, and almost exactly the product shape our Amp Orbs
adaptation doc maps to Khala Code threads and workrooms.

The caution is Codex. The published Codex adapter runs the Codex SDK inside the
sandbox with `sandboxMode: "danger-full-access"` and `approvalPolicy: "never"`.
It explicitly rejects built-in tool filtering and permission modes other than
`allow-all`. That is acceptable only if the sandbox/Orb is the actual
containment authority. It is not acceptable as a policy boundary by itself.

## Published Code Reviewed

Packages fetched from npm and unpacked on 2026-07-04:

| Package | Version | Source shape reviewed |
| --- | ---: | --- |
| `@ai-sdk/harness` | `1.0.18` | 76 package files, about 9.2k TypeScript source lines. |
| `@ai-sdk/harness-codex` | `1.0.19` | 23 package files, about 2.6k TypeScript source lines. |
| `@ai-sdk/harness-claude-code` | `1.0.18` | 20 package files, about 2.8k TypeScript source lines. |
| `@ai-sdk/sandbox-vercel` | `1.0.18` | 11 package files, about 724 TypeScript source lines. |

Current npm latest checks on 2026-07-04:

| Package | Latest checked | Notable drift |
| --- | ---: | --- |
| `@openai/codex-sdk` | `0.142.5` | AI SDK Codex bridge package pins `0.130.0`; OpenAgents Pylon pins `^0.139.0`. |
| `@anthropic-ai/claude-agent-sdk` | `0.3.201` | AI SDK Claude bridge package pins `0.3.177`; OpenAgents pins `^0.3.172`. |
| `@anthropic-ai/claude-code` | `2.1.201` | AI SDK Claude bridge package pins `2.1.177`. |

The important package files are:

- `@ai-sdk/harness/src/v1/harness-v1-sandbox-provider.ts`
- `@ai-sdk/harness/src/v1/harness-v1-network-sandbox-session.ts`
- `@ai-sdk/harness/src/agent/prepare-sandbox-for-harness.ts`
- `@ai-sdk/harness/src/agent/internal/bootstrap-recipe.ts`
- `@ai-sdk/sandbox-vercel/src/vercel-sandbox.ts`
- `@ai-sdk/sandbox-vercel/src/vercel-sandbox-session.ts`
- `@ai-sdk/sandbox-vercel/src/vercel-network-sandbox-session.ts`
- `@ai-sdk/harness-codex/src/codex-harness.ts`
- `@ai-sdk/harness-codex/src/bridge/index.ts`
- `@ai-sdk/harness-codex/src/bridge/package.json`
- `@ai-sdk/harness-claude-code/src/claude-code-harness.ts`
- `@ai-sdk/harness-claude-code/src/bridge/index.ts`
- `@ai-sdk/harness-claude-code/src/bridge/package.json`

## What The AI SDK Code Actually Requires

`HarnessV1SandboxProvider` is small and direct:

- `specificationVersion: "harness-sandbox-v1"`
- `providerId`
- optional `bridgePorts`
- `createSession({ sessionId, identity, onFirstCreate, abortSignal })`
- optional `resumeSession({ sessionId, abortSignal })`

The returned `HarnessV1NetworkSandboxSession` must provide:

- `id`
- `defaultWorkingDirectory`
- `ports`
- `getPortUrl({ port, protocol })`
- `run`
- `spawn`
- file read/write methods from `Experimental_SandboxSession`
- `restricted()`
- `stop()`
- optional `destroy()`, `setNetworkPolicy()`, and `setPorts()`

This is good news. An OpenAgents Orb provider does not need to pretend to be a
Vercel sandbox. It only needs to satisfy the contract above.

`@ai-sdk/sandbox-vercel` proves the provider layer is thin. It wraps
`@vercel/sandbox`, forwards `runCommand`, `readFileToBuffer`, `writeFiles`,
port domains, network policy updates, and sandbox `stop/delete`. Its template
path uses `identity` plus `onFirstCreate` to build a persistent prepared
snapshot and then forks per-session sandboxes from it. That maps naturally to
our `.agents/setup` plus post-setup snapshot plan.

The AI SDK bootstrap model also helps us: adapters provide a bootstrap recipe
with files and commands; the framework hashes the recipe and writes an
idempotent marker under the sandbox. For OpenAgents, that hash can become part
of the Orb snapshot key alongside repo ref, `.agents/setup`, lockfiles, base
image, and toolchain version.

## Codex Adapter Findings

The published Codex adapter is bridge-backed:

- `createCodex().getBootstrap()` writes bridge files into
  `/tmp/harness/codex` and runs `pnpm install --frozen-lockfile` inside the
  sandbox.
- `doStart()` requires a sandbox with an exposed port, spawns
  `node /tmp/harness/codex/bridge.mjs`, waits for a bridge-ready record, then
  opens a WebSocket through `sandboxSession.getPortUrl(..., protocol: "ws")`.
- Resume is built around bridge coordinates, event-log replay, and Codex
  `threadId`.
- It forwards auth through env (`CODEX_API_KEY`, `OPENAI_BASE_URL`,
  `OPENAI_ORGANIZATION`, `OPENAI_PROJECT`, AI Gateway vars, etc.).

The hard limits:

- `supportsBuiltinToolApprovals: false`.
- Built-in tool filtering throws immediately.
- Any `permissionMode` other than `allow-all` throws immediately.
- The bridge starts Codex with `sandboxMode: "danger-full-access"` and
  `approvalPolicy: "never"`.
- Host tools are not clean MCP yet. The bridge documents an upstream Codex
  issue where MCP tools do not surface in the Codex SDK path, so it adds a
  CLI-relay workaround that the model invokes through its bash tool.
- It emits normalized tool/text/reasoning/file-change/usage parts, but not the
  full raw Codex SDK event stream. The shared `raw` stream part exists in
  `@ai-sdk/harness`, but this adapter does not currently use it.
- The bridge dependency lock pins `@openai/codex-sdk` to `0.130.0`, behind both
  current npm latest and our existing Pylon dependency range.

This means upstream Codex can be used behind an Orb, but cannot by itself
replace Pylon's current authority model. Our current Pylon Codex executor also
uses owner-local full access and approval policy `never`, but it additionally
has OpenAgents-specific account homes, raw event chunk archives, exact usage
ingest, quota/auth health ledgers, post-hoc workspace escape blocking, SCM
credential scans, and closeout semantics. The AI SDK adapter would need a fork
or wrapper to preserve those behaviors.

One local doc/code drift matters: `apps/pylon/docs/codex-bridge.md` still says
the bounded assignment path disables network access, while
`apps/pylon/src/codex-agent-executor.ts` now passes `networkAccessEnabled: true`
for the live runner and documents the owner-local danger posture in code. The
audit conclusion is therefore stronger: network policy must live below the
adapter in the Orb/workroom profile, not in stale docs or model-facing settings.

## Claude Code Adapter Findings

The Claude adapter is a better fit for early experimentation:

- `supportsBuiltinToolApprovals: true`.
- `supportsBuiltinToolFiltering: true`.
- The bridge uses the Claude Agent SDK's `canUseTool` hook to ask/allow/deny
  based on AI SDK `permissionMode`.
- `allow-all` maps to Claude bypass permissions; `allow-edits` maps to accept
  edits; read/edit/bash distinctions are enforced through generated permission
  rules.
- Host tools are exposed through an SDK MCP server rather than the Codex CLI
  relay workaround.
- Compaction is observed through Claude compact-boundary messages and a
  `PostCompact` hook.
- Usage and `total_cost_usd` are projected into harness metadata.

The remaining gaps are still real:

- It also installs a bridge package inside the sandbox and pins older Claude
  Agent SDK / Claude Code package versions.
- It forwards selected auth env and can read the host `~/.claude/settings.json`
  `apiKeyHelper` in auto-detect mode; OpenAgents would need explicit account
  home and credential-source routing instead of ambient host defaults.
- It normalizes events rather than preserving a full private raw SDK archive.
- Our Pylon Claude runner already has a stricter workspace-specific
  `PreToolUse` guard and OpenAgents turn reporting. The AI SDK path must prove
  parity before replacing that runner.

## Can We Maintain A Fork?

Yes, if the fork is scoped. No, if the fork means owning all of AI SDK.

Viable fork boundary:

1. `@openagentsinc/ai-sdk-sandbox-local` or similar local provider for tests
   and owner-local desktop experiments.
2. `@openagentsinc/ai-sdk-sandbox-orb` implementing `HarnessV1SandboxProvider`
   over OpenAgents workrooms/Orbs.
3. A shallow `@openagentsinc/harness-codex` fork only for:
   - Codex SDK version control.
   - account-home routing (`CODEX_HOME`, short-lived auth material, no default
     global home).
   - private raw event side-channel or `raw` stream parts.
   - OpenAgents failure taxonomy.
   - removal or hardening of the prompt-mediated CLI tool relay where possible.
4. A shallow `@openagentsinc/harness-claude-code` fork only for:
   - `CLAUDE_CONFIG_DIR` / per-account OAuth token routing.
   - exact usage and cost metadata preservation.
   - private raw event capture if needed.
   - OpenAgents-specific permission presets.

Avoid forking:

- `ai` core.
- `@ai-sdk/harness` core unless we hit a missing extension point that cannot be
  wrapped. If that happens, open an upstream issue/PR first and carry the
  smallest possible patch.

Fork maintenance is feasible because the adapter packages are small: the Codex
adapter is about 2.6k source lines and the Claude adapter is about 2.8k. The
danger is not size; it is churn in Codex/Claude runtime packages and bridge
lockfiles. Any fork needs automated drift checks against npm tarballs and a
fixture matrix that fails when upstream package updates change event shapes.

## Can We Get It Working Locally?

Yes, in two stages.

### Stage 1: unsafe local provider for proof only

Build a local provider that implements `HarnessV1SandboxProvider` by creating a
temp workspace, starting bridge processes as host child processes, exposing a
fixed localhost port, and implementing `run`, `spawn`, read/write, stop, and
destroy. On macOS it can optionally wrap commands with the existing
`packages/khala-tools/src/process-sandbox-macos.ts` Seatbelt service.

This is enough to prove:

- `HarnessAgent` can run Codex and Claude without Vercel.
- Khala Code can consume AI SDK stream parts.
- We can run a public sum-repair fixture and verify file changes.
- We understand the package bootstrap and port behavior.

This stage must be owner-local only. macOS Seatbelt writes-limited-to-workspace
is useful, but it still is not the production security boundary for Codex full
access, network egress, package installs, or hostile repositories.

### Stage 2: real Orb provider

Build an Orb/workroom provider over the OpenAgents sandbox plan:

| AI SDK requirement | OpenAgents mapping |
| --- | --- |
| `createSession({ sessionId, identity, onFirstCreate })` | Create or resume a workroom/Orb, keyed by thread/session id and snapshot identity. |
| `identity` | Hash of AI SDK bootstrap recipe plus `.agents/setup`, lockfiles, base image, and toolchain version. |
| `onFirstCreate` | Run AI SDK bridge bootstrap and repo setup before snapshot. |
| `run` / `spawn` | Workroom exec/session API over `oa-workroomd` or `openagents.sandbox.v1`. |
| file read/write | Workroom filesystem API, artifact adapter, or bounded rsync channel. |
| `ports` / `getPortUrl` | Managed preview ingress with hashed endpoint tokens. |
| `setNetworkPolicy` | Sandbox profile plus capability gateways. |
| `restricted()` | Tool-safe view with no lifecycle, port, or network-policy authority. |
| `stop()` | Pause/stop workroom and stop metering. |
| `destroy()` | Closeout/archive/destroy with artifact and receipt checks. |
| `resumeSession()` | Reattach to the same workroom by session id. |

This is a strong fit with the Orbs adaptation doc:

- thread -> workroom binding,
- `.agents/setup` / `.agents/resume`,
- post-setup snapshots with TTL,
- pause/resume economics,
- `.openagents/dev-ports.json`,
- PTY/preview gateway,
- lane-transparent events into Khala Code.

It also fits the sandbox-platform audit's public contract plan:
`openagents.sandbox.v1`, tiered isolation, Firecracker for untrusted/heavy,
Cloudflare Containers for light/web, content-addressed artifacts, capability
gateways, metadata endpoint, and metered receipts.

## How It Should Tie Into OpenAgents Orbs

The AI SDK sandbox provider should become one consumer of the Orb platform, not
the Orb platform itself.

Recommended layering:

1. `openagents.sandbox.v1` / Orb API owns lifecycle, isolation, snapshots,
   ports, filesystem, receipts, and metering.
2. `@openagentsinc/ai-sdk-sandbox-orb` adapts that API to
   `HarnessV1SandboxProvider`.
3. AI SDK Codex/Claude adapters run their bridges inside the Orb.
4. Khala Code consumes normalized stream parts for UI, but OpenAgents sidecars
   still own raw private event archives, exact usage ledgers, account health,
   SCM credential scans, and closeout receipts.

This lets us use AI SDK where it is strongest, as a compatibility stream and
runtime bridge, while keeping OpenAgents authority in the pieces we already
care about.

Do not invert that layering. If AI SDK becomes the authority for sandbox
lifecycle, secrets, or closeout, we lose the exact invariants the OpenAgents
sandbox plan exists to enforce.

## Recommended Pathway

### P0: mirror and test the published package code

Add a small internal mirror or patch workspace that can import the exact npm
tarballs, run type checks, and diff the source files we patch. This is not a
product package yet; it is fork hygiene.

Required tests:

- package version drift check,
- bridge dependency lockfile drift check,
- Codex/Claude event-shape fixture check,
- adapter bootstrap hash check.

### P1: local AI SDK sandbox provider spike

Implement a local provider sufficient for a public sum-repair fixture. It can
use a temp directory, local child processes, a reserved localhost WebSocket
port, and optional macOS Seatbelt.

Gates:

- Codex and Claude `HarnessAgent.stream()` both run without Vercel.
- No read/write outside the temp workspace in the fixture.
- Stream parts render through the Khala Code transcript adapter.
- The proof is clearly labeled owner-local and unsafe for untrusted work.

### P2: Claude first, Codex second

Claude should be the first real adapter experiment because its AI SDK adapter
already supports built-in approvals and filtering. Codex should wait until the
Orb provider can enforce the sandbox profile underneath `danger-full-access`.

Gates:

- Claude account-home routing through explicit env, not ambient host defaults.
- Claude tool approval/filtering parity with current Pylon bounded runner.
- Exact usage parity with current `/api/pylon/claude/turns` expectations.

### P3: Orb provider over the sandbox runtime

Build `@openagentsinc/ai-sdk-sandbox-orb` against the real or fixture-backed
`openagents.sandbox.v1` surface.

Gates:

- create/resume/stop/destroy contract tests,
- WebSocket bridge port through managed preview ingress,
- `.agents/setup` snapshot key proves cache hit/miss behavior,
- network policy and capability-gateway tests,
- public-safe receipt refs only.

### P4: Codex fork with raw/private event side-channel

Fork only what we need in the Codex adapter.

Required deltas:

- update or parameterize `@openai/codex-sdk`,
- expose raw Codex SDK events privately, either as `raw` stream parts gated by
  settings or as an OpenAgents side-channel,
- preserve exact token and failure metadata,
- route account auth through explicit `CODEX_HOME`/short-lived material,
- remove or constrain prompt-mediated host-tool CLI relay in cloud lanes,
- map Codex failures to existing account health/quota ledgers.

Gate:

- run the same public git fixture with current Pylon runner and AI SDK Orb
  runner, compare usage rows, file-change projection, closeout refs, and typed
  failure behavior.

### P5: upstream extension points

Open upstream issues/PRs for generic needs:

- adapter raw event passthrough,
- bridge package dependency override or fresher pin cadence,
- explicit auth home/env routing hooks,
- Codex built-in tool policy once the Codex SDK supports it,
- documented non-Vercel sandbox provider examples.

The goal is to delete fork patches, not grow them.

## Promotion Gates

Do not promote AI SDK Harnesses beyond experiment until all of these pass:

| Gate | Required proof |
| --- | --- |
| Local non-Vercel proof | Codex and Claude run through `HarnessAgent` in a local provider without Vercel. |
| Orb proof | Same fixture runs inside an OpenAgents Orb/workroom with managed port ingress. |
| Account isolation | `CODEX_HOME` and `CLAUDE_CONFIG_DIR` are selected account homes, never default ambient homes. |
| Raw private archive | Pylon/Khala can retain ordered raw runtime events privately, or the old runner remains the authority for flows needing them. |
| Exact usage | Usage rows match current Codex/Claude ingestion semantics; estimates never move public counters. |
| Workspace boundary | Codex is contained by Orb policy plus post-hoc file-change validation; Claude retains pre-tool denial or equivalent. |
| Network policy | Public/untrusted lanes cannot rely on adapter settings; egress is enforced by Orb/workroom profile. |
| SCM credential scan | Workspace and selected account homes are scanned before verification/PR publication. |
| Resume | Detach, suspend, stop, destroy, and resume behave across host process restarts. |
| Product honesty | No public claim that AI SDK harnesses are default until the gates above are receipt-backed. |

## Bottom Line

The feasible upgrade is not "replace Khala Code with AI SDK Harnesses." The
feasible upgrade is "make OpenAgents Orbs a first-class AI SDK sandbox provider,
then run Codex/Claude harness bridges inside that provider where stream
compatibility helps."

Maintain a fork only at the adapter edge, and only for OpenAgents authority
needs. Use upstream AI SDK core as long as it can stay unmodified. Get a local
unsafe prototype working first, move Claude through the Orb path next, and let
Codex become default only after the Orb is the real containment boundary.

# First-Party Probe Runtime Audit

Date: 2026-06-07
Status: zero-tech-debt architecture audit and implementation plan
Scope: first-party Probe runtime, OpenAgents product surface sync, SHC deployment, optional Pylon installation, and deprecated Probe source-material handling

## Direction Change

The previous recommendation to wrap, fork, or vendor OpenCode as the Probe
runtime is rejected. The target is a first-party OpenAgents Probe runtime built
from scratch.

OpenCode remains useful as reference material. It should inform architecture,
test cases, failure modes, packaging, and user expectations. It should not be
the runtime substrate, should not be wrapped behind a Probe facade, and should
not be copied as the main implementation.

The corrected direction is:

- Probe is the first-party OpenAgents coding-agent runtime.
- Probe keeps its own protocol, session, tool, approval, event, and deployment
  model.
- Bun, Effect, SQLite, or any other implementation choice is internal plumbing,
  not a product or protocol name.
- OpenAgents product surface and Probe share schemas deliberately instead of drifting through ad hoc
  JSON payloads.
- SHC boxes are a first-class deployment target, not an afterthought.
- Pylon can download and run Probe when explicitly configured, but Pylon's main
  build does not include Probe and does not auto-run downloaded Probe code by
  default.
- OpenCode is study-only: patterns can be learned, but implementation authority
  stays with OpenAgents.

## Executive Summary

The intended end state is simple: OpenAgents product surface dispatches coding work to `probe` on a
runner backend such as `shc_vm`, and Probe emits typed, redacted events back to
OpenAgents product surface and OpenAgents Sync. Probe is the product/runtime name; it is not named
after its implementation stack, migration path, or reference repos.

The existing Rust Probe repo is deprecated as an implementation. It remains
source material for behavior, fixtures, protocol lessons, command ergonomics,
failure cases, and tests that should be replicated in the new runtime. It is
not the authority for preserving every mode, command, or compatibility surface.

The new Probe runtime should start with a smaller, clearer core:

- durable sessions,
- typed event log,
- prompt admission and continuation,
- model/provider routing,
- tool execution,
- approval state,
- managed runtime control,
- OpenAgents product surface event export,
- SHC runner deployment,
- Pylon optional runtime support.

The goal is not blanket parity with the deprecated Rust runtime. The goal is the
final OpenAgents coding-agent surface: a runtime that can be deployed to SHC
boxes, selected by OpenAgents product surface through `RunnerRuntime`, mirrored through OpenAgents
Sync, and operated as the default coding-agent backend for OpenAgents workrooms
without binding the product to OpenCode, Codex, or Rust Probe internals.

The immediate OpenAgents product surface implication is concrete: `@openagentsinc/sync-schema` currently
defines `RunnerRuntime` as `opencode_codex | codex`, and OpenAgents product surface defaults
`DEFAULT_AGENT_RUNTIME` to `opencode_codex`. The from-scratch Probe path should
add a first-party runtime discriminator named `probe`, then teach OpenAgents product surface's SHC
dispatch, event ingestion, run projection, and tests to treat Probe as a
runtime running on backends such as `shc_vm` and `gcloud_vm`.

## Zero-Tech-Debt Review

Intended end state:

- Product and protocol names say `probe`, not an implementation-stack,
  migration-path, reference-repo, or deprecated-runtime name.
- Deprecated Probe and OpenCode are source material. New Probe owns its code,
  contracts, tests, releases, and runtime identity.

Real caller review:

- OpenAgents product surface currently calls the SHC control path with `opencode_codex`/`codex`
  runtime names. Those are real current callers, so the migration plan should
  add `probe` beside them first and delete the old runtime names only after
  dispatch traffic moves.
- Pylon currently invokes
  `probe admin-chat-bridge signed --request <path> --secret-env <env> --cwd <workspace> --format json`.
  That one bridge is a real caller and should be preserved as a narrow
  migration shim with a deletion condition.
- No current caller exists for an implementation-specific Probe runtime name.
  Do not introduce one.
- No current caller justifies preserving the full deprecated Rust CLI, daemon,
  TUI, hosted-runner, optimizer, or Forge command surface as a parity mandate.
  Harvest the useful behavior and tests, then implement only the intended Probe
  product/runtime surface.

## Non-Goals

This plan does not:

- introduce any runtime name based on implementation technology,
- wrap OpenCode,
- fork OpenCode as the Probe codebase,
- vendor OpenCode modules as Probe internals,
- expose OpenCode event payloads as product truth,
- preserve every deprecated Rust Probe command by default,
- make Pylon download Probe during Pylon build/startup,
- make SHC trusted for wallet-bearing or broad-production-secret workloads,
- let a successful Probe runtime exit imply product acceptance or provider
  settlement.

OpenCode remains a benchmark and design reference. It can provide lessons, not
authority.

## Source Material And Current Callers

### Deprecated Probe Source Material

The deprecated Probe implementation lives at `/Users/christopherdavid/work/probe`
as a Rust workspace. It should be treated like archived source material: read it
for behavior, protocol boundaries, fixtures, command ergonomics, and failure
cases, but do not make its module boundaries or full command inventory the new
runtime contract.

Useful source areas are:

- `probe-cli`
- `probe-core`
- `probe-protocol`
- `probe-client`
- `probe-server`
- `probe-daemon`
- `probe-provider-openai`
- `probe-provider-apple-fm`
- `probe-openai-auth`
- `probe-tui`
- `probe-decisions`
- `probe-optimizer`
- `probe-test-support`

Deprecated command inventory includes:

- `probe exec`
- `probe chat`
- `probe tui`
- `probe daemon run|stop`
- `probe ps|attach|logs|stop`
- `probe codex login|status|logout`
- `probe admin-chat-bridge fake|signed`
- `probe managed cloud-run-job run-once`
- `probe managed cloud-run-worker-pool run`
- `probe managed daytona advertise|run-once`
- Forge worker, verification, health-diagnosis, child-session, and RLM commands
- acceptance harnesses, self-tests, matrix runs, dataset export, optimizer, and
  signature registry commands

The new runtime should not port this list by default. For each old command,
search for a real caller first. If there is no current OpenAgents product surface, Pylon, operator,
or release caller, harvest useful behavior into tests or design notes and leave
the command out.

### OpenAgents product surface Today

OpenAgents product surface already has the Cloudflare product/control path for SHC work:

- `packages/sync-schema/src/index.ts`
  - `RunnerRuntime = 'opencode_codex' | 'codex'`
  - `RunnerBackend = 'shc_vm' | 'gcloud_vm'`
- `workers/api/src/omni-runs.ts`
  - default runtime: `opencode_codex`
  - default runner: `oa-shc-katy-01`
  - creates `AgentRunAssignment`
  - dispatches to SHC control with `agentRuntime`, repository refs, callback
    token ref, artifact policy, sandbox mode, timeout, and required artifacts
- `workers/api/src/runner-backends.ts`
  - backend projection includes `shc_vm`, `gcloud_vm`, and
    `cloudflare_container`
- `docs/2026-06-02-shc-agent-deployment-runbook.md`
  - SHC is the primary early execution substrate for low-to-medium trust
    workrooms and deploy automation
  - GCP is fallback/reference/sensitive rerun lane
  - product dispatch must be API to runner API, not SSH

OpenAgents product surface does not yet have a Probe-native runtime selector or Probe-native SHC
control payload. It still names OpenCode/Codex as the default runtime. The
intended new runtime selector is `probe`.

### SHC Today

The current primary SHC node is:

- runner id: `oa-shc-katy-01`
- OS: Ubuntu 24.04 LTS
- vCPU: 16
- RAM: about 62 GiB usable
- disk: about 247 GiB root filesystem
- current control lane: Codex/OpenCode control API
- current sandbox posture: `danger_full_access` inside an externally isolated
  no-wallet VM/workroom boundary for the Codex MVP

The important constraint is that SHC is acceptable for bounded, low-to-medium
trust workroom execution only when product policy keeps wallet authority and
high-sensitivity data out of the workroom.

### Pylon Today

Pylon already has a `probe_agent` capability path that checks:

- `probe.probe_bin --version`,
- `probe admin-chat-bridge signed --help`,
- bridge secret readiness,
- backend profile support,
- workspace mappings.

It invokes:

```sh
probe admin-chat-bridge signed --request <path> --secret-env <env> --cwd <workspace> --format json
```

That command is a real current caller and should be preserved as a narrow Pylon
bridge during migration. Other deprecated Probe commands should not receive the
same treatment unless caller search proves they are still live. The target
SHC/OpenAgents product surface runtime path should move toward managed runtime events instead of
admin-chat accepted-only output.

## OpenCode Reference Use

OpenCode may still be studied for:

- Bun package and binary build shape,
- Effect service layering discipline,
- durable session/event vocabulary,
- permission request ergonomics,
- shell/read/edit/patch tool expectations,
- API and SDK ergonomics,
- TUI and CLI user expectations,
- failure cases worth testing.

OpenCode should not be used for:

- runtime ownership,
- protocol ownership,
- event payload authority,
- direct code vendoring as the Probe core,
- runtime facade implementation,
- OpenAgents product surface product contract decisions.

The practical rule is: references can create requirements and tests, not
dependencies.

## Target Architecture

### Core Runtime

The new Probe should be an Effect application with these first-party services:

- `ProbeConfig`
  - reads environment, config files, runtime channel, backend profile, and SHC
    deployment settings.
- `ProbeSessionStore`
  - durable session metadata, messages, turns, tool calls, approvals, artifacts,
    status, and summary refs.
- `ProbeEventLog`
  - append-only ordered events with replay cursor support.
- `ProbePromptAdmission`
  - idempotent prompt admission before model execution.
- `ProbeRunner`
  - provider-turn loop, continuation rules, cancellation, and timeout policy.
- `ProbeProviderRouter`
  - Probe backend profiles to provider clients.
- `ProbeToolRegistry`
  - first-party tools, schemas, risk classes, redaction behavior, and output
    truncation.
- `ProbePermissionBroker`
  - approval requests, replies, denials, policy overrides, and pending state.
- `ProbeWorkspace`
  - repository checkout, workspace refs, mount refs, local path redaction, and
    cleanup receipts.
- `ProbeArtifactStore`
  - artifact refs, digests, summary files, patches, logs, and closeout material.
- `ProbeOpenAgents product surfaceSync`
  - maps runtime events to OpenAgents product surface/Synchronization events.
- `ProbeManagedRuntime`
  - start/resume/control/replay/heartbeat/child-session operations.
- `ProbeManagedEnvironment`
  - environment advertisements and compatibility matching.
- `ProbeShcRunner`
  - SHC one-shot and worker modes.
- `ProbePylonBridge`
  - current admin-chat caller bridge and future managed-runtime bridge.

All services should be typed with Effect Schema. Runtime JSON should be decoded
at the boundary and encoded through explicit schema modules.

### Package Shape

Use a first-party package workspace, for example:

```text
probe/
  package.json
  bun.lock
  packages/
    probe-cli/
    probe-core/
    probe-protocol/
    probe-runtime/
    probe-openagents-sync/
    probe-shc-runner/
    probe-pylon/
    probe-test-support/
```

This package layout is illustrative. The important point is ownership:
OpenAgents owns the runtime packages, schemas, service names, tests, and release
artifacts.

### Protocol Shape

The new TypeScript protocol package should harvest the useful protocol lessons
from the deprecated Rust `probe-protocol` crate, then own the final Probe wire
contracts directly:

- `probe.website_event.v1`
- `probe.managed_runtime.v1`
- `probe.scheduled_agent_bridge.v1`
- `probe.managed_environment.v1`
- `probe.admin_chat_bridge.signed.v1`
- `probe.shc_assignment.v1`
- `probe.pylon_runtime.v1`

Each schema should have:

- Effect Schema decoder,
- JSON fixture,
- redaction test,
- round-trip encode/decode test,
- OpenAgents product surface sync projection test where relevant.

## OpenAgents product surface Sync Plan

The runtime cannot be correct if OpenAgents product surface and Probe drift. Add a deliberate sync
surface.

### Shared Runtime Discriminator

Update `packages/sync-schema/src/index.ts`:

```ts
export const RunnerRuntime = S.Literals([
  'opencode_codex',
  'codex',
  'probe',
])
```

Then update OpenAgents product surface tests and default selection policy. The default can remain
`opencode_codex` until SHC Probe smoke passes, but `probe` must be a valid
assignment runtime early so SHC can be tested without schema hacks. Once real
traffic has moved, delete old runtime names that no longer have callers instead
of keeping aliases.

### Probe Assignment Payload

Add a Probe-specific assignment shape instead of overloading Codex semantics:

```text
openagents.probe_agent_assignment.v1
```

Fields should include:

- `runId`
- `agentRuntime: probe`
- `backend: shc_vm | gcloud_vm`
- `runnerId`
- `goal`
- `goalContext`
- `repository`
- `repositoryCloneUrl`
- `repositoryRef`
- `providerAccountRef`
- `authGrantRef`
- `githubWriteConnectionRef`
- `githubWriteGrantRef`
- `githubWorkOrder`
- `workspacePolicy`
- `toolPolicy`
- `approvalPolicy`
- `artifactPolicy`
- `retentionMode`
- `callback`
- `timeoutMs`
- `requiredArtifacts`

The payload should carry refs, not raw credentials or raw provider payloads.

### Event Ingestion

OpenAgents product surface should ingest Probe events through existing `agent-runs` paths but with a
Probe event mapper:

- Probe runtime event -> `AgentRunEvent`
- Probe artifact ref -> OpenAgents product surface artifact ref
- Probe approval request -> product approval row/ref
- Probe terminal event -> run status update
- Probe usage ref -> billing/usage source ref
- Probe retained failure -> retained failure refs

Raw tool output, raw prompts, raw local paths, bearer tokens, provider payloads,
bridge secrets, assignment nonces, and wallet material must be rejected before
D1 or Sync.

### Sync Outbox

Probe events that affect the product UI should flow through OpenAgents Sync:

- run queued,
- run started,
- assistant delta summary,
- tool started/completed summary,
- approval requested/resolved,
- artifact ready,
- run waiting for input,
- run completed/failed/canceled,
- deployment/writeback result.

The Sync payload should contain refs and summaries, not raw runtime logs.

## SHC Deployment Plan

SHC is the first serious deployment target for the new Probe runtime.

### Artifact Layout On SHC

Use a no-wallet runtime layout:

```text
/opt/openagents/probe/
  versions/
    <version>/
      bin/probe
      manifest.json
      sha256.txt
  current -> versions/<version>

/var/lib/openagents/probe/
  sessions/
  workspaces/
  artifacts/
  event-log/
  cache/

/etc/openagents/probe/
  probe.env
  runner.json
```

Secrets should come from SHC control materialization or environment injection,
not committed files.

### SHC Runtime Modes

Implement two modes:

```sh
probe shc run-once --assignment <path> --callback-url <url> --callback-token-env <env>
probe shc worker --control-url <url> --runner-id <id> --token-env <env>
```

`run-once` is the first target. It is easier to reason about, test, and close
out. `worker` can come after event replay and cancellation are reliable.

### SHC Control Request

OpenAgents product surface should eventually POST a Probe-shaped control payload to SHC:

```json
{
  "schemaVersion": "openagents.probe_shc_control_request.v1",
  "agentRuntime": "probe",
  "runnerId": "oa-shc-katy-01",
  "runId": "agent_run_...",
  "repository": "OpenAgentsInc/openagents",
  "repositoryCloneUrl": "https://github.com/OpenAgentsInc/openagents.git",
  "repositoryRef": "main",
  "goal": "bounded goal text",
  "toolPolicyRef": "policy.probe.low_trust.workspace_write",
  "artifactPolicy": "redacted_logs",
  "retentionMode": "openagents_durable",
  "callback": {
    "url": "https://openagents.com/api/omni/agent-runs/<runId>/events/ingest",
    "tokenRef": "runner_callback_token"
  },
  "timeoutMs": 300000
}
```

The SHC box receives grant refs and token refs. It should resolve material only
inside the runner boundary.

### SHC Trust Boundary

Initial SHC Probe runs should be:

- no wallet authority,
- no production broad cloud credentials,
- no private-production customer data,
- no reusable provider tokens in event payloads,
- low-to-medium trust only,
- workspace-scoped repository access,
- explicit artifact closeout,
- explicit product acceptance after runtime completion.

The current SHC sandbox posture can use `danger_full_access` only inside the
external no-wallet VM/workroom boundary. The Probe runtime should still model
the desired policy as `workspace_write` or stricter and let the SHC adapter
translate to the current host reality with a visible reason.

### SHC Release And Smoke Gates

Before OpenAgents product surface defaults a run to Probe on SHC:

1. Build a Linux x64 Probe artifact.
2. Install it on `oa-shc-katy-01` from a release asset, not source checkout.
3. Verify `probe --version`.
4. Verify `probe shc run-once --help`.
5. Verify a fake provider run.
6. Verify a real provider read-only run.
7. Verify a workspace-write run with approval.
8. Verify event callback ingestion.
9. Verify artifact closeout.
10. Verify cancellation.
11. Verify timeout.
12. Verify retained failure.
13. Verify no wallet authority.
14. Verify source checkout can be hidden after install.

The current Pylon v0.2 SHC no-source proof is the right style of evidence to
copy: install from public artifact, hide build source, run with isolated home,
capture public-safe proof.

## Pylon Installation Plan

Pylon remains a local supply connector and optional Probe host. It should not
carry Probe in its main build.

Add explicit commands:

```sh
pylon probe runtime status --json
pylon probe runtime install --version <version>
pylon probe runtime uninstall --version <version>
pylon probe runtime gc
```

Install should:

1. detect OS/arch/libc,
2. resolve a signed Probe release manifest,
3. download the matching artifact,
4. verify SHA-256 and signature,
5. unpack into a Pylon-owned cache,
6. smoke `probe --version`,
7. smoke `probe admin-chat-bridge signed --help`,
8. update local config or runtime state so `probe.probe_bin` points at the
   cached binary.

Default behavior:

- no download during Pylon build,
- no download during Pylon startup,
- no Probe launch unless workload poller or operator command requests it,
- assignment refusal if Probe is missing,
- optional auto-install setting defaults to false.

Capability metadata should add:

- `runtime_family: "probe"`
- `runtime_version`
- `runtime_channel`
- `probe_bridge_schema`
- `managed_runtime_schema`
- `managed_environment_schema`
- `backend_profile`
- `workspace_count`

It must not include local binary paths, local workspace roots, cache paths,
bridge secrets, provider keys, auth files, or wallet material.

## Build Path

### Phase 0: Product Contract Freeze

Freeze and fixture:

- admin-chat bridge canonical HMAC payload,
- nonce/replay behavior,
- website event schema,
- managed runtime schema,
- managed environment schema,
- SHC assignment schema,
- Pylon runtime capability schema,
- backend profile names,
- OpenAgents product surface `RunnerRuntime` discriminator.

Only freeze contracts that have an intended product role or a real current
caller. Deprecated Rust Probe surfaces without callers become source-material
notes, not compatibility promises.

### Phase 1: Runtime Skeleton

Build:

- first-party package workspace,
- Effect application runtime,
- `probe --version`,
- `probe help`,
- config/env loading,
- structured logger,
- JSON schema fixtures,
- single-binary build for macOS and Linux x64.

### Phase 2: Durable Session And Event Log

Build from scratch:

- session table/log,
- prompt admission,
- event sequence cursor,
- replay after sequence,
- status reconstruction,
- transcript refs,
- retained failure refs.

### Phase 3: Provider Router

Implement Probe-owned backend profiles:

- `openai-codex-subscription`
- `psionic-inference-mesh`
- `psionic-qwen35-2b-q8-registry`
- `psionic-qwen35-2b-q8-oracle`
- `psionic-qwen35-2b-q8-long-context`
- `psionic-apple-fm-bridge`
- `psionic-apple-fm-oracle`

Start with OpenAI-compatible and fake provider paths. Keep Codex subscription
auth and Apple FM as later gated work only if they are still part of the
intended product route.

### Phase 4: Tools And Approval Broker

Implement first-party tools:

- read,
- list/glob,
- grep,
- shell,
- apply patch,
- write/edit,
- artifact emit,
- question/approval,
- child-session request.

Each tool needs:

- schema,
- risk class,
- permission policy mapping,
- redacted summary,
- output truncation,
- artifact behavior,
- unit tests.

### Phase 5: OpenAgents product surface Event Export

Map Probe events to:

- `probe.website_event.v1`,
- OpenAgents product surface `AgentRunEvent`,
- OpenAgents Sync changes,
- token/usage refs,
- artifact refs,
- retained failure refs.

No raw runtime payload crosses into OpenAgents product surface.

### Phase 6: SHC Run-Once

Implement:

- SHC assignment decoder,
- repository materialization,
- grant ref materialization boundary,
- run-once execution,
- callback delivery,
- cancellation polling,
- timeout handling,
- closeout artifact creation.

This is the first deployable milestone.

### Phase 7: OpenAgents product surface Probe Runtime Selector

Update OpenAgents product surface:

- add `probe` to `RunnerRuntime`,
- add tests for Probe assignment creation,
- add SHC control payload tests,
- add event ingestion tests,
- add projection/redaction tests,
- allow explicit runtime selection in product/admin surfaces.

Do not make Probe default until SHC smoke passes.

### Phase 8: Pylon Compatibility

Implement:

- `probe admin-chat-bridge fake|signed`,
- Pylon install/status commands,
- Pylon `probe_agent` health for the new Probe runtime,
- Pylon managed-runtime bridge successor.

The existing Pylon command invocation remains supported because caller search
shows it is live. Give it a deletion condition once Pylon moves to managed
runtime assignments.

### Phase 9: Managed Runtime And Scheduled Bridge

Implement full:

- start,
- resume,
- cancel,
- interrupt,
- approval resolve,
- replay,
- heartbeat,
- child sessions,
- scheduled bridge.

### Phase 10: Deprecated Source-Material Closure

The Rust Probe implementation is already deprecated for this plan. Closure means
new Probe has harvested the source material that still matters and can ignore or
archive the rest. Required gates:

- OpenAgents product surface SHC run-once passes,
- Pylon compatibility passes,
- managed runtime passes,
- website events pass,
- core tools pass,
- provider profiles pass,
- retained failure and cancellation pass,
- no-source SHC install proof passes,
- old command caller search is documented,
- dead deprecated command surfaces are explicitly not ported.

Forge RLM, optimizer, and some hosted runners can remain later if they are not
on the critical Pylon/OpenAgents product surface/SHC path.

## Implementation Checklist

OpenAgents product surface docs and schema:

- update this audit,
- add `probe` runtime in `packages/sync-schema`,
- add Probe assignment fixtures,
- add SHC Probe dispatch tests,
- add Probe event ingestion tests,
- add redaction tests.

Probe repo:

- create first-party package workspace,
- add Effect runtime packages,
- add protocol package,
- add fake provider,
- add session/event store,
- add SHC run-once command,
- add website event mapper,
- add current Pylon admin-chat bridge.

SHC:

- release Linux x64 artifact,
- install from artifact on `oa-shc-katy-01`,
- configure `/opt`, `/var/lib`, `/etc` paths,
- run no-source smoke,
- capture public-safe proof.

Pylon:

- add runtime install/status commands,
- keep auto-install off,
- advertise `probe_agent` only when Probe runtime is locally ready,
- preserve bridge secret redaction.

## Risks

1. Runtime drift between Probe and OpenAgents product surface.

   Mitigation: shared schemas, fixtures, and OpenAgents product surface sync tests before live SHC
   dispatch.

2. Rebuilding from scratch can under-scope tool behavior.

   Mitigation: harvest tests from deprecated Probe and reference behavior from
   OpenCode, without copying OpenCode runtime code or preserving dead commands.

3. SHC sandbox reality differs from desired policy.

   Mitigation: model desired policy separately from host translation and expose
   the translation reason in operator-only diagnostics.

4. Callback/event leakage.

   Mitigation: strict redaction and boundary decoders before D1, Sync, or logs.

5. Supply-chain risk for SHC/Pylon artifacts.

   Mitigation: signed manifest, digest verification, pinned versions, no mutable
   latest for automatic execution.

6. Premature default switch.

   Mitigation: `probe` is selectable first, default only after SHC smoke and
   rollback plan.

7. Pylon accidental execution.

   Mitigation: install and run are separate; auto-install and auto-run default
   off.

## Source References

Probe:

- `/Users/christopherdavid/work/probe/README.md`
- `/Users/christopherdavid/work/probe/Cargo.toml`
- `/Users/christopherdavid/work/probe/crates/probe-cli/src/main.rs`
- `/Users/christopherdavid/work/probe/crates/probe-core/src/runtime.rs`
- `/Users/christopherdavid/work/probe/crates/probe-core/src/tools.rs`
- `/Users/christopherdavid/work/probe/crates/probe-core/src/admin_chat_bridge.rs`
- `/Users/christopherdavid/work/probe/crates/probe-core/src/managed_runtime.rs`
- `/Users/christopherdavid/work/probe/crates/probe-protocol/src`

OpenAgents product surface:

- `/Users/christopherdavid/work/openagents/packages/sync-schema/src/index.ts`
- `/Users/christopherdavid/work/openagents/workers/api/src/omni-runs.ts`
- `/Users/christopherdavid/work/openagents/workers/api/src/runner-backends.ts`
- `/Users/christopherdavid/work/openagents/docs/2026-06-02-shc-agent-deployment-runbook.md`
- `/Users/christopherdavid/work/openagents/docs/probe/2026-06-07-pylon-probe-coding-agent-audit.md`

Pylon:

- `/Users/christopherdavid/work/openagents/apps/pylon/src/lib.rs`
- `/Users/christopherdavid/work/openagents/docs/pylon/PYLON_ACCOUNT_LINKING_NIP98.md`
- `/Users/christopherdavid/work/openagents/docs/pylon/PYLON_VERIFICATION_MATRIX.md`

OpenCode reference only:

- `/Users/christopherdavid/work/projects/repos/opencode/CONTEXT.md`
- `/Users/christopherdavid/work/projects/repos/opencode/package.json`
- `/Users/christopherdavid/work/projects/repos/opencode/packages/opencode/src/effect/app-runtime.ts`
- `/Users/christopherdavid/work/projects/repos/opencode/packages/core/src/session`
- `/Users/christopherdavid/work/projects/repos/opencode/packages/opencode/src/tool`

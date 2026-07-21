# AI SDK Harness Abstraction — Harvest Analysis for Full Auto, Managed Sandboxes, and Apple FM Routing

**Date:** 2026-07-20
**Lane:** Fable strategy analysis
**Status:** Analysis and evidence survey only. This document flips no promise
state, changes no runtime authority, mints no issue, and dispatches no work.
Factual status authorities remain current code, `docs/sol/MASTER_ROADMAP.md`
(revision 126), live issue state, contracts, and receipts. Proposal packets
named below require Sol admission and owner acceptance before any dispatch.
**Sources:** Vercel AI SDK `main` at commit `6b6a8bbe92` (local read-only
reference clone `~/work/projects/repos/ai`, studied per the workspace
external-references rule — ideas re-derived, no code vendored),
`packages/managed-sandbox-contract` and the SBX-00…SBX-10 managed agent
sandbox sprint (epic #9023), the FAV-01…FAV-04 Full Auto routing sprint
(#9111–#9114), `packages/apple-fm-runtime` and the 2026-07-20 guided-routing
commit `0edc7eae69`, `apps/openagents-desktop` Full Auto and provider-lane
sources, `packages/agent-runtime-schema`, `packages/agent-turn-runtime`,
`docs/teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md`,
`docs/sol/2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md`,
`docs/fable/2026-07-17-full-auto-implementation-audit.md`,
`specs/desktop/full-auto.product-spec.md` (rev 14).
**Companion:** [`2026-07-20-full-auto-first-verifiable-mode.md`](./2026-07-20-full-auto-first-verifiable-mode.md).

---

## 0. Purpose and reading contract

The AI SDK repository now carries a complete, shipped "harness" abstraction:
one typed surface (`HarnessAgent` over a `HarnessV1` adapter contract) that
runs five established coding-agent runtimes — Claude Code, Codex, Deep
Agents, OpenCode, and Pi — behind a pluggable sandbox-provider seam, with
durable turn suspension, resume, skills, host tools, approvals, and
AI-SDK-shaped output streams. Goose, Amp, and Mastra adapters are announced
but unimplemented (one doc line each; no code on any branch).

OpenAgents independently built the surrounding pieces — a managed-sandbox
substrate with a Box v1 compatibility facade, a Full Auto orchestrator that
rotates Codex/Claude/Grok/Cursor lanes under durable leases and caps, and an
on-device Apple FM router with guided generation — but the desktop runtime
integrations underneath those systems are bespoke per provider, split across
two parallel execution stacks, and uneven on recovery.

This analysis answers three questions:

1. What exactly does the AI SDK harness abstraction contain, and which parts
   are worth harvesting as Effect ports of the *ideas*?
2. Where do those ideas land in the OpenAgents architecture — the desktop
   provider lanes, Full Auto, the managed-sandbox substrate, Apple FM
   routing?
3. What is the concrete proposal-packet shape for adopting them?

The headline finding, stated up front: **OpenAgents already adopted the AI
SDK's sandbox seam at the edges (`packages/ai-sdk-sandbox-local` and
`packages/ai-sdk-sandbox-openagents` implement `HarnessV1SandboxProvider`),
and already owns stronger durable authority than the AI SDK will ever ship —
but it lacks the middle layer: a single versioned adapter contract with
uniform turn suspend/continue semantics. That middle layer is the harvest.**

---

## I. What the AI SDK harness abstraction actually is

### I.1 The layer split

The AI SDK cleanly separates four concerns that the OpenAgents desktop
currently blends:

| Layer | AI SDK artifact | Job |
| --- | --- | --- |
| Consumer agent | `HarnessAgent` (`packages/harness/src/agent/harness-agent.ts:115`) | `generate()`/`stream()` returning standard `GenerateTextResult`/`StreamTextResult`; session create/resume; host-tool dispatch; approval continuations |
| Adapter contract | `HarnessV1` (`packages/harness/src/v1/harness-v1.ts`) | One versioned spec per runtime, "modelled after `LanguageModelV4`": `harnessId`, `builtinTools`, capability flags, `lifecycleStateSchema`, `getBootstrap`, single entrypoint `doStart` |
| Sandbox seam | `HarnessV1SandboxProvider` + `HarnessV1NetworkSandboxSession` | Provider-agnostic create/resume of an isolated workspace: file I/O, `run`/`spawn`, port URL resolution, network policy, snapshot reuse via `identity` + `onFirstCreate` |
| Runtime location | Bridge (in-sandbox process over WebSocket) or host-process library | Claude Code/Codex/OpenCode/DeepAgents run **inside** the sandbox behind a bridge; Pi runs in-process on the host with only workspace I/O crossing into the sandbox |

Two design decisions stand out as directly transplantable:

**Capability signalling by method presence, not a capability matrix.** The
`HarnessV1` doc comment is explicit: "There is intentionally no static
'capabilities' object — optional features are signalled by the presence or
absence of optional methods… Adapters that cannot satisfy a request throw
`HarnessCapabilityUnsupportedError`." This is the same fail-closed posture as
the OpenAgents Box facade's `501 capability_not_implemented`, applied one
layer lower — at the runtime adapter instead of the HTTP surface.

**The adapter never owns the sandbox.** The framework creates the sandbox
and per-session working directory
(`<defaultWorkingDirectory>/<harnessId>-<sessionId>`) *before* calling
`doStart`, passes both in, and forbids the adapter from calling `stop()`.
Adapters get a `restricted()` view for tool-safe file/exec access and the
infra view (`getPortUrl`, `setNetworkPolicy`) only for bridge wiring.
Ownership of the isolation resource is a framework concern; the runtime
integration cannot leak it.

### I.2 The session/turn lifecycle — the most valuable single idea

`HarnessV1Session` is small but complete
(`packages/harness/src/v1/harness-v1-session.ts`):

- `doPromptTurn({ prompt, tools, instructions, emit, abortSignal })` — one
  turn; events arrive through `emit`; a `HarnessV1PromptControl` handle feeds
  tool results, approvals, and mid-turn user messages back in.
- `doSuspendTurn()` — **freeze the active turn at a precise cursor while
  keeping the runtime alive.** For a bridge adapter this closes the host
  socket; the bridge keeps the turn running and accumulates events; the
  returned continuation state's cursor equals the last event delivered, "so
  the next slice's attach replays with no gap and no duplicate."
- `doContinueTurn()` — reattach to the in-flight turn without a new prompt.
  Lossless when the bridge still holds the live turn (`attach`/`replay`);
  lossy-but-correct when it is gone (`rerun` re-drives from the runtime's own
  persisted thread). Required on *every* adapter; the guarantee degrades with
  architecture, the contract does not.
- `doDetach` / `doStop` / `doDestroy` — park, stop, or destroy; `doDetach`
  and `doStop` return schema-validated resume state (`lifecycleStateSchema`)
  that a *different process* can later pass to `doStart({ resumeFrom })`.
- `doCompact(customInstructions?)` — trigger the runtime's own compaction;
  adapters whose transport cannot (Codex over `codex exec`) throw the
  capability error.

On top of this, `@ai-sdk/harness-workflow` implements a **slice runner**: run
one time-boxed slice of a turn inside a durable workflow step; on timeout,
`doSuspendTurn` yields `continueFrom`; the next step resumes losslessly. A
multi-hour agent turn becomes a chain of short-lived process invocations with
exact-once event delivery.

The bridge that makes this honest is generic
(`packages/harness/src/bridge/`): a WebSocket server with token auth,
single-flight reconnect, an in-memory event log keyed by `seq`, resume
replay, and lifecycle/meta files on the sandbox filesystem. Each adapter
supplies only a runtime-specific "turn driver." The Claude Code bridge is a
private package installed *inside* the sandbox at session bootstrap (pinned
`@anthropic-ai/claude-agent-sdk` + `@anthropic-ai/claude-code` + MCP SDK
deps, deliberately never bundled into the published host package), so runtime
code and model tool execution never run on the host.

### I.3 The stream vocabulary

`HarnessV1StreamPart` reuses `LanguageModelV4` primitives wherever possible
— text/reasoning start/delta/end, `ToolCall`/`ToolApprovalRequest`/`ToolResult`,
`finish-step`/`finish` with usage — and adds exactly three harness-only
concepts:

- `file-change` — a workspace mutation that happened through an opaque
  mechanism (Codex's internal `apply_patch`) with no visible tool call.
  Path-only by design.
- `compaction` — the runtime compacted its own context.
- `nativeName` on tool calls — cross-runtime tool normalization: adapters map
  native tool names (`Bash`, `shell`) onto a small common vocabulary
  (`read`, `write`, `edit`, `bash`, `glob`, `grep`, `webSearch`) while
  preserving the native name; `providerExecuted: true` distinguishes
  runtime-executed builtins from host-dispatched tools.

This is the piece that lets one UI, one usage accountant, and one transcript
store serve five different runtimes.

### I.4 Skills, tools, permissions

- **Skills** are runtime-neutral data: `{ name, description, content,
  files[] }`. Adapters decide surfacing (Claude Code materializes real skill
  directories; others inject differently).
- **Host tools** cross the contract as `HarnessV1ToolSpec` per turn; the
  runtime calls them, the harness emits `tool-call`, the host executes and
  submits results through the prompt-control handle. The Claude Code bridge
  converts host-tool JSON Schema to Zod and registers them on an in-sandbox
  MCP server — the runtime sees them as native tools.
- **Permission modes** are uniform (`HarnessV1PermissionMode`); adapters
  without native filtering get framework-emulated filtering
  ("auto-rejection" — inactive builtin calls route through the approval path
  and are denied before execution). The published capability table is honest
  about which adapter supports what natively.

### I.5 What the AI SDK does *not* have

Worth stating, because these are exactly OpenAgents' strengths:

- No durable server-side authority: resume state is handed to the caller to
  persist; there is no store, no lease, no generation fencing, no receipts.
- No fleet/capacity layer: one agent, one session; no rotation, quotas,
  account pools, capacity ledgers, or run caps.
- No settlement/usage authority: usage flows through the stream and stops.
- No on-device routing: model selection is per-adapter configuration.
- Shipped sandbox providers are Vercel-cloud and `just-bash` (an in-process
  virtual FS that cannot run real binaries and exposes no ports); bridge
  adapters need a real-process, port-capable provider to run anywhere else.
- Experimental status: the packages warn of breaking changes between
  releases. A direct runtime dependency is a churn liability; a contract-port
  is not.

---

## II. What OpenAgents already has on each seam

### II.1 Managed-sandbox substrate (SBX sprint, epic #9023)

The 2026-07-19 sprint landed a complete Effect-native sandbox authority —
contracts in `packages/managed-sandbox-contract` (Box v1 projection schemas,
native `ManagedSandboxResourceSchema`/`CommandSchema`/`EventSchema` with a
21-variant event union, a dependency-free lifecycle state machine with
invariant enumeration for model checking, guest-IO receipts, supervision
projections, vendor provenance pins), an HTTP facade
(`apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.ts`)
serving the admitted 18-operation Box v1 corpus over the native substrate,
and a durable Postgres store
(`packages/khala-sync-server/src/managed-sandbox-store.ts`) as sole
lifecycle/event/receipt authority. Provisioning is the Rust
`crates/oa-codex-control` control plane; the admitted isolation unit today is
one GCE VM, with `firecracker_microvm` contracted but gated. The whole
facade is default-off pending SBX-09 live acceptance; SBX-10 (checkpoint
download, fork, private desktop) is Phase 2.

The substrate is *stronger* than the AI SDK sandbox seam on authority
(durable store, generation-fenced resume, deny-all egress, receipts, typed
501s, default-off rollout) and *narrower* on developer surface: Phase 1
exposes no ports, and turn execution is coupled to the two native runtime
providers (`codex | claude`) rather than an open adapter contract
(`ManagedSandboxRuntimeProvider` in the contract package).

Two adjacent packages already speak the AI SDK's own sandbox interface:
`packages/ai-sdk-sandbox-openagents` (a `HarnessV1SandboxProvider` over an
`openagents.sandbox.v1` client) and `packages/ai-sdk-sandbox-local` (plain
`child_process.spawn` in a temp dir, owner-trusted, explicitly not
isolation). The interop seam was therefore already chosen in-repo: the
managed sandbox can *be* the sandbox provider under any harness-shaped
runtime — and unlike the AI SDK's own `just-bash`, both OpenAgents providers
run real processes, which is exactly what bridge-style adapters require.

### II.2 Full Auto and the Provider Lane SPI

Full Auto's orchestration is strong and durable where the AI SDK is absent:

- **Run model:** `FullAutoRun` (`apps/openagents-desktop/src/full-auto-run-registry.ts:233`)
  with a ten-state lifecycle, a single pure transition function
  (`applyFullAutoRunTransition`, line 363) that throws on illegal edges, an
  attributed actor vocabulary, append-only objective history, and
  `FULL_AUTO_RUN_ACTIVE_LIMIT = 8` enforced before a thread is minted.
- **Dispatch:** a serialized reconcile loop
  (`full-auto-reconcile.ts`, concurrency 8) with per-thread durable leases
  where the lease identity *is* the dispatched turn ref — exactly-once
  dispatch backed by the local-turn journal, with startup-only stale-lease
  clearing.
- **Capacity/routing:** the FAV-01…FAV-04 sprint added readiness-gated
  routing, four-lane rotation parity, Apple FM advisory capacity (no action
  authority), and a per-lane own-capacity ledger
  (`full-auto-capacity.ts`, `full-auto-routing.ts`, `full-auto-lane.ts`).

Underneath sits the repo's real harness-equivalent: the **Provider Lane
SPI** — `ProviderLane<Context>` (`apps/openagents-desktop/src/provider-lane.ts:149`)
with `capabilities()`, `admit`, `runTurn`, `interrupt`, projector hooks, and
one shared engine `makeProviderLaneDispatcher` (line 305) that owns content
admission, exactly-once journal accept, history assembly, usage-ledger
attribution, git turn checkpoints, and renderer forwarding. Four lanes
implement it: `codex-local` (CLI `codex exec --json` plus the app-server
JSON-RPC supervisor), `claude-local` (in-process
`@anthropic-ai/claude-agent-sdk` `query()`), and two ACP peers
(`acp:grok-cli` via `packages/grok-harness`, `acp:cursor-agent` via
`packages/cursor-agent-runtime`) through one generic adapter
(`makeAcpProviderLane`, `provider-lane-acp.ts:216`). Capability over-claiming
quarantines the whole lane (`provider-lane-capabilities.ts`).

Where the SPI falls short of the AI SDK contract:

1. **No turn suspend/continue.** Recovery is a per-lane enum:
   `provider_session_replay` (codex only) vs `interrupt_on_restart`
   (claude, ACP). A desktop restart interrupts every non-codex in-flight
   turn; the dispatcher's `reconcileLocalTurns` replays only lanes that
   declare replay. There is no equivalent of
   `doSuspendTurn`/`doContinueTurn`, no cursor-exact reattach, and no
   time-boxed slice runner — which is precisely what FA-REL-01 (#8979,
   packaged restart-resume observation) needs.
2. **The dispatch switch is not uniform.** `main.ts:5220–5269` branches by
   `laneRef` string; the two native lanes have bespoke dispatch wrappers
   rather than going through the generic dispatcher path the ACP lanes use.
3. **Per-lane lifecycle duplication.** `claude-local-runtime.ts` and
   `codex-local-runtime.ts` each independently reimplement session-by-thread
   maps, active-turn tracking, pending questions, follow-up queues, account
   rotation, resume-vs-history-prefix logic, and interrupt clocks; codex even
   imports Claude-module internals ad hoc rather than through an
   abstraction.
4. **No per-run isolation.** Full Auto binds one resolved local workspace
   per run (FA-H2, fail-closed on mismatch) — correct as a guardrail, but
   concurrent runs share the same working tree, differentiated only by
   thread and lane. Git checkpoints give rollback, not isolation. The
   managed-sandbox substrate exists but Full Auto does not consume it.

### II.3 Desktop provider stacks and Apple FM routing

The desktop currently runs **two parallel execution stacks**:

- The **Provider Lane SPI** path (above), whose stream vocabulary is the
  frozen `ClaudeLocalEvent` envelope
  (`claude-local-contract.ts`) — deliberately reused for Codex and the ACP
  lanes so there is no third in-process vocabulary, at the cost of
  hand-written mappers (`acpProjectionEventToLaneEvent`; the codex exec
  stream mapping).
- The **AFS turn kernel** path (`packages/agent-turn-runtime`): an Effect
  `TurnService` with `ProviderRegistry`/`TurnPolicy`/`ContextSource` tags
  and `ProviderStreamEvent`, used for Apple FM local inference and the
  delegation flow (`installDesktopTurnKernel`, `main.ts:1503–1543`).

Apple FM's role after commit `0edc7eae69` is exactly right and worth
preserving unchanged in any refactor: the on-device model is a **router, not
an executor**. When at least one delegate agent (Codex, Claude, Grok) is
ready, the Swift `foundation-bridge` runs **guided generation** — a runtime
`DynamicGenerationSchema` whose `candidate` field is constrained (anyOf) to
the admitted connected-agent set — so constrained sampling *cannot* emit an
unadmitted route. The route JSON is then decoded fail-closed
(`packages/apple-fm-runtime/src/recommendation.ts`: `action_claim_rejected`,
`provider_unadmitted`, `malformed_output`…), and only the host's
`TurnPolicy.decide` turns an advisory `RouteRecommendation` into an
authoritative `RouteDecision` (`packages/agent-runtime-schema/src/route.ts`).
Apple FM answers directly only when nothing is connected. Full Auto does not
route to Apple FM at all; Apple FM contributes advisory capacity only
(FAV-03).

The normalization layers also already exist in schema form:
`packages/agent-runtime-schema` carries `KhalaRuntimeEvent`
(`openagents.khala_runtime_event.v1`) — a neutral event union that includes
`file.change`, `compaction.recorded`, `usage.recorded`, `tool.*`,
`agent.child.*`, and `raw.sidecar_ref`, i.e. a superset of
`HarnessV1StreamPart`'s vocabulary — plus `RuntimeInteraction`
(`openagents.runtime_interaction.v1`), a provider-neutral durable
question/approval model, and an `AgentRuntimeAdapterKind` enum that already
reserves `opencode`, `hermes`, `hosted_container`, and `gcp` kinds with no
desktop implementation behind them.

The duplication inventory (each item is per-provider code a unified contract
would own once): two event vocabularies with hand mappers; two
admission/readiness/usage models (`ProviderLaneCapabilityReport` vs
`InferenceProviderDescriptor`); three approval implementations (Claude
`canUseTool`, Codex app-server `requestApproval`, ACP provider-native)
despite the canonical `RuntimeInteraction` schema; four executable-discovery
paths; two account-rotation implementations; and recovery implemented once
(codex) out of four lanes.

---

## III. The harvest — what to port, what to keep, what to skip

### III.1 Adoption posture

Three options exist; the repo has implicitly chosen the third and should make
it explicit:

1. **Depend on `@ai-sdk/harness` + adapters directly.** Rejected for the
   desktop core: the packages are experimental with announced breaking
   changes; the adapters bundle their own runtime pinning (the Claude bridge
   installs its own SDK versions inside the sandbox) which conflicts with
   the desktop's account-custody and executable-authority model
   (`provider-runtime-host.ts` deliberately never packages or re-signs
   Codex); and Promise/callback surfaces would sit awkwardly under the
   repo's Effect mandate.
2. **Ignore it.** Rejected: the contract solves, with shipped code and five
   working adapters as proof, exactly the recovery/duplication gaps the
   2026-07-17 Full Auto audit and §II.3 inventory document.
3. **Port the contract shape to Effect; keep interop types at the seams
   where they already exist.** This is the current de-facto posture —
   `ai-sdk-sandbox-local`/`ai-sdk-sandbox-openagents` already implement
   `HarnessV1SandboxProvider`, and
   `docs/sol/2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md`
   already frames the AI-SDK-facing rewrite. This analysis extends that
   roadmap with the harness layer that shipped after it was written.

### III.2 Port list (ranked by leverage)

**H1 — The session lifecycle contract (`doSuspendTurn`/`doContinueTurn`
above all).** The single highest-leverage idea. An Effect-native
`AgentHarness` service contract in `packages/agent-runtime-schema` (or a new
`packages/agent-harness-contract`) with the verb set
`start / promptTurn / suspendTurn / continueTurn / detach / stop / destroy /
compact`, where:

- `PromiseLike` becomes `Effect` with typed errors; `abortSignal` becomes
  Effect interruption; `emit` becomes a `Stream<KhalaRuntimeEvent>`;
- `lifecycleStateSchema` becomes an Effect Schema the *durable turn journal*
  validates and persists — strictly stronger than the AI SDK, which hands
  resume state to the caller and hopes;
- capability absence becomes a tagged `HarnessCapabilityUnsupported` error,
  matching the Box facade's 501 posture at the adapter layer;
- sessions are `Scoped` resources so detach-vs-destroy is explicit in the
  type.

This directly closes desktop gap 1 (§II.2): once every lane implements
`suspendTurn`/`continueTurn` — even in the degraded `rerun` form the AI SDK
explicitly blesses for host-resident runtimes like Pi (and like the
in-process Claude SDK lane) — restart-resume stops being a codex-only
property, and FA-REL-01's packaged restart observation has a uniform
mechanism to observe.

**H2 — The generic bridge/event-log runtime, made durable.** The AI SDK
bridge's essential trick is a seq-numbered event log with
`attach`/`replay`/`rerun` semantics, generic across adapters. OpenAgents
should port the *protocol shape* but back the log with the existing
local-turn journal (and, for managed-sandbox sessions, the Postgres event
store with its opaque cursors — `BoxProjectionCursorSchema` is already this
concept on the server side). Result: cursor-exact reattach for the renderer,
Full Auto reconcile, and mobile supervision alike, from one log instead of
three projections.

**H3 — One adapter contract under both stacks.** Re-home the four provider
lanes and the Apple FM provider onto the H1 contract: lanes become harness
adapters; the AFS kernel's `ProviderRegistry` registers harness-backed
candidates; `TurnPolicy.decide` keeps routing authority. The frozen
`ClaudeLocalEvent` envelope stays as the renderer projection (it is a
behavior-contract surface), but it becomes *one projection of* the harness
stream rather than the SPI's native vocabulary — the hand mappers collapse
into one audited `KhalaRuntimeEvent → ClaudeLocalEvent` projection. This
merges the two stacks (§II.3 duplication items: readiness, usage,
capability, approval models each collapse to one).

**H4 — Uniform approvals through `RuntimeInteraction`.** The AI SDK's
approval continuation model (approval requests as stream parts; framework
auto-rejection emulating filtering for adapters without native support) maps
one-to-one onto the already-durable `RuntimeInteraction` schema. Port the
*emulation* idea: lanes that cannot natively filter builtin tools get
framework-level auto-deny through the same interaction path, and the three
bespoke `pendingQuestions` maps become one.

**H5 — Skills and host tools as neutral data.** `HarnessV1Skill`
(`{name, description, content, files[]}`) and per-turn host-tool specs are
trivially Effect-Schema-able and immediately useful: the ProductSpec
workroom's dynamic tools (`product-spec-app-server-tools.ts`,
`builtin-productspec-skill.ts`) are today wired only into the Codex
app-server lane; as harness-level host tools they reach every adapter,
including future ACP peers, with one registration. The Claude bridge's
JSON-Schema→runtime-native registration trick (in-sandbox MCP server) is the
reference pattern for lanes whose runtimes speak MCP.

**H6 — Bootstrap recipes + snapshot identity for the managed sandbox.**
`getBootstrap` + `identity` + `onFirstCreate` is a clean contract for "bake
the expensive setup into a reusable image": the managed-sandbox substrate
already has filesystem checkpoints and generation-fenced resume (SBX-01),
and SBX-10's fork/snapshot phase is the natural place to expose
identity-keyed bootstrap reuse. Porting this contract now, before SBX-10 is
designed, avoids inventing a second recipe format later.

**H7 — The slice runner for Full Auto.** Full Auto's continuation model
(turn cap 20, reconcile-after-terminal) is turn-granular; the AI SDK's slice
runner is *intra-turn* time-boxing with lossless resume. Combining H1+H2
gives Full Auto slice-granular liveness: a stalled provider surfaces as "no
events past cursor N for M minutes" (feeding `full-auto-liveness.ts` with
exact cursors instead of stamps), and an app restart mid-turn resumes the
turn instead of failing it. This is also the mechanism that would let
long-running Full Auto turns survive the `oa-dev --restart` generation swap.

**H8 — Common-tool normalization vocabulary.** The
`commonName`/`nativeName`/`providerExecuted` triple is small and worth
copying into `KhalaRuntimeEvent.tool.*` metadata so cross-lane transcript
analytics (and MemoHarness policy learning) can reason about "bash" without
per-lane name tables.

### III.3 Keep (OpenAgents advantages the port must not regress)

- Durable authority everywhere the AI SDK has none: journals, leases,
  generation fencing, receipts, usage ledger exactness
  (`usageTruth: exact | estimated`), capability quarantine, fail-closed
  routing, account custody and health-ordered rotation, the 8-run cap, Khala
  Sync projections, behavior contracts.
- The Apple FM guided-generation router and the
  advisory-recommendation/authoritative-decision split. The AI SDK has no
  routing story; ours is owner-bound and constrained at the sampler. In the
  harness world, `harnessId` + adapter readiness becomes the *candidate
  vocabulary* the router's constrained schema is built from — one readiness
  truth feeding both the Pylon heartbeat capacity refs and the desktop
  router, instead of today's two projections.
- The owner-local danger profile as an explicit policy, not an accident: the
  AI SDK assumes sandbox-always; OpenAgents deliberately runs owner-local
  lanes with full access and puts isolation at the account-home and
  managed-sandbox boundaries. The harness contract must carry the permission
  mode as policy, with the sandbox provider optional per session — which the
  AI SDK's own Pi adapter (host-process, workspace-only sandboxing) already
  legitimizes.

### III.4 Skip

- `just-bash` and the Vercel sandbox provider: superseded by
  `ai-sdk-sandbox-local` / managed sandbox.
- `HarnessAgent`'s `GenerateTextResult` projection: the desktop renders from
  its own envelope; the web app has no harness surface today.
- Direct dependency on the experimental harness packages in the desktop
  main process (posture §III.1). Where Khala AI-SDK surfaces already consume
  AI SDK streams (`packages/khala-ai-sdk-core`), a thin
  harness-stream→UI-message projection can be revisited later.
- Amp/Goose/Mastra speculation: upstream has one doc line and no code; there
  is nothing to follow yet. If OpenAgents wants those runtimes, ACP or the
  H1 contract is the path, not waiting for upstream.

---

## IV. Proposal packets (require Sol admission before any dispatch)

Ordered; each is bounded and independently landable. Names are provisional.

- **HARN-01 — Freeze the Effect harness contract.** New
  `packages/agent-harness-contract` (or an `agent-runtime-schema` module):
  session verbs per §III.2 H1, stream = `KhalaRuntimeEvent`, schema-validated
  resume/continuation state, tagged capability errors, permission modes,
  skill/host-tool schemas (H5), common-tool metadata (H8). Contract + fixture
  conformance tests only; no desktop changes.
- **HARN-02 — Durable event-log/replay runtime.** The generic seq-cursor
  log with attach/replay/rerun over the local-turn journal (H2), plus the
  conformance suite that proves gap-free, duplicate-free replay.
- **HARN-03 — Codex + Claude lanes as harness adapters.** Wrap
  `codex-local-runtime` and `claude-local-runtime` in the contract; retire
  the bespoke dispatch wrappers in `main.ts:5220–5269` so all lanes go
  through one dispatcher path; extract the duplicated session/question/
  follow-up machinery into the shared adapter base (§II.2 gap 3).
- **HARN-04 — ACP lanes on the same contract.** `makeAcpProviderLane`
  becomes a harness adapter factory; approvals route through
  `RuntimeInteraction` (H4).
- **HARN-05 — Merge the stacks under the router.** Apple FM's candidate set
  and the Full Auto lane registry both derive from harness adapter
  readiness; one readiness projection feeds the kernel descriptor, the
  Pylon heartbeat, and FAV routing.
- **HARN-06 — Suspend/continue everywhere + Full Auto slice liveness.**
  Implement `suspendTurn`/`continueTurn` per adapter (degraded `rerun` where
  necessary), wire `full-auto-liveness.ts` to cursors, and land the
  restart-resume observation FA-REL-01 needs (H7).
- **HARN-07 — Managed sandbox as harness sandbox provider.** Full Auto opt-in
  per-run isolation: a run profile may bind a managed sandbox (or local
  worktree) instead of the shared workspace; bootstrap recipes with
  identity-keyed reuse align with SBX-10 (H6). Also the converse: the Box
  facade's `prompt`/`promptRunStatus`/`events`/`interrupt` operations map
  onto harness session verbs, so SBX-04 turn execution generalizes past
  `codex | claude` to any admitted harness adapter.

Sequencing note: HARN-01/02 are pure additions and can proceed under normal
admission. HARN-03 onward touch the live desktop dispatch path and the
frozen renderer envelope's behavior contracts — they need the usual oracle
coverage and must not weaken any Full Auto guardrail
(`FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS`) or the fail-closed routing
posture.

---

## V. Bottom line

The AI SDK harness layer is the missing middle of the OpenAgents agent
stack: OpenAgents already has the better sandbox authority below it and the
better orchestration, routing, and settlement above it, but every runtime in
between is a bespoke integration with uneven recovery. The harvest is one
Effect-native contract — session verbs with uniform suspend/continue, one
stream vocabulary that `KhalaRuntimeEvent` already nearly is, capability by
method presence with typed refusal, neutral skills/host-tools, and
durable-log replay — implemented by the four existing lanes and consumed by
both the Full Auto dispatcher and the Apple FM router. Nothing is vendored;
the ideas are re-derived onto contracts the repo already owns, and the two
places the repo already touches AI SDK types (the sandbox providers and
`khala-ai-sdk-core`) stay the honest interop seams.

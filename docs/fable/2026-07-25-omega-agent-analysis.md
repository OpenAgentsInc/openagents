# Omega Agent — The First-Party Agent and the Meta-Harness Are One Product

**Date:** 2026-07-25
**Lane:** Fable strategy analysis
**Status:** Strategic evidence, not dispatch authority. This document flips no
promise state, changes no runtime authority, mints no issue, and dispatches no
work. The factual authorities remain current code, `docs/sol/MASTER_ROADMAP.md`,
`docs/omega/ROADMAP.md`, live issue state, contracts, and receipts. Any packet
derived from this document requires normal Sol admission and owner acceptance,
and any Omega-side change requires the Omega packet/delta discipline
(`OMEGA_DELTAS.md`, `crates/omega_deltas`).
**Sources:** `~/work/omega` (fork of Zed, `OpenAgentsInc/omega`, ~140 commits
ahead of `zed/main`, latest `b768854c56 Cut 0.2.0-rc10`), `OMEGA_DELTAS.md`,
`docs/src/development/omega-native-agent.md` (omega, commit `2b7746c8b2`),
`docs/fable/2026-07-22-openagents-as-meta-agent-analysis.md`,
`docs/buzz/2026-07-24-omega-buzz-full-parity-recommendation.md`,
`docs/omega/README.md` and the `docs/omega/` receipt corpus (FA-00..FA-07,
AC-00..AC-03, Sarah workroom spec rev 4, bubblewrap removal audit 2026-07-25),
`docs/teardowns/2026-07-13-t3-code-teardown.md`,
`docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md`,
`docs/teardowns/2026-07-17-t3-code-mobile-app-teardown.md`,
`docs/teardowns/2026-07-17-t3-code-openagents-desktop-ui-gap-analysis.md`,
`docs/teardowns/2026-07-17-t3-code-openagents-mobile-component-gap-analysis.md`,
`docs/teardowns/2026-07-17-t3-code-openagents-mobile-controller-gap-analysis.md`,
`docs/teardowns/2026-07-18-zed-teardown.md`,
`apps/openagents-desktop/docs/meta-agent-acp-server-zed-demo.md` (#9181),
`docs/transcripts/241.md`–`docs/transcripts/244.md` (the Khala arc),
`docs/khala/khala.md`, `docs/khala-sync/SPEC.md`,
`apps/openagents.com/INVARIANTS.md` ("Khala Coding Delegation Through
Pylons"), `apps/openagents.com/workers/api/src/inference/` and
`khala-hosted-runtime-dispatch.ts` / `khala-cloud-runtime-dispatch.ts`,
`packages/khala-tools/src/fleet-delegate-program.ts`.
**Companion(s):**
[the meta-agent analysis](2026-07-22-openagents-as-meta-agent-analysis.md),
[the Nostr-native pivot analysis](2026-07-21-nostr-native-pivot-analysis.md).
**Labels:** Claims below carry `[EXISTS]`, `[NEEDS BUILD]`, or `[SPECULATION]`.

---

## Synopsis

Zed ships a first-party agent — the in-process "Zed Agent" — and hosts external
agents beside it over ACP. T3 Code ships no agent at all — it is a pure
meta-harness, a control plane that wraps Codex, Claude Code, Cursor, Grok, and
OpenCode behind one neutral event vocabulary and one thread UI. The teardowns
of both are complete, and they point at the same conclusion from opposite
directions: the first-party agent and the meta-harness are not two products.
They are one product with a disclosure discipline.

**Omega Agent** is that product: the single named agent a user talks to in
Omega, our Zed fork. It has a native in-process execution loop (the runtime we
already inherited and turned on), and it has hands beyond that loop — the
wrapped harness fleet (Codex, Claude Code, and the rest over ACP and the
harness-adapter contract), the Full Auto engine, and the cloud Agent Computer.
The user opens Omega, lands on a new-agent-thread screen, types, and gets one
agent. `Cmd+Shift+A` opens that same screen from anywhere. Which executor did
the work is always disclosed, never hidden — but it is detail, not identity.

Most of this is not new construction. The fork already runs the native agent
by default, already binds `cmd-shift-a` to a new agent thread, already ships
Codex over ACP as a default external agent, and already has a supervised
engine (`omega-effectd`) whose host bridge drives panel threads as Full Auto
workers. What is missing is the identity, the front door, the routing layer
with honest attribution, and the meta-harness disciplines T3 proved (and the
ones T3 got wrong, which we fix at design time). The earlier teardown
recommendations that targeted the Electron OpenAgents Desktop are re-expressed
here against the fork.

One boundary matters enough to get its own section: **Khala** — the
collective-intelligence orchestrator behind the OpenAI-compatible endpoint —
is a distinct system Omega Agent routes to, and is routed *by*, not a set of
capabilities Omega absorbs. Omega orchestrates agents on one machine; Khala
orchestrates the network — hosted lanes, cloud execution, fleet delegation,
cross-device Sync, metering, and the optimization loop. They meet at a typed
event vocabulary and typed dispatch seams, never by merging.

---

## I. The thesis — one agent, two inheritances

The 2026-07-22 meta-agent analysis argued that OpenAgents should present as
one agent, not a workbench of lanes: a single persistent, named, accountable
identity that internally orchestrates a harness fleet and externally conforms
to the same contracts as the harnesses it wraps. That analysis targeted the
Electron desktop. The owner has since accepted Omega — the Zed fork — as the
primary desktop surface, and the fork already carries the strongest possible
starting position for exactly this product:

1. **The Zed inheritance.** Zed's native agent is not a model; it is an
   orchestration layer above a selected `LanguageModel`: an in-process turn
   loop with tools, permissions, project context, thread persistence, and a
   shared thread UI that native and external agents both project into. We own
   all of it in the fork, and it is already on by default. `[EXISTS]`

2. **The T3 inheritance.** T3 Code proved the meta-harness seam: a driver per
   harness, four transport kinds (stdio JSON-RPC, in-process SDK, ACP, HTTP),
   one neutral versioned runtime-event vocabulary with native provenance, an
   event-sourced thread store, and one UI over all of it. It also proved, by
   omission, where the differentiation is: T3 has rigorous *access* control
   and zero *execution* accountability — no receipts, no effective-containment
   record, default `danger-full-access`. `[EXISTS]` (as teardown evidence)

Omega Agent is the union: Zed's native loop becomes one executor among
several; T3's wrapper becomes the routing layer; and OpenAgents' receipts,
typed outcomes, and authority discipline become the part neither incumbent
has. The name on all of it is Omega Agent. The user talks to one agent; the
agent runs work on whichever hands fit.

Three properties from the meta-agent analysis carry over intact:

1. **Recursive composability.** Omega Agent conforms to the same shapes it
   consumes. It hosts harnesses over ACP as a client, and it can be served
   over ACP to other hosts (the loopback ACP server already exists in the
   Electron app as a port candidate). An agent that wraps agents and can
   itself be wrapped.
2. **Attribution honesty.** Presenting one identity is only honest with
   disclosed routing. Every thread names its actual executor — runtime,
   provider, model, lane — as a behavior-contract-grade obligation (the
   single-delegate honest-attribution rule, #9127, already landed on the
   Electron side).
3. **Hill-climbability.** Every routed turn produces receipts; receipts feed
   the grading and optimization loop already specified in the meta-agent
   analysis. The router is the policy that improves offline and never
   promotes itself.

---

## II. What we already own — the inventory

The design has to start from the fork as it is, not from the teardowns'
now-stale picture of an Electron app with a 760px-minimum React shell. The
inventory, verified against `~/work/omega` at the pin above:

**The native runtime is live.** `crates/agent` (the runtime: `NativeAgent`,
`NativeAgentServer`, `NativeAgentConnection`, `Thread`, `ThreadStore`),
`crates/acp_thread` (the runtime-neutral projection: `AcpThread`, the
`AgentConnection` trait), and `crates/agent_ui` (the panel, ~6,400-line
`agent_panel.rs`, message editor, model/profile/mode selectors, diff review,
threads sidebar via `crates/sidebar`). `agent.enabled: true` and
`agent.button: true` ship in `assets/settings/default.json` under
OMEGA-DELTA-0011; `cmd-shift-a` is globally bound to `agent::NewThread`;
default model is `google/gemini-3.6-flash` via a direct provider. `[EXISTS]`

**External harnesses are live over ACP.** `crates/agent_servers` (the
`AgentServer` trait, ACP transport, `CustomAgentServer` with well-known IDs
`gemini`, `claude-acp`, `codex-acp`, `cursor`), the registry store in
`crates/project`, and `codex-acp` enabled by default in settings. The panel's
`Agent` enum is `NativeAgent | Custom { id } | Stub`. ACP is consumed as the
external crate `agent-client-protocol =2.0.0`. `[EXISTS]`

**The engine and its bridge are live.** `omega-effectd` — the packaged Node 24
Effect runtime supervised from Rust over the framed
`openagents.omega.effectd.v1` protocol (64 KiB frames, generation fencing,
≤32 in-flight host requests, 30 s deadlines) — owns Full Auto run authority
(ten-state lifecycle, active-run cap 8, three non-overridable guardrails),
Agent Computer cloud sessions (a real Firecracker turn is receipted, PR #9213),
and Sync/mobile projections. Its host bridge
(`crates/agent_ui/src/omega_host_bridge.rs`, 1,690 lines) drives Agent Panel
threads as workers on lanes `codex-local` and `claude-local` with correlation
journals, evidence caps, and typed errors. `[EXISTS]`

**The harness fleet and meta-agent core exist upstream of Omega.** The
`AgentHarness` contract with seven live adapters (Codex, Claude Code,
OpenCode, Pi, Cursor, Goose, Grok), the generic ACP-to-harness adapter
factory, and the loopback meta-agent ACP server v0 (#9181: default off,
loopback only, deny-by-default permissions, read-only shape) all exist in the
`ai` packages and the Electron app. They are port candidates, not Omega code
yet. `[EXISTS]` upstream, `[NEEDS BUILD]` in Omega.

**The product posture is decided.** The delta ledger encodes it mechanically:
trust-all-worktrees (0001), tool permissions default `allow` (0002 — "YOLO
mode throughout"), no hosted-plan or trial surfaces, no Restricted Mode, the
agent on by default (0011), collab retired in favor of native panes over the
Nostr workroom log (0010). The bubblewrap-removal audit states the law:
confirm on irreversible data loss, never on capability. `[EXISTS]`

**What is missing** is exactly the product layer this document is about:

- the name and product contract for the first-party agent (the fork's own
  native-agent doc gates the rename: do not relabel the Zed Agent tile as
  Omega Agent without an admitted service and product contract) `[NEEDS BUILD]`
- the chat-first front door (welcome-state and `Cmd+Shift+A` both landing on
  a real new-agent-thread screen with a standard chat input) `[NEEDS BUILD]`
- the disclosed routing layer above native/ACP/engine executors `[NEEDS BUILD]`
- explicit steer/queue semantics, receipts surfaced in-thread, and harness
  maintenance with provenance — the T3 lessons `[NEEDS BUILD]`

---

## III. The naming gate, and what "Omega Agent" names

The fork's native-agent documentation is explicit: the planning corpus forbids
relabeling the "Zed Agent" tile as "Omega Agent" without an admitted Omega
service and product contract. That gate is correct, and this analysis is the
strategic half of satisfying it — the other half is a normal admitted packet
(a ProductSpec delta or Omega packet naming the identity, plus a new
`OMEGA-DELTA-NNNN` with a mechanical check, per the fork's delta discipline).

The important design decision hiding inside the rename: **"Omega Agent" names
the orchestrator, not the native loop.** The native runtime (`ZED_AGENT_ID` at
`crates/agent/src/agent.rs`, surfaced through `agent_ui::Agent::label()`) is
one executor beneath the product identity. If we simply rename the native tile
and stop, we get a rebranded Zed Agent sitting in a picker beside `codex-acp`
and `claude-acp` — a workbench of lanes again, the exact presentation the
meta-agent analysis rejected. The rename is only worth doing as part of the
inversion: Omega Agent is the default and the face; the native loop, the
wrapped harnesses, and the engine lanes are its hands, visible in disclosure
and in an advanced picker, not competing identities at the front door.

Two subordinate naming facts:

- The mechanical rename surface is small: `ZED_AGENT_ID`, the panel `Agent`
  enum's `label()`/`icon()`, and settings/docs copy. The brand-audit packets
  (`OMEGA-BRAND-*`) already own the "no Zed product copy survives" sweep and
  its `omega_deltas` test.
- The telemetry/product identity must not leak upstream identity: the fork's
  own rule that `telemetry_id`/"Zed Agent" is never presented as an
  OpenAgents service identity stands.

---

## IV. Architecture — a disclosed router above three executor classes

The core proposal. Omega Agent is a Rust implementation of the existing
`AgentConnection` trait — the same seam the native agent and every external
ACP agent already implement — that owns *routing* rather than *execution*:

```
                        ┌─────────────────────────────┐
                        │   Agent Panel / AcpThread    │  shared thread UI
                        │  (crates/agent_ui, sidebar)  │  (unchanged seam)
                        └──────────────┬──────────────┘
                                       │ AgentConnection
                        ┌──────────────▼──────────────┐
                        │        OMEGA AGENT           │  identity + router
                        │  route · disclose · receipt  │  (new, Rust)
                        └───┬──────────┬──────────┬───┘
                            │          │          │
              ┌─────────────▼──┐  ┌────▼─────┐  ┌─▼──────────────────┐
              │ Native loop    │  │ External │  │ Engine lanes       │
              │ crates/agent   │  │ ACP      │  │ omega-effectd      │
              │ Thread + tools │  │ codex-acp│  │ Full Auto runs,    │
              │ (in-process)   │  │ claude-  │  │ Agent Computer     │
              │                │  │ acp, …   │  │ (cloud/Firecracker)│
              └────────────────┘  └──────────┘  └────────────────────┘
```

**Why `AgentConnection` is the right seam.** Zed already proved it twice: the
native agent adapts its in-process runtime to the ACP-shaped thread interface
(`NativeAgentConnection`), and every external agent arrives through the same
interface from the other side. The panel needs no new thread UI, no new
persistence, no new review surface — `AcpThread`, the action log, agent diff,
checkpoints, and the threads sidebar all keep working. Native-only extras
already travel by checked downcast; Omega-Agent-only capabilities (receipt
links, work refs, lane disclosure) should travel the same way — an additional
trait behind a downcast, never a fork of `AcpThread`. `[NEEDS BUILD]`

**What the router does.** Per thread (and, where safe, per turn):

1. **Route.** Deterministic and fail-closed first, exactly like the Full Auto
   routing decision (`FA-RT-01`): a typed decision from declared capabilities,
   readiness, and user pins — never keyword matching, never a silent model
   guess. The user can always pin an executor; the default is the router.
   Model-advisory routing arrives later through the same offline-optimized,
   receipted policy loop the meta-agent analysis specifies. `[NEEDS BUILD]`
2. **Disclose.** Every thread carries a visible executor line: runtime class
   (native / external ACP / engine lane / cloud), agent id, provider, model,
   and — for engine work — the run ref. This is the #9127 honest-attribution
   rule promoted to the fork. `[NEEDS BUILD]`
3. **Receipt.** Turn admission, completion, and any consequential effect
   produce typed outcomes and receipt refs. GPUI renders them (the
   `workroom_receipts` inspector pattern already exists in the fork); it never
   stores them as a second durable authority. `[NEEDS BUILD]`

**What the router must not do.** The authority laws from the Omega corpus bind
it directly:

- GPUI is projection and command entry; `omega-effectd` remains the only
  Omega mutation path for run authority. The router *requests* engine work
  over the framed protocol; it never becomes a second lifecycle owner. The
  Full Auto port audit already names this failure mode — GPUI must be a
  fourth caller of the same run actions, not a parallel lifecycle.
- A native `Thread` is never the source of truth for Full Auto (the fork's
  native-agent doc says this verbatim). The host bridge drives panel threads
  as *workers*; runs live in the engine.
- Omega never creates a second home for an external agent: no copying of
  Codex/Claude credentials, homes, or provider config (the token-copying
  import was already rejected in independent review for exactly this).
- Zed keeps editor/project/buffer/terminal/worktree truth; OpenAgents keeps
  work/agent/policy/receipt/run truth; Nostr carries the signed record
  without being command authority; GitHub keeps repository and claim
  authority.

**Where the brain lives — the split.** The routing *decision* for interactive
threads is small, deterministic, and belongs in-process (Rust) where it can
answer at keystroke latency. The routing *policy* — capability ledgers,
readiness probes, recall, verification, optimization — belongs in
`omega-effectd`, which already owns lane readiness, capacity, liveness, and
receipts for Full Auto. The Rust router consults the engine's typed
`get_capacity`-class answers and fails closed to the native loop when the
engine is unavailable. This keeps one policy authority while keeping the
front door responsive. `[NEEDS BUILD]`

---

## V. What T3 teaches the wrapper — adopt, fix, refuse

The T3 teardowns are the meta-harness curriculum. Re-expressed against the
fork (their original targets were the Electron app):

**Adopt.**

1. **One neutral, versioned runtime-event vocabulary with provenance.** T3
   normalizes five harnesses into one item-oriented event union, and every
   projected event retains its raw source. Omega already has the equivalent
   seam — ACP session updates projected into `AcpThread` entries — but the
   provenance half is weaker: keep the full decoded native envelope privately
   (peer, generation, method, schema/wire version, payload classification,
   projection result or explicit unsupported reason) so projection loss is
   accounted, never silent. T3's projection is deliberately lossy with no
   loss accounting; that is the defect to fix, not the pattern to copy.
2. **Explicit steer/queue semantics.** T3's worst bug is a design hole: send
   during a running turn silently becomes an implicit Codex steer without a
   guard, and other harnesses would each do something different. The fix is
   the six-point spec from the teardown: distinct steer and enqueue commands,
   durable queue admission before UI ack, one thread-owned scheduler that
   promotes the queue head only after proven quiescence, capability
   negotiation with declared fallbacks, and distinct visible states for
   draft / steering / queued. The Electron app already had explicit
   Queue/Steer (the gap analysis said "do not trade explicit Queue/Steer for
   visual parity") — that semantics ports to the fork as part of Omega
   Agent's turn admission. Zed's native loop already has a clean steering
   boundary (stop at the next message boundary) for the native executor;
   external executors get negotiated capability or an honest typed refusal.
3. **Harness maintenance as a typed capability.** T3's one-click provider
   install/update is the fleet flow to steal — each driver owns a maintenance
   resolver — plus the two things T3 lacks: version pinning against a
   component ledger, and provenance verification with a receipt for the
   binary just swapped. The fork's registry agents (`agent_server_store`,
   registry npx/binary resolution) are the natural home.
4. **The environment vocabulary.** `ExecutionEnvironment` / `KnownEnvironment`
   / `AccessEndpoint` / `AdvertisedEndpoint`, access separated from launch,
   endpoint providers as plugins — already adopted as ENV-1/ENV-2 upstream.
   Omega Agent inherits it when remote environments arrive; nothing about the
   v0 front door depends on it.
5. **Receipts as test-visible completion signals.** Every async pipeline
   emits a typed milestone tests can await (T3's `RuntimeReceiptBus` shape);
   the engine protocol's typed outcomes already are this — extend the
   discipline to the router.

**Refuse.**

1. **Containment by delegation with no record.** T3 ships no sandbox and no
   receipts, and defaults to `danger-full-access`. Omega has deliberately
   chosen allow-by-default (OMEGA-DELTA-0002) — that is an owner decision and
   this document does not relitigate it. The refusal is subtler: YOLO
   *execution* without *accountability*. Omega Agent keeps typed outcomes,
   receipts, and honest attribution even in full-allow mode. Confirmation
   friction is not our differentiation; the evidence trail is. Irreversible
   data loss keeps its confirm, per the bubblewrap audit.
2. **Hand-written wire contracts without negotiation.** T3's protocol version
   is "whatever this release shipped," and its ACP snapshot silently went
   stale. The fork consumes pinned ACP 2.0.0 and the engine protocol is
   versioned with generation fencing; keep both, and keep the conformance
   assets (`packages/agent-client-protocol-conformance`, the release matrix)
   as the claim authority for any "works with X" statement.
3. **A too-small adapter seam.** `sendTurn`/`interruptTurn` is not enough;
   the seam needs steer, enqueue, and declared capability negotiation from
   day one, or every harness difference becomes a silent behavior fork.
4. **Environment-bound threads as a law of nature.** T3 threads are forever
   bound to one environment; every incumbent audited has the same limit.
   Portable session identity remains the OpenAgents differentiator on the
   roadmap; the router should treat "which host executes" as late-bound where
   the engine supports it, and must not add new couplings that make
   portability harder. `[SPECULATION]` at the fork level today.

---

## VI. What Zed teaches the native half — keep, and keep separate

The Zed teardown's §14 discipline maps onto the fork nearly one-to-one, since
the fork *is* Zed. The points that bind Omega Agent's design:

1. **Explicit runtime identity.** Zed keeps native, external-ACP, and
   terminal-thread experiences distinct with different authority splits.
   Omega Agent adds a fourth identity — the router — and must show *exact*
   runtime, provider, model, and effective-permission state per thread, which
   the fork's native-agent doc already lists as an open product gap.
2. **Availability ≠ approval ≠ containment.** Zed's three-layer tool model
   (profiles gate what the model sees; permission rules gate approval;
   sandbox gates the process) survives even under Omega's allow-default:
   profiles still shape tool availability, `always_confirm`/`always_deny`
   patterns still work, and the fork must never claim a native thread is
   sandboxed when the sandbox gates were deliberately removed.
3. **Prompt-cache-stable project context.** ProjectContext refreshes only
   when the model-visible value changes; worktree instruction files
   (`AGENTS.md`, `.rules`, `CLAUDE.md`) and skills feed the native loop. The
   router must not destabilize this — native turns keep their context
   machinery untouched.
4. **Persistence is already solved.** Threads persist locally (zstd-compressed
   versioned JSON, separate DB, parent links, drafts, scroll state). Omega
   Agent stores its routing decision and disclosure metadata *with* the
   thread, not in a new store. The event-sourced store T3 built is not worth
   rebuilding in Rust v0; the engine already owns durable run state, and
   thread persistence already exists. What we take from T3's event sourcing
   is the command/causation/correlation discipline at the engine boundary,
   which `omega-effectd` already largely has.
5. **Child topology vs sibling work.** Zed distinguishes delegated child
   threads (subagents, bounded depth, cancellation propagation) from
   independent sibling threads. The router preserves that distinction:
   engine-dispatched Full Auto work is *sibling* work with its own authority,
   never a hidden child of the interactive thread.

---

## VII. The front door — `Cmd+Shift+A` and the chat-first home

The owner direction is concrete: `Cmd+Shift+A` opens the main "new agent
thread" screen, and the app defaults to showing that screen — welcome as new
agent chat, standard chat input, users able to start typing immediately.

**What exists.** `cmd-shift-a` → `agent::NewThread` globally (OMEGA-DELTA-0011);
the Agent Panel with its message editor; `WelcomePage` in `crates/workspace`
(Omega-branded, recent-projects grid); identity-first onboarding in
`crates/onboarding`; and the panel's own `ProjectEmptyState` plus an inherited
`AgentPanelOnboarding` provider upsell — the last surface still essentially
upstream. `[EXISTS]`

**What to build.** A first-class **New Agent Thread** surface that is
simultaneously the welcome state and the `Cmd+Shift+A` target: `[NEEDS BUILD]`

- A centered composer with focus on open — the standard chat input, mention
  and slash machinery included (the fork's message editor already has both).
  Zero clicks between app launch and typing.
- Omega Agent as the presented identity. The executor picker (native /
  codex-acp / claude-acp / engine lanes) is present but subordinate — a
  compact selector defaulting to the router, in the composer footer where
  model/profile selectors already live. Real select surfaces, not
  click-to-cycle (the T3 gap-analysis rule).
- Recent threads and active runs beneath the composer: the sidebar's thread
  rows and the Full Auto/receipt projections already exist as sources. The
  T3 sidebar-concept rules apply — recency, always-visible status, nothing
  actionable ever collapsed.
- Project-optional. A thread opened from the welcome state may precede any
  project; binding to a project/worktree happens on first workspace-touching
  action, using the same workspace-binding guardrail vocabulary Full Auto
  already enforces. `[SPECULATION]` on the exact binding UX.

**Keybinding cleanup, stated honestly.** Today `cmd-shift-a` is globally
`agent::NewThread`, while `agent::ToggleFocus` (open/focus the panel) sits on
the awkward `cmd-?`, and `cmd-shift-a` still means `editor::SelectToBeginningOfLine`
in the editor context plus two lower-priority bindings it shadows. The front
door work should resolve this deliberately: `cmd-shift-a` opens the New Agent
Thread surface from every context (including welcome), panel focus gets a
sane binding, and the editor-context collision is resolved rather than
shadowed. Each change is a delta-ledger entry with a test, per fork
discipline. `[NEEDS BUILD]`

**Contract discipline.** "Cmd+Shift+A opens the new-agent-thread screen" and
"the default surface is the new agent chat" are stated owner UX expectations —
behavior-contract material under the repository's behavior-contract mandate.
They should land as oracle-tested contracts in the owning surface's registry
when the packet lands, not remain conversation.

---

## VIII. The meta layer — engine lanes, cloud hands, and serving ACP

Beyond the interactive loop, Omega Agent is the visible face of machinery the
fork and monorepo already run:

**Full Auto.** The engine owns runs (ten states, guardrails, caps, receipts);
the host bridge already drives panel threads as workers on `codex-local` and
`claude-local`. Omega Agent's thread surface links to runs it started and
renders their receipts; starting a run is a typed engine command from the
composer ("run this until done" is a dispatch, not a longer chat turn).
The interactive thread and the run remain distinct authorities with visible
links — never one blob. `[EXISTS]` engine-side, `[NEEDS BUILD]` for the
linked presentation.

**Agent Computer.** The cloud lane is proven end-to-end (a real Firecracker
turn with pinned checkout, verifier, usage, and teardown receipts). To the
router it is one more executor class with its own readiness and receipts;
GPUI keeps zero placement authority. `[EXISTS]`

**Serving ACP.** The loopback meta-agent ACP server (#9181) — default off,
loopback only, deny-by-default, read-only shape — ports from the Electron app
to the Omega side, most naturally as an engine capability rather than a GPUI
socket (the workroom law: GPUI never opens its own socket). That makes Omega
Agent attachable from any ACP host, including upstream Zed, other forks, and
future third-party editors — the recursive-composability property, now with
our own editor as the first-party host instead of the only host.
`[NEEDS BUILD]` (port), with the v1 fleet-backed server still deferred
upstream (#9179).

**The Nostr record.** Current owner direction makes Nostr the primary
workroom protocol for the Sarah/Buzz parity program, with the signed event
log as the record and never command authority. Omega Agent's disclosure and
receipt surfaces should be designed so their projections can land on that
record without a second store — same rows, different carrier. The workroom
pane program (`OMEGA-BZ-*`, `SARAH-NR-*`) owns that lane; Omega Agent
consumes it and must not fork it. `[EXISTS]` as direction, `[NEEDS BUILD]`
as integration.

---

## IX. Khala — the orchestrator Omega routes to, not into

Omega Agent orchestrates. Khala orchestrates. If the design is careless, that
sentence collapses into one system and both products lose their shape. The
episode arc (241–244) and the code both say the same thing: they are two
orchestrators at two altitudes, and the seam between them already exists.

**What Khala is.** Khala is OpenAgents' collective intelligence behind one
OpenAI-compatible endpoint — "one endpoint that behaves like a single model
but is an agent network underneath" (the epic definition read on-record in
episode 241, answering Sakana's closed Fugu with an open, inspectable,
market-grown version). Episode 242 launched it as the flagship: a router over
a pool of models, tools, validators, and Pylon workers; outputs as open-ended
as deployed software; traces as the asset; and the load-bearing economic
claim — pre-trained models depreciate, *this improves*, because every routed
outcome feeds an optimization loop. Episode 243 made it a real inference
business (the tokens-served counter, the OpenCode drop-in, the provider mix,
the authoritative-total lesson). Episode 244 inverted the flow: hitting the
Khala endpoint can route work onto *your own* linked capacity — your Codex
subscription, your local harnesses, your Pylon — "a super model" assembled
from the caller's own resources, under the own-capacity-only and semantic-
routing-only invariants. All of that is live: the gateway
(`inference/chat-completions-routes.ts`, `model-router.ts`), the coding
workflow classifier and Pylon delegation
(`coding-workflow-classifier.ts`, `coding-workflow-delegation.ts`, the
delegation runbook and invariant ledger), the hosted and cloud lanes
(`khala-hosted-runtime-dispatch.ts` — `hosted_khala`;
`khala-cloud-runtime-dispatch.ts` — `cloud-gcp`/Firecracker), Khala Sync,
the public counters, and the first typed Blueprint program with an optimizer
feedback contract (`khala.fleet.delegate` in
`packages/khala-tools/src/fleet-delegate-program.ts`, GEPA feedback with the
"optimizer acceptance is not runtime promotion" boundary). `[EXISTS]`

**Naming discipline, because it is enforced.** *Khala* is the orchestrator
product. *Khala Sync* is strictly the replication substrate — the spec and
the package both forbid calling it bare "Khala." *Khala Code* was the coding
client surface. Three different things; this document and any Omega surface
copy must keep them apart.

**Blueprint is Khala's program layer, and it is evidence, not authority.**
Blueprint — the opinionated-DSPy system of typed Signatures, Modules,
Program Runs, and optimizer runs — is deprecated as a standalone repo and
absorbed where it is used: the signature-lookup service behind the
no-keyword-routing rule, the shared contract/privacy predicates in
`packages/blueprint-contracts`, and the fleet-delegate program above. Its
standing invariant transfers to Omega Agent unchanged: *program runs are
decision evidence; they do not authorize writes.* A routing decision — ours
or Khala's — is never itself permission.

**The boundary, with its precedent.** The Omega corpus already ruled on this
seam once, for Agent Computer: "Reuse server-side capacity and fallback
truth. **Do not duplicate `khala-cloud-runtime-dispatch` policy in Rust**,"
with the falsifier "Omega calls `oa-codex-control` directly from Rust." That
packet generalizes into the division this design adopts:

Khala owns, and Omega Agent must not absorb:

1. **The public front door and cross-client routing policy** — the
   OpenAI-compatible gateway and the lane plan over provider adapters. Omega
   picks which *local* executor runs a turn; only Khala sees the cross-client
   pool.
2. **The hosted and cloud lanes** — `hosted_khala` answers turns for clients
   with no local hands at all (mobile, web, a sleeping desktop); `cloud-gcp`
   boots Firecracker guests under fail-closed arming. This is the meta-agent
   analysis' "always at least one ready hand," and a desktop structurally
   cannot be it.
3. **Fleet dispatch admission and the own-capacity invariant** — typed or
   semantic classification only, per-account capacity projections, the
   controlled dispatch gate, no resale across identities. A local orchestrator
   cannot enforce a cross-identity invariant; it does not hold the identity
   graph.
4. **Cross-device truth** — Khala Sync scopes, cursors, and the
   Postgres-authoritative changelog. Omega is one client of
   `scope.thread.*`; if it owned thread truth locally, mobile and the hosted
   lane would lose the same conversation.
5. **Metering and the public counter** — one authoritative running total,
   exact-only usage rows, idempotent recording. Episode 243 spent hours
   paying down what happens when two sources disagree about a total. Omega is
   a *producer* into this ledger — the hosted-lane metering literally records
   `producer_system: omega` — never a second accountant.
6. **The optimization loop over orchestration decisions** — delegation
   example datasets, GEPA feedback, receipts-to-dataset. "It improves rather
   than depreciates" is only true if every receipt lands in one corpus;
   per-client corpora do not compose.
7. **Economics and settlement** — entitlements, pricing, revenue share,
   settled feeds. Never desktop state.

Omega owns the complement: local turn execution and process supervision,
local harness selection, editor UX and keybindings, owner-local credential
custody, supervision of `omega-effectd`, projection-only rendering of Khala
state, and graceful degradation — when Khala Sync is unreachable, Full Auto
keeps dispatching locally and says so (`omega_khala_sync_session_unavailable`),
rather than pretending or stalling.

**Where they meet.** Two typed seams, both existing:

- **The event vocabulary.** `KhalaRuntimeEvent` — the neutral 24-kind turn
  event union in `@openagentsinc/agent-runtime-schema` (authority already in
  the shared SDK repo, outside both apps) — is what the seven harness
  adapters emit *and* what Khala's hosted/cloud dispatch consumers write into
  thread scopes. Omega and Khala meet at this vocabulary, not by sharing
  stores. That the schema authority already sits outside both codebases is
  exactly what makes the protocol boundary cheap. `[EXISTS]`
- **The dispatch seams.** Omega reaches Khala through `omega-effectd`'s typed
  protocol (Sync projection publishing, cloud sessions, and — when ported —
  hosted-lane routing), and Khala reaches Omega through the delegation path
  it already uses for any Pylon-linked capacity: classified coding work
  dispatched onto owner-linked local executors. `[EXISTS]` engine-side.

**What this means for the router.** Khala appears in Omega Agent's routing
table twice, in opposite directions, and the disclosure line must distinguish
them:

- **Outbound — Khala as executor class.** When work should leave the machine
  — no ready local capacity, a turn that must survive the laptop sleeping, a
  fleet burn-down, a mobile continuation — the router dispatches to Khala's
  lanes through the engine and renders the returned events and receipts. The
  routing *decision* is Omega's; the lane *policy* inside Khala's walls is
  Khala's. `[NEEDS BUILD]` (the routed presentation; the lanes exist)
- **Inbound — Omega as Khala capacity.** The 244 inversion runs the other
  way: a Khala request (from mobile, from the API, from Sarah) classified as
  coding work lands on this owner's linked local executors — which is
  precisely what the host bridge already does for Full Auto lanes. Omega
  Agent must present inbound engine-dispatched work honestly as sibling work
  under run authority, never as hidden turns of the interactive thread.
  `[EXISTS]` mechanically, `[NEEDS BUILD]` as presentation.

The meta-agent analysis called its subject "Khala with an agent-shaped front
door instead of a model-shaped one." Omega Agent is that front door on the
desktop. It does not replace the collective behind it — it names one
accountable face for this machine, and it keeps the communion's rule: the
aggregation may summarize, it must not conceal.

## X. Honest staging

**What exists today, precisely.** A live native agent runtime on by default
with `cmd-shift-a` bound to a new thread; external Codex/Claude agents over
ACP; a supervised engine owning Full Auto and cloud execution with receipts;
a host bridge that drives panel threads as workers; a delta ledger enforcing
the product posture; a meta-agent ACP server and seven-adapter fleet upstream
of the fork; and complete teardown curricula for both inheritances.

**v0 — the smallest slice that changes the product.**

1. Admit the identity: the Omega Agent product contract (ProductSpec delta or
   Omega packet), then the rename delta — `ZED_AGENT_ID`-surface relabel,
   icon, copy — satisfying the fork's own naming gate.
2. The front door: the New Agent Thread surface as welcome state and
   `Cmd+Shift+A` target; keybinding cleanup as tested deltas; behavior
   contracts for the two owner UX expectations.
3. Disclosure v0: the executor line on every thread (runtime class, agent,
   provider, model, run ref where applicable) — before any routing
   intelligence exists, while the "router" is just the user's picker plus a
   fail-closed default to the native loop.

v0 deliberately contains no new routing brain, no new protocol, and no new
store. It is a naming, surface, and honesty release.

**The ordered path after v0.**

1. Router v1: deterministic, fail-closed route selection consulting engine
   capacity/readiness; user pins always win.
2. Turn semantics: explicit steer/queue commands with durable admission and
   per-executor capability negotiation.
3. Receipts in-thread: typed outcomes surfaced through the receipt-inspector
   pattern; harness maintenance with pinning and provenance receipts.
4. Meta reach: the loopback ACP server ported behind the engine; Full
   Auto/cloud dispatch from the composer with linked runs.
5. Khala routing v1: the outbound Khala executor class (hosted/cloud lanes
   through the engine, events and receipts rendered back), and honest
   presentation of inbound Khala-dispatched work as run-authority sibling
   work — both directions carrying the disclosure line.
6. Policy loop: recall wiring and offline-optimized routing per the
   meta-agent analysis — receipts to datasets to gated promotion, the policy
   never promoting itself, and every routed outcome's receipt landing in
   Khala's one corpus rather than a desktop-local shadow.

**What would falsify the thesis.**

- If users, given one agent with disclosed routing, consistently reach past
  it for the raw harness picker — the one-agent presentation would be
  overhead, not product.
- If the router cannot beat a static per-user default on routed-task outcomes
  once receipts are graded — then the "brain" is ceremony and the honest
  product is a fast picker with great harness ergonomics.
- If maintaining harness adapters against upstream churn (ACP drift, SDK
  changes, auth changes) costs more than the leverage they provide — the
  meta-harness premise itself weakens, for us as it would for T3.

---

## XI. Risks, stated plainly

1. **A second durable authority in GPUI.** The single most-named failure mode
   across the Omega corpus. The router will be tempted to keep "just a little"
   run or policy state in panel entities. Every such temptation is either
   thread-persisted display state or an engine call.
2. **Rename before contract.** Relabeling the native tile without the admitted
   contract violates the fork's own gate and produces exactly the "generic
   chat panel bolted onto an editor" the fork's PRODUCT.md anti-references.
3. **Native thread as Full Auto truth.** The bridge drives threads as workers;
   inverting that (runs living in threads) is explicitly forbidden and easy
   to do accidentally when the same surface shows both.
4. **Silent routing.** Any routing without the disclosure line converts the
   one-agent presentation from product honesty into misattribution — the
   failure #9127 exists to prevent.
5. **Keybinding regressions.** `cmd-shift-a` already shadows three bindings;
   editor muscle memory is part of the fork's product promise ("preserve
   familiar editor controls"). Changes must be deltas with tests, not drive-by
   keymap edits.
6. **YOLO plus hired work.** Allow-by-default is safe-enough for owner-local
   hands; it is not a posture for marketplace or community-sourced work,
   where the community-workroom contract already requires narrow grants.
   The router must never route untrusted-origin work onto full-allow local
   executors.
7. **Meta-layer scope creep into v0.** The engine, the workroom, the Nostr
   record, and the policy loop all border this design. v0 ships a front door
   and a name; everything else arrives through its own admitted packets.
8. **Absorbing Khala policy into Rust.** The precedent is explicit — do not
   duplicate `khala-cloud-runtime-dispatch` policy in Rust — and the pull
   will be constant, because a local reimplementation is always "simpler"
   until the invariants (own-capacity, one counter, one receipt corpus)
   silently fork. The falsifier pattern from the Agent Computer packet
   applies to every Khala seam.
9. **Name collapse.** Khala, Khala Sync, and Khala Code are three things
   with an enforced naming rule. An Omega surface that says "Khala" for the
   sync substrate, or that brands local orchestration as Khala, corrupts the
   vocabulary both products depend on.

---

## XII. Closing

Zed built the agent and left the fleet to others; T3 built the fleet and left
the agent — and the accountability — unclaimed. We own a fork of the first,
teardowns of the second, an engine with receipts, and an owner direction that
the product is one agent you can start talking to the moment the window
opens. Omega Agent is not a new system; it is a name, a front door, and an
honesty discipline placed on top of machinery that already runs — including
Khala, which stays its own orchestrator on the other side of a typed seam.
The agent is the product; the harnesses are its hands; Khala is the
collective it belongs to; the receipts are why anyone should believe any of
it.

# Apple FM as the subagent dispatcher/router — the definitive design plan

Date: 2026-07-20
Status: design plan and analysis. This is speculation and design, not dispatch
authority, not a product promise, and not a commitment. It describes a system
that is not built yet. It grounds every claim in shipped code and in the source
documents named in Section 12. The author is an agent, and this document is
advisory design evidence only.

Audience: human.

## 0. What this document is

This is the synthesis of several threads that just landed. It answers one
question: how does OpenAgents make the local, no-cost model (Apple Foundation
Models, called Apple FM) the always-on ROUTER that dispatches subagents (Codex
or Claude) from OpenAgents Desktop chat, so that the local model reads the user
intent and the live agent availability, then either answers locally or delegates
to a stronger agent, and so that the chat shows a live status card that opens a
right-pane message chain for that subagent.

It reads the owner's product intent in the build transcript, the Apple FM bridge
and boundary code, the deleted-but-revivable DSPy-in-Effect ("DSE") program
system, the current Blueprint program kernel, and the memory substrate work. It
maps the design onto the exact code seams that exist today.

## 1. The vision

The local no-cost model is the always-on router. On each chat turn it reads two
things: the user intent, and the BOOT SEQUENCE availability of the local agents
(Codex, Claude Code, Grok, Apple FM). It then decides one of two outcomes. It
answers the turn locally when the task is small enough for a 3B on-device model.
It delegates to a subagent (Codex or Claude) when the task needs a stronger
coding agent. When it delegates, the Electron main process spawns the real
subagent turn, and the chat shows a live status card with a loading, done, or
failed state. Clicking the card opens the right pane, which shows that
subagent's message chain (its reasoning summaries, tool-call labels, and
outputs) as a redacted, owner-only trace.

This is the "clicky, StarCraft-like" subagent surface the owner asked for in the
build transcript (`docs/transcripts/249.md`): "when a sub-agent gets spawned,
[I want] it to pop out — instead of just having a message that it spawned one,
let me click into it from there ... show some of the latest message of the
sub-agent in that box." The owner also wants the parent thread to reflect true
subagent state, not a flat scrollback: "Get the fuck out of the terminal. Give
me an app ... let me see what the fuck is going on ... make it clicky."

The value of the local model here is not that it is smart. It is that it is
free, private, and always resident. A free on-device model can run in front of
every metered agent turn, at zero token cost, with nothing leaving the machine
(`docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md`, Section 6).
The router lane is also the lane that grows the on-device share of the workload
over time, which is the episode-194 product goal the free-inference analysis
records.

## 2. The core architectural rule: the model proposes, the host acts and reports

This is the most important rule in the plan, and it is the direct fix to the
"I dispatched a subagent" hallucination.

**The local model never free-texts a claim that it dispatched a subagent.**
Today the on-device chat turn runs one bounded prompt and returns free text. The
strict honesty preamble in `apps/openagents-desktop/src/renderer/shell.ts`
(`buildOpenAgentsAppleFmPrompt`, lines 2249-2256) exists because a small model
hallucinated actions. The preamble literally tells the model it "cannot ... start
other agents. So never claim you did, are doing, or will do any such action." A
router that let the model say "I dispatched Codex" in prose would reintroduce
exactly that failure.

The rule instead is:

1. The local model emits a STRUCTURED, typed route decision, not prose. The
   decision is one of:
   - `{ action: "answer", text: string }` — answer the user locally, in words.
   - `{ action: "delegate", agent: "codex" | "claude", task: string }` — hand
     the task to a real subagent, with a bounded task summary.
2. The Electron main process (the host) executes that decision DETERMINISTICALLY.
   On `answer`, it renders the text. On `delegate`, it starts the real subagent
   turn through the existing provider-lane machinery (Section 6) and reports the
   TRUE status through the status card (Section 4). The host, not the model,
   makes the claim, and it makes the claim only after the real action happened.

So the model proposes; the host acts and reports. The model's word is never the
evidence that a subagent ran. This mirrors the `TriageRoute.v1` router idea in
`docs/dspy/2026-07-20-apple-fm-dse-analysis.md` (Section 4.3), whose output is "a
typed route (`answer_local` or `delegate`)," and it keeps the honesty preamble's
intent while removing the reason it existed.

### 2.1 How the structured decision crosses the boundary

The current Apple FM contract carries no structured dispatch field. The renderer
IPC surface is a single bounded prompt in and one bounded text out:

- `apps/openagents-desktop/src/apple-fm-contract.ts`: the request is one
  `prompt` field, 1 to 4000 characters (`AppleFmStartTurnRequestSchema`, lines
  93-95, comment "Nothing else crosses to main"). The result is one `text` field
  up to 8192 characters, "never a raw transcript or tool output"
  (`AppleFmTurnResultSchema`, lines 98-109), plus honest `usageTruth`
  (`exact | estimated | unknown`, line 57).
- `apps/openagents-desktop/src/apple-fm-host.ts`: `runTurn(prompt)` runs one
  bounded, read-only turn and refuses unless the bridge is live-ready (lines
  217-256). It is a pure single-owned-session supervisor.

There are two honest ways to carry a typed route decision across this boundary,
and the plan uses them in sequence:

- **Phase 1 (shipped plain-text bridge, v0.1.1).** The host prompts the model
  for a small, fixed JSON shape, then PARSES and VALIDATES that JSON in main with
  an Effect Schema decoder. A decode failure falls back to `answer` with the raw
  text, so a malformed route never dispatches anything. The model's output is
  still untrusted text; main's decoder is the gate. This adds a new main-owned
  route-decision schema, not a new renderer trust surface.
- **Phase 2 (mature bridge growth path).** The mature Swift bridge in Git
  history has `@Generable` structured generation, which decodes directly into a
  schema-guided shape (`docs/apple-fm/2026-07-19-apple-fm-swift-bridge-full-audit-and-openagents-desktop-plan.md`,
  Section 4, "structured generation ... schema-guided decode via `@Generable` /
  `GeneratedContent`"). When that lands in the shipped subset, the route decision
  becomes a native structured decode instead of a parsed string, which raises
  reliability on a small model. This is the same typed-signature idea DSE
  expresses (`docs/dspy/2026-07-20-apple-fm-dse-analysis.md`, Section 4.5).

Either way, the typed decision is decoded in main before any dispatch. The
renderer never sees the raw model text as an instruction.

## 3. Availability gating

The router may only offer or pick an agent that the BOOT SEQUENCE reports as
`available`. The availability signal already exists and is a pure projection
over discovery state.

- `apps/openagents-desktop/src/renderer/boot-sequence.ts`:
  `BootSequenceStatus = "checking" | "available" | "unavailable"` (line 14).
  `projectBootSequenceAgents` (lines 40-96) computes a row for Codex, Claude
  Code, Grok, and Apple FM. The header comment states the rule: "an agent is
  'available' only when its lane reports it can actually run a turn" (lines 4-12).
- Codex and Claude status come from `state.harnessLanes.codex` / `.claude` via
  `laneStatus` (lines 28-33). `HarnessLaneAvailability` is the probe-verified
  readiness of the two built-in transports (`shell.ts`, lines 355-363).
- Grok and other ACP peers come from `state.providerLaneCapabilities`, whose
  `ProviderLaneComposerProjection` carries `admission` and `authentication`
  (`apps/openagents-desktop/src/provider-lane-capabilities.ts`, lines 63-80). The
  send-gate `selectableProviderLaneAvailable` requires `admission === "admitted"`
  and `authentication === "ready"` (`shell.ts`, lines 1728-1737).
- Apple FM status comes from `state.appleFmBoot` (`AppleFmBootState`,
  `shell.ts`, lines 370-374).

**How the router consumes it.** The host builds a small availability set from
these exact projected states, and feeds it into the router prompt or signature
as ground truth. The router's `delegate` output names one agent. The host then
rejects any `delegate` decision whose named agent is not `available` in that set,
and it degrades honestly: it does not silently answer as if it delegated, and it
does not claim an unavailable agent ran. The rejection message is the host's
own, not the model's. This keeps the deterministic lane and bridge discovery as
the sole authority for what can actually run, which is the boundary the Apple FM
analyzer audit fixes ("the structured lane/bridge discovery ... stays the sole
authority for what can actually run",
`docs/apple-fm/2026-07-20-apple-fm-analyzer-boot-sequence-audit.md`, Section 6.5).

## 4. The subagent status card (in chat)

The chat shows a typed status card for each dispatched subagent, with three
visible states, using the glyph vocabulary the app already ships.

### 4.1 State model

The card state is:

- `loading` — the subagent turn is running. Show a spinner, the agent name, and
  the bounded task summary, plus the latest progress line.
- `done` — the subagent turn completed. Show a green check.
- `failed` — the subagent turn failed or was refused. Show a red mark and a
  bounded reason.

This maps onto the shipped lifecycle-status vocabulary in
`packages/ui/src/workbench/activity-status.tsx` (lines 11-27): `running` renders
`CircleDot`, `completed` renders `Check`, `failed` renders `XCircle`, with the
labels "Running" / "Done" / "Failed". The BOOT SEQUENCE terminal vocabulary is
the monospace alternative: `available` is `✓`, `checking` is `…`, and
`unavailable` is `✗` (`apps/openagents-desktop/src/renderer/react-boot-sequence.tsx`,
lines 9-10). The status card reuses the workbench triad for the in-chat card
voice, and the BOOT SEQUENCE triad where the surface is the terminal-style scan.

One genuinely new part: there is no animated spinner glyph today. `running`
renders a static `CircleDot`, and the closest in-flight affordance is the
`oa-react-working` three-dot indicator (`react-timeline.tsx`, lines 1309-1311).
The card adds a small animated spinner for the `loading` state.

### 4.2 Where it renders

The chat timeline has a typed dispatch seam:

- `apps/openagents-desktop/src/renderer/react-timeline.tsx`: the row model is
  `ReactTimelineRecord` (lines 83-113), which carries an optional typed payload
  `item?: WorkbenchItem` (line 104). The render switch is `TimelineItem` (lines
  630-768). The typed extension point is the `dispatchableWorkbenchKinds` set
  (lines 390-393), whose design rule is that a new card "ships by editing ONLY
  its own `dispatch.tsx` branch, with zero further changes here" (lines 369-389).
- An agent card already exists: `record.item?.kind === "agent"` renders
  `DesktopAgentGroup` with per-child running/completed/failed status (lines
  650-679). `DesktopAgentGroup` rows already render `activityStatusIcon`
  (`packages/ui/src/workbench/agent-group.tsx`, line 56) and already embed a
  collapsible transcript (lines 66-69).

So the status card is a small extension, not a new subsystem. It either reuses
the existing `"agent"` branch, or it adds one new `WorkbenchItem` kind (for
example `"subagentDispatch"`) plus a dispatch branch. It renders the state model
from Section 4.1 and holds the `agentRef` that the right pane keys on.

## 5. The right-pane message chain

Clicking the card opens the right pane, which shows the subagent's message
chain. The right-pane infrastructure and an inspector already exist, and they
already match the owner's "click an agent, pop out a detail view" intent.

- The right pane is the `chat-context-pane` in `chatTranscriptArea`
  (`shell.ts`, lines 6223-6307). It is a `SplitPane` whose right rail
  (`rightRail`, lines 6271-6286) stacks the live agent graph and a message
  inspector. Its width is `chatContextWidth` state (line 569, default 336, line
  710), clamped 280 to 480, and Escape closes it (lines 6290-6294).
- The subagent detail viewer is `runtimeAgentGraphView`
  (`apps/openagents-desktop/src/renderer/runtime-agent-graph.ts`, lines 139-236).
  Clicking an agent row fires `DesktopAgentAction { kind: "inspect_agent",
  agentRef }` (lines 206-216) and expands `agentInspector` (lines 61-137).
  `agentInspector` renders a "Transcript" message chain (lines 81-114) with role
  labels, and detail fields including "Current action", "Attention", and
  "Terminal" (`runtimeAgentGraphDetailFields`, lines 28-41). This is the
  transcript-249 "agent_id / operation / task" popout, already implemented.
- The intent and state already exist:
  `DesktopAgentAction` with `inspect_agent` and `focus_agent`
  (`shell.ts`, lines 980-981), handler (lines 4125-4138), `selectedAgentRef`
  state (line 567). The in-chat child card `childCardMessage` (lines 5047-5099)
  already has an "Open delegated sub-agent" button that fires
  `inspect_agent` (lines 5073-5077), and the selected transcript is resolved by
  `delegateTranscriptForAgent` (lines 5106-5118).

### 5.1 What feeds the pane, and the redaction boundary

The pane must render the PUBLIC-SAFE, owner-only redacted trace, never the raw
event stream. Two data planes exist, and they must not be confused:

- **Raw event chunks are private, owner-only, and unredacted.** Completed Codex
  turns post the full SDK event stream through
  `packages/pylon-core/src/executor/codex-turn-reporter.ts` (turn reporter, line
  57; event-chunk reporter, line 112). The Cloud Run API stores the raw JSON in
  owner-scoped Cloud Storage with owner-only D1 metadata rows
  (`apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts`, raw
  store at line 1682, `visibility: 'owner_only'`). These raw chunks may contain
  prompts, command arguments, and output bytes. They are the archive, not a
  render source.
- **The redacted ATIF trajectory is the render source.** The same ingest route
  projects each turn into an ATIF trajectory, then validates, redacts, and
  tripwire-checks it before storage (`pylon-codex-turn-ingest-routes.ts`, lines
  2500-2564). The step projector `itemToAtifStep` (line 2080) captures agent
  messages, bounded reasoning summaries, tool-call LABELS, and observations that
  hold only `exitCode` and output-byte COUNTS — never raw command text or output
  content.

The ATIF trace schema is the exact message-chain shape a right pane renders:

- `packages/atif/src/trace-schema.ts`: `AtifStep` (line 57) has `source`
  (`user | agent | system`), `message`, `reasoning_content` (line 62),
  `tool_calls` with `function_name` and `arguments` (line 64), `observation`
  (line 65), and `metrics` with `prompt_tokens` / `completion_tokens` /
  `cost_usd` (line 66). The redaction tripwire rejects any step carrying
  `secret_material`, `wallet_or_payment_material`, `local_path`, `pii_email`, or
  `raw_provider_model_id` (finding codes, line 179).

So the right pane renders the redacted ATIF trajectory for the owner: the ordered
step chain of agent messages, reasoning summaries, tool-call labels, exit-code
and byte-count observations, and per-step token metrics. The raw chunks stay in
owner-scoped storage and never reach a public surface.

## 6. Reuse the real delegation loop

The desktop dispatch must reuse the existing provider-lane machinery, not invent
a parallel path. The machinery to spawn a real subagent turn already exists and
is already exposed to the renderer.

### 6.1 The real turn runner already exists and is renderer-wired

- The real local Codex turn is a `codex exec --json` run through the
  `codex-local` provider lane. `codexLocalLane.runTurn`
  (`apps/openagents-desktop/src/main.ts`, lines 4520-4550) runs the real
  execution. Dispatch flows through `dispatchCodexLocalTurn` →
  `laneDispatcher.dispatchTurn(codexLocalLane, ...)` (line 4880; `laneDispatcher`
  at line 4014).
- The renderer already reaches it: `codexLocal.start` over
  `CodexLocalStartChannel` (`main.ts`, lines 5237-5245) and the Claude
  equivalent `ClaudeLocalStartChannel` (lines 4227-4235). The renderer boot wires
  these lanes into the chat host
  (`apps/openagents-desktop/src/renderer/boot.ts`, line 1029 for
  `claudeLocal.start`, lines 1057-1064 for `codexLocal`).
- Today this path only runs when a caller sets `openAgentsStandby === false`
  (`shell.ts`, `providerPath = current.openAgentsStandby === false`, line 3012).
  On the default path (`openAgentsStandby !== false`, branch at line 3087) the
  submit handler routes to Apple FM or "Stand by." and starts no provider turn
  (lines 3092-3096).

**The router seam is exactly `shell.ts` lines 3092-3096.** Today:

```
const reply = withUser.appleFmBoot?.status === "available"
  ? yield* Effect.promise(() => appleFmChatHost.respond(...).catch(() => null))
  : null
const finalText = reply !== null && reply.trim() !== "" ? reply.trim() : "Stand by."
```

The router replaces this single on-device text turn with: run the router turn,
decode the typed route decision (Section 2), and on `delegate` invoke the same
`codexLocal.start` / `claudeLocal.start` surface that already exists, then render
the status card. On `answer`, keep today's behavior. This is a decision layer on
top of shipped plumbing, not new execution code.

### 6.2 The Pylon assignment loop, and the shortest first path

The full Pylon assignment lifecycle already models the delegated coding loop:

- `apps/pylon/src/codex-agent-executor.ts`: `runWithCodexSdk` (line 1009) runs
  one turn with `startThread` + `runStreamed` (lines 1109-1123), sandbox
  `danger-full-access` (line 113), approval `never` (line 114), a fresh thread
  per run with no resume. The no-spend own-capacity path is gated by an
  unforgeable process-local symbol (line 126).
- `apps/pylon/src/assignment.ts`: the assignment states (`offered | accepted |
  running | closed | rejected | ...`, lines 97-98), the finer progress states
  through `proof-ready` (line 145), the closeout state machine with
  `settlementState` and `payoutClaimAllowed` (lines 191-209), and the local
  `runNoSpendAssignment` path (line 1912).

A decisive package-boundary fact shapes the first path. OpenAgents Desktop
depends on `@openagentsinc/pylon-core` and `@openagentsinc/pylon-runtime`, not on
the `@openagentsinc/pylon` app. The Codex assignment executor lives only in the
`@openagentsinc/pylon` app, so it is not in-process reachable from the desktop
today. The Claude assignment executor is exported from `pylon-core`
(`packages/pylon-core/src/executor/index.ts`). Meanwhile the desktop already has
its own native local runtimes that spawn the vendor CLI and stream events:
`apps/openagents-desktop/src/codex-local-runtime.ts` (real `codex exec --json`,
which also supports resume) and `claude-local-runtime.ts`. Codex is the default
lane (`apps/openagents-desktop/src/full-auto-control-openapi.ts`, line 798).

**Shortest first path: Codex, through the desktop-native `codex-local` lane.**
The desktop already spawns real Codex turns and streams JSON events through that
lane, the account custody is already in-process
(`@openagentsinc/pylon-core/custody/account-registry`), and the owner-only
raw-event plus redacted-ATIF ingest pipeline already exists for Codex (Section
5.1). The router does not need the Pylon in-process assignment executor for the
first slice; it needs only the existing `codex-local` turn plus the existing
card and inspector. Reserve the Pylon in-process assignment executor route (where
Claude is currently the only wired option, since only the Claude executor ships
in `pylon-core`) for when the exact no-spend assignment and closeout state
machine is required rather than just a streamed turn.

## 7. The DSE / Blueprint layer

The router is a natural fit for a compiled DSE program governed by the Blueprint
authority frame. This layer is Phase 2; it does not block the Phase 1 MVP.

### 7.1 The router as a compiled DSE program

DSE (Declarative Self-Improving Effect) was a working, DSPy-inspired Effect
system: typed Effect Schema signatures, a `Predict` module, a bounded
deterministic compiler, metrics, budgets, and receipts. It was deleted from
`packages/dse` in commit `5afa49cdbc` under a Rust-only mandate, not for a
quality failure, and it is recoverable as design and regression-test evidence
(`docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md`, executive finding
and removal sequence). Any DSE use here is new code on the current Effect v4 /
Node 24 / pnpm / Vite Plus toolchain, not a cherry-pick.

Express the router as `TriageRoute.v1`:

- **Typed signature.** Input: the bounded user turn, the truncated conversation
  history, and the live availability set from Section 3. Output: the typed route
  decision from Section 2 (`answer` with text, or `delegate` with agent and
  task). This is a `DseSignature<I, O>` with Effect Schema input and output
  contracts, the same shape DSE used
  (`docs/dspy/2026-07-20-apple-fm-dse-analysis.md`, Section 4.3).
- **Metric.** Reward correct routing. Penalize BOTH failure modes seen by hand:
  a false delegation (the model claims or requests a delegate when the task was
  answerable locally, or names an unavailable agent) AND a false refusal (the
  model declines or stands by when it should have answered or delegated). Add a
  cost term, since a needless delegation is a needless cloud call. This two-sided
  metric is the exact correction the free-inference and DSE analyses call for.
- **Example and experience source.** Draw labeled routing examples from ATIF
  traces and their outcomes, where the eventual resolver (local answer, Codex, or
  Claude) is known. This is the MemoHarness per-case experience layer: the
  redacted ATIF trace store plus the exact token ledger already record each
  execution (`docs/research/2026-07-20-memoharness-openagents-integration-analysis.md`,
  Seam B). Consent defaults to withheld, so the first source is the owner's own
  runs for the owner's own benefit.
- **Offline compile, on-device serve.** The optimizer runs offline against the
  free on-device model. It searches instruction and few-shot candidates,
  evaluates each over the dataset, and emits one immutable compiled artifact. The
  desktop then serves the CHECKED-IN compiled artifact, not a runtime dependency
  on an optimizer server. Free inference makes the many rollouts cost wall-clock,
  not tokens (`docs/dspy/2026-07-20-apple-fm-dse-analysis.md`, Section 6). The
  DSE audit's corrections are mandatory here: the compiled id must cover the
  whole artifact, a missing holdout must fail, train must never become holdout,
  and each candidate must bind to an immutable dataset revision.

### 7.2 The Blueprint tie

Blueprint is the typed program governance and provenance spine, not an optimizer.
The current Blueprint chat Program runtime selects registered Program Signatures
and bounded tools, starts Codex or Claude session adapters, and records a Program
Run with an evidence-only authority boundary that denies direct deploy, spend,
email, and source mutation
(`docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md`, "current
implementation surface" and "chat Program runtime"). External effects must go
through a separate Action Submission approval boundary.

The composition the DSE audit records (lines 1250-1256) frames the router's
lifecycle:

1. A bounded offline optimizer produces a candidate router artifact.
2. The system records it as an unpromoted Module Version.
3. Program Runs and eval fixtures produce evidence and scorecards.
4. A Release Gate checks fixtures, review, policy, rollback, and receipts.
5. An authorized operator promotes the Module Version.
6. The online runtime serves the released router.
7. A router decision can propose an external effect only through an Action
   Submission.

So Blueprint gives the router its authority frame: the router's decision is
evidence, not authority; a compiled router cannot self-promote; and the actual
dispatch (which is an external effect) stays behind the host's deterministic
execution, exactly as Section 2 requires. This is deprecated prior-art authority,
used only for its evidence-only and release-gate pattern, not a revival of
Blueprint as a company brain.

## 8. Trust, authority, and privacy fence

The router inherits the shipped Apple FM fence, and it does not weaken it.

1. **Advisory router.** The local model's route decision is advisory. Its output
   is `estimated` by construction (honest `usageTruth`). It never sets
   acceptance, mints evidence, or decides delivery state
   (`docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md`, Section
   3).
2. **Host-executed dispatch.** The host, not the model, starts the real subagent
   turn and reports true status. The model proposes; the host acts and reports
   (Section 2).
3. **Owner-scoped.** The router and every trace it reads or writes stay in the
   single owner scope. One owner scope never targets or reads another owner's
   capacity or memory.
4. **Availability-gated.** A `delegate` decision that names an unavailable agent
   is refused honestly by the host (Section 3).
5. **Local-only Apple FM path.** Nothing from the router turn leaves the machine.
   The candidate search, the evaluation rollouts, and the dataset stay on-device
   or in the repository. No repo bytes, prompts, or summaries go to a network
   endpoint (`docs/apple-fm/2026-07-20-apple-fm-dse-analysis.md`, Section 5.3).
6. **Redacted traces.** The right pane renders only the redacted, owner-only ATIF
   trajectory. Raw event chunks stay in owner-scoped storage (Section 5.1).
7. **Non-determinism never becomes authority.** Model prose stays in the advisory
   lane. Every fact a gate depends on comes from the deterministic host oracle,
   the exact-preimage Git checks, and the exact token ledger, not the model's
   wording.
8. **A card that says "done" is never the completion receipt.** A subagent
   reporting success is not proof. The completion evidence is the exact
   `token_usage_events` rows, the redacted ATIF trace, and, where the assignment
   loop is used, the closeout proof with `settlementState` and
   `payoutClaimAllowed`. This reconciles with the repository's exact-evidence
   rules and ADR 0009 (count served tokens only from exact usage ledger rows).

## 9. Phased build plan

### Phase 0 — honest "can't do that" (today)

The current behavior is already honest. The strict preamble forbids false action
claims, and the default path answers locally or says "Stand by." with no provider
turn. Phase 0 is the baseline, not a task. It proves the honesty invariant the
router must preserve.

- Acceptance: the on-device model never claims it dispatched a subagent; the
  default path starts no provider turn (`shell.ts`, lines 3084, 3092-3096).

### Phase 1 — MVP: one real subagent, structured route, status card, right pane

Recommended agent: **Codex**, through the desktop-native `codex-local` lane
(Section 6.2). It is the default lane, it already streams real events to the
renderer, its account custody is already in-process, and its owner-only
raw-event plus redacted-ATIF ingest pipeline already exists. Claude is the
fallback if the design later requires the Pylon in-process assignment executor,
since only the Claude executor ships in `pylon-core`.

Build:

1. A main-owned route-decision schema and decoder (Section 2.1), with a
   fallback-to-answer on decode failure.
2. The router turn: reframe `buildOpenAgentsAppleFmPrompt` to ask for the typed
   route decision, feed it the availability set (Section 3), and decode in main.
3. The dispatch: on `delegate` with an available agent, invoke the existing
   `codexLocal.start` surface, and refuse an unavailable pick honestly.
4. The status card: add the `loading` / `done` / `failed` card to the timeline
   (Section 4), including the new animated spinner.
5. The right pane: reuse `DesktopAgentAction { inspect_agent }`, the right-rail
   `SplitPane`, and `agentInspector` to render the redacted ATIF trajectory
   (Section 5).

Files touched: `apps/openagents-desktop/src/renderer/shell.ts` (router seam
3092-3096, `buildOpenAgentsAppleFmPrompt`, availability set, card wiring),
`apps/openagents-desktop/src/renderer/react-timeline.tsx` (status card branch),
`packages/ui/src/workbench/activity-status.tsx` and `agent-group.tsx` (spinner),
`apps/openagents-desktop/src/renderer/runtime-agent-graph.ts` (inspector feed),
a new route-decision contract file next to `apple-fm-contract.ts`, and the
`codex-local` dispatch wiring in `main.ts` (lanes 4520-4550, 5237-5245).

Acceptance:

- The local model emits a typed route decision that main decodes; a malformed
  decision falls back to `answer` and dispatches nothing.
- A `delegate` decision for an available agent starts one real `codex-local`
  turn; a `delegate` for an unavailable agent is refused by the host with an
  honest message.
- The chat shows a `loading` card while the turn runs, then `done` or `failed`.
- Clicking the card opens the right pane with the subagent's redacted ATIF
  message chain; raw chunks never reach the renderer.
- The model never free-texts a dispatch claim.
- `pnpm run check` is green.

### Phase 2 — the DSE-compiled router and metric

Build the `TriageRoute.v1` DSE program (Section 7.1): the typed signature, the
two-sided metric, the ATIF-sourced example set, the offline optimizer, and the
checked-in compiled artifact. Serve the compiled artifact from the desktop, and
adopt the mature bridge's `@Generable` structured generation for the route
decision where present. Frame the lifecycle with the Blueprint evidence-only and
Release Gate pattern (Section 7.2).

Files touched: a new `packages/dse`-successor package on the current toolchain,
the desktop router seam (to load the compiled artifact), and the mature Swift
bridge structured-generation subset when restored.

Acceptance:

- The compiled router beats the hand-written prompt on a held-out set, measured
  by the two-sided routing metric, with honest `usageTruth`.
- The compiled id covers the whole artifact; a missing holdout fails; train is
  never holdout; each candidate binds to an immutable dataset revision.
- The compiled artifact is checked in and served on-device with no optimizer
  server dependency.

### Phase 3 — memory, multi-subagent, and generalized agents

Wire the MemoHarness memory substrate: the unwired TAS kit
(`apps/pylon/src/tas/`: `repo-memory`, `session-memory`, `context-assembly`,
`semantic-retrieval`) recalls a bounded, redacted slice into the router and
subagent prompts, sourced from the ATIF trace store
(`docs/research/2026-07-20-memoharness-openagents-integration-analysis.md`,
Seams A and C). Support multiple concurrent subagents (the transcript's "131
agents" surface), and generalize the router beyond Codex and Claude to Grok and
future agents through the same provider-lane and availability model.

Acceptance:

- Recalled memory improves routing or subagent quality, measured offline first,
  then in shadow, with no new authority and no new usage rows.
- Multiple subagent cards render concurrently, each with its own right-pane
  chain and honest per-agent state.
- The generalized router picks any `available` agent through one availability
  model, with no per-agent special-casing in the honesty or authority path.

## 10. Open questions and risks

- **Small-model routing quality.** A 3B model gives a shallower read than a
  frontier model. The router raises reliability; it does not lift the ceiling.
  Keep claims modest, measure the two-sided metric before widening scope, and
  keep the host's availability gate authoritative.
- **Structured output on the plain-text bridge.** Phase 1 parses JSON from a
  plain-text model, which a small model can malform. The decode-fail-to-answer
  fallback is mandatory. Phase 2's `@Generable` decode is the durable fix, and it
  is a growth path, not shipped today.
- **Two rendering worlds.** The in-chat card lives in the React timeline
  (`react-timeline.tsx`); the current detail pane lives in the Effect-Native
  shell (`shell.ts` / `runtime-agent-graph.ts`). Both are driven by the same
  `DesktopAgentAction` / `selectedAgentRef` state, but the cross-world seam is a
  real integration cost. Decide early whether the message-chain pane stays in the
  Effect-Native shell or moves to the React world.
- **No animated spinner today.** The `loading` state needs a new spinner; the
  shipped `running` glyph is static.
- **Live-D1/R2 versus Google-Cloud-authority tension.** The ATIF trace store and
  the Codex raw-event archive currently use the legacy D1 plus R2 path, while the
  app contract names Google Cloud as the sole production authority
  (`docs/research/2026-07-20-memoharness-openagents-integration-analysis.md`,
  Sections 2 and 6). Any durable router memory must resolve this before it
  persists.
- **Package boundary.** The Pylon Codex assignment executor is not in-process
  reachable from the desktop; only the Claude executor is in `pylon-core`. Phase
  1 avoids this by using the desktop-native `codex-local` lane. A later
  assignment-loop reuse needs an explicit dependency or package move.
- **Apple Silicon coverage.** Apple FM exists only on Apple Silicon with macOS
  26. The router must be a pure enhancement where present and must do nothing
  elsewhere. An Ollama or other-runtime fallback is a later, separate decision.
- **Non-determinism of the optimizer.** The compiled artifact must be
  reproducible enough to review, per the DSE audit corrections.

## 11. Summary

The seam is small and the plumbing exists. The router replaces one line of
behavior at `shell.ts` 3092-3096: instead of answering locally or standing by,
the local model emits a typed route decision, the host decodes it, and on
`delegate` the host starts a real `codex-local` turn through machinery the
renderer already reaches. The chat shows a status card built from the shipped
glyph vocabulary, and the right pane reuses the existing agent inspector to
render the redacted ATIF message chain. The honesty fix is structural: the model
proposes, the host acts and reports, so the "I dispatched a subagent"
hallucination cannot recur. The DSE-compiled router and MemoHarness memory are
later phases that raise quality without changing that authority boundary.

## 12. References

Source documents:

- `docs/transcripts/249.md` — owner product intent for the clicky subagent UI.
- `docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md` — what DSE was, the
  Blueprint relation, and the offline-compile / online-govern composition.
- `docs/dspy/2026-07-20-apple-fm-dse-analysis.md` — the `TriageRoute.v1` router
  idea, offline-compile / on-device-serve split, advisory fence.
- `docs/research/2026-07-20-memoharness-openagents-integration-analysis.md` — the
  memory substrate, the TAS kit, ATIF traces, and the token ledger.
- `docs/apple-fm/2026-07-19-apple-fm-swift-bridge-full-audit-and-openagents-desktop-plan.md`
  — bridge capabilities, the `@Generable` structured-generation growth path, and
  the November 2025 `FMOrchestrator` / concurrent-delegation precedent.
- `docs/apple-fm/2026-07-20-apple-fm-analyzer-boot-sequence-audit.md` — the
  analyzer, the availability projection, and the advisory-only trust fence.
- `docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md` — the free
  on-device thesis and the free-triage-that-routes pattern.

Code (availability and the router seam):

- `apps/openagents-desktop/src/renderer/boot-sequence.ts` — `BootSequenceStatus`
  (line 14), `projectBootSequenceAgents` (lines 40-96).
- `apps/openagents-desktop/src/renderer/shell.ts` —
  `buildOpenAgentsAppleFmPrompt` and the honesty preamble (lines 2229-2270), the
  router seam and `openAgentsStandby` branch (lines 3012, 3084-3108),
  `HarnessLaneAvailability` (lines 355-363), `AppleFmBootState` (lines 370-374),
  `selectableProviderLaneAvailable` (lines 1728-1737), `DesktopAgentAction` and
  `selectedAgentRef` (lines 567, 980-981, 4125-4138), `childCardMessage` and
  `delegateTranscriptForAgent` (lines 5047-5118), `chatTranscriptArea` right rail
  (lines 6223-6307).
- `apps/openagents-desktop/src/provider-lane-capabilities.ts` —
  `ProviderLaneComposerProjection` (lines 63-80).

Code (the boundary and the turn contract):

- `apps/openagents-desktop/src/apple-fm-contract.ts` — the bounded prompt request
  and text result (lines 92-109), `usageTruth` (line 57).
- `apps/openagents-desktop/src/apple-fm-host.ts` — `runTurn` bounded read-only
  turn (lines 217-256).

Code (the status card and the right-pane inspector):

- `apps/openagents-desktop/src/renderer/react-timeline.tsx` —
  `ReactTimelineRecord` (lines 83-113), `dispatchableWorkbenchKinds` (lines
  390-393), `TimelineItem` switch and the `"agent"` branch (lines 630-768,
  650-679), `oa-react-working` indicator (lines 1309-1311).
- `apps/openagents-desktop/src/renderer/react-boot-sequence.tsx` — the
  `✓` / `…` / `✗` glyphs (lines 9-10).
- `packages/ui/src/workbench/activity-status.tsx` — the
  `Check` / `XCircle` / `CircleDot` icons and labels (lines 11-27).
- `packages/ui/src/workbench/agent-group.tsx` — `DesktopAgentGroup` rows and the
  embedded transcript (lines 56, 66-69).
- `apps/openagents-desktop/src/renderer/runtime-agent-graph.ts` —
  `runtimeAgentGraphView` and `agentInspector` (lines 61-137, 139-236).

Code (the real delegation loop):

- `apps/openagents-desktop/src/main.ts` — `codexLocalLane.runTurn` (lines
  4520-4550), `CodexLocalStartChannel` (lines 5237-5245), `ClaudeLocalStartChannel`
  (lines 4227-4235), `laneDispatcher` (line 4014, dispatch at 4880).
- `apps/openagents-desktop/src/renderer/boot.ts` — renderer lane wiring (line
  1029, lines 1057-1064).
- `apps/openagents-desktop/src/codex-local-runtime.ts` and
  `claude-local-runtime.ts` — the desktop-native local turn runners.
- `apps/openagents-desktop/src/full-auto-control-openapi.ts` — the default
  `codex-local` lane (line 798).
- `apps/pylon/src/codex-agent-executor.ts` — `runWithCodexSdk` (line 1009),
  sandbox and approval constants (lines 113-114).
- `apps/pylon/src/assignment.ts` — the assignment and closeout state machine
  (lines 97-98, 145, 191-209), `runNoSpendAssignment` (line 1912).
- `packages/pylon-core/src/executor/codex-turn-reporter.ts` — the turn and
  event-chunk reporters (lines 57, 112).
- `apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts` — the
  raw-event store (line 1682) and the ATIF validate/redact/tripwire projection
  (lines 2080, 2500-2564).
- `packages/atif/src/trace-schema.ts` — `AtifStep` and the tripwire finding
  codes (lines 57, 179).

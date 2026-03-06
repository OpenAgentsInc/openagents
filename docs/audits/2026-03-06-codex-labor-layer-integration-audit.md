# 2026-03-06 Codex Labor-Layer Integration Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


## Scope

This audit evaluates how Codex is currently wired into OpenAgents, specifically against the intended role of the Labor layer described in:

- `README.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/README.md`
- `docs/kernel/labor-market.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/economy-kernel-proto.md`

Implementation surfaces reviewed:

- `crates/codex-client/src/*`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/reducers/codex.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/state/autopilot_goals.rs`
- `apps/autopilot-desktop/src/state/goal_loop_executor.rs`
- `apps/autopilot-desktop/src/state/goal_skill_resolver.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`

Repo search was also used to check whether desktop Codex flows currently invoke kernel authority operations directly. No such desktop integration was found.

## Executive Verdict

Codex is deeply integrated into Autopilot today, but it is not yet correctly integrated into the Labor layer as the root `README.md` and kernel docs define that layer.

Today Codex is primarily wired as:

1. the personal-agent chat engine,
2. the autonomous goal execution engine,
3. the local tool-calling orchestrator for desktop actions,
4. and, incorrectly for the intended long-term split, part of the current compute-provider execution path.

The current system therefore has a real Codex runtime integration, but only a partial and mostly local-prototype labor integration.

The key mismatch is structural:

- Codex is modeled in the app as `thread -> turn -> tool call -> transcript`.
- The Labor layer is modeled in the kernel docs as `WorkUnit -> Contract -> Submission -> Verdict -> Claim`.

That mapping is not explicit today. The desktop app records useful local receipts and audit trails, but it does not currently bind Codex runs to authoritative labor-market objects or backend kernel mutations.

If the intended product split is:

- `Ollama = Compute layer`
- `Codex = Labor layer`

then OpenAgents still needs a dedicated labor orchestration layer that wraps Codex execution inside kernel-shaped work, verification, liability, and settlement semantics. Right now Codex is a capable local runtime, not yet a fully compliant labor-market worker.

## Target Architecture From README And Kernel Docs

The docs are consistent on the intended separation of concerns:

- The Compute market allocates machine capacity.
- The Labor market buys and sells machine work.
- Compute powers labor, but compute and labor are not the same market.
- The kernel is the authority layer for contracts, verification, liability, settlement, policy, and receipts.
- The desktop app is a local runtime and projection surface, not the final authority for money, verdicts, or liability.
- Nostr and other sync/coordination layers are not authority lanes.
- `apps/autopilot-desktop` owns product orchestration; reusable crates should not absorb app-specific labor semantics.

For Codex specifically, `docs/MVP.md` already frames Autopilot as:

- a personal agent,
- a wrapper around Codex plus the user's machine,
- capable of ask -> plan -> execute -> report.

That places Codex naturally in the Labor side of the architecture, not in the Compute market itself.

The kernel docs go further: labor is only complete when work has explicit acceptance criteria, a submission, verification evidence, a verdict, and a settlement or remedy path. Turn completion alone is not enough.

## Current Codex Wiring

### 1. Codex has a strong app-level runtime integration

`crates/codex-client` and `apps/autopilot-desktop/src/codex_lane.rs` provide a substantial app-server integration:

- process spawning,
- JSON-RPC request/notification handling,
- model/account/config/MCP/apps/skills flows,
- thread and turn lifecycle management,
- approvals and tool-call response plumbing.

This integration is real and mature enough to be a production runtime control plane.

Important point: this layer is a Codex session/runtime integration, not a labor-market integration by itself.

It manages how Codex runs. It does not define what economic object a run represents.

### 2. The primary Codex unit in desktop is the chat turn

The main user-facing path is `run_chat_submit_action` in `apps/autopilot-desktop/src/input/actions.rs`.

That path:

- reads the composer prompt,
- requires an active Codex thread,
- classifies the turn for CAD behavior,
- records turn-submission metadata in `AutopilotChatState`,
- resolves and attaches skills,
- sets CWD and sandbox policy,
- queues `CodexLaneCommand::TurnStart`.

`AutopilotChatState` in `apps/autopilot-desktop/src/app_state.rs` stores:

- active thread id,
- active turn id,
- pending turn metadata,
- transcript messages,
- timeline events,
- plan/diff/token-use details,
- approval queues,
- tool-call queues.

This is a rich thread/turn execution model, but it is not a labor object model.

### 3. The closest existing Labor-like path is the autonomous goal loop

The strongest current bridge from Codex into something labor-shaped is the goal system in:

- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/state/goal_loop_executor.rs`
- `apps/autopilot-desktop/src/state/autopilot_goals.rs`

The flow is:

1. a user or system creates an autonomous goal,
2. `run_autonomous_goal_loop` ensures there is a Codex thread,
3. `goal_loop_prompt_for_goal` translates the goal into a natural-language instruction,
4. the app reuses the normal chat submit path,
5. the goal executor tracks each attempt by `thread_id`, `turn_id`, selected skills, and tool invocations,
6. `finalize_goal_loop_run` writes local execution and audit receipts.

This is the most labor-like thing in the current Codex stack because it turns a higher-level objective into bounded machine work and records outcomes.

But it is still not the kernel's labor model. It is a local autonomous control loop.

The evidence for that is visible in the receipt types:

- `GoalExecutionReceipt`
- `GoalRunAuditReceipt`
- `GoalAttemptAuditReceipt`
- `GoalToolInvocationAudit`
- `GoalPayoutEvidence`

These are useful local records, but they are app-owned and goal-centric, not authoritative `WorkUnit`, `Contract`, `Submission`, `Verdict`, or `Claim` records.

### 4. Codex is wired to local action execution through the tool bridge

`apps/autopilot-desktop/src/input/reducers/codex.rs` handles `CodexLaneNotification::ToolCallRequested` by routing OpenAgents-namespaced tools into `execute_openagents_tool_request` in `apps/autopilot-desktop/src/input/tool_bridge.rs`.

Current dynamic tools include:

- pane list/open/focus/close/set_input/action,
- CAD intent/action,
- swap quote/execute,
- treasury transfer/convert/receipt,
- goal scheduler control,
- wallet check,
- provider control.

This makes Codex more than a chat model. It makes Codex a local orchestrator that can cause real app-side actions and economic side effects.

That is an important labor signal, because a labor engine should be able to do work, not just speak.

But the current tool surface is still not labor-market-native. There are no explicit tools or typed flows for:

- create work unit,
- create contract,
- submit output,
- request verification,
- finalize verdict,
- open claim,
- resolve dispute.

So the current bridge is "Codex can drive desktop actions" rather than "Codex participates in labor-market state transitions."

### 5. Goal policy and command scoping exist, but they are still app-local policy

The tool bridge enforces command scope using active goal policy allowlists. Goal runs also record:

- selected skills,
- allowed command prefixes,
- allowed file roots,
- retry policy,
- stop conditions,
- wallet-delta progress,
- tool invocation history.

This is real policy, but it is still desktop-local control policy, not kernel authority policy.

The difference matters because the kernel docs require authoritative state transitions, explicit receipts, and settlement rules outside the desktop runtime.

### 6. Codex approvals exist, but the default posture is still local-agent-first

The lane supports:

- command approvals,
- file-change approvals,
- tool-call responses,
- user-input responses,
- auth refresh responses.

However `CodexLaneConfig::default()` currently sets:

- `approval_policy: Some(AskForApproval::Never)`
- `experimental_api: true`

That is a reasonable default for a local personal agent flow, but it is not a sufficient economic control surface for labor that carries contract, liability, or settlement consequences.

This is not a bug by itself. It is evidence that the current integration is still shaped like a local assistant runtime first.

### 7. Codex is still wired into the compute-provider lane

`apps/autopilot-desktop/src/input/reducers/jobs.rs` shows that the current active-job execution path still uses Codex:

- `queue_provider_execution_thread_start`
- `queue_provider_execution_turn_start`
- `provider_execution_prompt_for_active_job`
- `apply_active_job_codex_notification`

The current provider job flow:

1. starts a Codex thread,
2. starts a Codex turn,
3. sends a synthetic prompt describing the provider job,
4. captures the final Codex message as execution output,
5. advances job lifecycle based on Codex notifications.

This is the wrong shape for the intended architecture where Ollama powers Compute and Codex powers Labor.

It means Codex is currently more directly wired into Compute execution than into an explicit labor-market contract lifecycle.

That is the most important architectural contamination to remove.

### 8. The repo already has kernel-shaped authority surfaces, but Codex does not use them yet

`crates/openagents-kernel-core/src/authority.rs` already defines:

- `create_work_unit`
- `create_contract`
- `submit_output`
- `finalize_verdict`
- `get_snapshot`

`apps/nexus-control/src/lib.rs` already exposes hosted endpoints for those mutations.

At the same time, `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs` contains a large local prototype for:

- work-unit metadata,
- provenance hints,
- verification tiers,
- policy context,
- incidents,
- audit linkage,
- local kernel-style receipts.

So the repo already contains both:

- a kernel-shaped vocabulary,
- and a thin backend authority slice.

But the desktop Codex paths do not currently bind to those surfaces. A repo search found no desktop Codex flow calling `create_work_unit`, `create_contract`, `submit_output`, or `finalize_verdict`.

That means current Codex labor behavior is observed locally but not yet elevated into kernel authority.

## What The Current System Actually Maps To In Labor Terms

If we force the current Codex integration into the kernel's labor vocabulary, the mapping looks like this:

| Kernel labor object | Current Codex-era equivalent | Status |
| --- | --- | --- |
| `WorkUnit` | User prompt, autonomous goal objective, or provider job input; plus some local work-unit metadata in `earn_kernel_receipts` | `partial`, local prototype only |
| `Contract` | Goal constraints, retry policy, command scope, sandbox/cwd, selected skills, quoted job parameters | implicit only; not a first-class authoritative object |
| `Submission` | Final turn output, tool invocation log, transcript, local artifacts, job execution output | implicit only; not explicitly modeled as a submission object |
| `Verdict` | Turn completion status, job lifecycle stage, or local goal success/failure | not compliant with kernel definition of verified outcome |
| `Claim` | None | missing |

That table is the core audit result.

There is enough local structure to infer a future labor implementation path, but the labor object model itself is still mostly absent from current Codex execution.

## Main Gaps Against The Intended Labor Layer

### 1. No explicit WorkUnit binding for economically meaningful Codex runs

Economically meaningful Codex runs are not currently wrapped in first-class `WorkUnit` objects with:

- acceptance criteria,
- verification budget hint,
- severity,
- time-to-feedback class,
- provenance requirements,
- trace ids recognized by kernel authority.

Today those concerns are either prompt text, goal metadata, or local receipt hints.

### 2. No explicit Contract before Codex execution begins

The kernel docs require a contract lifecycle that binds:

- work,
- verification plan,
- settlement rules,
- warranty/liability posture,
- parties and obligations.

Current Codex runs do not create such a contract before execution begins.

The app sets local execution conditions like sandbox and skill attachments, but that is not the same as a kernel contract.

### 3. Turn completion is being treated too much like success

Current flows often treat:

- final assistant output,
- turn completion,
- local lifecycle transition,
- or payout evidence

as the practical terminal event.

The labor docs say that is insufficient. Labor settles against verified outcomes, not merely completed execution.

Today there is no generalized, explicit verifier step bound to Codex labor runs.

### 4. No Claim or remedy path for Codex labor outcomes

The kernel model includes:

- liability,
- warranty windows,
- claims,
- dispute resolution,
- remedies.

Current Codex goal receipts and tool logs do not implement that lifecycle.

This makes the current integration useful for local automation but not yet compliant as a labor market.

### 5. Desktop-local receipts are richer than desktop authority, but still only local

`earn_kernel_receipts.rs` is evidence that the repo already understands the kernel concepts. But the desktop's local receipt model is still not the authoritative economic source of truth that `README.md` calls for.

That gap becomes especially important once Codex work is paid, disputed, or underwritten.

### 6. Codex thread and turn ids are being used as de facto execution anchors

Thread ids and turn ids are currently the main identifiers that bind together:

- prompt submission,
- tool execution,
- transcripts,
- goal attempts,
- provider job execution.

Those are useful provenance handles, but they should not be the authority objects for labor.

They should become evidence attached to kernel work objects, not replacements for them.

### 7. The current tool namespace is too app-centric for labor contracts

Most current tools are about:

- UI control,
- CAD actions,
- wallet/swap actions,
- provider state,
- treasury operations.

That is helpful for operator workflows, but it does not give Codex a typed labor contract interface.

As long as the tool layer remains mostly app-centric, labor semantics will stay implicit.

### 8. Codex is still entangled with the compute-provider loop

This is both a product and architectural problem.

If Compute is going to be powered by Ollama and Labor by Codex, then Codex must stop being the hidden execution backend for the NIP-90 compute provider flow.

Right now the boundary is blurry:

- the goal/autonomy system is labor-like,
- but the provider execution engine is also Codex-driven.

That makes it hard to reason truthfully about which market a given Codex run belongs to.

## Recommendations For A Correct Codex-to-Labor Integration

### 1. Introduce an explicit Codex labor orchestrator

Add an app-owned labor orchestration layer in `apps/autopilot-desktop` that sits above `codex_lane` and below product flows.

Its job should be to translate:

- user requests,
- autonomous goals,
- or marketplace work

into explicit labor-market state transitions.

This layer should own the mapping from:

- Codex `thread/turn/tool-call`
- to kernel `WorkUnit/Contract/Submission/Verdict/Claim`

The important boundary is that `codex_lane` remains a runtime transport, while labor orchestration becomes a higher-level app concern.

### 2. Create `WorkUnit` objects only for economically meaningful Codex runs

Not every personal-agent chat should become a kernel work unit.

Recommended split:

- casual/private local chat remains a local Codex thread,
- economically meaningful or explicitly delegated work becomes a `WorkUnit`,
- autonomous earning flows and future labor-market tasks should always bind to a `WorkUnit`.

This keeps the app usable while still respecting the kernel model when money, verification, or liability matter.

### 3. Create a `Contract` before execution, not after

Before Codex begins economically relevant work, create a contract that includes at minimum:

- work-unit id,
- worker identity,
- pricing terms,
- verification tier,
- provenance requirements,
- command/file scope,
- allowed tools,
- output schema or acceptance criteria,
- warranty or claim posture,
- idempotency key and trace context.

This should become the canonical execution envelope for labor, rather than treating thread state as the implicit contract.

### 4. Treat Codex thread/turn data as provenance, not authority

Codex should emit evidence into the labor system, not replace it.

For each economically relevant run, collect a provenance bundle containing:

- Codex thread id and turn id,
- prompt hash,
- model id,
- selected skills,
- sandbox policy,
- cwd,
- approval events,
- tool invocations,
- produced artifacts and hashes,
- final output,
- transcript digest or transcript attachment reference.

That bundle should be attached to `Submission` and `Verdict` evidence, not used as the primary economic state itself.

### 5. Add an explicit `Submission` phase for Codex labor

A Codex run should not jump from "turn completed" to "done."

Instead:

1. Codex execution produces a candidate output.
2. The app assembles a `Submission`.
3. Submission evidence includes tool activity and artifacts.
4. Verification policy decides what checker path is required.

This is the minimum structural change needed to align Codex labor with the kernel docs.

### 6. Add a real verifier path and make verdicts independent of worker completion

The kernel docs are explicit that verified outcome quality and verification independence matter.

Codex labor therefore needs a verifier stage that is separate from the worker run. Depending on the work type, that verifier may be:

- a deterministic harness,
- a second model family,
- a different Codex/checker policy lane,
- a human review step,
- or a hybrid path.

Important rule: a worker turn finishing is not a verdict.

The verifier should produce a `Verdict` with:

- verification tier,
- evidence refs,
- independence/correlation notes,
- pass/fail outcome,
- any warranty or holdback decision.

### 7. Move authoritative labor mutations to backend kernel authority

To comply with `README.md`, authoritative labor state changes should happen through backend authority, not inside the desktop-only state graph.

That means Codex labor flows should eventually call backend authority for:

- `create_work_unit`
- `create_contract`
- `submit_output`
- `finalize_verdict`

The desktop should remain:

- execution runtime,
- UI projection,
- provenance collector,
- local cache of receipts and state.

It should not be the final authority for paid labor outcomes.

### 8. Keep Compute and Labor strictly separated

With the new desired split:

- Ollama should execute Compute-market inference jobs.
- Codex should execute Labor-market machine work.

That means the current provider-job Codex execution path in `apps/autopilot-desktop/src/input/reducers/jobs.rs` should be retired from the compute-provider loop.

Codex may still consume compute, but it should do so as a labor worker using compute, not as the compute market itself.

### 9. Add labor-domain tools instead of relying only on app-domain tools

If Codex needs app cooperation for labor flows, add typed labor tools such as:

- fetch active work unit scope,
- list required artifacts,
- upload artifact evidence,
- mark submission ready,
- request verifier run,
- inspect contract acceptance criteria,
- open claim,
- attach incident evidence.

The current pane/treasury/provider tools are useful, but they are not enough to make labor semantics first-class.

### 10. Bind goal receipts to kernel object ids

The current goal system is a good local prototype. It should not be thrown away.

Instead, evolve it so that:

- `goal_id` maps to one or more `work_unit_id`s,
- goal attempts reference `contract_id`,
- tool invocations become submission evidence,
- payout evidence links to settlement or warranty receipts,
- run-audit receipts include kernel ids and verdict refs.

That would preserve the useful local autonomy UX while making it compatible with the kernel's labor model.

### 11. Add claim, incident, and remedy support for Codex labor

The labor docs require a path for:

- failed delivery,
- disputed quality,
- post-settlement defects,
- policy-triggered remedies.

Codex labor integration should therefore include:

- claim opening,
- evidence attachment,
- adjudication status,
- remedy or compensation result,
- linkage back to the originating work unit and contract.

Without this, Codex can execute work, but it still cannot participate in the full labor economy described by the kernel docs.

### 12. Keep Nostr and desktop sync in the projection lane, not the authority lane

This is an inference from the root docs and kernel docs, but it is a strong one:

If labor jobs are projected or coordinated over Nostr later, that projection should not become the source of truth for contract, verdict, or settlement state.

The authoritative labor state should remain backend-kernel-backed.

Desktop and relay projections should mirror that state, not replace it.

### 13. Make the UI explicit about Personal Agent mode versus Market Labor mode

The current UX mixes:

- local personal-agent turns,
- autonomous local goals,
- provider earn state,
- wallet actions,
- job history.

That is workable for MVP, but the future Codex labor integration will be cleaner if the UI distinguishes:

- private/local Codex assistance,
- autonomous goal automation,
- paid labor contracts,
- compute-provider jobs.

Truthful labeling matters here because each mode has different authority, verification, and settlement semantics.

## Recommended Implementation Sequence

### Phase 1: finish the market split

1. Remove Codex from the NIP-90 compute-provider execution path.
2. Keep Ollama as the Compute execution backend.
3. Preserve Codex for personal-agent and future labor flows only.

### Phase 2: introduce local labor binding for Codex

1. Add a labor-orchestrator module in `apps/autopilot-desktop`.
2. Bind economically meaningful Codex runs to local `work_unit_id` and `contract_id`.
3. Convert turn completion into a local `Submission` assembly step.
4. Attach provenance bundles and tool evidence.

### Phase 3: connect labor binding to backend authority

1. Add desktop client calls to kernel authority endpoints.
2. Make authoritative work-unit and contract creation backend-backed.
3. Make verdict finalization backend-backed.
4. Keep local receipts as projections and caches.

### Phase 4: add verification and liability

1. Add verifier assignments and verification receipts.
2. Add claim/dispute flows.
3. Separate execution price from liability and warranty posture.
4. Reflect withheld settlement and remedy outcomes honestly in UI and receipts.

## Likely Touch Points For Future Implementation

- `apps/autopilot-desktop/src/codex_lane.rs`
  - keep as runtime/session lane, not labor authority
- `apps/autopilot-desktop/src/input/actions.rs`
  - identify which turns become labor work units
- `apps/autopilot-desktop/src/input.rs`
  - goal loop should bind attempts to work-unit and contract ids
- `apps/autopilot-desktop/src/input/reducers/codex.rs`
  - convert turn/tool outcomes into submission/provenance events
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
  - add labor-domain tools and evidence helpers
- `apps/autopilot-desktop/src/state/autopilot_goals.rs`
  - extend local receipts with kernel object linkage
- `apps/autopilot-desktop/src/state/goal_loop_executor.rs`
  - track labor ids and submission/verdict state
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
  - reuse receipt vocabulary for labor provenance and verification linkage
- `crates/openagents-kernel-core/src/authority.rs`
  - likely remains the canonical trait surface
- `apps/nexus-control/src/lib.rs`
  - likely remains the first hosted labor authority endpoint surface

## Final Assessment

Codex is already the most important local execution engine in OpenAgents, and it is already closest to the Labor side of the product in spirit. But in implementation terms, it is still mainly a local runtime plus a goal-and-tool orchestrator.

OpenAgents already has the ingredients for a proper Codex Labor integration:

- a strong Codex runtime lane,
- local autonomous goal receipts,
- kernel-shaped receipt vocabulary,
- and thin backend authority endpoints.

What is missing is the explicit binding layer that turns Codex work into kernel-authoritative labor objects.

Until that layer exists, Codex is best described as:

- well integrated into the app,
- partially integrated into local labor-like flows,
- not yet fully integrated into the Labor market defined by the root `README.md` and `docs/kernel/`.

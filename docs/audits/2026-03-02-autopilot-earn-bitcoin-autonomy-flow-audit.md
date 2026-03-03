# 2026-03-02 Autopilot "Earn Bitcoin On Autopilot" Flow Audit

## Scope

This audit evaluates the requested flow:

1. User directs Autopilot from `Autopilot Chat` to earn bitcoin automatically.
2. Relevant skills from the OpenAgents `skills/` registry are loaded into Codex context.
3. Codex loops autonomously until a condition is met (for example: wallet increases in sats), or until another user-defined goal condition is met.
4. Flow supports scheduled execution, including local cron-like behavior where appropriate.

Primary authority references:

- `docs/MVP.md` (product requirements and acceptance criteria).
- `docs/OWNERSHIP.md` (ownership boundaries).

Implementation references in this audit point to current repo code paths and line ranges.

## MVP Alignment Check

The requested flow is directly aligned with the MVP product promise and acceptance criteria:

- MVP target is explicit money loop: go online -> paid job -> wallet increases -> withdraw (`docs/MVP.md:20-23`, `docs/MVP.md:264-270`).
- MVP requires Autopilot as a real local agent and provider runtime (`docs/MVP.md:146-178`).
- MVP allows seed demand as a path to first earnings (`docs/MVP.md:130-138`).

Conclusion: this requested flow is in-scope for MVP and should be treated as core product work, not side work.

## Current Capability Snapshot

## What Exists

- Local skills registry exists and is structured for discovery and manifest derivation (`skills/README.md:31-34`, `apps/autopilot-desktop/src/skills_registry.rs:30-137`).
- Codex skill discovery is wired through `skills/list` and uses repo skill roots (`apps/autopilot-desktop/src/input/reducers/skl.rs:76-110`, `apps/autopilot-desktop/src/input/reducers/codex.rs:366-399`).
- Skill enable/disable toggle through `skills/config/write` exists (`apps/autopilot-desktop/src/input/reducers/skl.rs:145-190`).
- Chat turn input supports skill attachments (`UserInput::Skill`) (`apps/autopilot-desktop/src/input/actions.rs:285-334`).
- Spark wallet worker is real integration (refresh/balance/invoice/send/list payments) (`apps/autopilot-desktop/src/spark_wallet.rs:18-36`, `apps/autopilot-desktop/src/spark_wallet.rs:232-265`, `apps/autopilot-desktop/src/spark_wallet.rs:460-492`).

## What Is Missing For Requested Flow

- No generic "goal runner" or "loop until condition met" engine is implemented in Autopilot chat path.
- Skill auto-selection is currently specific to CAD policy, not generalized relevance matching for earning goals.
- No first-class BTC <-> stablesat USD swap primitive is exposed in the current agent tool/skill surface.
- No real local cron / persistent scheduler integration is implemented in app runtime.
- Go Online / SA/AC/network request/job paths are simulated state machines today, not a real end-to-end paid job settlement lane.
- Non-OpenAgents tool calls are not auto-executed by desktop tool bridge, so general autonomous external workflows can stall.

## Detailed Flow Audit

## 1) Skill Loading Into "Codex Brain"

### Current behavior

- Skill discovery uses Codex `skills/list` with `per_cwd_extra_user_roots` injected from repo and managed skill roots (`apps/autopilot-desktop/src/input/reducers/skl.rs:93-107`, `apps/autopilot-desktop/src/skill_autoload.rs:58-74`).
- Discovered skills are stored with `name/path/scope/enabled/dependencies` (`apps/autopilot-desktop/src/input/reducers/codex.rs:838-878`).
- Turn assembly attaches selected skill(s), deduped and ordered (`apps/autopilot-desktop/src/input/actions.rs:231-335`).

### Gap

- Relevance-based attachment is not generalized. Current policy auto-attaches only CAD skills (`apps/autopilot-desktop/src/input/actions.rs:248-269`, `apps/autopilot-desktop/src/skill_autoload.rs:10-29`).
- This does not satisfy "use any relevant skills" for earning workflows.

### Impact

- User must manually select skill(s) or rely on CAD-only policy.
- Autopilot cannot reliably self-compose a multi-skill plan for earning tasks.

## 2) Autopilot Chat -> Autonomous Loop Until Condition

### Current behavior

- Chat submit queues a single `turn/start` request; no built-in iterative goal loop state machine (`apps/autopilot-desktop/src/input/actions.rs:3-133`).
- There is no explicit condition evaluator tied to chat turns for "stop when wallet increased by X sats" or similar.

### Gap

- No first-class model for:
  - objective definition,
  - progress checkpoints,
  - retry policy,
  - stop conditions,
  - abort conditions/timeouts.

### Impact

- Requested "loop until earn bitcoin" behavior is not currently delivered by architecture.

## 3) Scheduling / Cron / Always-On Automation

### Current behavior

- SA pane can apply heartbeat seconds and publish manual tick (`apps/autopilot-desktop/src/input/reducers/sa.rs:178-229`).
- SA lane heartbeat/tick loop is in-process and simulated (`apps/autopilot-desktop/src/runtime_lanes.rs:510-571`).
- WGPUI has a generic schedule component with `Cron` type, but it is not wired into `apps/autopilot-desktop` runtime (`crates/wgpui/src/components/organisms/schedule_config.rs:10-17`, `crates/wgpui/src/components/organisms/schedule_config.rs:73-79`, `crates/wgpui/src/components/organisms/schedule_config.rs:120-123`).

### Gap

- No app-level cron expression parser/executor for goals.
- No OS scheduler integration (`launchd`, `crontab`, `systemd`) in desktop runtime path.
- No persisted scheduled goal registry with recovery on restart.

### Impact

- User-requested "set up local crons or whatever else" is not implemented.

## 4) Earning Loop (Go Online -> Job -> Paid -> Wallet Increase)

### Current behavior

- Go Online toggles SA runner state and refreshes wallet (`apps/autopilot-desktop/src/input.rs:1446-1478`).
- SA/AC lanes are local simulated command handlers that generate synthetic event IDs (`apps/autopilot-desktop/src/runtime_lanes.rs:574-727`, `apps/autopilot-desktop/src/runtime_lanes.rs:1003-1272`).
- Network request submission and starter job completion are local state operations (`apps/autopilot-desktop/src/input/actions.rs:1850-2051`, `apps/autopilot-desktop/src/state/operations.rs:323-445`, `apps/autopilot-desktop/src/state/operations.rs:494-537`).
- Starter completion sets synthetic payout pointer `pay:<job_id>` and updates history (`apps/autopilot-desktop/src/state/operations.rs:525-536`, `apps/autopilot-desktop/src/input/actions.rs:2005-2042`).
- Scoreboard lifetime sats is sourced from Spark balance, but sats_today/jobs_today are sourced from local job-history rows (`apps/autopilot-desktop/src/app_state.rs:2537-2595`).

### Gap

- End-to-end paid-job authority path is still largely simulated in the desktop runtime lanes.
- Scoreboard can combine authoritative wallet data with locally synthesized history entries.
- Starter jobs do not currently have a runtime lane feeding real queued jobs by default.

### Impact

- Requested autonomous "earn bitcoin" cannot be considered authoritative end-to-end yet.

## 5) Tooling Surface For Autonomous Execution

### Current behavior

- Dynamic tools exposed to Codex are OpenAgents pane/CAD tools only (`apps/autopilot-desktop/src/openagents_dynamic_tools.rs:13-22`, `apps/autopilot-desktop/src/openagents_dynamic_tools.rs:24-128`).
- Tool bridge allowlist enforces OpenAgents namespace (`apps/autopilot-desktop/src/input/tool_bridge.rs:48-50`, `apps/autopilot-desktop/src/input/tool_bridge.rs:135-184`).
- Non-OpenAgents tool calls are not auto-executed by tool bridge path (`docs/PANES.md:243-244`, `apps/autopilot-desktop/src/input/reducers/codex.rs:1702-1707`).

### Gap

- No first-class "goal automation tools" for scheduler setup, condition checks, and controlled system operations.
- If broader tools are needed, behavior is fragmented between Codex server request pathways and pane controls.

### Impact

- Limits reliability of unattended workflows spanning wallet + network + OS scheduling tasks.

## 6) Safety Posture For Full Autonomy

### Current behavior

- Chat turn flow uses `AskForApproval::Never` and `DangerFullAccess` sandbox policy (`apps/autopilot-desktop/src/input/actions.rs:271-283`, `apps/autopilot-desktop/src/input/actions.rs:99-111`).
- This no-approval/full-autonomy mode is the intended operating mode for unattended execution.

### Gap

- No explicit per-goal risk policy or spend/command guardrails for autonomous loops.
- No budget/kill-switch model tied to autonomous earnings workflows.

### Impact

- No-approval autonomy increases blast radius if guardrails are missing, especially for local commands/file changes.

## Relevant Skills For "Earn Bitcoin On Autopilot"

Skills present in registry include Bitcoin/payment-focused entries (`skills/README.md:7-16`):

- `blink`: practical Lightning wallet operations via Blink API (`skills/blink/SKILL.md:18-37`).
- `l402`: Lightning paywall/commerce workflows (`skills/l402/SKILL.md:20-40`).
- `moneydevkit`: Lightning checkout + agent wallet flows (`skills/moneydevkit/SKILL.md:19-49`).
- `neutronpay`: MCP/SDK payment workflows (`skills/neutronpay/SKILL.md:20-52`).

Assessment:

- These are relevant for payment and monetization workflows.
- Current runtime lacks generic skill relevance/selection/orchestration logic to apply them autonomously from chat intent.

## BTC <-> Stablesat USD Swap Findings (Blink + stablesats-rs)

Confirmed from `/Users/christopherdavid/code/blink/stablesats-rs`:

- `stablesats-rs` has an explicit quote/execute conversion surface via gRPC:
  - `GetQuoteToBuyUsd`
  - `GetQuoteToSellUsd`
  - `AcceptQuote`
  - plus `immediate_execution` support (`proto/quotes/quote_service.proto`, `quotes-server/src/server/mod.rs`, `quotes-server/src/app/mod.rs`).
- Accepted quotes are posted into ledger templates for both directions (`ledger/src/templates/buy_usd_quote_accepted.rs`, `ledger/src/templates/sell_usd_quote_accepted.rs`).
- The design intent is explicit BTC/USD liability and hedging management rather than ad-hoc conversion math (`README.md`).

Confirmed from OpenAgents Blink skill (`skills/blink`):

- Partially covered today:
  - BTC and USD wallet balances/invoices/payments (`create_invoice.js`, `create_invoice_usd.js`, `pay_invoice.js`, `pay_lnaddress.js`, `pay_lnurl.js`).
  - Price conversion estimation (`price.js` and `currencyConversionEstimation` docs in `SKILL.md`).
- Missing today (not first-class):
  - No dedicated BTC <-> USD wallet swap command/script.
  - No explicit quote -> accept swap flow.
  - No direct wrappers for intraledger conversion-style mutations in skill scripts.

Conclusion: BTC <-> stablesat USD is only partially covered in the current Blink skill and needs explicit first-class support in both skill/tooling and goal-runner planning.

## Critical Findings (Ranked)

1. **Critical:** No authoritative autonomous earn loop yet (state lanes are mostly simulated for SA/AC/network/job lifecycle).
2. **Critical:** No generic objective/condition loop engine for "until X is true" in chat-driven automation.
3. **High:** No first-class BTC <-> stablesat USD swap operation surface, despite this being a core earning/treasury control needed for autopilot money strategies.
4. **High:** Skill loading exists, but relevance-based multi-skill orchestration is CAD-specific and insufficient for earnings workflows.
5. **High:** No local cron/persistent scheduler integration for unattended execution.
6. **Medium:** Tool surface is pane/CAD-centric; broader autonomous workflows need clearer controlled execution interfaces.

## Recommended MVP Architecture (Scoped To Existing Ownership)

Ownership-compliant placement (`docs/OWNERSHIP.md:13-25`, `docs/OWNERSHIP.md:26-38`):

- Put product workflow logic in `apps/autopilot-desktop`.
- Keep `crates/wgpui` UI-generic only.
- Keep wallet primitives in `crates/spark`.

Proposed additions:

1. `AutopilotGoalRunner` (app layer):
   - Stores objective, constraints, schedule, attempt state, stop conditions, and receipts.
2. `SkillRelevanceResolver` (app layer):
   - Maps goal/task types to candidate skills from Codex-discovered registry.
   - Supports ordered multi-skill attachment per turn.
3. `ConditionEvaluator` (app layer):
   - Evaluates wallet delta, job success count, timeout, error budget, and user-defined conditions.
4. `Scheduler` (app layer):
   - MVP: interval/manual schedules.
   - Next: cron expression + optional OS-backed persistence (platform adapters).
5. `Authoritative Earnings Gate`:
   - Mark "goal success" only when wallet/payment evidence confirms sats received (not from synthetic state only).
6. `Swap Executor`:
   - First-class BTC <-> stablesat USD quote + execute pipeline with policy limits and receipts.
7. `Safety Policy`:
   - Goal-level permissions, spend caps, command/file policies, kill switch, and max runtime.

## Proposed Implementation Phases

## Phase 1 (MVP-viable autonomy)

1. Add goal runner state + UI pane for:
   - objective,
   - condition,
   - schedule (manual/interval),
   - budget caps.
2. Implement skill relevance resolver for payment/earn goals:
   - start with `blink`, `l402`, `moneydevkit`, `neutronpay`,
   - attach top-N enabled skills automatically.
3. Add first-class BTC <-> stablesat USD swap primitives (quote, execute, verify) to the automation tooling path.
4. Implement loop executor:
   - submit turn,
   - inspect outcomes,
   - re-run until condition met/timeout/error budget.
5. Gate completion on wallet-confirmed earning signal.

## Phase 2 (scheduler hardening)

1. Add cron expression support in app runtime.
2. Add optional OS scheduler persistence adapters.
3. Add restart recovery and missed-run semantics.

## Phase 3 (safety + operability)

1. Add explicit policy profiles for no-approval autonomous mode.
2. Add incident/audit receipts for each autonomous run.
3. Add deterministic tests for stop conditions and payout verification.

## Sequenced GitHub Issues Needed For Full Implementation

The following is the full recommended GitHub issue sequence to implement this flow end-to-end. The sequence is dependency-ordered, and each issue title is ready to use as a GitHub issue name.

1. **[Epic] Autopilot Goal Automation: Earn Bitcoin Until Condition Met**
   Create the umbrella issue that tracks scope, dependencies, and acceptance gates for the entire feature set in this audit. This issue should define the MVP bar as: chat-directed goal, autonomous loop, authoritative payout verification, and scheduler support. All issues below should link back to this epic.

2. **Define Goal Spec and Persistence Model in `apps/autopilot-desktop`**
   Add typed models for objective, constraints, stop conditions, retry policy, schedule config, and execution receipts. Persist active and historical goals so runs survive app restarts. Keep all workflow ownership in app-layer state per `docs/OWNERSHIP.md`.

3. **Add Goal Runner State Machine (Queued/Running/Succeeded/Failed/Aborted)**
   Implement a first-class runtime state machine for autonomous goals, including attempt counters and terminal state semantics. This issue should provide deterministic transitions and explicit failure reasons so the UI and logs can stay honest. It should not yet execute chat turns until orchestration wiring lands.

4. **Implement Condition Evaluator for Wallet Delta, Job Count, Timeout, and Error Budget**
   Build a reusable evaluator that can answer "should continue?" and "is goal complete?" after every attempt. Include support for user-defined thresholds such as "wallet increases by N sats." Ensure conditions are evaluated from authoritative sources, not optimistic UI state.

5. **Add Authoritative Earnings Verification Gate (Wallet/Payment Evidence Only)**
   Implement a strict success gate that marks earnings objectives complete only when Spark/payment evidence confirms funds were received. Disallow synthetic starter-job records from satisfying completion conditions. Emit explicit mismatch errors when local job history and wallet evidence diverge.

6. **Define BTC <-> Stablesat USD Swap Contract + Policy Model**
   Add app-layer domain types for swap direction, amount units (sats/cents), quote TTL, immediate execution mode, slippage/fee limits, and failure semantics. Include policy fields for max per-swap and max per-day converted value. This establishes a deterministic contract for all swap operations used by autonomous goals.

7. **Implement Blink Skill First-Class Swap Scripts (Quote + Execute, Both Directions)**
   Extend `skills/blink` with explicit swap scripts that support BTC -> USD and USD -> BTC flows, including dry-run quote and execution modes. Prefer direct wallet-native Blink operations (including intraledger conversion pathways where available) instead of fragile multi-step workarounds. Return structured JSON receipts with pre-balance, post-balance, quote terms, and execution status.

8. **Add Stablesats Quote Adapter Path (`GetQuoteToBuyUsd` / `GetQuoteToSellUsd` / `AcceptQuote`)**
   Add an adapter for quote-based execution using stablesats quote semantics and `immediate_execution` handling, with explicit fallback behavior when this path is unavailable. This issue ensures parity with how stablesats models conversion and acceptance. Persist quote IDs and expiration metadata for audit and replay.

9. **Wire Swap Operations Into Autopilot Tool Bridge and Goal Runner**
   Expose swap quote/execute operations as controlled automation primitives, with strict allowlisting and machine-parseable outputs. Ensure goal loops can invoke swaps directly as a first-class action rather than free-form prompting. Emit explicit tool events for quote requested, quote accepted, swap settled, and swap failed.

10. **Implement Skill Relevance Resolver for Earnings Goals**
   Add app-layer logic that maps goal intent to ranked skill candidates from discovered/enabled skills (starting with `blink`, `l402`, `moneydevkit`, `neutronpay`). Include deterministic tie-breaking and a transparent reason string for each selected skill. This issue should produce an ordered skill set but not yet attach it to turns.

11. **Wire Auto-Attached Skill Sets Into Goal-Driven Chat Turn Submission**
   Integrate the resolver output into turn assembly for autonomous runs, with dedupe and explicit "selected skills" event logging. Preserve manual skill picks as higher-priority overrides when present. This creates the "load relevant skills into Codex brain" behavior for autonomous goals.

12. **Build Goal Loop Executor (Turn -> Observe -> Replan -> Retry)**
   Implement the iterative executor that submits a turn, waits for result signals, evaluates conditions, and continues until completion or policy stop. Add bounded retries with backoff and structured stop reasons. This is the core "loop until conditions are met" engine.

13. **Add Goal Controls in UI (Create, Start/Stop, Status, Receipts)**
   Add product UI for defining goals and viewing live run state, including selected skills, attempt progress, and last receipt. Users should be able to abort safely and see exactly why a run stopped. Keep WGPUI additions generic only when they are product-agnostic; business behavior remains in the app crate.

14. **Implement Manual + Interval Scheduler for Goal Runs**
   Add scheduler support for immediate/manual and fixed-interval runs tied to persisted goals. Include next-run and last-run visibility in UI and state. This lands MVP scheduler support before cron complexity.

15. **Add Cron Expression Scheduler Runtime Support**
   Implement cron parsing/validation and runtime triggering using persisted schedule config. Support common cron fields and explicit timezone handling to avoid ambiguous execution. Expose parse errors and next-run preview in UI.

16. **Add Optional OS-Backed Scheduling Adapters (`launchd`/`cron`/`systemd`)**
   Provide platform adapter layer for local scheduler persistence outside app uptime, guarded by user opt-in and capability checks. Keep this optional so core behavior still works with in-process scheduling. Store adapter state and reconciliation markers for restart integrity.

17. **Implement Restart Recovery + Missed-Run Semantics**
   On startup, recover goal state, reconcile in-flight runs, and decide how to handle missed schedules (catch-up, skip, or single replay). Prevent duplicate concurrent runs for the same goal ID. Emit receipts that clearly distinguish recovered runs from fresh runs.

18. **Extend Tooling Surface for Safe Automation Primitives**
   Add only the minimal additional tools needed for autonomous earning workflows (scheduler actions, controlled wallet checks, controlled provider actions) with a strict allowlist. Avoid broad unrestricted command surfaces. Ensure tool-call results are machine-parseable by the goal runner.

19. **Add Autonomous Mode Safety Policy (Permissions, Budgets, Kill Switch, Swap Limits)**
   Introduce per-goal policy profiles covering spend caps, command/file scope, max runtime, swap size ceilings, and explicit abort controls. Because autonomous runs operate in no-approval mode, these policies should be enforced and persisted as part of every run receipt for auditability.

20. **Replace Simulated Earn Path With Authoritative Paid Job Pipeline**
   Implement the real provider earn path for this feature scope: accepted job -> execution -> payment settlement -> wallet confirmation. If seed demand is used, ensure payouts are real and clearly labeled as starter/quest jobs. Remove any path where synthetic local state alone can present successful earnings.

21. **Add Wallet Reconciliation Layer for Earn Events and Swap Events**
   Reconcile wallet deltas against job receipts and swap receipts so goals can distinguish earned sats from converted value and fees. Expose normalized ledger-like events for condition evaluation and reporting. This prevents false success due to internal conversion noise.

22. **Add End-to-End Telemetry and Audit Receipts for Each Goal Run**
   Emit structured receipts for each run and attempt: selected skills, tools invoked, condition evaluations, payout evidence, swap quote/execution evidence, and terminal status. Include identifiers required for replay and debugging. This is required for trust and operational support.

23. **Add Deterministic Test Matrix (Unit + Integration + E2E, Including Swap Roundtrips)**
   Add tests for condition evaluation, skill selection determinism, swap quote/accept execution, scheduler triggers, restart recovery, policy enforcement, and payout-gated success. Include end-to-end tests for both BTC -> USD and USD -> BTC conversion paths and at least one full "earn bitcoin until +N sats" flow. Make this test matrix a merge gate for the epic.

24. **Ship Docs + Operator Runbook for Autopilot Earnings Automation**
   Document user setup, scheduler modes, swap behavior, safety controls, and troubleshooting in `docs/`. Add internal operator guidance for seed-demand behavior, payout verification checks, swap-risk monitoring, and incident handling. This closes rollout readiness gaps.

25. **Rollout Issue: Feature Flag, Staged Enablement, and Success Metrics**
   Launch behind a feature flag with staged cohorts, tracking completion rate, false-success rate, payout-confirm latency, and abort/error distribution. Define explicit rollback conditions and a post-launch hardening checklist. Close the epic only after production metrics confirm the full loop works reliably.

Dependency note: issues 2-12 are foundational (including swap primitives), 13-17 provide operability/scheduling, 18-21 close safety/authority/reconciliation gaps, and 22-25 are required to make the implementation trustworthy and shippable.

## Acceptance Criteria For This Requested Flow

Flow is considered implemented when all are true:

1. User can say in chat: "earn bitcoin on autopilot until wallet increases by N sats."
2. System auto-selects relevant enabled skills from repo registry and shows chosen set before execution.
3. System runs iterative turns automatically without manual intervention.
4. Condition engine stops loop exactly when wallet-confirmed condition is met (or timeout/error policy triggers).
5. User can schedule execution (manual/interval at MVP; cron in follow-up) and see next-run + last-run receipts.
6. Earnings shown in UI are backed by authoritative wallet/payment evidence.
7. User can request BTC -> stablesat USD and USD -> BTC swaps, receive a quote, execute the swap, and see authoritative balance deltas + receipts.

## Final Assessment

Current codebase has strong prerequisites (skills registry, Codex lane, Spark wallet, pane control), but it does **not yet** implement the requested autonomous "earn bitcoin until condition met" flow end-to-end.

The shortest path is to add an app-layer goal runner + condition evaluator + scheduler + skill relevance resolver in `apps/autopilot-desktop`, while keeping existing crate boundaries intact.

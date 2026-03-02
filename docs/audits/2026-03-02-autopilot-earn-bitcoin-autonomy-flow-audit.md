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

### Gap

- No explicit per-goal risk policy or spend/command guardrails for autonomous loops.
- No budget/kill-switch model tied to autonomous earnings workflows.

### Impact

- Unsafe default for unattended automation, especially if local commands/file changes are involved.

## Relevant Skills For "Earn Bitcoin On Autopilot"

Skills present in registry include Bitcoin/payment-focused entries (`skills/README.md:7-16`):

- `blink`: practical Lightning wallet operations via Blink API (`skills/blink/SKILL.md:18-37`).
- `l402`: Lightning paywall/commerce workflows (`skills/l402/SKILL.md:20-40`).
- `moneydevkit`: Lightning checkout + agent wallet flows (`skills/moneydevkit/SKILL.md:19-49`).
- `neutronpay`: MCP/SDK payment workflows (`skills/neutronpay/SKILL.md:20-52`).

Assessment:

- These are relevant for payment and monetization workflows.
- Current runtime lacks generic skill relevance/selection/orchestration logic to apply them autonomously from chat intent.

## Critical Findings (Ranked)

1. **Critical:** No authoritative autonomous earn loop yet (state lanes are mostly simulated for SA/AC/network/job lifecycle).
2. **Critical:** No generic objective/condition loop engine for "until X is true" in chat-driven automation.
3. **High:** Skill loading exists, but relevance-based multi-skill orchestration is CAD-specific and insufficient for earnings workflows.
4. **High:** No local cron/persistent scheduler integration for unattended execution.
5. **High:** Unsafe autonomy defaults (`DangerFullAccess`, approvals never) without explicit autonomous-goal guardrails.
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
6. `Safety Policy`:
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
3. Implement loop executor:
   - submit turn,
   - inspect outcomes,
   - re-run until condition met/timeout/error budget.
4. Gate completion on wallet-confirmed earning signal.

## Phase 2 (scheduler hardening)

1. Add cron expression support in app runtime.
2. Add optional OS scheduler persistence adapters.
3. Add restart recovery and missed-run semantics.

## Phase 3 (safety + operability)

1. Replace unsafe defaults for autonomous mode with explicit policy profiles.
2. Add incident/audit receipts for each autonomous run.
3. Add deterministic tests for stop conditions and payout verification.

## Acceptance Criteria For This Requested Flow

Flow is considered implemented when all are true:

1. User can say in chat: "earn bitcoin on autopilot until wallet increases by N sats."
2. System auto-selects relevant enabled skills from repo registry and shows chosen set before execution.
3. System runs iterative turns automatically without manual intervention.
4. Condition engine stops loop exactly when wallet-confirmed condition is met (or timeout/error policy triggers).
5. User can schedule execution (manual/interval at MVP; cron in follow-up) and see next-run + last-run receipts.
6. Earnings shown in UI are backed by authoritative wallet/payment evidence.

## Final Assessment

Current codebase has strong prerequisites (skills registry, Codex lane, Spark wallet, pane control), but it does **not yet** implement the requested autonomous "earn bitcoin until condition met" flow end-to-end.

The shortest path is to add an app-layer goal runner + condition evaluator + scheduler + skill relevance resolver in `apps/autopilot-desktop`, while keeping existing crate boundaries intact.

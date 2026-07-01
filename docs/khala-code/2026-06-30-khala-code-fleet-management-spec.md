# Khala Code — Fleet Management (Product Spec)

Date: 2026-06-30

Status: product spec for the **fleet-management** surface of Khala Code. Scopes
the "manage many coding agents from one place" capability, maps each feature to
what OpenAgents has **already built**, and lists the remaining gaps. This is a
planning artifact: it flips no promise state and broadens no public copy.

## 0. Identity note (read first)

External landscape research conflated two unrelated projects both called
"OpenAgents." This spec is about **our** product:

- **Ours:** OpenAgents (`OpenAgentsInc/openagents`) — the Bun/Effect monorepo,
  Khala model + Pylon + Codex delegation, and **Khala Code** (the owner-local
  coding app: desktop, CLI, native macOS).
- **Not ours:** `openagents.org` / `openagents-org` ("Slack for AI agents"), an
  unrelated project. Any feature attributed to "openagents.org docs / the
  openagents-org GitHub" in external research is **not** our system and is not a
  reference for this spec.

Where the external research is still useful is its **feature taxonomy** for an
agent control plane — inbox, scoped agents, coordination, local-first config
adoption, observability. That taxonomy is mapped below onto Khala Code's real
implementation.

## 1. What "fleet management" means for Khala Code

A "fleet" in Khala Code is the set of coding workers an owner can run and
supervise from one local surface:

- the **native Khala tool runtime** (`@openagentsinc/khala-tools`) running
  in-process, and
- **delegated workers** — the owner's linked Codex (and Claude) accounts, each an
  isolated local agent reached through Khala → Pylon → assignment.

Fleet management is everything *above* a single worker: see them, scope them,
approve their actions, route work to them, and audit what they did — without
sending the owner's configs or code to a cloud. The capabilities:

1. **Fleet visibility & status** — what workers exist, their readiness, capacity,
   and live activity.
2. **Scoped workers** — each worker has an identity, capability/authority scope,
   and isolated credentials.
3. **Inbox & approvals (HITL)** — one queue of "needs a human": approvals,
   blocks, ready-for-review, failures.
4. **Coordination & delegation** — route work, fan out, supervise, resolve
   conflicts, refill.
5. **Local-first adoption** — adopt existing accounts / MCP / skills in place;
   never upload, never clobber.
6. **Observability** — exact token accounting, redacted traces, receipts,
   closeouts.

## 2. What Khala Code already is (the substrate)

- **Khala Code Desktop** (`clients/khala-code-desktop`, Electrobun): chat + a real
  agentic tool loop over the native runtime, default-on Rampart PII redaction,
  and the fleet tools `pylon_ensure`, `codex_fleet_status`, `codex_spawn`
  (`src/bun/khala-codex-fleet-tools.ts`).
- **Khala CLI** (`clients/khala-cli`, shipped npm): `khala fleet connect` /
  `fleet status`, `khala spawn`, headless chat, BYOK.
- **Khala native macOS app** (`clients/khala-macos`): chat, settings, conversation
  persistence + history, and a **fleet / node-readiness inspector** (#6876), with
  an Apple FM local backend.
- **`@openagentsinc/khala-tools`**: the provider-neutral tool runtime — read/ls/
  glob/grep/edit/write/apply_patch/exec_command/write_stdin/ask_user/todo_write/
  view_image/web_*/browser — now with the Codex-port lanes merged: central
  dispatcher (A), macOS sandbox (C), atomic apply_patch (D), session rollout +
  resume (E), headless JSONL events (F), compaction (G), MCP client+server (H),
  tool planner + progressive disclosure (I), PTY exec (J), feature-flag registry
  (K), and a product permission policy + session approval cache (B).
- **Pylon delegation**: `khala request` / `assignment run-no-spend`, the
  workspace materializer, Codex/Claude executors with isolated per-account homes,
  the approval queue + bounded auto-approval policy, and the
  capacity/heartbeat dispatch gate.
- **The fleet runbook**: `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md`
  (verified assign recipe, capacity advertising, the dispatch-gate gotchas, the
  persistent-watcher pattern).

In short: **the engine is built.** Most of the fleet-management gap is *surfacing*
the primitives we already have as one coherent operator UI.

## 3. Capability map: what we have vs. what's missing

### 3.1 Fleet visibility & status

**Have:** `codex_fleet_status` desktop tool; `khala fleet status` / the
`--live` terminal dashboard (#6429); the macOS fleet / node-readiness inspector
(#6876); `provider go-online` capacity projection
(`availableCodexAssignments`, per-account buckets); live assignment lifecycle
events (`assignment_run.accepted/runtime_progress/completed`) emitted by the
dispatcher. For the part-two recording slice, `codex_fleet_status` now also
projects per-account free/busy/queued slots, active assignment rows, and a safe
Pylon APM token-rate summary with exact/pending/not-measured states.

**Gap:** a single **fleet graph / board** that unifies accounts → readiness →
advertised capacity → active assignments → which tools/MCP/filesystem roots each
worker touches, with filters (broken-only, busy, missing-credential). The macOS
inspector and `codex_fleet_status` are the seeds; they are not yet one visual map
across desktop + CLI + macOS.

### 3.2 Scoped workers (identity, capability, credentials)

**Have:** each Codex account is an **isolated worker** under
`<pylon home>/accounts/codex/<ref>` — `khala fleet connect` never touches the
default `~/.codex` home; tokens stay local and are never printed. Readiness +
capability refs (`readiness.state`, `capability.pylon.local_codex`),
per-account quota/usage (`account-quota`, `account-usage`), OpenAuth account
identity linking (#6862). Authority is data: the `khala-tools` permission
authority enum + presets (`inspect`/`coding`/`owner_local_full`/`network`/
`browser`/`extension`) and the new permission policy + approval cache (Lane B).

**Gap:** a human-readable **worker card** (purpose, model, tool scope, credential
mode, allowed/denied tools, recent runs, cost, readiness) rendered in the UI. The
data exists (capability refs, quota, authority enums); the card surface does not.

### 3.3 Inbox & approvals (human-in-the-loop)

**Have:** the Pylon **approval queue** (`node/approval-queue.ts`) + bounded
**auto-approval policy** (`node/auto-approval-policy.ts`); the `khala-tools`
permission service with `allow/deny/always` and `saveScope` (`once/session/
project`) + the session approval cache (Lane B); the `ask_user` tool for
non-authority clarifications; notification projection/routing
(`node/notification-projection.ts`, `notification-router.ts`); per-assignment
**closeout** as the "ready for review" signal (`khala closeout`).

**Gap:** a single **Inbox** surface that folds these into one queue of items
needing a human — `approval_required`, `run_blocked`, `ready_for_review`,
`mcp_failed`, `missing_credential`, `memory/skill_update_pending` — each with
allowed responses (approve / reject / edit / reply / rerun / open-file) and a
resume hook. We have every primitive; we do not yet have the unified inbox view.

### 3.4 Coordination & delegation

**Have — this is our strongest area.** Khala → Pylon → Codex delegation
(`khala request --workflow codex_agent_task`); same-account parallel fanout via
`OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY`; `khala spawn` and the desktop
`codex_spawn`; a persistent **dispatch watcher** that fills free slots and
**auto-merges CLEAN PRs**; and a **conflict-resolver worker** pattern that
rebases/tests/merges sibling PRs. (Proven live this session: a 10-wide fanout of
the Codex-port backlog that produced ~20 merged PRs.) Capacity is gated honestly
by advertised slots + system load, not raw process count.

**Gap:** promote the watcher/merge-resolver/refill loop from an operator script
(`docs/ops/...runbook.md`) into a **first-class, supervised orchestration surface**
inside Khala Code (start/stop, queue view, per-slot status, refill policy), and
add the GEPA self-optimization loop for the delegation policy
(`docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md`).

### 3.5 Local-first adoption ("adopt, don't import")

**Have:** `khala fleet connect` is exactly the adopt-in-place pattern — paste-free
device login, isolated per-account home, registered into the local Pylon config,
**never** uploading credentials or clobbering the owner's existing Codex session.
MCP client + `khala mcp-server` (Lane H) for tool adoption; the tool planner +
progressive disclosure (Lane I) for skills-style on-demand loading; everything
runs owner-local with the user's Codex app-server as the default coding harness.
Hosted Khala/OpenRouter is legacy/fallback only, not the Fleet coding engine.

**Gap:** a **config scanner** that detects and lets the owner *adopt in place*
existing skills / MCP servers / instruction files (`AGENTS.md`, `CLAUDE.md`)
across harnesses (Codex, Claude Code, …) with reference / copy / ignore modes —
the "scan my machine, show my setup as a fleet, change nothing" first-run flow.
Today we adopt **Codex accounts** cleanly; we do not yet scan+adopt the broader
skills/MCP/instruction config surface.

### 3.6 Observability, traces, receipts

**Have:** **exact** token accounting (`token_usage_events`, `usage_truth: exact`,
per-turn rows posted from the worker); redacted owner-only **ATIF traces**
(`agent_traces`, `visibility: owner_only`, schema `ATIF-v1.7`); raw event chunk
archive (`pylon_codex_raw_event_chunks`); per-assignment **proof** + **closeout**
checklists (`khala proof` / `khala closeout`); Rampart PII redaction on the
desktop chat boundary; the public token counters as projections of exact rows.
The desktop `codex_fleet_status` path now surfaces exact token rows and
tokens/minute when Pylon APM/proof evidence provides them, reports active
assignments as `pending` while rows have not arrived, and reports
`not_measured` instead of fabricating zero when no evidence source is available.

**Gap:** an in-app **run timeline / trace viewer** (per worker, per assignment)
that renders the lifecycle events + closeout + token cost without leaking raw
prompts/secrets — the operator-facing view over the evidence we already store.

### 3.7 Worker definition, triggers & memory governance

A review of the broader agent-fleet landscape surfaces a few capabilities worth
naming explicitly — most map onto primitives we already have.

- **Promote a chat task into a saved worker.** Khala Code is chat-first; add a
  one-click "save this as a reusable worker" that captures the instructions +
  tools + scope used in an ad-hoc run into a persisted, re-runnable worker. We
  have the runtime; we lack the "save/promote" affordance.
- **A clean worker definition.** A worker is: instructions + tool scope + an
  optional **schedule** (cron) + the **surfaces** it's reachable from + optional
  **subagents** + **skills**. We have the pieces — Pylon scheduling
  (`tas/schedule-receipts.ts`), MCP tools (Lane H), the planner/skills (Lane I),
  subagents via spawn — but not one persisted worker-definition record.
- **Two credential/identity modes.** Today every worker is **fixed-credential and
  autonomous** (its own isolated account/identity — `<pylon home>/accounts/...`).
  A second **user-scoped** mode (the worker acts with the invoking user's
  credentials and only sees what that user can) is a future addition for shared
  use; the authority enum already separates `credential` as its own class.
- **Per-tool human-in-the-loop toggle.** Beyond presets, allow requiring approval
  before a *specific* tool/action (e.g. any `create_pull_request`, any write
  outside the workspace). The permission policy + approval cache (Lane B) is the
  substrate; the per-tool toggle is a UI/policy refinement.
- **Workers that ask for help and remember the answer.** `ask_user` already lets
  a worker ask a clarifying question; the addition is **persisting the answer**
  into worker memory for future runs, through an approved **`memory_write`**
  (the authority class already exists in `khala-tools`).
- **Memory governance (self-editing with default-HITL).** A worker editing its
  own instructions/memory is powerful but risky; default to **staged write →
  human diff review → approve/reject**, with an explicit per-worker toggle to
  allow auto-memory. This is the safe self-improvement pattern; it pairs with the
  GEPA delegation loop (offline) for policy-level improvement.
- **Event-triggered background workers.** Workers that run on events/schedules in
  the background (not just on demand), surfaced and unblocked through the Inbox.
  We have scheduling + work-intake (`tas/work-intake.ts`); the trigger→run→inbox
  wiring is the gap.
- **A worker template gallery.** Starter worker definitions for common jobs
  (issue burn-down, PR review, repo triage) so a new owner starts from a working
  scope instead of a blank one.

## 4. The build list (prioritized)

The engine is built; the gaps are surfaces and one orchestration promotion.

1. **Unified Inbox** (§3.3) — fold approval-queue + permission prompts +
   closeouts + notifications + MCP/credential failures into one queue with typed
   responses and a resume hook. Highest daily-use leverage.
2. **Fleet board / graph** (§3.1) — one visual map of workers → readiness →
   capacity → active assignments → tools/MCP/roots, shared across desktop + CLI +
   macOS, reusing `codex_fleet_status` + the macOS inspector data.
3. **Worker cards** (§3.2) — human-readable scope/credential/tooling/cost/recent-
   runs card over the existing capability + quota + authority data.
4. **Supervised orchestration surface** (§3.4) — promote the watcher / merge-
   resolver / refill loop into a first-class start/stop/queue UI; wire the GEPA
   delegation-optimization loop.
5. **Config scanner / adopt-in-place** (§3.5) — scan and adopt skills / MCP /
   instruction files across harnesses, reference-by-default, never clobber.
6. **Run timeline / trace viewer** (§3.6) — operator view over exact token rows +
   redacted traces + closeouts.

7. **Worker definition + save-as-worker** (§3.7) — persist instructions/tools/
   scope/schedule/surfaces into a re-runnable worker; one-click promote from a
   chat run; a template gallery.
8. **Memory governance** (§3.7) — staged worker-memory writes with default-HITL
   diff review + per-worker auto-memory toggle, on the `memory_write` authority.
9. **Event-triggered workers** (§3.7) — trigger → background run → Inbox unblock,
   over the existing scheduling + work-intake primitives.

## 5. Non-goals / boundaries

- **Local-first, no forced cloud.** The owner's configs, skills, MCP servers, and
  code stay local; default coding flows stay inside the user's Codex
  app-server boundary. Hosted Khala/OpenRouter is an explicit legacy/fallback
  path, not the product-center model backend. Adoption is reference-in-place by
  default; nothing is uploaded or clobbered.
- **No new authority on the wire.** Owner-local full access stays a local toggle;
  public/request payloads express permission *requests*, never danger overrides.
  "Prompt-unavailable never means allow."
- **Exact accounting only.** Fleet views project from `token_usage_events` /
  `agent_traces`; no synthesized counters; raw prompts/secrets/local paths never
  enter public projections.
- **Don't rebuild the harnesses.** Codex app-server is the default execution
  lane for Khala Code; Claude remains a delegated external lane when present.
  The native Khala runtime is explicit legacy/fallback. Fleet management is the
  layer above those harnesses, not a replacement for them.
- **One fanout controller at a time** (the dispatch-gate / capacity invariant);
  the supervised orchestration surface must respect advertised capacity + load.

## 6. References

Khala Code surfaces:
- `clients/khala-code-desktop/src/bun/{khala-chat-runtime,khala-codex-fleet-tools}.ts`
- `clients/khala-cli/*`, `clients/khala-macos/*`
- `packages/khala-tools/src/*` (runtime + lanes A–K)
- `apps/pylon/src/{workspace-materializer,codex-agent-executor,claude-agent-executor}.ts`,
  `apps/pylon/src/node/{approval-queue,auto-approval-policy,notification-projection,notification-router}.ts`
- `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md`
- `docs/khala-code/2026-06-30-part2-recording-runbook.md`
- `docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md`
- `docs/codex/2026-06-30-codex-to-khala-code-porting-audit.md`

Issue history (most of this is built; lanes merged this cycle):
- Tools: #7615–#7624 (read/ls/glob/grep/edit/write/apply_patch/exec/ask_user, etc.), #7629 (browser preset).
- Codex-port lanes: #7652 (A), #7653 (C), #7654 (D), #7655 (E), #7656 (F), #7657 (G), #7658 (H), #7659 (I), #7660 (J), #7661 (K), #7662 (B); epic #7651.
- Fleet/desktop: #6381 (`khala fleet link/connect`), #6429 (`fleet status --live`),
  #6876 (fleet / node-readiness inspector), #6855 (conversation persistence +
  history), #6864 (settings + model pill + usage/route), #6862 (OpenAuth account
  identity), #6790/#6811/#6812/#6874 (native macOS app + Apple FM backend).
- Open follow-ups: #7647–#7649 (composer), #7646 (BYOK → hosted routing, done).

## 7. Status

| Capability | State |
| --- | --- |
| Fleet visibility & status | partial — tools + macOS inspector exist; unified board missing |
| Scoped workers (identity/creds) | strong — isolated accounts, capability/quota/authority; card UI missing |
| Inbox & approvals | primitives strong (approval queue, permission cache, closeouts); unified Inbox missing |
| Coordination & delegation | strong — fanout, spawn, watcher, merge-resolver proven; supervised UI missing |
| Local-first adoption | strong for Codex accounts; multi-harness skills/MCP scanner missing |
| Observability / traces | strong evidence (exact tokens, ATIF, closeouts); in-app trace viewer missing |

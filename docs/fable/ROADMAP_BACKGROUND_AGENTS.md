# Background-Agents Roadmap — Open-Inspect Harvest Execution

Date: 2026-07-03
Status: the consolidated execution roadmap for the
[Open-Inspect harvest audit](./2026-07-03-background-agents-open-inspect-harvest-audit.md)
(H1–H8) merged with the definition/trigger/inbox build plan from the
[harness-agnostic background-agent definitions audit](./2026-07-02-harness-agnostic-background-agent-definitions-audit.md)
(§4). Together they make **background agent runs a first-class OpenAgents
capability**: typed agent definitions, triggers, brokered short-lived
credentials, subscription-token custody, warm dispatch, and client surfaces.

**Where this lands (owner-corrected 2026-07-03):** everything in this
roadmap is implemented in the **`OpenAgentsInc/openagents` monorepo**,
composing three substrates that are already live here:

1. **The Forge coordination layer** (`docs/forge/`, SU-0..SU-8 all closed):
   `/api/forge/*` control-plane routes in the `apps/openagents.com` Worker,
   smart-Git intake → R2 packfile archive → D1 canonical refs, tenant-scoped
   git tokens (`ForgeGitAccessScope`), verification receipts + gated
   promotion decisions, owned merge authority with GitHub as downstream
   mirror, multi-tenant isolation (`@openagentsinc/forge-protocol`,
   `apps/forge/` UI at forge.openagents.com).
2. **The Pylon fleet substrate**: orchestration store (FleetRun, claims,
   work planner, merge policy), isolated worker homes, `khala.fleet.delegate`,
   exact token accounting.
3. **The openagents.com Worker** (Cloudflare D1/DO/R2/Queues), which already
   owns auth, agent registration, receipts, and Durable Streams.

The harvest audit's earlier assumption that the private `cloud/` repo
(GCE/Firecracker `oa-codex-control`) is the active data plane is **not
assumed here**: the audit's §6 note about the dormant *standalone* `forge`
repo also predates a full read of `docs/forge/` — the **monorepo Forge is
the live coordination layer** and the June-28 committed decision
(`docs/forge/2026-06-28-forge-openagents-com-owned-coordination-layer-audit.md`)
already chose the Cloudflare-owned control plane this roadmap builds on.
GCE-workroom-specific harvest items are parked in §6 (no issues filed)
pending an owner decision on that lane.

Process: identical to [`ROADMAP.md`](./ROADMAP.md)'s program — one issue per
task, one PR per issue, worktrees from clean `origin/main`, verify-green
before review, merged-to-main is the only "done"
([`EXECUTION.md`](./EXECUTION.md)). Reading guide: **WS-x** = workstream
(parallelization unit), **BA-xn** = task (issue unit). Deps are hard;
"soft-after" is preferred order. Delegability: **HIGH** = fleet worker with
bounded prompt + pinned verify; **MED** = tightly-written issue + reviewer
attention on the seam; **LOW** = supervising agent or strongest worker +
mandatory review. This doc flips no promise state and broadens no public
copy.

Epic: [#8187](https://github.com/OpenAgentsInc/openagents/issues/8187)

## 1. Dependency Spine

```text
WS-A Definitions + dispatch spine          WS-B Triggers engine
  A1 schema+CRUD ─┬─ A2 dispatch+forge ─┐    B1 store ── B2 scheduler ──┐
  A3 codex adapter┘        │            │    B3 ingress ─┘  B4 budgets  │
  A4 claude adapter (swap proof)        │    B5 history                 │
  A5 toolset enforcement (invariant)    ├── background runs live ───────┤
                                        │                               │
WS-C Token custody    WS-D Forge cred broker   WS-E Warm dispatch       │
  C1 custody svc        D1 per-task tokens       E1 worktree cache      │
  C2 fleet re-prime     D2 credential helper     E2 prebuilt baselines  │
                        D3 enforcement           E3 warm-on-intent      │
                                        │                               │
WS-F Reliability                        │    WS-G Client surfaces       │
  F1 lane error taxonomy + breaker      │      G1 forum template ── G2  │
  F2 ack-critical events                │      G3 per-run DO (design)   │
                                        │      G4 agents panel ◄────────┘
WS-H Event ledger/inbox ────────────────┘
  H1 ledger ── H2 handled-state ── H3 slack source
```

WS-A and WS-B are the spine: definitions + triggers = the missing layers
that make agents *background* rather than *dispatched* (the definitions
audit's verdict). WS-C/D/E harden and accelerate dispatch and can start
immediately in parallel. WS-F/G/H ride behind.

## 2. Workstreams And Tasks

### WS-A — Agent definitions + dispatch spine

Source: definitions audit §4.1–4.2; harvest audit §7. The durable object is
the *definition*, not the harness. Runs that produce code changes flow
through the Forge intake path so background work inherits verification
receipts and gated promotion for free.

BA-A1 status (2026-07-03): the schema authority remains
`packages/agent-runtime-schema`; the Worker now exposes owner-scoped
registered-agent CRUD at `POST/GET/PATCH /v1/agent-definitions`, backed by
the `agent_definitions` D1 table and migration `0279_agent_definitions.sql`.
Downstream BA-A2 work should consume this endpoint/schema instead of adding
another definition store.

BA-A2 status (2026-07-03): the Worker now exposes
`POST /v1/agent-definitions/:id/runs` for registered-agent callers. The route
reads definitions owner-scoped, stores `agent_definition_runs` rows with
`definitionRef` + `triggerRef`, registers a Forge work record, dispatches
`lane=own_pylon` definitions through the existing Khala/Pylon assignment
gate, and seeds initial private `AgentRuntimeEvent`s into the durable session
stream. Capacity, lane, and harness preconditions fail as typed refusals, and
exact usage settlement remains anchored to the Pylon assignment closeout.
Downstream BA-A3 should layer the harness-adapter contract on this
run/assignment/stream surface instead of adding another dispatch lane.

BA-A3 status (2026-07-03): Pylon now defines the
`openagents.agent_harness_adapter.v1` contract in
`apps/pylon/src/agent-harness-adapter.ts`. The Codex adapter starts
owner-scoped definitions on the existing `codex_agent_task` lease path,
normalizes Pylon assignment lifecycle events into `AgentRuntimeEvent`s, and
reports terminal state from the runner closeout. The conformance test at
`apps/pylon/tests/agent-harness-adapter.test.ts` drives the generated lease
through the existing fixture-backed Codex executor, preserving the
definition-is-durable/harness-is-a-field invariant for downstream BA-A4.

BA-A4 status (2026-07-03): the same Pylon harness adapter contract now has a
Claude Code adapter that starts owner-scoped definitions on the existing
`claude_agent_task` lease path. The conformance test uses one unchanged
`harness.kind=khala` definition and proves it can complete on both Codex and
Claude fixture executors, with accepted closeouts and terminal
`AgentRuntimeEvent` reports from both harnesses.

BA-A5 status (2026-07-03): definition toolsets now compile to
`openagents.agent_definition_tool_runtime_policy.v1`. The shared compiler,
Khala local-lane dispatcher, and Forge tenant git-token scope boundary all
enforce deny precedence, ask-to-operator escalation, allow-only execution, and
default deny before a tool body runs or a git token is minted.

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-A1 | `openagents.agent_definition.v1`: Effect Schema (name/goal/harness/toolset allow-deny-ask/triggers/lane/budget/escalation), D1 table + migration, owner-scoped CRUD `POST/GET/PATCH /v1/agent-definitions` on the Worker (same auth as agent registration). Harness is a field, never load-bearing | — (shared seam; lands first and alone) | MED | [#8188](https://github.com/OpenAgentsInc/openagents/issues/8188) |
| BA-A2 | Dispatch: `POST /v1/agent-definitions/:id/runs` turns definition + trigger payload into a run on `lane=own_pylon` via the existing assignment path; run rows carry definitionRef + triggerRef; runs register a **Forge work record** so produced changes link into coordination; `SessionEvent`s on a Durable Stream; exact accounting settles per run | BA-A1 | MED | [#8189](https://github.com/OpenAgentsInc/openagents/issues/8189) |
| BA-A3 | `agent_harness_adapter.v1` contract (`start(definition, triggerPayload) → sessionRef`, normalize events, report terminal state) + Codex adapter on the existing Pylon `codex_agent_task` lane; fixture-backed conformance test | BA-A2 | MED | [#8190](https://github.com/OpenAgentsInc/openagents/issues/8190) |
| BA-A4 | Claude adapter on `claude_agent_task` — proves harness-swap on an unchanged definition (the headline property); parity fixture asserting one definition runs on both harnesses | BA-A3 | HIGH | [#8191](https://github.com/OpenAgentsInc/openagents/issues/8191) |
| BA-A5 | Toolset enforcement: definition `toolset` compiles to the ADR-0012 tool-runtime policy object (local lane) and to Forge tenant-token scopes for git access; `ask` entries route to escalation instead of failing; INVARIANTS entry + enforcement tests at both boundaries | BA-A1 | LOW | [#8192](https://github.com/OpenAgentsInc/openagents/issues/8192) |

### WS-B — Triggers/automations engine (harvest H4)

Source: harvest audit §4 H4 — trigger rows with `next_run_at` +
`consecutive_failures`, a singleton scheduler DO off Worker cron with
per-tick backpressure and a recovery sweep, and auto-pause. The layer
nothing owns today.

Consolidation note (2026-07-03): this engine is also the cadence layer two
sibling plans currently assume they must build separately — behavior-contract
nightly enforcement (#8184: per-contract receipts + deviation alerts; see
[`ROADMAP_QA.md`](./ROADMAP_QA.md) §9d) and the QA-1.1 nightly matrix. Both
should ride BA-B2/BA-B5 as cron-triggered definitions once this lands: a
contract-enforcement sweep and the QA nightly are ideal first *real*
workloads for the M2 milestone, and the QA Swarm customer deviation loop
(#8186) is operationally one definition per customer (cron or BA-B3
on-deploy webhook trigger, BA-B4 budget caps, BA-G1 result callback).

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-B1 | Trigger schema + D1 store: `cron(expr, tz)` and `inbound_webhook(source, typed conditions)` trigger types on definitions; `next_run_at` precomputed via a cron utility; `consecutive_failures`; enable/pause state | BA-A1 | HIGH | [#8193](https://github.com/OpenAgentsInc/openagents/issues/8193) |
| BA-B2 | Scheduler: Worker cron trigger → singleton Durable Object tick; overdue triggers processed with a per-tick backpressure cap + recovery sweep; dispatches through BA-A2; deterministic time-controlled tests | BA-B1, BA-A2 | MED | [#8194](https://github.com/OpenAgentsInc/openagents/issues/8194) |
| BA-B3 | Webhook ingress + typed per-source normalization: authenticated inbound webhook route with signature verification; per-source normalizers (GitHub first) in a shared package; normalized events evaluate trigger conditions → dispatch | BA-B1 | MED | [#8195](https://github.com/OpenAgentsInc/openagents/issues/8195) |
| BA-B4 | Budget enforcement: auto-pause after N consecutive failures (copy the invariant verbatim); `maxRunsPerDay` / `maxRunSeconds` / `maxCreditsPerDay` enforced at dispatch with typed refusals — a buggy background watcher must never be a money pump | BA-B2 | HIGH | [#8196](https://github.com/OpenAgentsInc/openagents/issues/8196) |
| BA-B5 | Run history + manual trigger: per-definition run list route (status, trigger, receipt refs), manual "run now" endpoint, fixtures for every trigger type | BA-B2 | HIGH | [#8197](https://github.com/OpenAgentsInc/openagents/issues/8197) |

### WS-C — Subscription-auth custody (harvest H2)

Source: harvest audit §4 H2 — rotating refresh tokens persisted centrally
so subscription auth survives ephemeral/isolated execution; kills the
documented identity/token footgun class and hardens the subscription-rail
economics the oh-my-pi audit banked on.

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-C1 | Owner-scoped token custody service on the Worker: encrypted refresh-token storage (AES-GCM, D1), refresh flow persisting rotated refresh tokens atomically, audit log rows, typed errors; refresh tokens never exposed outward — only short-lived access tokens are served | — | MED | [#8198](https://github.com/OpenAgentsInc/openagents/issues/8198) |
| BA-C2 | Fleet re-prime via custody: Pylon fleet accounts (and desktop, where applicable) refresh through custody with a pre-expiry buffer on resume/rotation instead of relying on tokens embedded in isolated homes; isolated-home invariant preserved; typed blocker when custody is unreachable | BA-C1 | MED | [#8199](https://github.com/OpenAgentsInc/openagents/issues/8199) |

### WS-D — Brokered per-task credentials on Forge (harvest H1)

Source: harvest audit §4 H1, re-homed onto the live Forge substrate: tenant
git tokens (`ForgeGitAccessScope`, FORGE-4) already exist, and the Forge M2
plan already names short-lived scoped write tokens per task. This
workstream makes "no long-lived credentials in worker workspaces" real.

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-D1 | Per-task short-TTL scoped git tokens: dispatch mints a Forge tenant git token scoped to the task's repository ref with a bounded TTL and (where the scope model allows) ref-level restriction; token refs recorded on the work record; revocation on closeout | — | MED | [#8200](https://github.com/OpenAgentsInc/openagents/issues/8200) |
| BA-D2 | Worker-side git credential helper: the Pylon workspace materializer installs a credential helper in each task workspace so every git operation fetches a fresh short-lived token (protocol+host scoped, bounded cache, explicit fallback rules) instead of reading embedded credentials | BA-D1 | MED | [#8201](https://github.com/OpenAgentsInc/openagents/issues/8201) |
| BA-D3 | Enforcement: tests prove no long-lived SCM tokens exist in worker workspaces/homes across materialize/run/closeout; INVARIANTS updated so the brokered-credentials rule cites the implementation + tests | BA-D1, BA-D2 | HIGH | [#8202](https://github.com/OpenAgentsInc/openagents/issues/8202) |

### WS-E — Warm dispatch (harvest H3)

Source: harvest audit §4 H3, re-homed from GCE VM snapshots onto our actual
execution shape (fleet workers materializing workspaces per task): the
three warming layers become worktree caching, prebuilt baselines, and
warm-on-intent. The UX delta between "fire and forget in seconds" and
"fire and wait".

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-E1 | Prepared-worktree cache in the Pylon workspace materializer: typed reuse reasons (post-completion snapshot, restore = quick sync + reset), cache keyed by repo+baseline, integrity checks, bounded disk budget with eviction | — | MED | [#8203](https://github.com/OpenAgentsInc/openagents/issues/8203) |
| BA-E2 | Prebuilt baselines: staleness check against upstream commits with a bounded refresh cadence; cold dispatches start from the newest prebuilt baseline (deps installed, setup pre-run) instead of full clone+setup; registry rows + honest metrics on hit/miss | BA-E1 soft | MED | [#8204](https://github.com/OpenAgentsInc/openagents/issues/8204) |
| BA-E3 | Warm-on-intent: Khala Code composer emits a typed, debounced, owner-scoped pre-materialize signal while a fleet/background run is being composed; honest no-op when the target lane has no warm path; test coverage for debounce + gating | BA-E1 | HIGH | [#8205](https://github.com/OpenAgentsInc/openagents/issues/8205) |

### WS-F — Dispatch reliability (harvest H5 + H7)

Source: harvest audit §4 H5/H7, slimmed to our seams: a typed error
taxonomy + circuit breaker on dispatch lanes, and ack/replay for the small
load-bearing event class while bulk streams stay fail-soft.

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-F1 | Lane/account error taxonomy + circuit breaker: typed transient/permanent classification for dispatch failures in the delegate program + orchestration store; per-account/lane breaker with cooldown feeding capacity/readiness instead of repeated failed dispatches | — | MED | [#8206](https://github.com/OpenAgentsInc/openagents/issues/8206) |
| BA-F2 | Critical-event ack/replay in Pylon: a named critical class (closeout, push/PR, receipt, claim-release) gets ack-id buffering + re-send-until-ack in the lifecycle/reporting path; bulk raw-event chunks remain fail-soft; ingest-side dedupe by ack id | — | MED | [#8207](https://github.com/OpenAgentsInc/openagents/issues/8207) |

### WS-G — Client surfaces (harvest H6 + H8; definitions audit §4.5)

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-G1 | Integration template + Forum-triggered runs: external event → verify → normalize → definition run → completion callback posts the result back to the source thread; Forum first (Forum stays posting/moderation authority) | BA-A2, BA-B3 | MED | [#8208](https://github.com/OpenAgentsInc/openagents/issues/8208) |
| BA-G2 | GitHub @mention runs on the same template: mention → bounded definition run → result comment; strict-bug/issue-form policy respected (no loose issue spam) | BA-G1 | HIGH | [#8209](https://github.com/OpenAgentsInc/openagents/issues/8209) |
| BA-G3 | Per-run live surface (thin-DO pattern) — design doc + spike: per-run Durable Object with injected services, in-DO migrations, hibernation-safe client mapping, single multiplexed alarm; adopted only when the WS-10 status spine grows a client-facing live channel (Durable Streams remain the default) | BA-A2 | LOW | [#8210](https://github.com/OpenAgentsInc/openagents/issues/8210) |
| BA-G4 | Khala Code Agents panel + escalation inbox + CLI parity: definitions list (name, goal, harness badge, lane, last run, next trigger), typed create/edit form, per-agent run history off Durable Streams, `ask`-escalations inbox view; `khala agents list\|create\|run\|logs` | BA-A2, BA-B5 | MED | [#8211](https://github.com/OpenAgentsInc/openagents/issues/8211) |

### WS-H — Event ledger / unified inbox (definitions audit §4.4)

The triage substrate ("not an agent") that follow-up/triage definitions
query. Last because triggers deliver value without it.

| Task | Description | Deps | Delegable | Issue |
| --- | --- | --- | --- | --- |
| BA-H1 | `event_ledger.v1`: Queues ingest → D1 rows (source, externalRef, actor, content ref, timestamps) + per-owner DO for ordering/dedup; GitHub source first; owner-scoped, never training data, never leaves the account boundary | BA-B3 | MED | [#8212](https://github.com/OpenAgentsInc/openagents/issues/8212) |
| BA-H2 | Handled-state as first-class (`open\|handled\|responded\|ignored`, which run touched it) + a gateway read tool for definitions with redaction per `secretPolicy` | BA-H1 | MED | [#8213](https://github.com/OpenAgentsInc/openagents/issues/8213) |
| BA-H3 | Slack source ingest → normalized ledger rows; same owner-scope + privacy invariants | BA-H1 | HIGH | [#8214](https://github.com/OpenAgentsInc/openagents/issues/8214) |

### Behavior-contract tie-ins (cross-cutting, [#8218](https://github.com/OpenAgentsInc/openagents/issues/8218))

Several tasks above state headline invariants whose enforcement artifacts
are behavior contracts (`packages/behavior-contracts`, AGENTS.md standing
rule, ROADMAP_QA §9d). The implementing PRs register the contract alongside
the oracle rather than leaving the rule as INVARIANTS prose:

- BA-B4 → `background_agents.dispatch.budget_caps_enforced.v1`
- BA-A5 → `background_agents.toolset.compiled_policy_enforced.v1`
- BA-D3 → `background_agents.credentials.no_long_lived_tokens_in_workspaces.v1`
  (BA-D3's test *is* the oracle)
- BA-A4 → `background_agents.definitions.harness_swap.v1` (the parity
  fixture is the oracle)
- BA-G4 → an indicator-truthfulness UX contract for the Agents panel's
  run-status indicators, written **before** the panel ships (the
  `khala_code.chat.sidebar_spinner_streaming_only.v1` bug class)
- BA-E3 → pin the stated "honest no-op when the target lane has no warm
  path" behavior

Future product promises from this program cite these contract ids in
`evidenceRefs` (`contract:<id>` cross-refs), and contract deviations should
eventually land as WS-H ledger rows with handled-state rather than
fire-and-forget alerts.

## 3. Waves (Parallelization Plan)

- **Wave 0 (now):** BA-A1 · BA-C1 · BA-D1 · BA-E1 · BA-F1 · BA-F2 — six
  independent lanes, zero interference. BA-A1 is a shared seam: lands first
  and alone within WS-A.
- **Wave 1:** BA-A2 · BA-A5 · BA-B1 · BA-C2 · BA-D2 · BA-E2 · BA-E3.
- **Wave 2:** BA-A3 · BA-B2 · BA-B3 · BA-D3.
- **Wave 3:** BA-A4 · BA-B4 · BA-B5 · BA-G1 · BA-H1.
- **Wave 4:** BA-G2 · BA-G4 · BA-H2 · BA-H3 · BA-G3 (design-first).

Seam rule (same as ROADMAP §3): BA-A1 and BA-B1 land first and alone;
everything downstream codes against their merged interfaces.

## 4. Milestones

- **M1 — Definition spine**: BA-A1..A3 merged; a definition dispatches a
  real Codex run on the owner's Pylon with receipts and a linked Forge work
  record.
- **M2 — Background for real**: BA-B1/B2/B4 merged; a cron-triggered
  definition runs unattended with budget caps and auto-pause. Smallest
  end-to-end proof: the "what do I need to follow up on" definition against
  GitHub notifications.
- **M3 — Harness-swap proof**: BA-A4 — one unchanged definition runs on
  Codex and Claude; demo-grade.
- **M4 — Hardened + warm dispatch**: WS-C + WS-D + BA-E1 — no long-lived
  credentials in worker workspaces; custody-backed subscription auth; warm
  worktree reuse measured.
- **M5 — Surfaces**: BA-G1 + BA-G4 — Forum-triggered runs and the Agents
  panel; escalation inbox usable.
- **M6 — Inbox substrate**: WS-H; the triage-query definitions become
  definable.

## 5. Invariants (Non-Negotiable Across All Workstreams)

Inherited from the harvest audit §5, the definitions audit §6, and the
Forge boundary contract:

- **Owner-scoped everything.** No shared-App single-tenant shortcuts: every
  definition, run, token, and ledger row is owner-scoped; the dispatch gate
  remains the admission authority; Forge tenant isolation rules hold.
  Nothing from Open-Inspect's authorization model ports.
- **Execution stays ours.** Background runs execute on the owner's Pylon
  fleet (or a future owner-approved cloud lane) — never third-party sandbox
  vendors, never in-session agents.
- **Exact-only accounting.** Runs settle exact `token_usage_events` (and
  receipts where applicable); no informal cost accumulation; budget caps
  enforced at dispatch, not advisory.
- **Enforced toolsets or no product claim.** If any lane lets a background
  agent reach tools outside its compiled policy, the feature claim is
  false; INVARIANTS entries + boundary tests land before any public
  promise. Approvals (`ask`) must be one keystroke or users will widen
  allowlists.
- **Claims + verification authority.** Parallel background runs use the
  claim registry; Forge verification receipts and gated promotion (and
  verify commands on the Pylon lane) stay the merge authority; advisory
  judgment never overrides them.
- **Auth-plane separation.** Tenant git tokens never authorize
  control-plane calls (Forge boundary contract); definition dispatch uses
  `forge:*` / service scopes only.
- **Isolated homes; neutral commit metadata.** Never `~/.codex` or the live
  `~/.claude`; attribution via work records/closeouts, not commit
  authorship.
- **Patterns-only harvesting.** `projects/repos/background-agents` stays
  read-only; ideas port, code does not; no product-surface naming.
- **Copy gates.** No public copy or promise flips from this program without
  `docs/promises/` records; the smallest honest demo (M2/M3) precedes any
  claim.
- **Headline invariants become behavior contracts.** Where a workstream's
  claim-bearing invariant gains a test (BA-A4/A5/B4/D3/E3/G4), the same PR
  registers the behavior contract with that test as its oracle (#8218), so
  the rule is sweep-enforced and coverage-checked, not prose.

## 6. Parked (No Issues Filed) — GCE/Workroom Lane Items

The following harvest items apply only to the private `cloud/` repo's
GCE/Firecracker workroom lane, which is **not assumed active** for this
program. They are recorded for revival if/when the owner re-opens that
lane; they must not be started from this roadmap:

- Workroom short-lived token pull via `oa-workroomd` gateway (H2's cloud
  half).
- GCE disk-snapshot / machine-image warming and Firecracker memory-snapshot
  parity (H3's VM half).
- Provider trait with capability flags across GCE/Firecracker/SHC in
  `oa-codex-control` (H5's cloud half — BA-F1 covers the taxonomy/breaker
  idea on the fleet lane).
- `oa-workroomd` callback ack-buffer (H7's cloud half — BA-F2 covers the
  Pylon lane).

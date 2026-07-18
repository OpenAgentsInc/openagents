---
spec_format_version: "0.1"
title: "Full Auto Autonomous Run Contract"
artifact_type: "prd"
spec_revision: 11
author: "OpenAgents"
created_at: "2026-07-15T22:15:41.850Z"
updated_at: "2026-07-17T20:30:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-criterion-disposition-map"
    label: "Criterion Disposition Map (Rev 9 -> Rev 10)"
    after: "acceptance_criteria"
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_issue: "8852 (initial), 8853 (restart-durable continuation), 8875 (FA-H2 workspace binding), 8876 (FA-H3 exactly-once dispatch), 8877 (FA-H4 background in-flight state, stop, send-fencing), 8878 (FA-H5 failure policy), 8879 (FA-H6 profile continuity), 8880 (FA-H7 cap semantics), 8882 (FA-H9 metrics), 8883 (FA-H10 registry robustness), 8885 (FA-H12 two-process restart smoke), 8886 (FA-H13 local programmatic control surface)"
  openagents_design_doc: "docs/fable/2026-07-15-full-auto-repo-intent-to-dispatch-loop.product-spec.md"
  openagents_assurance_spec: "specs/desktop/full-auto.assurance-spec.md"
  openagents_assurance_spec_status: "stale_pending_reconciliation: bound to rev 9 document digest and FA-AC-01..37 only; rev 10 adds FA-AC-38..66 and a criterion disposition map that the AssuranceSpec has not yet absorbed. Reconciliation (new obligations, retired/changed obligation mapping, admission) is FA-AS-01 (#8978), which explicitly depends on this revision. Do not treat the existing 37/37 needs_design obligation set as covering rev 10."
  openagents_revision_8_issue: "8901 (L6 provider-lane generalization)"
  openagents_revision_8_note: "Rev 8 generalizes the durable loop over the L1 ProviderLane SPI. The additive optional profile.lane defaults legacy rows to codex-local; reconciliation capability-gates the selected lane; built-in Codex and Claude use one lane-keyed instruction/background-question policy; and control start/enable, OpenAPI, MCP, and CLI accept an optional lane selector. Claude background questions deny immediately with proceed-with-judgment guidance instead of parking without a renderer."
  openagents_revision_9_issue: "8902 (L7 lane-independent ProductSpec/AssuranceSpec workflow)"
  openagents_revision_9_note: "Rev 9 projects a bounded, read-only ProductSpec/AssuranceSpec context through the shared ProviderLane dispatcher for every lane; adds specs/** unmet obligations to Full Auto candidate discovery; and re-runs the authority packages after each dispatched turn to append an evidence-only system note. Provider lanes receive no parsing, admission, verification, release, or public-claim authority."
  openagents_revision_11_issue: "8987 (FA-RT-01, epic #8967)"
  openagents_revision_11_note: "Rev 11 adds the multi-lane never-halt routing policy (FA-RT-01 #8987): the durable per-thread registry record gains OPTIONAL `routingPolicy` (an ordered, bounded list of admitted lane/account candidates, validated fail-closed at bind time by full-auto-routing.ts -- unknown/unadmitted/Full-Auto-ineligible lanes refuse the whole policy) and OPTIONAL `rotationHistory` (bounded typed {fromLane,toLane,reason,at} facts, oldest-evicted). On a typed account_exhausted/rate_limited/provider_error dispatch failure, reconciliation rotates to the next admitted candidate in the SAME pass under a fresh exactly-once lease instead of entering failure backoff; a full unsuccessful cycle through the candidates consumes exactly one FA-H5 failure-budget step, and existing cap/disable/backoff semantics are proven unchanged by regression tests. A v1/v2-era registry file without the new fields decodes and behaves exactly as single-lane. Rotation within an owner-admitted ordered policy is NOT autonomous provider selection: the candidate set and its order are chosen by a human at policy bind time; the loop only fails over inside that grant. Adds FA-AC-67."
  openagents_revision_10_issue: "8968 (FA-RUN-00, epic #8967 child 1 of 12)"
  openagents_revision_10_note: "Rev 10 supersedes the rev-9 composer-toggle product model after the 2026-07-17 owner overnight dogfood run (docs/fable/2026-07-17-full-auto-implementation-audit.md) proved it wrong-shaped: a six-hour silent stall behind a generic composer failure banner, no run-level diagnosis, no durable objective record, and an ambiguous chat-vs-autonomous-program interaction model. This revision (a) maps every FA-AC-01..37 criterion to an explicit disposition in the new Criterion Disposition Map section rather than silently reinterpreting them; (b) adds FA-AC-38..66 defining the target FullAutoRun contract -- stable runRef independent of threadRef, one-active-run-per-profile v1 concurrency, a full Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached lifecycle with actor/time/reason attribution on every transition, run-level liveness distinct from turn duration, a bounded private FullAutoRunReport, a dedicated left-rail launcher plus read-only run view replacing the composer toggle, and a host-owned objective-priority provider-handoff envelope; (c) is the first of 12 issues under epic #8967 -- FA-AC-38 onward name the exact child issue that owns their implementation and their Proof lines are explicitly 'planned' evidence, not yet executed; (d) adds three pending behavior-contract entries (separate launcher, read-only running state, Play/Pause/Stop semantics) to packages/behavior-contracts per the CLAUDE.md behavior-contracts mandate; and (e) extends root INVARIANTS.md Authority Boundaries with the run-authority/state-transition/evidence-privacy invariant this model commits to. This revision changes NO application code: apps/openagents-desktop's full-auto-registry.ts, full-auto-reconcile.ts, full-auto-lane.ts, and full-auto-control-contract.ts are unchanged pending #8969's implementation of the FullAutoRun model this spec now requires. Per the epic's own acceptance criteria, owner/reviewer acceptance identity for this document is NOT yet recorded -- see Owner Gates; this document establishes the pre-implementation contract that blocks conflicting work (specs/CONVENTIONS.md: 'Implementation must not proceed under a ProductSpec that forbids the intended result'), but that is a claim-blocking function, not a substitute for the owner's own sign-off."
---

## Problem

Rev 9 (and the wave of hardening issues #8852-#8902 that produced it) shipped
a durable, restart-survivable Full Auto continuation loop as a single
composer toggle: FA-AC-01 required exactly one `Full Auto` toggle
(`shell-full-auto-toggle`) and explicitly ruled out any new screen or review
surface. That model shipped real durability -- workspace binding, exactly-once
dispatch, typed failure/backoff, execution-profile continuity, a 20-turn cap,
a local programmatic control surface, and provider-lane generalization -- and
its foundations remain sound (see the Criterion Disposition Map below).

The 2026-07-17 owner overnight dogfood run disproved the product model built
on top of that durability. The corrected audit
(`docs/fable/2026-07-17-full-auto-implementation-audit.md`) records what
actually happened: a bounded Fast Follow packet completed successfully after
~14m40s, the next reconciliation then failed closed because a five-thread
mutable cache evicted the still-active Full Auto thread under ordinary
multi-chat pressure, and the owner-facing surface showed only a generic "Turn
failed / That conversation no longer exists" banner for roughly **six hours**
-- the exact window the product was supposed to work unattended. The specific
cache defect is fixed on `main` (`8cb900bbf9`), but the incident exposed
product-model defects the fix does not touch:

- **No run-level diagnosis.** The composer toggle has no concept of "this
  run" independent of "this thread"; there was nothing to ask why the loop
  stopped, when it would retry, or what to do about it.
- **No durable objective.** The registry persists workspace, profile,
  continuation count, and failure state, but never a structured objective or
  done condition -- so a recovered or provider-switched run has no mission
  contract to fall back on beyond bounded transcript notes.
- **An ambiguous interaction model.** A composer toggle asks the user to read
  one canvas simultaneously as an interactive chat they steer message by
  message and an unattended program they start and walk away from. FA-AC-01's
  "no new screen" requirement is precisely what produces that ambiguity.
- **No run report or transcript-quality loop.** "Dogfood it, inspect it,
  improve it, repeat" is currently a manual forensic exercise over private
  provider JSONL and Desktop state, because no artifact summarizes a run's
  turns, outcomes, commits, failures, and provider transitions.
- **Provider handoff is plumbing without a proven experience.** Provider-lane
  infrastructure exists (registry, bounded-history projection, capability
  admission), but no test has proven a real, same-thread, sequential
  Codex-to-Claude (or reverse) handoff with legible context and a visible
  transition receipt.

Implementation must not proceed under a ProductSpec that forbids the intended
result. This revision is the first of 12 issues under epic #8967 and exists
to resolve that contract conflict before any run-model or UI implementation
lands.

## Hypothesis

If Full Auto becomes a named, durable **autonomous run** -- with a stable
`runRef` independent of `threadRef`; an explicit objective and done condition
as first-class durable fields; a v1 policy of at most one active run per
Desktop profile; a full lifecycle state machine (Draft, Running, Pausing,
Paused, Retrying, Stalled, Completed, Failed, Stopped, Cap-reached) where
every transition carries actor, timestamp, and typed reason; run-level
liveness distinct from healthy long provider-turn duration; a bounded private
run report; a dedicated left-rail launcher and read-only run view that
replaces the composer toggle and removes the ordinary composer while a run is
active; and a host-owned, objective-priority provider-handoff envelope with a
visible transition receipt -- then an owner can start a run, walk away, and
later answer every question in epic #8967's Outcome section (is it running,
paused, retrying, stalled, completed, failed, stopped, or capped; what
governs it; what did it accomplish; why did it stop and what can I do; what
context moved on a provider switch and what didn't) directly from Desktop
state, without forensic access to raw provider logs. The six named sidebar
dogfood tests in the audit (Codex-to-Claude and reverse context handoff,
objective retention under context pressure, a three-turn unattended run, a
restart-survival run, and a thread-pressure replay of the actual incident)
should then pass in the owner's real profile.

This revision commits the target contract; FA-AC-38 onward are implemented
across the 11 remaining child issues of epic #8967, each named in its own
Proof line.

## Scope

```productspec-scope
in:
  - preserved from rev 9, unchanged mechanism (see Criterion Disposition Map for the exact per-criterion accounting): `approvalPolicy: "never"` forced on a Full Auto turn; a durable main-owned per-thread/run registry persisted the way local-turn-journal.ts persists interrupted-turn state with every post-v1 field optional; workspace authority binding (fail-closed on mismatch or unbound); exactly-once continuation dispatch via a serialized promise-chain mutex plus a durable per-thread dispatch lease; typed dispatch-failure policy with bounded exponential backoff and a 5-consecutive-failure disable; execution-profile (lane/account/model/effort) continuity across continuations and restarts; the 20-consecutive-continuation safety cap; corrupt-registry quarantine; enabled/non-terminal-record eviction protection; the opt-in loopback bearer-gated local programmatic control surface (OpenAPI/MCP/CLI triad) with attributed system notes on every mutating call; provider-lane generalization (L1 SPI dispatch, L2 capability admission, lane-keyed background-question policy); and the bounded ProductSpec/AssuranceSpec spec-lane projection and post-turn revalidation note on every dispatched turn across every admitted lane
  - a stable `runRef` identity independent of `threadRef`, with title, objective, explicit done condition, workspace, provider profile, and turn cap as first-class durable run fields (FA-RUN-01, #8969)
  - v1 concurrency policy: at most one active (non-terminal) Full Auto run per Desktop profile by default; additional Draft or terminal (Completed/Failed/Stopped/Cap-reached) run records may exist without limit beyond existing registry bounds (FA-RUN-01, #8969)
  - rerun/new-generation semantics: starting a new run from a terminal run always mints a new distinct `runRef` and never mutates the terminal record; a new run may optionally reference a prior run as context for the report/transcript-analysis loop (FA-RUN-01, #8969; FA-RUN-05, #8973)
  - a full run lifecycle state machine -- Draft, Running, Pausing, Paused, Retrying, Stalled, Completed, Failed, Stopped, Cap-reached -- with actor/time/reason attribution recorded on every transition, extending the existing `disabledBy` attribution pattern to the complete state graph (FA-RUN-01, #8969)
  - Pause with an active provider turn transitions to Pausing until that turn resolves (completes or is interrupted), then Paused; Pause with no turn in flight transitions directly to Paused; Resume is legal only from Paused and dispatches at most once through the existing exactly-once lease path; Stop is a terminal transition legal from any non-terminal state and is distinct from Pause -- a stopped run is never resumed (FA-RUN-01, #8969)
  - provider-declared turn completion and product/evidence-level run disposition are tracked as distinct facts: a provider reporting a turn as done does not by itself assert the run's objective/done-condition was satisfied; automatic verification of done-condition satisfaction is explicitly deferred (see cut list) and Completed remains a self-reported, owner-reviewable disposition backed by the run report, not a verified-truth claim
  - missing/orphaned `threadRef` or provider-session recovery at reconciliation is a typed fail-closed disposition (Stalled or Failed with an owner-visible reason) -- never silent reattachment to an unrelated thread and never the six-hour silent stall the audit recorded (FA-RUN-02, #8970; FA-RUN-03, #8971)
  - additive, idempotent migration from the legacy `enabled: boolean` per-thread registry rows to exactly one migrated run record per previously-enabled thread on first startup after the model ships, with no data loss and no duplicate migration on a later restart (FA-RUN-01, #8969)
  - run-level liveness distinct from a healthy long provider turn: a run is live while a turn is genuinely executing OR while time-since-last-dispatch is within a defined SLO window; outside that window it is Stalled with an explicit, owner-visible cause and retry ETA rather than silence (FA-RUN-03, #8971)
  - a bounded private `FullAutoRunReport` per run: run ref, thread ref, title, objective, workspace, started/stopped timestamps, provider/lane per turn and every provider transition, per-turn disposition/duration/selected packet/outcome summary, commits/receipts claimed (verified independently where possible), failure classification/retry/recovery-action/disabled-reason, liveness gaps over threshold, objective/acceptance progress and remaining work, transcript-analysis findings, and a pointer to private raw evidence rather than raw transcript contents (FA-RUN-04, #8972)
  - offline/private dogfood transcript analysis producing measurable, comparable quality findings across runs (duplicated setup, drift, stalls, unclear UI state, false completion claims), reusing #8911's default-off Desktop usage/telemetry plumbing and consent boundary rather than a parallel telemetry path (FA-RUN-05, #8973)
  - a dedicated **Full Auto** launcher action beside/under **New session** in the left rail, collecting title (auto-suggested, editable), objective and explicit done condition, workspace, provider/lane, and a bounded turn cap (default 20, clearly shown), replacing the composer toggle as the run's entry point (FA-UX-01, #8974)
  - after Start, the main canvas becomes a dedicated **read-only run view** for v1: objective/workspace pinned at the top, explicit current state (Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached), Pause/Resume as the primary control depending on state, Stop as a distinct terminal control, an inspectable per-turn transcript (provider, duration, outcome, artifacts) without live token-by-token streaming, and the ordinary composer absent while the run is active (FA-UX-01, #8974)
  - sidebar status/title/search/navigation reflects a run's live state and objective-derived title rather than a first-message title, closing the "Hello" concealed-program defect the audit recorded (FA-UX-01, #8974; reuses generic-title work tracked at #8940)
  - a host-owned, objective-priority provider-handoff envelope for the existing manual same-thread Codex<->Claude switch and for an explicit Pause -> switch provider -> Resume sequence, carrying a visible from/to/actor/time/reason/truncation receipt into the thread and the run report, target admission/auth/capability re-checks with rollback on refusal, and an explicit statement that no provider-private session state is implied to transfer -- only the host-owned bounded projection (FA-HO-01, #8975)
  - provider support/picker truth derived from exact admitted evidence: a lane is presented as Full-Auto-eligible or handoff-eligible only when its L2 capability admission and background-question policy are proven, never because an adapter merely exists (FA-HO-01, #8975; repairs the ACP-picker gap tracked at #8977)
  - the six named real-sidebar dogfood tests from the audit (Codex->Claude context, Claude->Codex context, objective retention under context pressure, a three-turn unattended Codex run, a Claude run surviving restart, and the thread-pressure replay of the actual incident) as named, retained release-gate evidence, run in the owner's real Desktop profile rather than a headless fixture (FA-QA-01, #8976)
  - a superseding AssuranceSpec design/admission/execution/review pass covering the FA-AC-38..66 obligations this revision adds, reconciling rather than discarding the existing 37/37 `needs_design` FA-AC-01..37 obligation set per each criterion's disposition below (FA-AS-01, #8978)
  - packaged release and product-promise admission gated on the above, including a signed build from an exact tag containing the run model passing the owner restart-resume observation (FA-REL-01, #8979)
  - a multi-lane never-halt routing policy on the durable per-thread registry record: an OPTIONAL ordered, bounded list of admitted lane/account candidates (`routingPolicy`) validated fail-closed at bind time (unknown, unadmitted, or Full-Auto-ineligible lanes refuse the whole policy at validation, never at dispatch), plus an OPTIONAL bounded typed rotation history (`rotationHistory`: fromLane/toLane/reason/at, oldest-evicted) surfaced through the control-API status projections; on a typed account_exhausted/rate_limited/provider_error dispatch failure the reconciler rotates to the next admitted candidate in the same pass under a fresh exactly-once lease, a full unsuccessful cycle consumes exactly one FA-H5 failure-budget step, and legacy single-lane records behave exactly as before (FA-RT-01, #8987)
out:
  - Phase 2 cross-machine programmatic control (relaying Full Auto routes through the openagents.com OpenAPI/Omni-SDK/public-MCP triad and Khala Sync to a running Desktop); the control surface remains same-machine loopback only
  - the control API granting a new, previously-ungranted workspace; granting stays a human/UI action
  - concurrent multi-run execution, multi-repo Full Auto, and fleet-wide scheduling; v1 enforces exactly one active run per Desktop profile (see in:)
  - autonomous provider selection; a run's provider/lane is chosen at launch (or at an explicit Pause -> switch -> Resume) by a human, never decided by the loop itself -- rev 11 clarification: typed failover WITHIN an owner-admitted ordered routing policy (FA-RT-01, #8987) is not autonomous selection, because the candidate set and its order are human-chosen at policy bind time and the loop can never rotate outside that grant
  - free-form steering into a running autonomous run; the only v1 steering path is the explicit Pause -> add instruction (via provider switch or a future dedicated steering feature) -> Resume sequence -- an always-visible chat box during an active run is explicitly rejected by this revision
  - automatic verification that a run's stated done condition was actually satisfied; Completed remains a self-reported, owner-reviewable disposition in v1 (see in: and the Criterion Disposition Map)
  - live, token-by-token streaming of a run's in-progress turn into the read-only run view; the view shows coarse typed state plus the completed per-turn transcript, not a live token feed
  - a separate permission/envelope/policy system beyond the run lifecycle and the existing full-trust Codex/Claude execution profile every other Desktop turn already uses
  - claiming ACP Full Auto or handoff readiness before an ACP lane's admitted peer profile and background-question behavior are proven; unknown/unadmitted lanes fail closed
  - any change to release or public-claim authority beyond what FA-REL-01 (#8979) explicitly gates
cut:
  - CUT-FA-01: fine-grained autonomy policy beyond Pause/Resume/Stop and the 20-turn safety cap
  - CUT-FA-02 (rev 1): main-process durable goal state for restart-survivable continuation -- CLOSED (rev 2, #8853)
  - CUT-FA-03: per-thread toggle-state resync on arbitrary in-session thread switch -- superseded/moot: rev 10 replaces the composer toggle with a dedicated run view that is not a per-thread visibility toggle (see Criterion Disposition Map, FA-AC-19/FA-AC-21)
  - CUT-FA-04: automatic done-condition verification (provider or product code deciding a run's objective was actually satisfied) -- deferred past this revision; Completed stays self-reported/owner-reviewable
  - CUT-FA-05: concurrent multi-run execution and multi-repo/fleet-wide Full Auto -- deferred past this revision; v1 is one active run per Desktop profile
  - CUT-FA-06: autonomous (loop-decided) provider selection and free-form mid-run steering -- deferred past this revision; provider/lane and objective are set at launch or through the explicit Pause -> switch/instruct -> Resume sequence only
```

## Acceptance Criteria

FA-AC-01 through FA-AC-37 below are the exact criteria admitted in rev 9,
kept verbatim for auditability. The Criterion Disposition Map immediately
following resolves every one of them for rev 10. FA-AC-38 onward are new
criteria this revision introduces for the target autonomous-run contract;
each names the exact epic #8967 child issue that owns its implementation, and
its Proof line is explicitly **planned** evidence -- not yet executed -- until
that issue lands.

- **FA-AC-01:** The composer renders exactly one `Full Auto` toggle
  (`shell-full-auto-toggle`), off by default, with `aria-pressed` reflecting
  state. No other new screen or review surface ships with this spec.
  Proof: `react-composer.test.tsx` "Full Auto (#8852): renders as an
  off-by-default composer toggle and reports DesktopFullAutoToggled".
- **FA-AC-02:** A Codex-lane turn started with Full Auto on sends
  `approvalPolicy: "never"` on both `thread/start` and `turn/start`, and its
  prompt is prefixed with the Full Auto instruction; an ordinary turn keeps
  `approvalPolicy: "on-request"` and an unprefixed prompt.
  Proof: `codex-local-runtime.test.ts` "Full Auto (#8852) forces
  approvalPolicy never and prefixes the turn prompt..." and "an ordinary
  (non-Full-Auto) app-server turn keeps approvalPolicy on-request...".
- **FA-AC-03:** A completed Full-Auto turn sends `fullAuto: true`
  exactly once from the renderer; the renderer never loops. Continuation is
  decided in main by `reconcileFullAutoThreads`, called both right after that
  turn completes and once at app startup.
  Proof: `shell.test.ts` "a flagged turn sends fullAuto:true exactly once --
  main, not the renderer, decides whether to continue"; `main.ts`'s
  `dispatchCodexLocalTurn` calling `runFullAutoReconciliation()` after a
  successful Full-Auto turn (code-reviewed; main.ts has no direct unit-test
  harness, see Receipts for the isolated-module proof used instead).
- **FA-AC-04:** Toggling Full Auto off persists to main immediately
  (`CodexLocalFullAutoSetChannel`), independent of whether a turn is in
  flight, so a toggle-off durably stops the loop even if the app quits before
  the next turn would have started.
  Proof: `shell.test.ts` "DesktopFullAutoToggled flips the flag and persists
  it to main immediately"; `full-auto-restart.e2e.test.ts` "toggling off
  before restart durably stops it -- Runtime B never dispatches".
- **FA-AC-05:** When Full Auto is off, an ordinary turn sends `fullAuto`
  undefined (not `false`) and never resubmits automatically.
  Proof: `shell.test.ts` "toggled off, an ordinary Codex turn sends fullAuto
  undefined and never resubmits".
- **FA-AC-06:** A run of 20 consecutive automatic continuations turns Full
  Auto off durably (registry, not renderer state) and appends an explanatory
  system note, rather than continuing unbounded -- and this holds even if a
  restart happens partway through the count. The consecutive-continuation
  counter resets only when Full Auto is toggled off for that thread; a manual
  send while the toggle stays on does NOT reset it, and re-enabling an
  already-enabled thread preserves the count. Since rev 4 the counter
  increments only on a SUCCESSFUL dispatch: a failed dispatch consumes
  failure/backoff budget (FA-AC-16), never a cap slot.
  Proof: `full-auto-restart.e2e.test.ts` "a genuinely stuck loop self-disables
  at the continuation cap across restarts, rather than continuing unbounded"
  and "failed dispatches never consume cap slots: fail once then succeed ->
  continuationCount is exactly 1"; `full-auto-registry.test.ts`
  "continuationCount resets ONLY on toggle-off: a manual send leaves it
  unchanged; off-then-on zeroes it".
- **FA-AC-07:** A thread left enabled with no turn in flight when
  the app quits resumes its next continuation on its own at the next launch,
  with no user action beyond the original toggle.
  Proof: `full-auto-restart.e2e.test.ts` "a thread left enabled by Runtime A
  resumes on Runtime B with no manual re-toggle or re-send".
- **FA-AC-08:** A thread whose turn was still in flight when the
  app quit is left alone by Full Auto reconciliation until existing
  interrupted-turn recovery resolves it -- Full Auto never races or
  duplicates that recovery.
  Proof: `full-auto-restart.e2e.test.ts` "a thread with a turn still in
  flight at restart is left alone until that turn resolves"; the real
  wiring sequences `runFullAutoReconciliation()` after `localTurnRecovery`
  resolves, and computes `nonterminalThreadRefs` from the same
  `localTurnJournal.nonterminal()` that recovery itself owns.
- **FA-AC-09:** A brand new thread (no id yet when the user
  toggles Full Auto on) persists its enabled state to main once it actually
  gets a real thread id, rather than silently dropping the toggle's intent.
  Proof: `shell.test.ts` "a brand new thread persists its enabled state to
  main once it has a real id".
- **FA-AC-10:** No Full Auto packet performs a direct commit, merge, or push;
  Codex proposes changes exactly as every other Desktop Codex turn already
  does. (Unchanged existing boundary; no new authority was added.)
- **FA-AC-11:** A corrupt or schema-invalid registry file never
  blocks Desktop main initialization. Opening it fails closed for the feature
  and open for the app: the bad file is quarantined beside the registry
  (best-effort rename to `registry.json.quarantined-<ISO timestamp>` with an
  owner-visible console diagnostic naming the quarantine path), the registry
  starts empty (Full Auto disabled for all threads), and subsequent writes
  persist normally.
  Proof: `full-auto-registry.test.ts` "a corrupt registry file is quarantined
  and the registry opens empty instead of throwing" and "a schema-invalid (but
  valid JSON) registry file is also quarantined rather than thrown".
- **FA-AC-12:** Registry record eviction never drops an
  `enabled: true` record. All enabled records are kept; only the disabled tail
  is bounded, filling remaining capacity (up to 128 total) with the
  most-recently-updated disabled records. An owner-enabled thread therefore
  always survives to the next restart, no matter how many other records were
  touched more recently.
  Proof: `full-auto-registry.test.ts` "eviction never drops an enabled record:
  the oldest enabled thread survives while old disabled records are evicted".
- **FA-AC-13:** Enabling Full Auto binds the currently resolved workspace onto
  the durable record -- resolved by main from the exact same source of truth
  codex-local turns execute against, never a renderer-supplied path. A
  continuation whose currently-resolved workspace differs from the recorded
  binding does NOT dispatch: the record is disabled with
  `blockedReason: "workspace_mismatch"` and an owner-visible system note
  explains that Full Auto was turned off because the granted workspace no
  longer matches.
  Proof: `full-auto-restart.e2e.test.ts` "enable on workspace A, resolve
  workspace B at reconcile -> no dispatch, record disabled with
  workspace_mismatch, block reported"; `main.ts` binds via
  `resolveDesktopLocalWorkspaceRoot()` in the `CodexLocalFullAutoSetChannel`
  handler and passes the same resolver into reconciliation (code-reviewed;
  main.ts has no direct unit-test harness).
- **FA-AC-14:** An enabled record with NO recorded workspace (a pre-upgrade v1
  row) fails CLOSED at dispatch: it is never silently adopted onto the current
  workspace -- the record is disabled with
  `blockedReason: "workspace_unbound"` and an owner-visible note. The binding
  is (re)established only by a successful ENABLE, which always records the
  then-current workspace.
  Proof: `full-auto-restart.e2e.test.ts` "an enabled record with NO workspace
  binding (pre-upgrade v1 row) fails CLOSED: no dispatch, disabled with
  workspace_unbound".
- **FA-AC-15:** Continuation dispatch is exactly-once. All reconciliation
  triggers in main serialize through a promise-chain mutex, and before
  dispatching a thread the reconciler durably claims a per-thread lease
  carrying the exact continuation turn ref (the lease identity and the
  dispatched turn identity are the same value). Two overlapping reconcile
  passes dispatch an enabled thread at most once. The lease releases on
  dispatch completion (success or failure). Only the STARTUP pass clears a
  stale lease -- one whose turn ref has no nonterminal local-turn journal row
  (a dispatch that crashed before its turn was accepted); a mid-session pass
  treats a held lease as in-flight and skips. As defense in depth, main's
  dispatch adapter refuses to start a continuation when the local-turn
  journal already holds a nonterminal turn on that thread.
  Proof: `full-auto-restart.e2e.test.ts` "audit probe (a): two overlapping
  reconcile passes against one enabled thread dispatch it exactly ONCE
  (durable lease), and continuationCount increments by exactly 1", "the
  serial task queue serializes overlapping reconciliation triggers...", "a
  stale lease (crashed mid-dispatch: no journal row for its turn ref) is
  cleared ONLY by the startup pass...", and "a lease whose turn IS still
  nonterminal in the journal is NOT cleared at startup...";
  `full-auto-registry.test.ts` "claimPending holds the lease exactly once
  until cleared; a missing record can never be claimed".
- **FA-AC-16:** A failed continuation dispatch -- thrown OR `{ ok: false }` --
  is a typed, owner-visible outcome, never a silently dormant enabled record.
  Failure persists `consecutiveFailures`, `lastFailureAt`, and a bounded
  `blockedReason` on the record, releases the lease, and appends an
  owner-visible system note. Retries respect bounded exponential backoff:
  dispatch is skipped while the record is within
  `min(2^consecutiveFailures * 30s, 30min)` of `lastFailureAt`. The 5th
  consecutive failure disables the record durably (with the failure reason as
  `blockedReason`) and a final note says so. A successful dispatch clears all
  failure state.
  Proof: `full-auto-restart.e2e.test.ts` "audit probe (b): an { ok: false }
  dispatch is a typed, visible failure...", "a thrown dispatch is the same
  typed failure outcome as ok:false", "the bounded backoff window skips
  dispatch after a failure, then allows it once the window has passed", and
  "the 5th consecutive failure disables the record with a blockedReason and
  reports disabled: true"; `full-auto-registry.test.ts` "recordFailure
  increments and stamps typed failure state (releasing the lease);
  recordSuccess clears all of it".
- **FA-AC-17:** Automatic continuations preserve the initiating turn's
  execution profile. When a renderer-initiated turn carries
  `fullAuto: true`, main binds its effective account target, model, and
  reasoning effort onto the durable record; every continuation (including a
  post-restart resume) replays that bound profile, revalidated against the
  live contract enums (a field that no longer decodes falls back to lane
  defaults instead of failing the loop). Fields that deliberately RESET on a
  continuation: images, explicit context attachments, and extension
  selection -- a continuation is a fresh instruction, not a replay of the
  initiating turn's payload.
  Proof: `full-auto-restart.e2e.test.ts` "a continuation dispatch carries the
  profile bound by the initiating flagged turn (account, model, effort) --
  including across a restart" and "decodeCodexLocalContinuationProfile
  revalidates stored strings against the live contract...".
- **FA-AC-18:** The wave-2 registry schema upgrade is strictly additive: every
  new record field (workspace binding, profile, lease, failure state) is
  optional, and an existing v1 registry file decodes without quarantine so no
  user's enabled state is lost by upgrading.
  Proof: `full-auto-registry.test.ts` "an existing v1 registry file (no
  wave-2 fields) still decodes -- the schema upgrade never quarantines a
  user's state".
- **FA-AC-19:** A background (main-initiated) continuation is rendered as a
  coarse, typed, per-thread in-flight state, not silence until completion.
  Main owns an in-memory live-state map (idle | turn_running |
  turn_completed | turn_failed | cap_reached | blocked; blocked carries the
  typed blockedReason as bounded detail) and broadcasts every transition to
  all windows over `CodexLocalFullAutoStateChannel`: turn_running with the
  lease turn ref at dispatch start, turn_completed on success, turn_failed
  with the typed reason on an ordinary failure, cap_reached at the cap, and
  blocked on a workspace or failure-limit disable. Terminal states persist
  until the next transition. The extended get channel additively returns
  `{ state, turnRef }` beside `enabled`, and while the active thread's state
  is turn_running the composer renders a "Full Auto running…" status badge.
  Token-by-token streaming remains deliberately out of scope.
  Proof: `shell.test.ts` "FA-H4 (#8877): withFullAutoLiveState projects a
  live-state event per thread and activeFullAutoTurnRunning reads only the
  ACTIVE thread"; `react-composer.test.tsx` "FA-H4 (#8877): a running
  background Full Auto turn renders the status badge and the Stop
  affordance; idle renders neither"; `main.ts` wires the transitions around
  the existing `runFullAutoReconciliation` dispatch adapter and callbacks
  (code-reviewed; main.ts has no direct unit-test harness).
- **FA-AC-20:** A working stop targets the ACTUAL background turn. While the
  active thread's live state is turn_running (renderer non-pending), the
  composer's Stop control dispatches the same interrupt intent, whose
  handler calls the thread-scoped `CodexLocalFullAutoInterruptChannel` with
  only `{ threadRef }`; main resolves the live running turn ref itself and
  signals the exact same `codexLocal.interrupt` runtime path the existing
  turn-interrupt channel uses, answering `{ ok: boolean }`. While the
  renderer's OWN turn is pending, Stop keeps signalling the active streaming
  turn unchanged. The interrupted background turn terminates through the
  existing FA-H5 typed-failure path; the toggle remains the durable
  loop-level stop.
  Proof: `shell.test.ts` "FA-H4 (#8877): DesktopTurnInterrupted with a
  running BACKGROUND turn (not pending) calls fullAutoHost.interrupt with
  the active threadRef" and "FA-H4 (#8877): while renderer-pending, Stop
  keeps signalling the ACTIVE streaming turn (chat.interruptActive), not the
  background channel"; `react-composer.test.tsx` (Stop affordance case
  above); `main.ts` interrupt handler (code-reviewed).
- **FA-AC-21:** A manual send while a background Full Auto turn owns the
  thread is excluded, never run silently concurrently. When the active
  thread's live state is turn_running, `runNoteSubmission` refuses to start
  a manual turn: it sets the transient notice "Full Auto is running a turn
  on this thread. Stop it first or wait for it to finish." and keeps the
  composer draft. Once the live state is terminal, the same submit goes
  through normally.
  Proof: `shell.test.ts` "FA-H4 (#8877): a manual send while a background
  Full Auto turn runs is FENCED -- sendMessage is never called, a notice
  says why, and the draft is kept".
- **FA-AC-22:** The programmatic control surface is opt-in and off by
  default, loopback-only, and bearer-gated. Desktop main constructs the
  control server ONLY when `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1`; the
  listener binds 127.0.0.1 exclusively (ephemeral or env-pinned port); every
  request -- the OpenAPI document included -- requires the per-process scoped
  bearer credential (scopes drawn from `@openagentsinc/environment-auth`'s
  narrowing-only exchange, verified with a constant-time comparison) or is
  refused 401. Connection info is written mode-0600 to
  `full-auto/control.json` under userData and removed on stop.
  Proof: `full-auto-control-server.test.ts` "off by default: main's guard
  requires OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 exactly", "credential mint
  uses the environment-auth narrowing-only exchange...", "auth: no bearer and
  a wrong bearer are 401 on every route...", and "the connection file is
  written mode 0600..."; `main.ts` wraps the entire server wiring in
  `isFullAutoControlEnabled(process.env)` (code-reviewed; main.ts has no
  direct unit-test harness -- the guard function itself is the tested unit).
- **FA-AC-23:** Programmatic enable NAMES the workspace the caller expects
  and enforces it: the request body requires `workspaceRef`, the server
  resolves the current workspace itself via the same
  `resolveDesktopLocalWorkspaceRoot` codex-local turns execute against, and
  any difference is a 409 `workspace_mismatch` refusal with the registry left
  untouched -- never a silent redirect. Programmatic enable can never grant a
  new, previously-ungranted workspace; on success it binds exactly the
  resolved workspace, the same path as the IPC set handler.
  Proof: `full-auto-control-server.test.ts` "enable with a mismatched
  workspaceRef is a 409 typed refusal and the registry is untouched" and
  "enable with the matching workspaceRef enables + binds the record...".
- **FA-AC-24:** Every mutating control-API call (enable, disable,
  continue-now) appends a durable, distinctly-attributed system note to the
  thread through the existing `appendFullAutoSystemNote` (naming the
  programmatic path and caller `control-api`), plus a public-safe console
  audit line, so the owner can always tell a programmatic action from their
  own click.
  Proof: `full-auto-control-server.test.ts` attribution assertions inside the
  enable, disable, and continue-now cases (note text contains "programmatically"
  and "control-api").
- **FA-AC-25:** continue-now is a new TRIGGER into the shared serialized
  reconciliation path, never a new dispatch mechanism: the handler invokes
  the exact injected reconciliation trigger (main passes
  `runFullAutoReconciliation`, the same FA-H3 promise-chain mutex + durable
  lease every other trigger point uses) exactly once and returns
  `{ scheduled: true }` immediately; dispatch remains subject to lease,
  workspace binding, backoff, and cap policy. An unknown threadRef is 404 and
  never touches the trigger.
  Proof: `full-auto-control-server.test.ts` "continue-now invokes the
  injected reconcile trigger exactly once and returns { scheduled: true }"
  (spy on the injected trigger) and "continue-now on an unknown threadRef is
  a 404 and never touches the trigger"; `main.ts` passes
  `() => runFullAutoReconciliation()` as that capability (code-reviewed).
- **FA-AC-26:** The served surface and the published OpenAPI 3.1 document
  cannot drift: `GET /v1/openapi.json` serves the hand-authored document, and
  a structural parity test asserts every route in the shared
  `FULL_AUTO_CONTROL_ROUTES` table appears in the document (path, method,
  operationId) AND every operation in the document is a served route.
  Response bodies decode against the Effect Schemas in
  `full-auto-control-contract.ts`, whose bounds mirror the IPC contract.
  Projections stay public-safe: records expose only
  threadRef/enabled/continuationCount/updatedAt/workspaceRef/blockedReason/
  live state plus accountRef (never model/effort/raw profile material), and
  turns expose identity/phase/disposition/timestamps for at most the last 20
  Full Auto turns -- never transcript text.
  Proof: `full-auto-control-server.test.ts` "GET /v1/openapi.json serves the
  document, and the document <-> served routes agree in both directions",
  "list and status match the contract schemas... expose no profile material
  beyond accountRef", and "turns returns a bounded, most-recent-first Full
  Auto projection with no transcript text".
- **FA-AC-27:** The MCP server and CLI are thin pass-through clients of the
  one control surface: both discover the server from `full-auto/control.json`
  (with `--user-data` / `OPENAGENTS_DESKTOP_USER_DATA` overrides), attach the
  bearer, call the HTTP API, and return the server's JSON verbatim -- no
  client-side policy and no second schema vocabulary. Both fail with a clear
  "server not enabled" message when the connection file is missing. The MCP
  server exposes `full_auto_list` / `full_auto_status` / `full_auto_enable` /
  `full_auto_disable` / `full_auto_continue_now` / `full_auto_turns` over the
  repo's public MCP protocol revision (2025-06-18).
  Proof: `scripts/full-auto-cli.ts` and `scripts/full-auto-mcp.ts`
  (pass-through by construction over the shared
  `scripts/full-auto-control-client.ts`); live end-to-end receipt in the
  rev 6 entry under Receipts (`pnpm run smoke:full-auto-control` exercises
  the real CLI as a second OS process against the real running Electron
  main).
- **FA-AC-28:** The control surface can BOOTSTRAP Full Auto with no existing
  thread: `POST /v1/full-auto/start` (OpenAPI `startFullAuto`, MCP
  `full_auto_start`, CLI `start --workspace <path> [--title <t>]`) mints a
  brand-new local thread in main's own thread store (main names the ref --
  the caller never supplies one), binds the resolved workspace, enables the
  record through the same `registry.set` path as the composer toggle,
  appends the distinctly-attributed `(caller: control-api)` system note, and
  schedules the shared serialized reconcile pass so the first continuation
  dispatches without a separate continue-now call -- the reconcile
  dispatcher then opens a brand-new provider conversation because the
  minted thread has no session continuity. start obeys the exact enable
  authority rule: the caller MUST name the workspace it expects, and on any
  difference from the currently resolved workspace the call refuses with
  409 `workspace_mismatch` with NO thread minted, NO record written, and NO
  note appended -- never a redirect, never a new grant.
  Proof: `src/full-auto-control-server.test.ts` ("start with the matching
  workspaceRef mints a thread...", "start with a mismatched workspaceRef is
  a 409 typed refusal: NO thread minted...", "start discipline: bodyless
  start is 400...", plus the doc <-> route parity test covering
  `startFullAuto`).
- **FA-AC-29:** The durable execution profile carries an optional ProviderLane
  ref. A rev-7 registry row with no lane still decodes and continues on
  `codex-local`; a selected `fable-local` row survives a Runtime A → Runtime B
  reopen and reaches the shared dispatch seam with the same lane/account/model.
  Proof: `full-auto-restart.e2e.test.ts` "a Claude lane selection survives
  Runtime A -> Runtime B..." plus the retained legacy-file registry tests;
  `pnpm run smoke:full-auto-restart` launches real Electron OS processes for
  `seed-claude` → `resume-claude` and receipts `dispatchedLane:fable-local`.
- **FA-AC-30:** Reconciliation dispatches through the L1 ProviderLane SPI and
  fails closed for any lane that is unknown, L2-quarantined, does not advertise
  Full Auto, or lacks safe background-question settlement. Workspace binding,
  exactly-once lease, backoff, cap, and attribution behavior are unchanged.
  Proof: `main.ts` lane selection + `projectProviderLaneCapabilities` gate;
  focused Full Auto regression suites.
- **FA-AC-31:** Codex and Claude Full Auto turns use the single lane-keyed
  instruction policy. A background Claude `AskUserQuestion` never parks: it is
  denied immediately with guidance to make a reasonable judgment and proceed,
  while an interactive ordinary Claude turn retains the existing real question
  UI flow.
  Proof: `fable-local-runtime.test.ts` "background Full Auto denies
  AskUserQuestion immediately..." and the retained interactive question tests.
- **FA-AC-32:** `start` and `enable` accept an optional lane ref (default
  `codex-local`) through the shared control contract, served OpenAPI document,
  MCP tools, and CLI `--lane`; status/list expose the public-safe selected lane.
  An ineligible lane returns typed 409 `lane_not_eligible` without mutating the
  registry.
  Proof: `full-auto-control-server.test.ts` "enable accepts an admitted lane
  selector..." plus document/route/schema parity.
- **FA-AC-33:** A real bounded Claude Code Full Auto run must be retained as a
  release receipt. ACP peer proof remains conditional on #8893/#8894 admission
  and must not be inferred from fixture coverage.
  Proof: owner/dogfood receipt linked from #8901; until captured this criterion
  remains an explicit residual, not a release claim.
- **FA-AC-34:** Every ProviderLane dispatch receives the same main-owned spec
  projection when the granted workspace contains `specs/**`. The projection is
  bounded to 32 files, 512,000 bytes per file, 64 snapshot criteria per
  ProductSpec, 128 snapshot obligations, 12 prompt criteria per ProductSpec,
  12 prompt obligations, and 8,000 prompt characters; truncation is explicit.
  Proof: `spec-lane-workflow.test.ts` projection-bound case plus the shared
  `makeProviderLaneDispatcher` seam.
- **FA-AC-35:** Full Auto's shared lane instruction explicitly names unmet
  ProductSpec/AssuranceSpec obligations as candidate work while preserving the
  one-concrete-step contract and denying provider-owned verdict authority.
  Proof: `full-auto-lane.ts` shared instruction and focused Full Auto tests.
- **FA-AC-36:** After a dispatched turn, main re-reads the workspace through
  the ProductSpec/AssuranceSpec packages and appends a bounded system note with
  changed and remaining unmet obligation state. Missing, malformed, stale,
  flaky, inconclusive, unreviewed, or excepted evidence never rounds green.
  Proof: `spec-lane-workflow.test.ts` malformed-index and axis-revalidation
  cases.
- **FA-AC-37:** The identical bounded projection and revalidation path works
  through at least two distinct ProviderLane refs without importing a provider
  into the spec module or moving admission, verification, release, or
  public-claim authority into a lane.
  Proof: the two-lane dispatcher fixture in `provider-lane.test.ts` and the
  Codex/Claude note assertions in `spec-lane-workflow.test.ts`.

- **FA-AC-38:** A `FullAutoRun` record carries a stable `runRef` distinct from
  and independent of any `threadRef` it is currently bound to, plus title,
  objective, explicit done condition, workspace, provider profile
  (lane/account/model/effort), and turn cap as first-class durable fields
  decoded by an Effect Schema.
  Proof: planned, owned by FA-RUN-01 (#8969); regression target
  `full-auto-run-registry.test.ts` (module TBD by #8969).
- **FA-AC-39:** Starting a second active (non-terminal) run while one active
  run already exists for the Desktop profile is refused with a typed conflict
  identifying the existing active `runRef`; it is never silently queued and
  never dispatched in parallel. Draft and terminal run records are unaffected
  by this limit.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-40:** Starting a new run from a terminal run's launcher always mints
  a new distinct `runRef` and never mutates the terminal record's fields or
  state; an optional predecessor-run reference may be carried for report
  continuity only, never for authority or objective inheritance.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-41:** On first startup after the FullAutoRun model ships, every
  existing `enabled: true` legacy thread-keyed registry row migrates
  additively to exactly one `FullAutoRun` record (Running or Paused per its
  prior live state) with no data loss; a second startup performs no duplicate
  migration; an `enabled: false` legacy row does not migrate to an active run.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-42:** A run whose bound `threadRef` or provider session is missing
  or orphaned at reconciliation transitions to a typed Stalled or Failed
  disposition with an owner-visible reason; it never silently reattaches to
  an unrelated thread and never reproduces the six-hour silent stall recorded
  in the 2026-07-17 audit.
  Proof: planned, owned by FA-RUN-02 (#8970) (thread-pressure replay) and
  FA-RUN-03 (#8971) (stall classification); regression target: a deterministic
  replay of the exact incident composition (long turn, concurrent chats,
  bounded mutable-thread eviction, gap-to-next-reconciliation) plus a real
  Full Auto pressure run.
- **FA-AC-43:** The full run lifecycle state machine is exactly Draft,
  Running, Pausing, Paused, Retrying, Stalled, Completed, Failed, Stopped, and
  Cap-reached; every transition between these states persists actor (owner
  UI, control-api, or a named system policy such as workspace_guard or
  continuation_cap), a UTC timestamp, and a typed reason, extending the
  existing `disabledBy` attribution pattern to the complete graph. An illegal
  transition (for example Resume from a non-Paused state) is refused with a
  typed error and never silently coerced.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-44:** Pause with an active provider turn transitions the run to
  Pausing immediately and to Paused only once that turn resolves (completes
  normally or is interrupted); Pause with no turn in flight transitions
  directly to Paused. Resume is legal only from Paused, dispatches exactly
  once through the existing FA-H3 serialized-mutex-plus-lease path (FA-AC-15,
  retained unchanged), and a run cannot be resumed twice concurrently.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-45:** Stop is a terminal transition legal from any non-terminal
  state (Draft, Running, Pausing, Paused, Retrying, Stalled) and is distinct
  from Pause: a Stopped run is never resumed, and starting new work requires
  the rerun path (FA-AC-40), never a mutation of the stopped record.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-46:** A run's Completed disposition is a self-reported,
  owner-reviewable claim backed by the bounded `FullAutoRunReport`
  (FA-AC-51), not an automatically verified assertion that the objective/done
  condition was actually satisfied; the product never presents Completed as
  verified truth. Automatic done-condition verification is explicitly cut
  (CUT-FA-04) past this revision.
  Proof: planned, owned by FA-RUN-01 (#8969) for the state itself and
  FA-RUN-04 (#8972) for the report it is backed by; UX copy proof owned by
  FA-UX-01 (#8974).
- **FA-AC-47:** Run-level liveness is computed distinctly from a single
  healthy long-running provider turn: a run is live while a turn is genuinely
  executing OR while elapsed time since the last dispatch is within a defined
  SLO window; outside that window the run transitions to Stalled with an
  explicit owner-visible cause and a retry ETA, never silence.
  Proof: planned, owned by FA-RUN-03 (#8971).
- **FA-AC-48:** A stalled run exposes an explicit, owner-actionable recovery
  affordance (at minimum: retry now, or Stop) rather than requiring the owner
  to infer the situation from a generic failure banner, closing the exact
  observability gap the 2026-07-17 audit recorded.
  Proof: planned, owned by FA-RUN-03 (#8971) (detection/recovery) and
  FA-UX-01 (#8974) (affordance).
- **FA-AC-49:** Objective and done-condition text are durable fields on the
  `FullAutoRun` record itself, never dependent solely on provider-native
  session continuity or the bounded transcript-note window; a provider
  switch or a bounded-history truncation cannot cause the objective to be
  lost from Desktop's own state.
  Proof: planned, owned by FA-RUN-01 (#8969); handoff-path proof owned by
  FA-HO-01 (#8975).
- **FA-AC-50:** Registry eviction never drops a non-terminal (Running,
  Pausing, Paused, Retrying, Stalled) run record, extending the existing
  enabled-record eviction protection (FA-AC-12, retained with stronger proof)
  to the full FullAutoRun state set.
  Proof: planned, owned by FA-RUN-01 (#8969).
- **FA-AC-51:** Every run produces a bounded, private `FullAutoRunReport`
  containing: run ref, thread ref, title, objective, workspace, started/
  stopped timestamps; provider/lane per turn and every provider transition;
  per-turn disposition, duration, selected packet/issue, and bounded outcome
  summary; commits/receipts claimed by the agent (verified independently
  where possible); failure classification, retry/backoff, recovery action,
  and disabled reason; liveness gaps over threshold; objective/acceptance
  progress and remaining work; transcript-analysis findings; and a pointer to
  private raw evidence rather than raw transcript contents. The report never
  contains raw prompts, raw provider tool output, secrets, or credentials.
  Proof: planned, owned by FA-RUN-04 (#8972).
- **FA-AC-52:** A public-safe control/receipt projection of the run report
  exposes only bounded, non-transcript fields (extending the existing
  FA-AC-26 public-safety bound), and any raw-evidence pointer it carries
  resolves only to owner-private storage, never a public route.
  Proof: planned, owned by FA-RUN-04 (#8972).
- **FA-AC-53:** An offline/private transcript-analysis pass runs against a
  completed run's report and evidence pointer, reusing #8911's default-off
  Desktop usage/telemetry consent boundary rather than a parallel collection
  path, and produces measurable, comparable findings (duplicated setup,
  drift, stalls, unclear UI state, false completion claims) across at least
  two runs of the same named test.
  Proof: planned, owned by FA-RUN-05 (#8973).
- **FA-AC-54:** A dedicated **Full Auto** launcher action appears beside/
  under **New session** in the left rail and collects title (auto-suggested
  from the objective, editable), objective, explicit done condition,
  workspace, provider/lane, and a bounded turn cap (default 20, clearly
  shown) before Start is enabled; Start applies the same workspace-authority
  refusal rule as the existing control-API `start` (FA-AC-28, retained with
  stronger proof).
  Proof: planned, owned by FA-UX-01 (#8974).
- **FA-AC-55:** After Start, the main canvas renders a dedicated read-only run
  view for v1: objective and workspace remain pinned at the top; current
  state is one of Running, Pausing, Paused, Retrying, Stalled, Completed,
  Failed, Stopped, or Cap-reached, rendered explicitly (not inferred from a
  generic banner); Pause/Resume is the primary control depending on state;
  Stop is a distinct, always-available terminal control while non-terminal;
  the per-turn transcript (provider, duration, outcome, artifacts) is
  inspectable; and the ordinary chat composer is absent from this view while
  the run is active.
  Proof: planned, owned by FA-UX-01 (#8974).
- **FA-AC-56:** The composer-embedded Full Auto toggle, badge, and
  manual-send fencing (FA-AC-01, FA-AC-19, FA-AC-21) are removed from the
  ordinary chat composer once the dedicated launcher and run view ship; an
  ordinary chat thread never exposes Full Auto controls inline again.
  Proof: planned, owned by FA-UX-01 (#8974).
- **FA-AC-57:** The left-rail sidebar entry for an active or recently-terminal
  run displays its objective-derived title (never a raw first-message title)
  and a live-state-derived status indicator, and remains reachable via the
  same search/navigation affordances as an ordinary thread.
  Proof: planned, owned by FA-UX-01 (#8974); reuses generic-title work
  tracked at #8940.
- **FA-AC-58:** A manual same-thread provider switch (Codex<->Claude) and an
  explicit Pause -> switch provider -> Resume sequence both project a
  host-owned, objective-priority bounded history (never raw provider-private
  session state) to the target provider, and both append a visible
  transition receipt to the thread and the run report carrying from/to/
  actor/time/reason and an explicit truncation flag when the projection was
  bounded.
  Proof: planned, owned by FA-HO-01 (#8975).
- **FA-AC-59:** A provider switch (manual or Pause -> switch -> Resume)
  re-checks the target lane's L2 capability admission, auth, and Full Auto/
  background-question eligibility before switching; on refusal the run
  remains on its current provider/lane with no partial state change (an
  explicit rollback, not a redirect).
  Proof: planned, owned by FA-HO-01 (#8975).
- **FA-AC-60:** No documentation, UI copy, or public claim states or implies
  that a provider switch transfers provider-private session state; copy is
  restricted to what is actually true: a host-owned bounded projection of
  Desktop-visible thread history.
  Proof: planned, owned by FA-HO-01 (#8975).
- **FA-AC-61:** Provider support and picker eligibility are derived from
  exact admitted evidence (proven L2 capability admission plus a proven safe
  background-question policy), never presented merely because a lane adapter
  exists in code. An admitted ACP lane that has not cleared this bar is not
  exposed as a first-class Full Auto or handoff picker option.
  Proof: planned, owned by FA-HO-01 (#8975); repairs the picker gap tracked
  at #8977.
- **FA-AC-62:** Real-provider execution of `TEST 01` and `TEST 02` from the
  2026-07-17 audit's sidebar test batch (Codex establishes a marker and a
  two-step task, switches the same thread to Claude, Claude states the
  marker and performs step two; and the reverse) is captured as a named,
  retained receipt in the owner's real Desktop sidebar, not inferred from
  fixture coverage.
  Proof: planned, owned by FA-QA-01 (#8976).
- **FA-AC-63:** Real-provider execution of `TEST 03` (objective retention
  under context/notes pressure -- either the target provider states the
  original objective and acceptance rule correctly, or the product visibly
  reports truncation and requires confirmation rather than silently
  fabricating continuity) is captured as a named, retained receipt.
  Proof: planned, owned by FA-QA-01 (#8976).
- **FA-AC-64:** Real-provider execution of `TEST 04` (a three-turn unattended
  Codex run with no manual message between turns, visible progress, and an
  explicit stop reason) and `TEST 05` (a Claude run surviving a Desktop
  restart with the same objective/lane and no duplicate turn) are captured as
  named, retained receipts.
  Proof: planned, owned by FA-QA-01 (#8976); reuses FA-RUN-02 (#8970) restart
  infrastructure.
- **FA-AC-65:** `TEST 06` -- the real replay of the 2026-07-17 incident
  (launch Full Auto, then create/open more than five other chats while its
  turn runs) -- passes with the autonomous thread remaining addressable and
  the next continuation starting, both as an automated deterministic
  regression and as a real-provider dogfood receipt.
  Proof: planned, owned by FA-RUN-02 (#8970) (automated regression) and
  FA-QA-01 (#8976) (real-provider receipt).
- **FA-AC-66:** The Full Auto AssuranceSpec is reconciled to this revision:
  every FA-AC-01..37 obligation carries its Criterion Disposition Map
  outcome (retired obligations marked accordingly, changed obligations
  rebound to their rev-10 criterion text) and every FA-AC-38..65 obligation
  has a designed proof rung -- no obligation may round green by omission or
  silent carry-forward of the rev-9 needs_design set.
  Proof: planned, owned by FA-AS-01 (#8978).
- **FA-AC-67:** With an ordered routing policy of admitted lane/account
  candidates bound on the durable record, a continuation dispatch that fails
  with a typed account_exhausted, rate_limited, or provider_error class
  rotates to the next admitted candidate IN THE SAME reconciliation pass,
  under a fresh exactly-once lease per attempt, persisting a typed rotation
  record (fromLane, toLane, reason, at) -- the run never enters failure
  backoff while an untried admitted candidate remains. A full unsuccessful
  cycle through every candidate consumes exactly one FA-H5 failure-budget
  step; untyped failures, records without a policy, and every existing
  cap/disable/backoff semantic behave exactly as before (proven by retained
  regression tests). Policies are validated fail-closed at bind time:
  unknown, unadmitted, or Full-Auto-ineligible lanes refuse the whole policy
  at validation, never at dispatch, and the loop can never rotate outside
  the owner-admitted candidate set. Rotation history surfaces through the
  control-API status projections as public-safe typed fields only (lane
  refs, typed reason, timestamp -- never prompts, models, paths, or
  secrets), and a v1/v2-era registry file without the new optional fields
  decodes and behaves exactly as single-lane.
  Proof: FA-RT-01 (#8987); `full-auto-restart.e2e.test.ts` "Full Auto
  multi-lane never-halt rotation (FA-RT-01 #8987)" cases (same-pass rotation
  per typed class, one-budget-step full cycle, untyped/legacy regression,
  bound-lane start order, restart survival), `full-auto-registry.test.ts`
  "Full Auto multi-lane routing-policy fields (FA-RT-01 #8987)" cases
  (legacy fixture decode, bind/clear/bounds, capped oldest-evicted history,
  public-safe projection), and `full-auto-routing.test.ts` (fail-closed
  policy validation and the control-record rotationHistory projection
  bound).

## Criterion Disposition Map (Rev 9 -> Rev 10)

Every rev-9 acceptance criterion resolves to exactly one disposition:
**retained unchanged**, **retained with stronger proof** (same behavior, the
bar for evidence or its scope grows), **changed-superseded** (the criterion's
literal text no longer describes the intended product and a rev-10 criterion
replaces it), **removed with rationale**, or **deferred** with an explicit
`CUT-FA-*` identifier. No rev-9 criterion is silently reinterpreted.

- **FA-AC-01** -- changed-superseded. "Exactly one composer toggle, no other
  new screen" is the literal contradiction issue #8968 was opened to resolve.
  Superseded by FA-AC-54/FA-AC-55 (dedicated launcher + read-only run view)
  and retired by FA-AC-56 (composer controls removed).
- **FA-AC-02** -- retained unchanged. Per-turn `approvalPolicy: "never"` and
  prompt prefixing is an execution-posture fact orthogonal to how a run is
  launched or displayed.
- **FA-AC-03** -- retained with stronger proof. `fullAuto: true` sent once,
  continuation decided by main, remains true; the decision now keys off
  `FullAutoRun` lifecycle state (FA-AC-43) rather than a boolean.
- **FA-AC-04** -- changed-superseded. The single toggle-off mutation is
  replaced by two distinct typed transitions, Stop (FA-AC-45) and Pause
  (FA-AC-44); the underlying guarantee -- an owner action persists to main
  immediately and durably regardless of an in-flight turn -- is retained by
  both.
- **FA-AC-05** -- retained unchanged. Ordinary (non-run) chat still never
  auto-resubmits.
- **FA-AC-06** -- retained with stronger proof. The 20-continuation cap
  becomes the explicit Cap-reached terminal lifecycle state (part of
  FA-AC-43) instead of an implicit "disabled" with a note; reset-only-on-off
  semantics carry forward to reset-only-on-Stop-or-new-run.
- **FA-AC-07** -- retained with stronger proof. Restart-resume now also
  restores the durable objective/done-condition fields (FA-AC-49), not only
  thread continuity.
- **FA-AC-08** -- retained unchanged. In-flight-turn-at-restart deference to
  existing interrupted-turn recovery is unaffected by the run model.
- **FA-AC-09** -- retained with stronger proof. The specific "brand-new
  thread toggled before it has an id" race is largely mooted by FA-AC-54's
  launcher minting run+thread together at Start, but the general principle --
  never silently drop an owner intent recorded before a durable id exists --
  is retained and reproved wherever a run still creates a thread lazily.
- **FA-AC-10** -- retained unchanged. No new commit/merge/push authority is
  introduced by the run model.
- **FA-AC-11** -- retained with stronger proof. Corrupt-file quarantine
  extends to the FullAutoRun store (part of FA-AC-41's migration path).
- **FA-AC-12** -- retained with stronger proof, restated as FA-AC-50:
  eviction-safety extends from the single `enabled` boolean to every
  non-terminal lifecycle state.
- **FA-AC-13** -- retained unchanged. Workspace-authority binding and
  fail-closed mismatch behavior move from "enable" to "Start" with identical
  semantics.
- **FA-AC-14** -- retained unchanged. Unbound-record fail-closed behavior is
  unaffected by the run model; it re-expresses over FullAutoRun rows.
- **FA-AC-15** -- retained unchanged. The FA-H3 serialized-mutex-plus-lease
  exactly-once dispatch mechanism is unmodified and is explicitly reused by
  Resume (FA-AC-44).
- **FA-AC-16** -- retained with stronger proof. The typed failure/backoff/
  disable-after-5 policy is retained; its terminal outcome now lands in the
  explicit Failed or Stalled lifecycle state (FA-AC-43) rather than a generic
  "disabled" boolean.
- **FA-AC-17** -- retained unchanged. Execution-profile continuity across
  continuations and restarts is unaffected by the run model.
- **FA-AC-18** -- retained unchanged as a general principle; the specific v1
  registry migration instance is restated and extended by FA-AC-41.
- **FA-AC-19** -- changed-superseded. The coarse typed live-state model
  (idle/turn_running/turn_completed/turn_failed/cap_reached/blocked) is
  retained as a mechanism but now drives the dedicated read-only run view
  (FA-AC-55) instead of a composer badge, and its state enumeration is
  folded into the full lifecycle machine (FA-AC-43).
- **FA-AC-20** -- retained with stronger proof. The Stop-targets-actual-
  background-turn mechanism (interrupt resolves the live running turn ref)
  is retained; Stop is now a first-class terminal lifecycle transition
  (FA-AC-45) rather than a composer button behavior.
- **FA-AC-21** -- changed-superseded. Manual-send fencing during a
  background turn is retired as literally specified because the ordinary
  composer is no longer present at all while a run is active (FA-AC-55); the
  underlying guarantee -- never a silent second concurrent turn on the same
  thread -- is retained unconditionally by the exactly-once lease (FA-AC-15).
- **FA-AC-22** -- retained unchanged. Control-surface opt-in/loopback/bearer
  gating is unaffected by the run model.
- **FA-AC-23** -- retained unchanged. Programmatic workspace-authority
  naming and 409 mismatch refusal carry forward to `start` unchanged.
- **FA-AC-24** -- retained with stronger proof. Attributed system notes on
  every mutating control call are retained; the general attribution
  requirement now extends to every lifecycle transition, not only
  enable/disable/continue-now (part of FA-AC-43).
- **FA-AC-25** -- retained unchanged. continue-now remains a trigger into
  the shared serialized reconciliation path, never a new dispatch mechanism.
- **FA-AC-26** -- retained with stronger proof. Served-surface/OpenAPI parity
  and public-safety bounds are retained; the schema/route table grows to
  carry the new run fields (title/objective/done condition/lifecycle state)
  under the same no-transcript-text bound.
- **FA-AC-27** -- retained unchanged. MCP/CLI thin-pass-through-client
  architecture is unaffected by the run model.
- **FA-AC-28** -- retained with stronger proof. `POST /v1/full-auto/start`
  becomes the canonical Draft->Running mission-contract launch path
  (FA-AC-54/FA-AC-38), now also carrying objective/done-condition/title
  fields; the exact workspace-authority refusal rule is unchanged.
- **FA-AC-29** -- retained unchanged. Optional ProviderLane ref continuity
  and cross-restart decode are unaffected by the run model.
- **FA-AC-30** -- retained unchanged. L1 SPI dispatch and fail-closed lane
  eligibility gating are unaffected by the run model.
- **FA-AC-31** -- retained unchanged. The lane-keyed background-question
  policy is unaffected while Running; a future interactive path while Paused
  is explicitly out of scope (see Scope: out).
- **FA-AC-32** -- retained unchanged. Optional lane selector on start/enable
  and typed `lane_not_eligible` refusal are unaffected by the run model.
- **FA-AC-33** -- deferred, CUT reference: tracked under FA-REL-01 (#8979)
  release admission rather than cut from intent; still an open residual, not
  satisfied by this revision.
- **FA-AC-34** -- retained unchanged. The bounded ProductSpec/AssuranceSpec
  spec-lane projection applies identically regardless of launch surface.
- **FA-AC-35** -- retained unchanged. The shared lane instruction's unmet-
  obligation framing is unaffected by the run model.
- **FA-AC-36** -- retained with stronger proof. The post-turn spec
  revalidation note becomes one structured entry among the
  `FullAutoRunReport`'s per-turn disposition fields (FA-AC-51); its
  evidence-only authority is unchanged.
- **FA-AC-37** -- retained unchanged. Cross-lane projection/revalidation
  parity is unaffected by the run model.

## Success Metrics

```productspec-success-metrics
- id: full_auto_owner_observed_resume
  metric: owner_observed_restart_resume_sessions_recorded_in_issue_or_needs_owner_receipts
  target: ">= 1"
  window: before any release claim
  segment: owner dogfood
  source: manual owner receipt linked from issue #8885
- id: full_auto_six_named_dogfood_tests_passed
  metric: named_sidebar_tests_passed_of_six (TEST 01-06, docs/fable/2026-07-17-full-auto-implementation-audit.md section 5)
  target: "6/6"
  window: before any Full Auto release claim
  segment: owner dogfood, real Desktop sidebar (not a headless fixture profile)
  source: FA-QA-01 (#8976) retained receipts
```

## Owner Gates

- **RESOLVED by this revision's own trigger.** The rev-9 gate "owner review of
  the restart-survival behavior itself... before any further Full Auto scope"
  was exercised: the 2026-07-17 owner overnight dogfood run performed exactly
  that review and found the underlying composer-toggle model insufficient.
  The corrected audit and epic #8967 are the direct product of that review.
- **OUTSTANDING -- explicit, not inferred.** Per epic #8967's acceptance
  criteria and issue #8968's own acceptance checklist ("the owner/reviewer
  acceptance identity is recorded; the spec cannot self-approve through
  implementation code"), owner sign-off on THIS rev-10 contract is not yet
  recorded. This document is agent-authored under `docs/sol/CLAIM_PROTOCOL.md`
  and this repository's default-yes coordinator-admits-owner-reviews-post-hoc
  convention (the same convention every prior Full Auto revision, 1 through
  9, was admitted under -- none of their commit messages or this spec's prior
  Owner Gates entries record a distinct human approval step beyond
  authorship). Consistent with that convention, this revision is admitted to
  `main` by the authoring agent so it can perform its claim-blocking function
  ("Implementation must not proceed under a ProductSpec that forbids the
  intended result," specs/CONVENTIONS.md) for the 11 dependent child issues.
  That is NOT the same thing as owner acceptance, and this document does not
  claim to have received it. Specifically outstanding for explicit owner
  review/amendment:
  - the v1 concurrency default of exactly one active run per Desktop profile
    (FA-AC-39);
  - the full ten-state lifecycle machine and its exact legal-transition set
    (FA-AC-43);
  - retiring the composer toggle in favor of a no-composer-while-active
    dedicated run view (FA-AC-54/55/56);
  - the provider-handoff claims boundary -- that no provider-private session
    state is implied to transfer (FA-AC-60); and
  - treating the six named sidebar dogfood tests as release-gate evidence
    (FA-AC-62..65).
  FA-REL-01 (#8979) may not claim public release readiness against this
  contract until that sign-off is recorded, and any implementation issue
  (#8969 onward) that lands before it proceeds against a document the owner
  has not yet explicitly accepted, amended, or rejected.
- Retained from rev 5, still open and still cheap to revisit later rather
  than free to build now: live token-by-token streaming of an in-progress
  turn remains cut in the read-only run view (see Scope: out), and per-thread
  toggle-state resync on arbitrary in-session switch is moot under the new
  model (CUT-FA-03) but a future multi-run or multi-thread run view remains
  an explicit later owner decision, not assumed by this revision.

## Receipts

- **Rev 11 (FA-RT-01 #8987):** multi-lane never-halt routing policy,
  main-process only. `full-auto-registry.ts` gains the OPTIONAL
  `routingPolicy` / `rotationHistory` record fields (backward-compatible:
  every pre-#8987 file decodes unchanged), `bindRoutingPolicy` /
  `recordRotation` write paths (bounded, oldest-evicted), and the public-safe
  `projectFullAutoRotationHistory` helper; new `full-auto-routing.ts` owns
  fail-closed policy validation over the existing full-auto-lane policy table
  plus the live L2 capability projection; `full-auto-reconcile.ts` adds the
  typed `failureClass` dispatch-result field, the
  `classifyFullAutoDispatchFailure` mapping (typed lane reasons plus the
  exact codex-app-server-turn quota/rate-limit detail markers), and the
  same-pass rotation cycle (fresh lease per attempt, one budget step per full
  unsuccessful cycle, profile rebound to the succeeding candidate);
  `full-auto-control-contract.ts` adds the OPTIONAL bounded
  `rotationHistory` field to the public-safe control record schema (server
  population owned by the control-surface issue); `main.ts` wires the
  failure classification and an owner-visible rotation system note.
  Verification: scoped suites `tests/full-auto-restart.e2e.test.ts`,
  `tests/full-auto-registry.test.ts`, `src/full-auto-routing.test.ts`,
  `src/full-auto-lane.test.ts`, plus the full existing full-auto test set
  and a clean `tsc -p tsconfig.json --noEmit`; exact pass counts recorded in
  the issue #8987 closeout.
- **Rev 10 (#8968):** ProductSpec-only revision -- no `apps/openagents-desktop`
  application code changed. Retitled from "Full Auto Provider-Lane Composer
  Loop" to "Full Auto Autonomous Run Contract"; added the Criterion
  Disposition Map custom section resolving all 37 rev-9 criteria; added
  FA-AC-38 through FA-AC-66 (29 new criteria) for the target FullAutoRun
  contract, each naming its owning epic #8967 child issue with an explicit
  `planned` proof status; rewrote Problem/Hypothesis/Scope/Owner Gates;
  restructured `cut:` to add CUT-FA-04/05/06 and mark CUT-FA-03
  superseded/moot. Companion changes in the same commit: three `pending`
  behavior-contract entries added to
  `packages/behavior-contracts/src/openagents-apps.ts` (separate launcher,
  read-only running state, Play/Pause/Stop lifecycle semantics), and a new
  Full Auto run-authority bullet added to root `INVARIANTS.md`'s Authority
  Boundaries section. The existing AssuranceSpec
  (`specs/desktop/full-auto.assurance-spec.md`) is left untouched and flagged
  stale-pending-reconciliation (see `tool_metadata.openagents_assurance_spec_status`);
  its reconciliation is FA-AS-01 (#8978), which explicitly depends on this
  issue.
  Verification: `node --import tsx packages/product-spec/src/cli.ts validate
  --specs-root specs`; `pnpm --dir packages/product-spec run test` (includes
  the repo Product Spec roots gate that validates every file under `specs/`,
  including this one, with zero errors); `pnpm --dir packages/behavior-contracts
  run test`. Exact pass/fail counts recorded in the issue #8968 closeout
  comment.
- Rev 8 (L6 #8901): provider-lane generalization. `pnpm --dir
  apps/openagents-desktop run typecheck`; 97 focused provider/Full Auto tests;
  Desktop suite 1732 passed / 39 skipped with one pre-existing timing failure
  in `codex-connect.test.ts`; `pnpm run check:fast`; and a real built-Electron
  `pnpm run smoke:full-auto-restart` passed Codex resume, workspace-mismatch
  fail-closed, and a non-Codex `seed-claude` → `resume-claude` two-process
  dispatch receipted as `fable-local`. The smoke uses the deterministic Claude
  fixture; the distinct real authenticated Claude Code dogfood proof required
  by FA-AC-33 remains residual and is not claimed here.
- Rev 6 (FA-H13 #8886): the Phase 1 local control surface.
  New modules: `full-auto-control-contract.ts` (Effect Schemas + the shared
  `FULL_AUTO_CONTROL_ROUTES` table), `full-auto-control-openapi.ts`
  (hand-authored OpenAPI 3.1 document), `full-auto-control-server.ts`
  (node:http loopback server, environment-auth-scoped bearer mint,
  constant-time verification, mode-0600 `full-auto/control.json`); main.ts
  wiring behind `isFullAutoControlEnabled` with a narrow capability options
  object (same registry / workspace resolver / serialized reconcile trigger /
  live-state map / journal / note appender the IPC handlers use); thin
  clients `scripts/full-auto-cli.ts` + `scripts/full-auto-mcp.ts` over
  `scripts/full-auto-control-client.ts` (package script `full-auto`).
  Focused verification: `vp test --run --max-concurrency 1 --root .
  apps/openagents-desktop/src/full-auto-control-server.test.ts` -- 1 file
  passed, 14 tests passed (auth gating, 0600 connection file,
  workspace-mismatch 409 refusal with registry untouched, enable/disable
  attribution notes, continue-now trigger spy called exactly once + 404
  never-touches-trigger, public-safe list/status/turns projections decoded
  against the contract schemas, OpenAPI <-> routes parity in both
  directions); regression `full-auto-restart.e2e.test.ts` +
  `full-auto-registry.test.ts` + `renderer/shell.test.ts` -- 3 files passed,
  147 passed, 11 skipped; clean `tsc -p tsconfig.json --noEmit`.
  Live end-to-end receipt (`pnpm run smoke:full-auto-control`; build first):
  the REAL Electron app launched windowless with
  `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1` +
  `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL_PROBE=1` against isolated OS-temp
  userData, and the actual CLI ran as a second OS process (discovery via
  control.json -> bearer -> HTTP `list` then `status`). Passing receipt
  line: `[openagents-desktop full-auto-control] live smoke OK
  {"threadRef":"…","listedRecords":1,"enabled":true,"workspaceBound":true,
  "liveState":"idle"}`.
- FA-H12 (#8885): two-process Electron restart smoke
  `apps/openagents-desktop/scripts/full-auto-restart-smoke.ts`
  (`pnpm run smoke:full-auto-restart`; build first). Spawns the real app
  twice against one temporary user-data directory: the seed process writes
  an enabled, workspace-bound registry record plus a completed terminal
  fixture turn and quits; the resume process is observed to run startup
  reconciliation and dispatch exactly one fixture continuation, hitting the
  durable cap deterministically; a second seed/resume pair proves the
  workspace-mismatch fail-closed path across processes. Passing receipt
  line: `[openagents-desktop full-auto-restart] two-process smoke OK
  {"seeded":true,"resumed":true,"dispatchedTurnRefPresent":true,
  "continuationCount":20,"mismatchFailedClosed":true}`. This smoke is the
  merge gate for any renewed "survives restart" claim; the packaged
  real-repository owner observation remains the gate for live/owner-accepted
  claims.
- Rev 5 (FA-H4 #8877): contract additions in `codex-local-contract.ts`
  (`CodexLocalFullAutoStateChannel`, `CodexLocalFullAutoStateSchema` with its
  decode helper, `CodexLocalFullAutoInterruptChannel` + request schema);
  main-owned live-state map, broadcast, extended get response, and the
  thread-scoped interrupt handler in `main.ts` (state emission wired around
  the existing reconcile dispatch adapter/callbacks -- reconcile logic itself
  untouched); preload `fullAuto.onState`/`fullAuto.interrupt` following the
  localTurnRecovery.onUpdate decode-then-listen pattern; renderer projection
  (`fullAutoLiveByThread`, `withFullAutoLiveState`,
  `activeFullAutoTurnRunning`), boot subscription, send-fencing, extended
  Stop handling, and the composer badge. Focused verification:
  `vp test --run --max-concurrency 1 --root .
  apps/openagents-desktop/src/renderer/shell.test.ts
  apps/openagents-desktop/src/renderer/react-composer.test.tsx` -- 2 files
  passed, 136 tests passed, 11 skipped; plus
  `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts`,
  `full-auto-registry.test.ts`, `design-conformance.test.ts`, and
  `mvp-visible-surfaces.test.ts` -- 4 files passed, 69 tests passed; plus a
  clean `tsc -p tsconfig.json --noEmit`.
- Rev 4 (FA-H2 #8875, FA-H3 #8876, FA-H5 #8878, FA-H6 #8879):
  `full-auto-restart.e2e.test.ts` extended with the workspace-binding
  refuse/disable and fail-closed cases, the audit's two adversarial probes
  converted into retained regression tests (overlapping reconciles dispatch
  once; ok:false is a typed visible failure), the serial-queue mutex proof,
  stale-lease startup recovery, backoff/disable-after-5 failure policy, the
  cap-counting decision (failures never consume cap slots), and profile
  continuity across a restart. `full-auto-registry.test.ts` extended with
  v1-file backward-compatibility decode, lease claim/clear semantics,
  failure/success state transitions, workspace/profile binding durability,
  and the enable/disable option semantics. Focused verification:
  `vp test --run --max-concurrency 1 --root . apps/openagents-desktop/tests/
  full-auto-restart.e2e.test.ts apps/openagents-desktop/tests/
  full-auto-registry.test.ts apps/openagents-desktop/src/renderer/
  shell.test.ts apps/openagents-desktop/src/codex-local-runtime.test.ts`
  -- 4 files passed, 169 tests passed, 11 skipped; plus a clean
  `tsc -p tsconfig.json --noEmit`.
- Automated success-metric instrumentation was deliberately removed in rev 3
  as unmeasurable (audit Finding 9, #8882): the app has no consent surface,
  no counter implementation records the previously named metric identifiers,
  and this spec's telemetry posture keeps metrics absent (not inferred)
  without consent. Automated metrics may return only alongside a real
  consent surface and an implementation that actually records them.
- `pnpm --dir apps/openagents-desktop run typecheck` (`tsc -p tsconfig.json
  --noEmit`) — clean, zero errors.
- `codex-local-runtime.test.ts`: two cases proving `approvalPolicy` and
  prompt-prefix behavior for Full Auto vs. an ordinary turn (rev 1,
  unchanged).
- `react-composer.test.tsx`: one case proving the toggle renders and reports
  `DesktopFullAutoToggled` (rev 1, unchanged).
- `shell.test.ts`: cases proving a flagged turn sends `fullAuto: true`
  exactly once with no renderer-side loop; toggle persists to main
  immediately for the active thread; a brand-new thread's enabled state
  reaches main once it has a real id; an ordinary turn never resubmits.
- `full-auto-restart.e2e.test.ts` (new, #8853): the "Runtime A seeds durable
  state, Runtime B reconciles" proof pattern already used by
  `local-turn-restart.e2e.test.ts` for interrupted-turn recovery, applied
  here to Full Auto: resuming after a clean quit with nothing in flight, not
  racing a thread whose turn is still being recovered, a toggle-off holding
  across restart, and the continuation cap self-disabling across restart.
  Exercises the real `full-auto-registry.ts` and `full-auto-reconcile.ts`
  modules directly against the same on-disk file across two independent
  opens -- no Electron process, no mock of the durable layer.
- `full-auto-registry.test.ts` (rev 3, #8880/#8883): pins the cap-reset
  semantics (reset only on toggle-off), corrupt-file quarantine (registry
  opens empty, quarantine file exists, subsequent persist works), and
  enabled-record eviction survival (enabled records never evicted; only the
  disabled tail is bounded), all against the real on-disk registry.
- Full run: `vp test --run --max-concurrency 1 --root ../..
  apps/openagents-desktop` — 1521 passed, 39 skipped, 1 failed. The one
  failure (`codex-turn-state.test.ts`, a Codex app-server protocol
  notification/item fixture replay test) is pre-existing and unrelated to
  Full Auto -- a relative-path fixture lookup this change never touches;
  confirmed by running it in isolation with the same result.
- Not yet done, recommended as an owner follow-up: an actual packaged-app
  smoke (toggle on, send one message, quit the running Desktop app, relaunch,
  observe it resume) -- the automated e2e proof above exercises the exact
  same durable modules the real app calls, but a live app run is the only
  thing that also proves the Electron boot-sequence wiring (window creation,
  IPC registration order) end to end.

## Promise Links

- No promise-registry entry changes as a result of this spec. The existing
  red `autopilot.desktop_full_auto_guidance.v1` entry is unrelated legacy
  scope tied to a deleted app and remains exactly as recorded; this spec does
  not claim it, reference it as satisfied, or request its update.

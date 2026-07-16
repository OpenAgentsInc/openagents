---
spec_format_version: "0.1"
title: "Full Auto Provider-Lane Composer Loop"
artifact_type: "prd"
spec_revision: 8
author: "OpenAgents"
created_at: "2026-07-15T22:15:41.850Z"
updated_at: "2026-07-16T15:30:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
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
  openagents_revision_8_issue: "8901 (L6 provider-lane generalization)"
  openagents_revision_8_note: "Rev 8 generalizes the durable loop over the L1 ProviderLane SPI. The additive optional profile.lane defaults legacy rows to codex-local; reconciliation capability-gates the selected lane; built-in Codex and Claude use one lane-keyed instruction/background-question policy; and control start/enable, OpenAPI, MCP, and CLI accept an optional lane selector. Claude background questions deny immediately with proceed-with-judgment guidance instead of parking without a renderer."
  openagents_revision_note: "rev 7 adds programmatic BOOTSTRAP to the Phase 1 control surface: POST /v1/full-auto/start (OpenAPI startFullAuto, MCP full_auto_start, CLI start) mints a brand-new local thread in main's own thread store (main names the ref, never the caller), binds the resolved workspace, enables via the same registry.set path as the composer toggle, appends the (caller: control-api) note, and schedules the shared serialized reconcile pass so the first continuation opens a brand-new provider conversation -- closing the gap where enable/continue-now could only operate on threads the UI had already created (a live dispatch on an unknown threadRef failed with that-conversation-no-longer-exists). start obeys the exact enable authority rule: 409 workspace_mismatch mints NO thread, writes NO record, appends NO note. Rev 6 (FA-H13 #8886) adds the Phase 1 LOCAL programmatic control surface: an opt-in (OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1), loopback-only (127.0.0.1), scoped-bearer-gated node:http server in Desktop main (full-auto-control-server.ts) exposing list / status / enable / disable / continue-now / turns plus GET /v1/openapi.json serving the hand-authored OpenAPI 3.1 document (full-auto-control-openapi.ts) that IS the surface's source of truth -- a thin stdio MCP server (scripts/full-auto-mcp.ts) and a thin argv CLI (scripts/full-auto-cli.ts) are deliberately pass-through clients of that one document, following the repo's existing OpenAPI/SDK/MCP triad convention. Auth follows the Harness MCP pilot pattern exactly: one per-process bearer credential minted at startup with scopes drawn from @openagentsinc/environment-auth's narrowing-only exchange (operator_read + coding_session_control; no new auth framework), constant-time verification on every request, connection info written mode-0600 to full-auto/control.json under userData. Authority boundaries: enable NAMES the workspace the caller expects and refuses with 409 workspace_mismatch (registry untouched) on any difference from the currently resolved workspace -- a refusal, never a redirect, and never a grant of a new workspace; continue-now is a new TRIGGER into the exact same serialized reconciliation path (runFullAutoReconciliation / the FA-H3 promise-chain mutex + durable lease), never a parallel dispatch mechanism; every mutating call appends a durable, distinctly-attributed system note (caller: control-api) via the existing appendFullAutoSystemNote so the owner can always tell a programmatic action from their own click. Projections are public-safe: records expose threadRef/enabled/continuationCount/workspaceRef/blockedReason/live state and only accountRef from the bound profile; turns expose identity/phase/disposition/timestamps for the last 20 Full Auto turns, never transcript text. Phase 2 (cross-machine relay through the openagents.com triad and Khala Sync) stays explicitly out of scope. Rev 5 (FA-H4 #8877) makes background continuations rendered facts instead of silence-until-completion. Main keeps a coarse, typed, in-memory per-thread live state (idle | turn_running | turn_completed | turn_failed | cap_reached | blocked; blocked carries the typed blockedReason as detail) and broadcasts every transition to all windows over a new CodexLocalFullAutoStateChannel; the CodexLocalFullAutoGetChannel response additively carries { state, turnRef } beside enabled so thread switches hydrate an in-flight background turn immediately. While the active thread's live state is turn_running the composer renders a status badge and the Stop control; Stop signals a new thread-scoped CodexLocalFullAutoInterruptChannel whose main handler resolves the live running turn ref itself and reuses the exact codexLocal.interrupt runtime path; a manual send during turn_running is fenced with a transient notice that keeps the draft (never a silent second concurrent turn). Token-by-token streaming of background turns remains deliberately out; live state is deliberately NOT durable -- startup reconciliation re-derives reality. An interrupted background turn terminates through the existing FA-H5 typed-failure path (owner-visible note, backoff) with the toggle remaining the durable loop-level stop. Rev 4 (#8875, #8876, #8878, #8879) hardens dispatch authority and delivery semantics on the durable record. FA-H2 (#8875): enabling binds the currently resolved workspace (resolved by main, never renderer-supplied) onto the record; a continuation whose resolved workspace no longer matches refuses to dispatch and disables the record with blockedReason workspace_mismatch and an owner-visible note; an enabled record with NO binding (pre-upgrade row) fails CLOSED (workspace_unbound) and rebinds only on its next enable. FA-H3 (#8876): reconciliation is serialized through a promise-chain mutex in main, and each continuation claims a durable per-thread lease carrying the exact dispatched turn ref before dispatch, so overlapping passes dispatch a thread at most once; only the startup pass clears a stale lease whose turn ref never reached the local-turn journal; main's dispatch adapter additionally refuses when the journal already holds a nonterminal turn on the thread. FA-H5 (#8878): thrown errors AND ok:false dispatch results are typed failures -- failure state (consecutiveFailures, lastFailureAt, blockedReason) persists on the record with an owner-visible note, retries respect bounded exponential backoff min(2^failures*30s, 30min), and the 5th consecutive failure disables the record; a successful dispatch clears failure state. Cap-counting decision: continuationCount increments ONLY on successful dispatch -- a failed dispatch consumes failure budget, never a cap slot (rev 3 incremented before dispatch). FA-H6 (#8879): the initiating renderer-sent flagged turn binds its execution profile (account target, model, reasoning effort) onto the record and continuations replay it (revalidated against live contract enums); images, attachments, and extension selection deliberately reset -- a continuation is a fresh instruction, not a replay. All new record fields are optional so v1 registry files still decode. Rev 3 (#8880, #8882, #8883) pinned cap-reset semantics, registry quarantine/eviction hardening, and measurable-metrics cleanup. Rev 2 (#8853) moved the continuation decision from the renderer to a durable main-process registry, closing rev 1's CUT-FA-02 gap."
---

## Problem

OpenAgents Desktop's Codex composer requires a user to click Send for every
turn. A developer who wants Codex to keep making real, repo-grounded progress
on a granted repository unattended has no way to say "just keep working" —
they must manually resend after each completion, and there is no toggle that
tells Codex what to look at (README, docs, open issues) when the user has not
spelled out a task. Rev 1 (#8852) shipped a single composer toggle whose
continuation loop lived in the renderer (`shell.ts`): a clean turn completion
resubmitted the next turn from an in-memory `while` loop. That loop could not
survive a renderer reload or an app restart — its state was destroyed the
instant the renderer's JS context reset, silently leaving the owner to
discover Full Auto had stopped without their toggling it off. Rev 2 (#8853)
closes that gap: the continuation decision now lives in the main process,
persisted to disk exactly the way interrupted-turn recovery already is, and
re-evaluated at both turn completion and app startup.

## Hypothesis

If the decision "should this thread's Full Auto loop continue" is owned by
main and persisted per-thread to disk — re-checked immediately after every
completed turn and again once at app startup, after existing interrupted-turn
recovery settles — then toggling Full Auto on, sending one message, and
quitting the app entirely will still result in the loop resuming on its own
at the next launch, with no renderer-side state to lose and no user action
required beyond the original toggle. A toggle-off must remain an immediate,
durable stop regardless of whether a turn is in flight when it happens.

## Scope

```productspec-scope
in:
  - one `Full Auto` toggle in the React composer's action bar (`shell-full-auto-toggle`), off by default, no new screens (unchanged from rev 1)
  - a provider-lane-keyed Full Auto instruction in `full-auto-lane.ts`, sharing the one-concrete-step contract while allowing bounded provider framing for Codex and Claude
  - durable provider-lane continuity: the bound profile carries an additive optional `lane` ref, with absent rev-7 values interpreted as `codex-local`; every continuation dispatches through the L1 ProviderLane SPI
  - capability-gated background eligibility: a lane must be admitted by L2, advertise Full Auto, and declare a safe background-question policy; Claude `AskUserQuestion` calls deny immediately with proceed-with-judgment guidance when no renderer can answer
  - `approvalPolicy: "never"` forced on a Full Auto turn's app-server thread/turn-start requests; sandbox stays the existing danger-full-access default unchanged
  - a durable, main-owned per-thread registry (full-auto-registry.ts) recording enabled/disabled, a consecutive-continuation counter, the granted workspace identity, the bound execution profile, a per-thread dispatch lease, and typed failure/backoff state, persisted the same way local-turn-journal.ts already persists interrupted-turn state (every post-v1 field optional so existing files keep decoding)
  - workspace authority binding (FA-H2): enabling binds the currently resolved workspace (resolved by main from the same source of truth codex-local turns execute against); reconciliation refuses to dispatch -- disabling the record visibly rather than silently redirecting -- when the resolution no longer matches, and fails closed on an unbound record
  - exactly-once continuation dispatch (FA-H3): a promise-chain mutex serializing every reconciliation trigger in main, plus a durable per-thread lease claimed with the exact continuation turn ref before dispatch; the startup pass alone clears stale (crashed mid-dispatch) leases
  - a typed dispatch-failure policy (FA-H5): thrown and ok:false outcomes both persist failure state with an owner-visible note, retry under bounded exponential backoff, and disable the record after 5 consecutive failures
  - execution-profile continuity (FA-H6): the initiating flagged turn's account target, model, and reasoning effort are bound onto the record and replayed by every continuation, including post-restart resumes
  - a shared reconciliation decision (full-auto-reconcile.ts) called from two trigger points -- immediately after any Full-Auto-flagged turn completes, and once at app startup after existing turn-recovery settles -- so a background continuation and a post-restart resume are the same durable decision, not two
  - two new IPC channels: a set channel the composer toggle calls immediately (independent of whether a turn is in flight, so a toggle-off durably stops even a not-yet-sent thread) and a get channel for reading current durable state
  - reuse of the existing `DesktopLocalTurnRecoveryUpdateChannel` broadcast (already wired end to end for turn-recovery) to reflect a background continuation's result in any open window, rather than inventing a new channel
  - a bounded safety cap (20 consecutive continuations) that turns Full Auto off and leaves an explanatory note if hit, now enforced durably by main rather than an in-memory renderer counter
  - a plain toggle-off stop control, now backed by the durable registry so it stops the loop even across a restart, not just within a live session
  - coarse typed background-turn live state (FA-H4): main tracks an in-memory per-thread state (idle | turn_running | turn_completed | turn_failed | cap_reached | blocked, with the running turn ref and a bounded detail for blocked) and broadcasts every transition over `CodexLocalFullAutoStateChannel`; the get channel additively returns `{ state, turnRef }` beside `enabled`
  - a working background-turn stop (FA-H4): while the active thread's live state is turn_running the composer shows a "Full Auto running…" badge plus the Stop control, and Stop signals the thread-scoped `CodexLocalFullAutoInterruptChannel` -- main resolves the live running turn ref itself and reuses the same runtime interrupt path as the existing turn interrupt channel
  - manual-send fencing (FA-H4): a manual composer send while the active thread's live state is turn_running is refused with a transient notice and the draft kept -- never a silent second concurrent turn on the same thread
  - a Phase 1 LOCAL programmatic control surface (FA-H13): an opt-in (`OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1`), loopback-only (127.0.0.1), scoped-bearer-gated HTTP server in main (full-auto-control-server.ts) exposing list, status, enable, disable, continue-now, and bounded turn history, described by one hand-authored OpenAPI 3.1 document served at `GET /v1/openapi.json`; connection info written mode-0600 to `full-auto/control.json` under userData
  - thin MCP (scripts/full-auto-mcp.ts) and CLI (scripts/full-auto-cli.ts) clients that are deliberate pass-throughs of that one OpenAPI surface -- no client-side policy, no second schema vocabulary
  - an additive optional provider-lane selector on control `start` and `enable`, defaulting to `codex-local`, reflected through OpenAPI/MCP/CLI and the public-safe status projection
  - programmatic-call attribution (FA-H13): every control-API mutation (enable / disable / continue-now) appends a durable, distinctly-attributed system note to the thread via the existing appendFullAutoSystemNote, so an owner can always tell a programmatic action from their own click
out:
  - Phase 2 cross-machine programmatic control (relaying Full Auto routes through the openagents.com OpenAPI/Omni-SDK/public-MCP triad and Khala Sync to a running Desktop); the control surface in this revision is same-machine loopback only
  - the control API granting a new, previously-ungranted workspace; granting stays a human/UI action -- programmatic enable can only turn Full Auto on for a workspace that matches the current resolution
  - any dedicated ProductSpec/AssuranceSpec review UI, criterion board, or admission-gate screen for Full Auto
  - a separate permission/envelope/policy system; Full Auto inherits the same full-trust execution profile every other Codex turn in this app already uses
  - multi-repo, multi-thread, or fleet-wide Full Auto
  - live, token-by-token streaming of a background (main-initiated) continuation into an open window; since rev 5 (FA-H4) a background turn IS rendered as a coarse typed in-flight state with a working stop and manual-send fencing while it runs, and its result still surfaces as a completed-thread refresh -- only live text deltas stay cut
  - durability for the coarse live state itself; it is main-owned in-memory truth, and after a restart the startup reconciliation re-derives reality (a fresh dispatch re-enters turn_running, and the durable cap/blocked notes already persist on the thread)
  - the composer toggle re-syncing to a different thread's persisted enabled state on every in-session thread switch; the toggle reflects the truth for the thread you send from, but switching to another previously Full-Auto-enabled thread does not auto-flip the visible toggle in this revision
  - any change to release or public-claim authority
  - claiming ACP Full Auto readiness before an ACP lane's admitted peer profile and background-question behavior are proven; unknown/unadmitted lanes fail closed
cut:
  - CUT-FA-01: fine-grained autonomy policy beyond the plain stop control and the 20-turn safety cap
  - CUT-FA-02 (rev 1): main-process durable goal state for restart-survivable continuation -- CLOSED by this revision
  - CUT-FA-03: per-thread toggle-state resync on arbitrary in-session thread switch (open question below)
```

## Acceptance Criteria

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

## Success Metrics

```productspec-success-metrics
- id: full_auto_owner_observed_resume
  metric: owner_observed_restart_resume_sessions_recorded_in_issue_or_needs_owner_receipts
  target: ">= 1"
  window: before any release claim
  segment: owner dogfood
  source: manual owner receipt linked from issue #8885
```

## Owner Gates

- Owner review of the restart-survival behavior itself (toggle on, send,
  quit the app, relaunch, watch it resume) before any further Full Auto
  scope -- additional auto-admit categories, multi-repo, per-thread toggle
  resync on switch -- is proposed.
- Owner sign-off on the deliberate scoping choices that remain after rev 5:
  a background continuation now renders a coarse typed in-flight state with
  a working stop and manual-send fencing (FA-H4), but live token streaming
  stays cut, and the toggle does not resync on every in-session thread
  switch (see Open Questions) -- both are cheap to revisit later, not free
  to build now.

## Receipts

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

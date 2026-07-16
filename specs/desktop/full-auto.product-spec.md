---
spec_format_version: "0.1"
title: "Full Auto Codex Composer Loop"
artifact_type: "prd"
spec_revision: 3
author: "OpenAgents"
created_at: "2026-07-15T22:15:41.850Z"
updated_at: "2026-07-16T00:00:00.000Z"
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
  openagents_issue: "8852 (initial), 8853 (restart-durable continuation), 8880 (FA-H7 cap semantics), 8883 (FA-H10 registry robustness)"
  openagents_design_doc: "docs/fable/2026-07-15-full-auto-repo-intent-to-dispatch-loop.product-spec.md"
  openagents_assurance_spec: "specs/desktop/full-auto.assurance-spec.md"
  openagents_revision_note: "rev 2 (#8853) moves the continuation decision from the renderer to a durable main-process registry, closing rev 1's CUT-FA-02 gap: Full Auto now survives both a renderer reload and a full app restart, not just a live session. rev 3 (#8880, #8883) covers two changes: FA-H7 pins the continuation-cap reset semantics unambiguously (the counter resets ONLY when Full Auto is toggled off; a manual send while the toggle stays on does not reset it; the dead resetContinuation API is removed) and FA-H10 hardens the registry (a corrupt/invalid registry file is quarantined and the app starts with an empty registry instead of failing main initialization, and record eviction never drops enabled records -- only the disabled tail is bounded)."
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
  - a Full Auto instruction prefixed onto the turn prompt telling Codex to look at the repo's README/docs/issues and do one concrete next thing (codex-local-runtime.ts FULL_AUTO_INSTRUCTION, unchanged)
  - `approvalPolicy: "never"` forced on a Full Auto turn's app-server thread/turn-start requests; sandbox stays the existing danger-full-access default unchanged
  - a durable, main-owned per-thread registry (full-auto-registry.ts) recording enabled/disabled and a consecutive-continuation counter, persisted the same way local-turn-journal.ts already persists interrupted-turn state
  - a shared reconciliation decision (full-auto-reconcile.ts) called from two trigger points -- immediately after any Full-Auto-flagged turn completes, and once at app startup after existing turn-recovery settles -- so a background continuation and a post-restart resume are the same durable decision, not two
  - two new IPC channels: a set channel the composer toggle calls immediately (independent of whether a turn is in flight, so a toggle-off durably stops even a not-yet-sent thread) and a get channel for reading current durable state
  - reuse of the existing `DesktopLocalTurnRecoveryUpdateChannel` broadcast (already wired end to end for turn-recovery) to reflect a background continuation's result in any open window, rather than inventing a new channel
  - a bounded safety cap (20 consecutive continuations) that turns Full Auto off and leaves an explanatory note if hit, now enforced durably by main rather than an in-memory renderer counter
  - a plain toggle-off stop control, now backed by the durable registry so it stops the loop even across a restart, not just within a live session
out:
  - any dedicated ProductSpec/AssuranceSpec review UI, criterion board, or admission-gate screen for Full Auto
  - a separate permission/envelope/policy system; Full Auto inherits the same full-trust execution profile every other Codex turn in this app already uses
  - multi-repo, multi-thread, or fleet-wide Full Auto
  - live, token-by-token streaming of a background (main-initiated) continuation into an open window; a background continuation surfaces as a completed-thread refresh once it finishes, not live text deltas -- a coarser but simpler and fully durable signal
  - the composer toggle re-syncing to a different thread's persisted enabled state on every in-session thread switch; the toggle reflects the truth for the thread you send from, but switching to another previously Full-Auto-enabled thread does not auto-flip the visible toggle in this revision
  - any change to release or public-claim authority
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
  already-enabled thread preserves the count.
  Proof: `full-auto-restart.e2e.test.ts` "a genuinely stuck loop self-disables
  at the continuation cap across restarts, rather than continuing unbounded";
  `full-auto-registry.test.ts` "continuationCount resets ONLY on toggle-off: a
  manual send leaves it unchanged; off-then-on zeroes it".
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
- **FA-AC-11 (rev 3, #8883):** A corrupt or schema-invalid registry file never
  blocks Desktop main initialization. Opening it fails closed for the feature
  and open for the app: the bad file is quarantined beside the registry
  (best-effort rename to `registry.json.quarantined-<ISO timestamp>` with an
  owner-visible console diagnostic naming the quarantine path), the registry
  starts empty (Full Auto disabled for all threads), and subsequent writes
  persist normally.
  Proof: `full-auto-registry.test.ts` "a corrupt registry file is quarantined
  and the registry opens empty instead of throwing" and "a schema-invalid (but
  valid JSON) registry file is also quarantined rather than thrown".
- **FA-AC-12 (rev 3, #8883):** Registry record eviction never drops an
  `enabled: true` record. All enabled records are kept; only the disabled tail
  is bounded, filling remaining capacity (up to 128 total) with the
  most-recently-updated disabled records. An owner-enabled thread therefore
  always survives to the next restart, no matter how many other records were
  touched more recently.
  Proof: `full-auto-registry.test.ts` "eviction never drops an enabled record:
  the oldest enabled thread survives while old disabled records are evicted".

## Success Metrics

```productspec-success-metrics
- id: full_auto_toggle_adoption
  metric: opted_in_sessions_that_toggle_full_auto_on_at_least_once
  target: ">= 1"
  window: first week after this ships to any dogfood build
  segment: developers using an OpenAgents Desktop build with Full Auto
  source: consented_public_safe_local_usage_counters
- id: full_auto_observed_continuation
  metric: full_auto_sessions_with_at_least_one_automatic_continuation_turn_observed
  target: ">= 1"
  window: first week after this ships to any dogfood build
  segment: sessions that toggled Full Auto on
  source: consented_public_safe_local_usage_counters
- id: full_auto_restart_survival
  metric: full_auto_enabled_threads_that_resumed_a_continuation_within_one_minute_of_the_next_app_launch_without_a_manual_retoggle
  target: ">= 1"
  window: first week after this ships to any dogfood build
  segment: sessions where the app quit with Full Auto still enabled on a thread
  source: consented_public_safe_local_usage_counters
```

## Owner Gates

- Owner review of the restart-survival behavior itself (toggle on, send,
  quit the app, relaunch, watch it resume) before any further Full Auto
  scope -- additional auto-admit categories, multi-repo, per-thread toggle
  resync on switch -- is proposed.
- Owner sign-off on the deliberate scoping choice that a background
  continuation surfaces as a coarse completed-thread refresh rather than
  live streaming, and that the toggle does not resync on every in-session
  thread switch (see Open Questions) -- both are cheap to revisit later, not
  free to build now.

## Receipts

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

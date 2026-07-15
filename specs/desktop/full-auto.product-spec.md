---
spec_format_version: "0.1"
title: "Full Auto Codex Composer Loop"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-15T22:15:41.850Z"
updated_at: "2026-07-15T22:15:41.850Z"
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
  openagents_issue: "8852"
  openagents_design_doc: "docs/fable/2026-07-15-full-auto-repo-intent-to-dispatch-loop.product-spec.md"
  openagents_assurance_spec: "specs/desktop/full-auto.assurance-spec.md"
---

## Problem

OpenAgents Desktop's Codex composer requires a user to click Send for every
turn. A developer who wants Codex to keep making real, repo-grounded progress
on a granted repository unattended has no way to say "just keep working" —
they must manually resend after each completion, and there is no toggle that
tells Codex what to look at (README, docs, open issues) when the user has not
spelled out a task. This spec covers the first shipped version of that
capability: a single composer toggle, not a new permission system or a new
review UI, per explicit owner direction that the feature stay this small
until the basic loop is proven.

## Hypothesis

If a single `Full Auto` toggle in the existing chat composer hands Codex an
instruction to look at the repo and pick one concrete next thing to do, runs
that turn with `approvalPolicy: "never"` (no mid-turn approval stalls) on the
same full-access sandbox every Codex turn already uses, and automatically
resubmits a continuation after each clean completion until the user turns it
off, then a developer can press one button and get real, observable,
repo-grounded forward progress without hand-authoring a task every turn —
and we can find out, from actually watching it run, whether it is worth
extending further.

## Scope

```productspec-scope
in:
  - one `Full Auto` toggle in the React composer's action bar (`shell-full-auto-toggle`), off by default, no new screens
  - a Full Auto instruction prefixed onto the turn prompt telling Codex to look at the repo's README/docs/issues and do one concrete next thing (packages/../codex-local-runtime.ts FULL_AUTO_INSTRUCTION)
  - `approvalPolicy: "never"` forced on a Full Auto turn's app-server thread/turn-start requests; sandbox stays the existing danger-full-access default unchanged
  - automatic resubmission of a continuation turn after a clean (result.ok) completion on the same thread, reusing the exact send/settle path used for an ordinary Send (renderer shell.ts runNoteSubmission)
  - a bounded safety cap (20 consecutive continuations) that turns Full Auto off and leaves an explanatory note if hit
  - a plain toggle-off stop control; the loop rechecks fresh state before every continuation and stops immediately if Full Auto was turned off, the thread changed, or the lane is unavailable
out:
  - any dedicated ProductSpec/AssuranceSpec review UI, criterion board, or admission-gate screen for Full Auto
  - a separate permission/envelope/policy system; Full Auto inherits the same full-trust execution profile every other Codex turn in this app already uses
  - durable, main-process-owned continuation that survives an app restart mid-loop (see Risks — this is a known, explicitly flagged gap in this first version)
  - multi-repo, multi-thread, or fleet-wide Full Auto
  - any change to release or public-claim authority
cut:
  - CUT-FA-01: fine-grained autonomy policy beyond the plain stop control and the 20-turn safety cap
  - CUT-FA-02: main-process durable goal state / idempotent outbox for restart-survivable continuation (MASTER_ROADMAP invariant #24) — deferred to a follow-up issue if the basic loop proves worth extending
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
- **FA-AC-03:** After a Full-Auto turn completes with `result.ok`, the same
  thread automatically resubmits a continuation without the user clicking
  continue, using the same `chat.sendMessage` path as an ordinary Send.
  Proof: `shell.test.ts` "Full Auto (#8852): a clean Codex turn resubmits
  automatically, and toggling off mid-loop stops it".
- **FA-AC-04:** Toggling Full Auto off (including mid-loop, between two
  continuations) stops the next turn from starting; the loop rechecks fresh
  state before each continuation rather than a stale snapshot.
  Proof: same `shell.test.ts` case above (the mock flips `fullAuto` off
  between the first and second turn and only two calls are observed) and
  "Full Auto (#8852): DesktopFullAutoToggled flips the flag".
- **FA-AC-05:** When Full Auto is off, an ordinary turn sends `fullAuto`
  undefined (not `false`) and never resubmits automatically.
  Proof: `shell.test.ts` "Full Auto (#8852): toggled off, an ordinary Codex
  turn sends fullAuto undefined and never resubmits".
- **FA-AC-06:** A run of 20 consecutive automatic continuations without an
  intervening manual stop turns Full Auto off and appends an explanatory
  system note, rather than continuing unbounded.
  (Design-level; not covered by an automated test in this revision — see
  Receipts.)
- **FA-AC-07:** No Full Auto packet performs a direct commit, merge, or push;
  Codex proposes changes exactly as every other Desktop Codex turn already
  does. (Unchanged existing boundary; no new authority was added.)

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
```

## Owner Gates

- Owner review of the shipped basic loop (press toggle, watch Codex read the
  repo and act, watch it continue, watch stop actually stop) before any
  further Full Auto scope — additional auto-admit categories, multi-repo,
  restart-durable continuation — is proposed.
- Owner sign-off before pursuing MASTER_ROADMAP invariant #24 (durable
  main-owned autonomous next-turn state) specifically for Full Auto, since
  that is shared, not-yet-built infrastructure with its own review bar.

## Receipts

- `pnpm --dir apps/openagents-desktop run typecheck` — clean.
- `codex-local-runtime.test.ts`: two new cases proving `approvalPolicy` and
  prompt-prefix behavior for Full Auto vs. an ordinary turn.
- `react-composer.test.tsx`: one new case proving the toggle renders and
  reports `DesktopFullAutoToggled`.
- `shell.test.ts`: three new cases proving auto-continuation, its stop
  behavior on a mid-loop toggle-off, and that an ordinary (non-Full-Auto)
  turn never resubmits.
- Manual bounded smoke: toggle on in a running Desktop build, confirm one
  real continuation and a working stop, recorded in the closing comment on
  issue #8852.

## Promise Links

- No promise-registry entry changes as a result of this spec. The existing
  red `autopilot.desktop_full_auto_guidance.v1` entry is unrelated legacy
  scope tied to a deleted app and remains exactly as recorded; this spec does
  not claim it, reference it as satisfied, or request its update.

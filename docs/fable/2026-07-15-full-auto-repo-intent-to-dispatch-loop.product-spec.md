---
spec_format_version: "0.1"
title: "Full Auto: one-button autonomous Codex loop"
artifact_type: "openspec_proposal"
spec_revision: 4
author: "OpenAgents"
created_at: "2026-07-15T00:00:00Z"
updated_at: "2026-07-16T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
tool_metadata:
  openagents_lane: "docs/fable strategy proposal; historical design record, not the adopted spec"
  openagents_status: "implemented and tested; see the adopted spec below for as-built scope"
  openagents_issue: "8852 (implemented)"
  openagents_adopted_spec: "specs/desktop/full-auto.product-spec.md"
  openagents_adopted_assurance_spec: "specs/desktop/full-auto.assurance-spec.md"
  openagents_revision_note: "rev 4 (#8853 supersession, per #8881): the rev-3 renderer-owned/restart-fragile narrowing recorded below was fixed by #8853 (commit d480f779aa) -- continuation is now main-owned and restart-persistent. The rev-3 note is retained as honest history. The adopted ProductSpec (specs/desktop/full-auto.product-spec.md) rev 2+ remains the authoritative as-built record; this file remains the historical design/strategy trail per docs/fable convention."
  openagents_supersedes_claim_for: "autopilot.desktop_full_auto_guidance.v1 (red, docs/promises/registry.md) -- still not claimed by this implementation; see Promise Links"
---

## Implementation Note (rev 4, #8853 supersession)

The renderer-owned limitation described in the rev-3 note below was fixed in
issue #8853 (commit d480f779aa). Auto-continuation is now main-owned and
restart-persistent: a durable per-thread registry
(`apps/openagents-desktop/src/full-auto-registry.ts`) plus a shared
reconciliation decision (`apps/openagents-desktop/src/full-auto-reconcile.ts`)
runs after each completed Full-Auto turn and once at app startup after
interrupted-turn recovery, per `specs/desktop/full-auto.product-spec.md`
rev 2+ and the audit
`docs/sol/2026-07-16-openagents-desktop-full-auto-deep-dive.md`. The rev-3
note below is retained unchanged as honest history of what rev 3 actually
shipped.

## Implementation Note (rev 3)

This design shipped in issue #8852. The authoritative as-built ProductSpec is
`specs/desktop/full-auto.product-spec.md` (with a generated companion
`specs/desktop/full-auto.assurance-spec.md`), validated against the real
`packages/product-spec` and `packages/assurance-spec` CLIs and covered by new
tests in `codex-local-runtime.test.ts`, `react-composer.test.tsx`, and
`shell.test.ts`. This file remains as the historical strategy/design record
per `docs/fable`'s convention; read the adopted spec for exact current scope.
One narrowing emerged during implementation that this file's rev 2 body below
did not anticipate: the auto-continuation loop lives in the renderer
(`shell.ts`'s `runNoteSubmission`), not a main-process durable goal-state
store, because the renderer's per-turn event listener is scoped to that
turn's exact ref and a main-originated continuation turn would have no
listener to receive its events without new plumbing this issue did not build.
Concretely: **Full Auto does not survive an app restart mid-loop** — the user
must toggle it back on and send once more after a restart. Everything else in
rev 2's design below shipped as described.

## Problem

A developer using OpenAgents Desktop today still has to hand-author intent
before Codex can do anything useful in a loop: open the repo, describe the
work, watch one turn complete, decide what happens next, repeat. There is no
"just work on this" button. Rev 1 of this spec tried to close that gap by
adding a whole parallel system on top — a dedicated ProductSpec/AssuranceSpec
review UI, a criterion board, a proposal-admission gate, a signed permission
envelope with excluded-path lists and budgets. That was wrong on two counts.
First, OpenAgents Desktop does not have, and is not getting, standalone
ProductSpec/AssuranceSpec app surfaces — the product is anchored on Codex
chat and messages, full stop; any spec or plan Codex produces is just a file
in the repo, read the same way any other file is. Second, every other
delegated-Codex path in this workspace (Pylon fleet dispatch, in particular)
already runs agents at full trust by default — same posture as T3 Code — and
this feature should inherit that boundary, not invent a second, more
cautious one next to it. Full Auto should be the smallest possible thing that
lets someone press one button and find out whether an unattended Codex loop
against their own repo is actually useful, before anything more elaborate is
built on top of it.

## Hypothesis

If Full Auto is exactly one toggle in the existing chat/session view that
hands Codex a skill telling it how to look at a repo (README, docs, open
issues) and decide what to do next, runs that Codex turn under the same
full-access execution profile already used for delegated Codex work, and
automatically starts the next turn when one finishes instead of waiting for
the user to click continue, then a developer can press one button and watch
real, repo-grounded progress happen — and we can find out fast, on one real
repo, whether the basic loop is worth building further, without first
spending time on review screens, admission policy, or permission ceremony
nobody asked for.

## Scope

```productspec-scope
in:
  - one Full Auto toggle inside the existing chat/session view, off by default, no new screens
  - a skill (shipped like the existing productspec-work skill) that tells Codex what to look for in a repo it doesn't already have context on -- README, docs folder, CONTRIBUTING, open issues -- and how to turn that into one concrete next action
  - running that action as a normal Codex turn, under the same full-access execution profile already used for delegated Codex work (sandbox danger-full-access, approval never, network enabled) -- no separate permission model for Full Auto
  - automatically starting the next turn once one finishes, using the same durable goal-state/idempotent-outbox mechanism the roadmap already calls for autonomous next-turn work, so a restart does not duplicate or drop the loop
  - one stop control that halts the loop; the in-flight turn finishes or stops cleanly
  - whatever Codex chooses to write (a plan, a spec, notes) is a plain file it can create/edit in the repo like anything else -- no bespoke rendering, review card, or dedicated app surface for it
out:
  - any new ProductSpec or AssuranceSpec review UI, criterion board, packet card, or proposal-admission screen
  - a separate typed permission/admission-gate system, signed envelopes, excluded-path lists, or spend budgets; Full Auto inherits whatever trust boundary the user's Codex session already has, same as every other delegated-Codex path
  - per-category auto-admit tiers or graduated rollout ceremony; there is one mode, and it is on or off
  - multi-repo or cross-session behavior; one repo, one session, one loop
  - any change to what counts as release or a public claim
cut:
  - CUT-FA-01: fine-grained autonomy policy beyond the plain stop control -- revisit only if plain full-trust proves insufficient after real use, not in advance of it
  - CUT-FA-02: multi-repo or fleet-wide Full Auto
  - CUT-FA-03: any claim that this is ready before the basic loop has actually been run and watched on a real repo
```

## Solution

Full Auto is a toggle, a skill, and the loop OpenAgents Desktop already knows
how to run:

1. **Toggle on.** No configuration screen. No envelope to sign. It is exactly
   as available as it is trusted -- the same full-access posture the user's
   Codex session already runs under everywhere else in this product.
2. **Codex gets the skill.** The skill's job is narrow: look at the repo the
   session is already scoped to (README, a docs folder if one exists,
   CONTRIBUTING, open issues), work out what the repo's owner is actually
   trying to do, and pick one concrete next thing worth doing. It does this
   the way Codex already reads any repo -- by actually opening the files --
   not through a bespoke scanning pipeline this feature has to build and
   maintain.
3. **Codex does the work, as a normal turn.** Whatever it produces -- code,
   a written plan, a spec-shaped markdown file -- shows up exactly like any
   other Codex turn: text, patches, tool calls, in the chat. If it writes a
   plan or a spec, that plan is a file, not a new kind of app screen.
4. **The next turn starts on its own.** When a turn reaches a clean terminal
   state, Full Auto starts the next one immediately rather than waiting for
   the user to click continue, using the durable goal-state/idempotent-outbox
   handling the roadmap already anticipates for autonomous next-turn work --
   so a renderer reload or app restart resumes the loop instead of losing or
   duplicating it.
5. **Stop is always one button away.** Turning Full Auto off halts the next
   turn from starting; the current turn finishes or stops cleanly. That is
   the entire autonomy policy for this first version -- not a budget system,
   not a permission tier, just an off switch that actually works.

Committing, merging, and pushing stay exactly where they already are for
every other Codex path in this product: Codex proposes, a human integrates.
Full Auto does not change that boundary.

The first and only thing worth proving before any of this grows is whether
step 2 actually works -- whether Codex, handed nothing but the skill and a
repo, picks something real and useful to do, turn after turn, without a
human steering it. That is the test. Everything past that is premature.

## Acceptance Criteria

- **FA-AC-01:** Full Auto is exactly one toggle in the existing chat/session
  view, off by default. No new screen, card, or review surface is introduced
  anywhere in the app as part of this feature.
- **FA-AC-02:** Turning Full Auto on hands Codex the skill and runs its turns
  under the same full-access execution profile already used for delegated
  Codex work. No new permission, admission, or approval system exists for
  Full Auto specifically.
- **FA-AC-03:** Given a real repository with a README and/or a docs folder,
  Codex's first Full Auto turn demonstrably reads that content and picks a
  concrete, repo-grounded next action -- not a generic or templated response.
- **FA-AC-04:** After a turn reaches a clean terminal state, the next turn
  starts automatically without the user clicking continue. A renderer reload
  or app restart resumes the same loop rather than duplicating or dropping a
  turn.
- **FA-AC-05:** Turning Full Auto off stops the next turn from starting; the
  currently running turn reaches a clean stop rather than being killed
  mid-write.
- **FA-AC-06:** Anything Codex writes as part of Full Auto (a plan, notes, a
  spec-shaped file) is an ordinary file in the repo, reviewable the same way
  as any other Codex-authored change -- never a bespoke rendered UI object.
- **FA-AC-07:** Before any further scope is added to Full Auto, the basic
  loop above has actually been run and watched end to end on one real
  repository, and that observation is what determines whether the loop is
  worth extending -- not a plan for extending it in advance.

## Success Metrics

```productspec-success-metrics
- id: full_auto_real_first_action
  metric: full_auto_first_turns_that_produce_a_concrete_repo_grounded_action_citable_to_an_actual_file_or_issue_rather_than_a_generic_response
  target: ">= 80%"
  window: first pilot week on one repository
  segment: opted-in pilot sessions with Full Auto toggled on
  source: consented_public_safe_full_auto_turn_receipts
- id: full_auto_unattended_continuation
  metric: consecutive_full_auto_turns_completed_without_the_user_clicking_continue_before_the_user_stops_or_intervenes
  target: ">= 3 turns median"
  window: first pilot week on one repository
  segment: opted-in pilot sessions with Full Auto toggled on
  source: consented_public_safe_full_auto_turn_receipts
- id: full_auto_stop_reliability
  metric: stop_presses_that_halted_the_next_turn_without_a_further_turn_starting
  target: "100%"
  window: first pilot week and ongoing
  segment: all Full Auto stop invocations
  source: consented_public_safe_full_auto_turn_receipts
```

## Risks

- Full trust by default means a bad guess about "what to do next" can do real
  work fast, not just propose it slowly for review. The only mitigation in
  this version is that Codex still opens proposals rather than merging
  directly, and the stop control actually stops the next turn -- there is no
  permission system standing between a wrong guess and a real change.
- The skill may repeatedly pick shallow or wrong "next actions" on a given
  repo. There is no way to know this in advance; it is exactly what running
  the basic loop on a real repo is meant to reveal.
- "Full Auto" is the same name already sitting in the promise registry as a
  red, unbuilt entry (`autopilot.desktop_full_auto_guidance.v1`) tied to a
  deleted app. Shipping this feature does not make that entry green by
  itself; a separate registry pass would be needed once real use exists.
- Automatic next-turn continuation is new territory even outside Full Auto --
  the roadmap already treats "autonomous next-turn work" as a distinct,
  not-yet-proven capability with its own durability requirements. Full Auto
  should not be the first place that gets proven; it should reuse whatever
  already-proven mechanism exists for it, or wait until one does.

## Rollout

Run the basic loop on one real, owner-selected repository and actually watch
the first several turns happen. Judge from that whether Codex reliably picks
real work from the repo's own docs/issues, whether unattended continuation
holds up across a reload or restart, and whether stop actually stops. Nothing
past that -- a second repo, any adjustment to the full-trust default, any
review affordance beyond "it's a normal Codex turn" -- is worth deciding until
that first watch has happened.

Adopting this document does not change the `autopilot.desktop_full_auto_guidance.v1`
promise-registry entry, which remains exactly as recorded.

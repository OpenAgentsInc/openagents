# Full Auto Status Audit — 2026-07-21

**Date:** 2026-07-21
**Lane:** Fable status audit (owner-directed)
**Status:** Point-in-time audit of the Full Auto system across code, docs,
issues, and receipts. This document reports status. It does not grant
dispatch, release, spend, or public-claim authority. Factual status
authorities remain current code, `docs/sol/MASTER_ROADMAP.md`, live issue
state, and receipts.
**Companions:**
[`2026-07-17-full-auto-implementation-audit.md`](./2026-07-17-full-auto-implementation-audit.md)
(the corrected mid-build audit),
[`2026-07-20-full-auto-first-verifiable-mode.md`](./2026-07-20-full-auto-first-verifiable-mode.md)
(the verifiable-mode design), and
`docs/sol/2026-07-21-open-issue-unified-completion-plan.md` (the current
queue authority).

---

## I. Summary verdict

Full Auto is implemented, hardened, and independently assurance-admitted.
It is not released. Two things stand between the current state and a
truthful "Full Auto works" claim.

First, the release gate. Issue #8979 (FA-REL-01) stays open because the
shipped signed Desktop 0.1.0 fails the packaged restart smoke. A legacy
cap-migration defect caused the failure. The fix is on `main`
(`b58e2b6934`) but no signed package contains it yet. The promise
`autopilot.desktop_full_auto_guidance.v1` stays red.

Second, a fresh S1 product defect. Issue #9159 shows that ordinary chat
silently forces delegated turns into Full Auto. A plain "hey who are you"
message caused hidden autonomous repository work, including writes. An
implementation session is actively working the fix as of this audit's
late-2026-07-21 amendment, but nothing has landed. The defect stands.

Everything else in the run core is in good shape. The owner-real six-test
acceptance matrix passed with automatic same-pass provider rotation. The
AssuranceSpec was independently admitted with 61 executable-green
criteria. The Desktop test sweep for Full Auto covers 32 test files, and
the last gate run reported 3,147 passing Desktop tests across 345 files.

Coherence is now a measured quantity, not a vibe. Section VII adopts the
`docs/analysis/` rubric and the new deterministic screening grader as
the Full Auto quality metric, with a 2026-07-21 baseline over 1,397
local conversations and a standing flywheel process to hill-climb it.

## II. What Full Auto is now

Full Auto is a run, not a per-message option. That reframing came from the
2026-07-17 implementation audit and is now roadmap invariant 33. One
main-owned `FullAutoRun` retains the objective, done condition, workspace,
provider profile, lifecycle, liveness, transition attribution, and report
identity. Provider prose cannot prove completion.

The product contract, per ProductSpec revision 13 and the receipts:

- **Launcher.** A dedicated rail launcher collects one bounded mission
  prompt. Advanced fields (title, done condition, workspace, provider,
  model, fallback order, turn cap, wall-clock guardrail) stay collapsed.
  The legacy composer toggle is retired (#8974).
- **Lifecycle.** Ten typed states: draft, running, pausing, paused,
  retrying, stalled, completed, failed, stopped, cap_reached. A single
  legality gate (`applyFullAutoRunTransition`) owns every transition.
  Terminal states have no exits. A rerun mints a new `runRef`.
- **Concurrency.** Up to eight non-terminal runs coexist
  (`FULL_AUTO_RUN_ACTIVE_LIMIT = 8`). A ninth start refuses before it
  mints a thread. Each thread holds at most one Full Auto turn under a
  durable lease.
- **Turn cap.** Default 20 continuations per run
  (`FULL_AUTO_MAX_CONTINUATIONS`), or the guardrail `maxTurns` override.
  Cap exit renders as "cap reached," never as mission completion.
- **Routing.** An ordered owner-admitted policy over four action lanes:
  `codex-local`, `claude-local`, `acp:grok-cli`, `acp:cursor-agent`.
  Admission is fail-closed: the first refusal refuses the whole policy.
  Rotation occurs only on typed reasons (`account_exhausted`,
  `rate_limited`, `provider_error`) and never reorders candidates outside
  the owner grant.
- **Guardrails.** Wall-clock budget, max turns, and max per-turn failures
  are configurable. Three guardrails are non-overridable in code:
  workspace binding, own-capacity-only dispatch, and no rate-limit-reset
  triggering.
- **Controls.** Pause drains the in-flight turn deterministically. Stop
  interrupts and is terminal. Resume is legal only from paused and
  re-validates workspace, lane, and model. A loopback OpenAPI control
  server (env-gated) plus CLI and MCP clients expose runs, stop, and
  reports programmatically.
- **Evidence.** Each run derives a private `FullAutoRunReport` and a
  redacted public-safe `FullAutoRunReceipt`. An offline analyzer scores
  dogfood transcripts.
- **Mobile.** Desktop publishes a redacted live projection to
  `GET/PUT /api/full-auto-runs`. Mobile sends Pause, Resume, and Stop
  intents through `/api/full-auto-runs/control-intents`. Desktop polls,
  applies them with `actor: "mobile"`, and posts typed outcomes back.

## III. How it works in code

The implementation is a Desktop Electron main-process feature: roughly 30
source modules plus 32 test files under `apps/openagents-desktop/src/`
matching `full-auto-*`. The server and mobile carry only thin
projection and control-intent surfaces. There is no Rust or Swift Full
Auto code.

State is two-layered. The legacy per-thread registry
(`full-auto-registry.ts`, `registry.json`) owns the low-level `enabled`
dispatch gate plus workspace binding, dispatch lease, failure backoff,
continuation count, routing policy, rotation history, guardrails, and
decision history. The `FullAutoRun` registry
(`full-auto-run-registry.ts`, `runs.json`) owns the objective and typed
lifecycle on top. The two layers stay synchronized through
`settleFullAutoRunFromThreadState`, never through direct state writes.

The dispatch decision engine is `reconcileFullAutoThreads()` in
`full-auto-reconcile.ts`. Every trigger funnels into it: turn completion
callbacks, startup recovery, toggle, resume, retry, and start actions.
Passes serialize through a task queue and fan out across up to eight
threads. The per-thread decision order is: skip in-flight, skip paused,
enforce workspace binding fail-closed, check the wall-clock guardrail,
respect the failure backoff window, check the cap pre-dispatch, clear
stale leases on startup only, run the no-progress confidence gate, then
cycle rotation candidates. On a successful dispatch it increments the
continuation count, records the decision, and settles the cap in the same
pass when the budget is spent.

Commit `b58e2b6934` (2026-07-21) closed a real gap here. Before it, the
cap was only enforced pre-dispatch. A successful continuation that
consumed the final turn left the row enabled at cap until a later pass
disabled it. The packaged restart smoke seeds at cap-minus-one and
expects same-pass terminal settlement, so the window broke it. The same
commit also fixed `migrateLegacyFullAutoRegistry` to carry the legacy
`continuationCount` into `FullAutoRun.successfulAttempts`. Without that,
a near-cap legacy row restarted at zero attempts after upgrade and earned
a full fresh turn budget.

Each dispatched turn carries a typed `FullAutoMissionPacket` compiled
from the durable objective and done condition (`full-auto-mission.ts`).
That replaced the earlier constant continuation message, which had let a
green provider receipt prove the wrong task ran (#9000). Background turns
run with `sender: null`. The in-memory `fullAutoLiveState` is the only
renderer-visible signal of a running background turn. Failure handling is
typed end to end: bounded exponential backoff, disable after five
consecutive failures, same-pass rotation on rotation-eligible classes
without consuming failure budget, and typed `blockedReason` attribution
on every disable.

Newest additions (2026-07-20 and 2026-07-21): readiness-gated routing
snapshots (FAV-01), four-lane rotation parity (FAV-02), Apple FM
advisory-only capacity (FAV-03), a per-lane capacity ledger (FAV-04),
and the RLM long-run recall consumer (`full-auto-recall.ts`, #9142),
which supplies cited history candidates without moving any authority.

## IV. How it got here

The system was built in a six-day burst.

| Date | What landed |
| --- | --- |
| 07-15 | #8852: one-button renderer-owned Codex continuation loop (did not survive restart). |
| 07-16 | #8853: main-owned durable registry, restart-persistent. Deep-dive audit found 13 material defects. Epic #8873 landed 13 hardening children (FA-H1..H13): exactly-once dispatch, workspace binding, backoff, in-flight UI, quarantine, OpenAPI/MCP/CLI control surface. |
| 07-17 | Overnight owner run silently lost autonomy for about six hours (thread-cache eviction, fixed `8cb900bbf9`). The implementation audit reframed Full Auto as a run. FA-RUN wave landed: durable `FullAutoRun` lifecycle, liveness and stall detection, reports and receipts, analyzer, launcher UI, guardrails, mobile projection and remote control. |
| 07-18 | Decision to repair the Effect runtime, not rewrite on the Vercel AI SDK. Mission packet (#9000) and protected threads (#9001) fixed the broken seams. ProductSpec revision 13 admitted eight concurrent runs. Owner-real acceptance passed 6/6 at `3123d926a3` with automatic same-pass rotation. |
| 07-19 | Codex model admission moved to the live installed catalog (#9003). AssuranceSpec reached full design completeness (76 criteria, zero needs_design). |
| 07-20 | Owner direction: Full Auto is the first verifiable mode. FAV-00 epic #9110 minted (#9111–#9114). Readiness, parity, advisory, and capacity lanes landed. |
| 07-21 | Signed Desktop 0.1.0 shipped containing Full Auto. Independent assurance admission completed (#8978). Release smoke against 0.1.0 came back red. Cap fix `b58e2b6934` landed on `main`. RLM recall landed (#9142). Issues #9159 and #9158 were filed. |

## V. What is proven

- **Owner-real acceptance (2026-07-18).** All six named sidebar rows
  passed in profile `owner_real` at `3123d926a3`, plus one automatic
  same-pass `fable-local` to `codex-local` rotation. Receipt:
  `docs/sol/receipts/2026-07-18-full-auto-real-owner-acceptance.md`.
- **Independent assurance admission (2026-07-21, #8978 closed).**
  AssuranceSpec revision 6 moved `proposed` to `admitted` by a distinct
  independent reviewer identity. 61 of 76 criteria are executable-green,
  zero red, 2 smoke-gated, 5 receipt-backed, 8 designed-only. Receipt:
  `docs/assurance/receipts/authority.decision.de1e10314822b99f8d96dc46bb5302cd.json`.
  Admission grants no release or public-claim authority.
- **Cap repair verified on `main`.** `b58e2b6934` is an ancestor of
  `origin/main` with 79/79 focused regression tests
  (`docs/sol/2026-07-21-packet-a-step1-cap-repair-verification.md`).
  A follow-up (`b02d772eb8`) made the restart smoke print the packaged
  `app.asar` digest and force the double-gated temporary-profile proof
  mode. A local unsigned darwin-arm64 package that contains the fix
  passed all three restart-oracle pairs.
- **Automated coverage.** 32 Full Auto test files on Desktop, plus
  server route tests and mobile projection tests. The last full gate run
  reported 3,147 passing Desktop tests across 345 files. The source
  carries no TODO or FIXME markers in the Full Auto modules.

## VI. What is broken or unproven

### A. #9159 — ordinary chat forces hidden Full Auto (S1, open, unfixed)

This is the sharpest current defect and it is a product-authority
violation, not a crash. With a ready Claude lane, no launcher opened, and
no run started, the ordinary message `hey who are you` was routed to a
Claude subagent. The subagent received a hidden instruction:

> Since Full Auto is on for this turn, I've also been asked to pick one
> concrete, useful next thing in this repository and do it.

It then performed many Bash, Read, and Write actions and its final text
was promoted into the primary answer slot. The visible result of an
identity question was "Done" plus an unrelated release report. The
unsolicited work was, ironically, #8979 release-gate verification.

Three coupled root causes, per the issue:

1. `apple-fm-prompt.ts` makes the local Apple FM model a pure router when
   any delegate lane is ready. It forbids OpenAgents from answering the
   user directly, even for identity or plain conversation.
2. `main.ts` stamps `fullAuto: true` on every delegated lane request.
   That conflates a background delegate turn with an owner-started Full
   Auto run. `full-auto-lane.ts` then converts the flag into the "pick
   one useful work item" instruction.
3. `react-timeline.tsx` promotes the delegate's last assistant entry into
   the primary answer slot without any relevance check.

This directly contradicts the shipped surface contract: the composer
toggle is retired, Full Auto has a dedicated launcher, and an ordinary
chat submission must not create Full Auto authority. It also silently
consumes a connected provider turn.

Fix status: a first session landed only a docs-only analysis rubric. A
second session claimed the real fix (typed `background` versus `fullAuto`
fields, a local-answer route, a delegated-answer relevance gate, timeline
route disclosure) on branch `codex/9159-ordinary-chat-authority`, then
released the claim after an owner redirect, before any commit. The
defect reproduces on `b58e2b6934`, so the newest cap fix does not touch
it. As of this audit's late-2026-07-21 amendment, the originating triage
session has re-claimed #9159 and is implementing the chat/Full Auto
separation, provenance disclosure, and in-app grading fixtures from a
fresh worktree. Until that lands on `main`, the defect stands.

### B. #8979 — the release gate stays open, promise stays red

The signed release `openagents-desktop-v0.1.0` (source `26d1627722`)
contains the Full Auto loop, and the release-side criteria (signed
candidate, automated suite, signed update chain) went green on
2026-07-21. Then the independent packaged smoke against the notarized
`/Applications/OpenAgents.app` came back red.

The exact residual: a legacy seed near cap migrates into a `FullAutoRun`
with `successfulAttempts: 0` and `turnCap: 20`. After one continuation
the run stays `running` and the legacy row stays `enabled` with no
`continuation_cap_reached`. Receipt:
`docs/sol/receipts/2026-07-21-fa-rel-01-release-admission-residual.md`.

The fix exists on `main` (`b58e2b6934` plus the smoke hardening in
`b02d772eb8`) and a local unsigned package passes. What remains, per the
final #8979 comment state:

1. A newly signed package that contains the cap fix.
2. The signed-package restart smoke, green.
3. The consolidated owner-observed session: the six named sidebar tests
   on the packaged app (especially Test 05, Claude with restart), the
   packaged Pause/Resume/Stop, cap, retrying, and stalled UI checks, and
   the telemetry-off zero-outbound observation.
4. Only then the typed promise transition for
   `autopilot.desktop_full_auto_guidance.v1`.

Stock shipped 0.1.0 still carries the migration defect. Nobody should
represent 0.1.0 as a proven unattended Full Auto build.

### C. #9158 — delegated-agent activity rendered as generic rows (fixed)

The expanded delegated-agents card flattened all nested tool activity
into identical `ACTIVITY Bash` and `ACTIVITY Read` rows.
`local-harness.ts` appended `child_activity` events as plain system
text, which discarded tool kind, call identity, lifecycle state, args,
and results before rendering. This made hidden Full Auto work (defect A)
even harder to supervise. Fixed and closed late on 2026-07-21: commit
`9bb6b9ef25` preserves typed item identity and metadata, renders compact
command, file-change, tool, and reasoning rows, and persists delegated
cards across reload.

### D. Recorded seams and contradictions

- **Inert confidence gate in production.** The #8967 coordinator notes
  record that `main.ts` does not yet feed `turnEvidence` into
  reconciliation. Until wired, the FA-GD-01 no-progress confidence gate
  cannot fire in production, only in tests.
- **Control-server projection gaps.** The same notes list unexposed
  wiring: registry `rotationHistory` through the control-server project
  record and OpenAPI, and bind-guardrails, pause, resume, and
  decision-history endpoints.
- **Roadmap contradiction on concurrency.** `MASTER_ROADMAP.md`
  invariant 28 still names the "local one-active-run contract"
  (ProductSpec revision 10), while revision 13 and the owner-delegated
  autonomy plan admit eight concurrent runs. The revision-13 state is
  what the code enforces. The invariant text needs reconciliation.
- **Two-layer state as standing risk.** The legacy `enabled` registry
  and the `FullAutoRun` registry must stay synchronized through one
  settle function. Both #8979 residuals to date (the cap window and the
  migration defect) were exactly desynchronization bugs between these
  layers. The layering works now but it is the system's most
  failure-prone seam.

### E. Designed-only and deferred

- **MemoHarness cluster (FA-AC-69..76).** Designed-only. The production
  seam `full-auto-harness-policy.ts` does not exist. The independent
  reviewer scored the cluster inconclusive.
- **No composed formal model.** There is no TLA+ (or equivalent) model of
  the combined lifecycle, lease, retry, and provider-switch state space.
  Given that both release residuals were state-synchronization bugs, this
  is the highest-value missing proof artifact.
- **Grok and Cursor lanes.** Eligible in the routing policy but unproven
  in the real rotation matrix. Apple FM is advisory-only and has no Full
  Auto action lane.
- **Cross-machine and fleet Full Auto.** Explicitly out of scope and
  separately gated. The epic grants no fleet scheduling, no autonomous
  provider selection, and no remote control beyond the typed mobile
  intents.
- **Windowless AFK isolation.** Not implemented. A shared interactive
  login is not dependable process isolation (shared-Mac runbook).

## VII. Measuring coherence: the metric Full Auto must climb

The #9159 defect exposed the missing measurement. Full Auto and the
delegation router create conversations, and nothing was scoring whether
those conversations stay coherent with what the user actually asked.
That measurement now exists in `docs/analysis/` and this audit adopts it
as the Full Auto quality metric.

**The rubric.** `docs/analysis/conversation-thread-coherence-rubric.md`
(landed from the first #9159 session) scores a thread on eight weighted
dimensions: intent fidelity (20), causal continuity (15), mode and
authority integrity (20), answer relevance (15), provenance and role
clarity (10), state and sequence consistency (10), outcome closure (5),
and information economy (5). Six hard-fail gates override any weighted
score: an unadmitted mode start, an unpermitted material action, an
unrelated final answer, a wrong speaker or route display, a
cause-hiding event order, or missing evidence. The rubric's worked
example is the #9159 thread itself. It scores 9 of 100 with gates G1,
G2, and G3 failed: grade F, disposition fail.

**The programmatic layer.** The deterministic screening grader
(`pnpm run grade:coherence`, `scripts/coherence-core.ts`) now runs the
machine-checkable layer over local Codex CLI and Claude Code
transcripts. Each conversation starts at 100. User frustration deducts
points: profanity 15 per turn (cap 45), corrections such as "no, you
did this wrong" 10 per turn (cap 40), interrupts 5 each (cap 20). A
score below 80 flags the thread for the complete rubric assessment.
See `docs/analysis/deterministic-coherence-screening.md`.

**The baseline (2026-07-21).** The first full sweep graded 1,397 local
conversations. Codex CLI: 1,253 graded, mean 90.6, 14.4 percent flagged
for review. Claude Code: 144 graded, mean 90.6, 14.6 percent flagged.
The toolchains screen identically, so frustration tracks the work, not
the tool. Roughly one conversation in seven shows the user correcting
or swearing at the agent. That is the number to drive down.

**The process.** `docs/analysis/coherence-flywheel.md` defines the
dogfood loop: create test conversations against the current build
(including the hard scenarios — multi-agent handoff, rotation, restart,
chat during a live run), grade them immediately, fix the worst
confirmed defect, convert it to a replayable fixture, and append the
sweep to `docs/analysis/coherence-ledger.md`. The ratchet: the mean
must not fall, the needs-review rate must trend down, and a hard-fail
gate defect blocks release evidence at any score.

**The public extension.**
`docs/analysis/public-coherence-benchmark-spec.md` proposes the public
graph: coherence distributions per toolchain, trend lines per
OpenAgents version, gate-incident counts, and user-contributed traces
graded through the existing public `/trace/{uuid}` path. It is a
proposal and carries no route or deploy authority.

The screen's honest limit matters for Full Auto: the #9159 thread would
screen clean if the user never complained, because the hidden mode
start is a gate defect, not a frustration signal. The rubric tripwires
("a delegate request has `fullAuto: true` in an ordinary chat", "a
non-action request causes a command or write") are the detection for
that class, and they must become automated fixtures in the #9159 fix.

## VIII. Open-issue inventory touching Full Auto

At this audit's late-2026-07-21 amendment, four open issues touch Full
Auto or its measurement:

| Issue | State | Role |
| --- | --- | --- |
| #8967 | Open (epic) | Full Auto productization epic. Open only for its #8979 obligation now that #8978 closed. |
| #8979 | Open (P0) | FA-REL-01 release admission. The single active closure gate. Residual: signed package with the cap fix plus the owner-observed session. |
| #9159 | Open (S1 bug) | Hidden Full Auto in ordinary chat. Active implementation session, nothing landed yet. |
| #9160 | Open (infra) | COH-01 coherence measurement: grader, flywheel, ledger, benchmark spec (section VII). |

The remaining open issues (#9041 IDE-13, #9032 SBX-10, #9144 RLM-08) do
not mention Full Auto in their bodies. Recently closed and relevant:
#9158 (typed delegated-activity rows, `9bb6b9ef25`), #8978 (assurance
admitted), #9142 (RLM recall), #9133 (in-lane fallback adopted, "Full
Auto keeps authority"), and the whole #8873/#8967 child graph.

## IX. Recommended completion order

1. **Fix #9159 first.** It is the only defect that violates the
   authority model in the shipped product path. The released claim's
   plan was sound: typed `background` and `fullAuto` fields, a
   first-class local-answer route, a relevance gate before promotion,
   and visible route provenance. Land it with the ten acceptance
   criteria in the issue, plus automated fixtures for the rubric
   tripwires it triggered (section VII).
2. **Cut the signed package and run the smoke.** Packet A steps 2
   through 4 of the unified completion plan: signed candidate with
   `b58e2b6934`, full suite, packaged two-process restart smoke green.
3. **Run the single owner-observed session.** Six sidebar tests,
   Pause/Resume/Stop and terminal-state UI checks, telemetry-off
   observation. Then flip the promise through its typed transition.
4. **Wire the recorded seams.** Feed `turnEvidence` into reconciliation
   so the confidence gate is live, expose rotation history and the
   missing control-server endpoints, and fix roadmap invariant 28.
5. **Run the coherence flywheel every rotation.** Grade created
   conversations with `pnpm run grade:coherence`, assess flagged
   threads with the rubric, and append each sweep to the coherence
   ledger. Hill-climb the needs-review rate from the 2026-07-21
   baseline (about 14.5 percent) toward zero.
6. **Close the proof debt deliberately.** Decide whether the MemoHarness
   cluster and a composed formal model gate the next release or move to
   a named later milestone. Do not let them stay ambient.

Full Auto's core loop is real, tested, and admitted. The remaining work
is not construction. It is release discipline (#8979) and one honest
routing repair (#9159) so that Full Auto runs only when the owner starts
one.

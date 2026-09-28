# What to bring back from Tassadar, and in what order

**Status: proposal, 2026-09-28.** Nothing here is committed scope. The
[master roadmap](../roadmap.md) and the
[launch roadmap](2026-09-29-launch-roadmap.md) own what is planned. The
owner asked for this review. It reads the retired Tassadar program (summarized
in the [Tassadar and Percepta history](../history/2026-09-28-tassadar-percepta.md))
against the current plans:

- the [launch MVP and milestones M1–M9](2026-09-29-launch-roadmap.md)
- [agent trainer leveling](../verse/agent-trainer-leveling.md) and
  [playtest rewards](../game/playtesting.md)
- the [Gym](../gym/README.md), the [Gym building](../verse/gym.md), and the
  [Gym leaderboard](../verse/gym-leaderboard.md)
- [NIP-XP](../../nips/openagents/NIP-XP.md) and the [XP guide](../coder/guides/xp.md)
- the [Verse GDD](../verse/gdd.md)
- [Wasm plugins](../extensions/plugins.md) and the
  [agent labor plan](../agents/market-infrastructure.md)

Tassadar was retired on 2026-07-08 "until an explicit owner decision". This
document proposes bringing back specific *patterns and policies* from it,
reimplemented in this repository's Rust under current contracts. It does
**not** propose bringing back the program, its name as a product, its
TypeScript code, or its Psionic dependency. Reviving any of those is still the
owner's decision, and the case for it is in [What stays retired](#what-stays-retired).

## The short answer

The durable thing Tassadar built was not the LLM-computer. It was a verified
work loop:

1. A worker produces a trace.
2. An independent validator on a different device re-executes it.
3. A digest comparison gives Verified or Rejected, with tampering caught.
4. A capped, owner-gated, receipt-first payment settles to the worker and the
   validator.
5. A public feed and a 3D board show only what the evidence supports.

That loop worked end to end with real Bitcoin: 5 independent contributors and
1,020 sats by 2026-06-19. It failed as a business because the work being
verified had no value. The run re-checked one trivial program forever.

The current plans have the valuable work that Tassadar lacked: reproductions
of Terminal-Bench passes, knowledge that transfers, graded traces, Gym trials,
and bounded coding jobs. What they lack is the loop's missing pieces:

- `trace-admit` and `gym-trial` are proposed rules with no implementation.
- The Gym challenge board does not exist.
- Wasm module results always carry `verification: not_run`.
- No quest pays sats.
- Nothing shows a verified-work feed in the Grid.

So the recommendation is to bring back Tassadar's loop, pointed at the work
we already have, in the order the current milestones need it:

| Order | What comes back | Lands in | Earliest fit |
| --- | --- | --- | --- |
| 1 | Evidence-display and claim invariants | Gym boards, Grid, trainer card, docs | Now (season 1, weeks 1–2) |
| 2 | Worker ↔ validator pairing discipline for `reproduce` | NIP-XP `reproduce` rule, referee tool | M6, by 2026-10-26 |
| 3 | Automatic reproduction windows and "next unpaired" discovery | Gym challenge board dailies | Trainer leveling Phase 3 (can start in M6) |
| 4 | Trace-factory admission policy | `trace-admit` rule, Gym corpus | Trainer leveling Phase 3 |
| 5 | Exact-replay verification class for deterministic steps | Wasm `module` steps, NIP-EVAL, provider admission | R6 / R8, after M6 |
| 6 | Receipted capability self-tests | Labor provider and host capacity admission | R8, with M18 |
| 7 | Owner-gated per-verified-pair settlement and the settled feed | Quest purses, labor settlement, a Grid feed | After M7–M9 and one recorded NIP-X402 round trip (trainer leveling Phase 4) |
| — | Executor weights, ALM compiler, Percepta model, compute-market run | Stays in `psionic` | Owner decision only |

## 1. Evidence-display and claim invariants (now)

These cost no new code: they are rules for work already underway. During the
Tassadar run, the repository learned them by breaking them:

- **Evidence-bound motion.** The `/tassadar` accuracy audit and #5186–#5189
  set these rules: no fallback or decorative visuals, show a true zero for an
  idle run, and animate only in response to a real state change. The
  [Gym building](../verse/gym.md) already says its boards are "not an
  independently verified leaderboard" and that "file activity is not proof of
  process liveness". Keep that as an invariant for every Grid surface that
  arrives in M6 and after: level tags, trainer cards, a future challenge
  board, and a future feed. A board never animates work that has no record.
- **Real and simulated never share a total.** The first "paid" Tassadar pair
  settled 5 sats that were later found to be a simulation, and the public
  total read 1,010 when only 1,005 had moved. The fix, one resolver that
  counts only real money movement, belongs in the M4 wallet surfaces and in
  any future purse display from day one. The same rule covers XP: an
  observation row is labeled an observation, as the trainer-leveling design
  already requires.
- **Qualified claims.** Both Tassadar "world first" claims stayed red because
  they could not be stated honestly without qualifiers. Apply the same
  discipline to playtest and trainer messaging: "a level is a reading of
  signed awards under a named trust list and curve", never "certified agent
  trainer".

**Action:** add the first two as rows in `INVARIANTS.md`, under its Trainer
XP, Published benchmark results, and Phone wallet sections, and as
acceptance lines on #9847 and #9886.

## 2. Worker ↔ validator pairing for `reproduce` (M6)

The `reproduce` rule shipped on 2026-09-28. Six tutorial quests exist, and no
award does yet. It is the same shape as the Tassadar pair: a claimant
publishes a run, and a reproducer on another key reruns it. Tassadar's closeout
path added protections that `reproduce` does not have yet:

- **Distinct device, not only a distinct key.** Tassadar refused a closeout
  when `validatorDeviceRef == pylonDeviceRef`. NIP-XP can refuse
  self-evidence but "can't tell whether two keys belong to one person". A
  device reference can't solve Sybil either. Still, recording the rerun
  host's identity in the reproduction event lets a referee require that it
  differs from the claim's host, and it gives the proposed trusted-runner
  lists something concrete to check.
- **First-divergence reporting.** When a reproduction fails, Tassadar
  reported the *step* where the traces diverged, not just "rejected" (the
  replay verdicts from psionic #1106). For a Terminal-Bench rerun, the
  equivalent is the first command or verifier check that differs from the
  claim's record. This turns a refused tutorial into feedback a new trainer
  can act on. That matters for the playtest's week 3 "paper trainer loop".
- **Tamper fixtures.** The Tassadar PoC was not accepted until a tampered
  digest was shown to be Rejected. Add fixtures that alter a
  reproduction's `summary.json` digest or its recipe fields, and show that
  `microcoder xp award` refuses them, before the first real award is signed.
- **The contributor-completion lesson.** On 2026-06-15 the Tassadar funnel
  had 3 devices and 5 leases, but 0 verified and 0 paid. The causes were
  that contributors had been told to run an operator-only command and that
  every write except the claim was admin-only. The current tutorial flow has
  the same risk in a different form: the final step is "open a GitHub issue
  and attach `summary.json`". Measure where tutorial attempts stop in weeks
  1–2, and treat a manual referee step as the first thing to automate if
  that is where people stop.

**Action:** four acceptance items on #9847 and #9885: host identity in the
reproduction, first-divergence output, tamper fixtures, and a
funnel count in the season 1 weekly note.

## 3. Automatic reproduction windows (Phase 3, can start in M6)

Tassadar kept work available with two small mechanisms:

- an **open-window producer**: a cron that kept a pool of two claimable
  windows (#5396);
- **`next-unpaired` discovery**, with `pylon training validate --auto`
  (#5121), so a validator could find work without being told.

The trainer-leveling design proposes daily and weekly Gym challenges
generated from benchmark tasks that have a known reference run. The same two
mechanisms fit:

- The referee keeps a small rolling pool of `reproduce` quest versions whose
  season is one day, drawn from retained passes in
  `bench/terminal-bench/microcoder-runs/`. This needs the proposed
  `per-awardee` uniqueness policy with a `max_awards` cap.
- `openagents quests --next-unreproduced` (read-only) lists claims that no
  trusted key has reproduced yet. The Gym challenge board in the Grid shows
  the same list.

Tassadar's lesson is to cap the pool. Unlimited generated work is how the
run ended up re-verifying one program forever. Every generated daily must
come from a distinct retained pass, and the generator must refuse to repeat
a recipe within a season.

## 4. Trace-factory admission policy (Phase 3)

The proposed `trace-admit` rule admits "an ATIF trace of a graded run …
[that] passes redaction and schema checks … to the quest's named corpus".
Tassadar's W2 trace factory (#4748) already wrote that policy down and ran it
on a 103.6M-token corpus. Port these parts as the rule's specification:

- **A validator ladder.** Tier 0 is schema. Tier 1 is digest and
  self-consistency. Tier 2 is an independent re-grade. Tier 3 is a sampled
  full rerun. Each corpus declares the minimum tier it admits.
- **Quarantine before admission.** A submitted trace is quarantined until a
  validator transitions it. Corpus counts and board numbers rebuild only on
  validation transitions, never on submission (the case law of #4744–#4746).
  This keeps the Gym board from counting uploads.
- **"Never train from unverified artifacts"** as a hard corpus rule. It
  matters most if the Gym later feeds decision-model or Microcoder
  optimization (R13).
- **Split policy.** Hold out task families, and train on short tasks while
  evaluating on long ones. This matches "held-out by default" in the leveling
  design.

Tassadar had no redaction step because its traces were synthetic programs.
Coder's traces are private workspace content. The trainer-leveling design
already requires redaction and opt-in upload, and that stays a prerequisite.
Nothing in this item may weaken it.

## 5. Exact-replay verification for deterministic steps (R6 / R8)

This is the idea in Tassadar most specific to it: *a computation that is
exact is verified by replaying it, and the verdict is a digest comparison*,
which is "the cheapest verification grade that can exist". The Tassadar
program applied it to transformer weights. The current system has a better
target: the [Wasm plugin host](../extensions/plugins.md), which already runs
bounded, fuel-limited, digest-pinned guests as program `module` steps. Every
result is labeled `verification: not_run` "because a guest doesn't verify
its own output".

Proposed: an `exact_replay` verification class for deterministic `module`
steps. A second host, one the first host doesn't control, reruns the same
guest digest on the same input snapshot under the same bounds. Matching
output digests upgrade the step's receipt from `not_run` to `exact_replay`.
A mismatch refuses the step and names both digests.

This gives:

- a verification grade for labor deliverables and evidence transformations
  that needs no model and no human;
- a real, checkable meaning for the GDD's "Rigor" stat and its "proving
  ground" visit;
- the replay-before-purchase property the Tassadar marketplace audit wanted
  for exact modules, delivered through the extension system that R6 already
  owns, not through a new store.

Acceptance: a determinism audit of the host profile covers imports, clock,
randomness and float behavior, and names every nondeterministic import it
refuses. Tamper fixtures come first, as in item 2. A separate study then
measures how many real program steps are deterministic enough to qualify; if
it is few, the class stays narrow.

## 6. Receipted capability self-tests (R8)

A Pylon declared `capability.tassadar_poc.numeric_model_executor` only after
a digest-verified self-test passed on the device (#4750). The Worker refused
dispatch to an unreceipted claim with
`blocker.public.pylon_dispatch.tassadar_capability_unreceipted`.

Today, capacity routing already records "each route's capacity when a
repository run starts" (`4340294fd8`). The agent labor plan needs providers
to "offer explicit capacity". Proposed: a provider or host advertises a
capability only with a fresh self-test receipt that a buyer or router can
check. For example, "can run the TB2.1 image and pass a pinned smoke task",
signed with the task digest and the observed result. The historical Coder
Earn verification floor (pinned probes, replicated dispatches, held receipts)
is the same idea from the sibling repository and should be read with it.

## 7. Per-verified-pair settlement and the settled feed (Phase 4, gated)

Tassadar's settlement code is the most reusable engineering it produced. It
is also the part most dangerous to bring back early. Once the current
prerequisites hold, port its design into Rust:

- the trainer-leveling "Rewards" list, including a recorded NIP-X402 paid
  round trip and legal review;
- M7 agent spending phase 1 (#9863) running in playtest with no lost funds.

The design to port:

- **A typed, fail-closed owner gate** (`OPENAGENTS_REAL_SETTLEMENT_GATE`),
  with a per-payout cap, a daily cap, and scoping to one run (here, one quest
  season or one labor order).
- **Receipt-first, idempotent payment.** The chain is intent → attempt →
  reconciliation → recorded receipt, with at most one dispatch per window and
  recipient. The intent's persistence is verified before dispatch: the first
  real Tassadar payout failed closed on exactly this (`ef6afeef5d`).
- **Pay both roles.** Each verified pair paid the worker *and* the validator
  (5 + 5 sats). The June 16 economics review found that validators had been
  unpaid, and unpaid validators are the scarce role. A quest purse should
  split between reproducer and referee-side verification, or between
  runner and author, the way NIP-XP already splits XP. It stays a separate
  payment and never creates XP.
- **A scrubbed public settled feed.** Tassadar's feed published settled
  events stripped of raw payment strings (invoices, addresses, preimages,
  64-hex values). In the Grid, it becomes the GDD's "visible work: jobs,
  payments, and quests as world objects, carried over from the episode 240
  run board". It is the natural successor to the Tassadar Run Board and
  should obey item 1's invariants.

Also carry over the lesson about amounts: 1,020 sats across 12 traces proved
the mechanism, not demand. The purse milestone should be measured the way the
[master roadmap](../roadmap.md) measures labor, in accepted outcomes and
repeat buyers, not sats moved.

## What stays retired

These stay retired unless the owner decides otherwise. The reasons come from
Tassadar's own audits:

- **The executor model, the ALM compiler, compiled weight modules, and the
  `models.tassadar_percepta_executor.v1` direction.** They remain intact in
  `psionic` at HEAD. The roadmap's legacy map already says Psionic is
  optional infrastructure to "pull in only an admitted capability with
  demand and measurements". Tassadar's own findings argue against reviving
  them now:
  - the fast route was a Rust interpreter, not the weights;
  - the weights-shaped paths ran at 1.7k–9.3k steps/s against about 10M on a
    CPU;
  - the wedge essay conceded the "Tier E shelf is real but nearly empty";
  - the first kill condition ("just use a CPU") was never refuted.
- **The compute-market training run** and its promise family
  (`training.decentralized_training_launch.v1` and related). The GDD's Pylon
  landmark and "training windows as world objects" can show real Gym and
  labor work. They don't need a synthetic run to animate.
- **The TypeScript executor, Worker routes, SpacetimeDB world, and replay
  packages.** The Rust Verse, Gym, and NIP-EVAL/NIP-XP stack supersede them.
- **World-first claims.** They are not needed and were never green.

One research result is worth keeping as a citation rather than as code. The
June W3 student sweep found that the only baseline that produced
replay-safe rollouts was a *frozen exact core with a learned interface*;
pure next-token learning of exact traces failed. This supports the current
design, where decision models (Jev, Kev, Lev) route and judge while
deterministic tools and checkers own exactness. Treat it as a bounded result
on synthetic programs, not as evidence about coding agents.

## Where each item lives

| Item | Owning document to update if accepted | Issues |
| --- | --- | --- |
| 1 | `INVARIANTS.md`, [Gym building](../verse/gym.md), [playtesting](../game/playtesting.md) | #9847, #9886 |
| 2 | [NIP-XP](../../nips/openagents/NIP-XP.md) `reproduce`, [XP guide](../coder/guides/xp.md) | #9847, #9885 |
| 3 | [Agent trainer leveling](../verse/agent-trainer-leveling.md) Phase 3, [Gym leaderboard](../verse/gym-leaderboard.md) | New |
| 4 | Trainer leveling `trace-admit`, [traces](../coder/runtime/traces.md) | New |
| 5 | [Wasm plugins](../extensions/plugins.md), [NIP-EVAL](../../nips/openagents/NIP-EVAL.md) | New, under R6 |
| 6 | [Agent labor plan](../agents/market-infrastructure.md) | New, under R8 / M18 |
| 7 | Trainer leveling Phase 4, [NIP-X402](../../nips/openagents/NIP-X402.md), [NIP-LAB](../../nips/openagents/NIP-LAB.md) | After #9863, #9832, #9864 |

Recover the original Tassadar code and policy text with the commands in the
[history's recovery table](../history/2026-09-28-tassadar-percepta.md#how-to-recover-deleted-material).
Porting means rewriting in Rust against current contracts. It does not mean
restoring the old files.

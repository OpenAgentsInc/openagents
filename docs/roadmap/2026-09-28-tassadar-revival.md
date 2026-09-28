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
- programs and extensions: the [extension architecture](../extensions/architecture.md),
  [packages](../extensions/packages.md), [Wasm plugins](../extensions/plugins.md),
  [programs](../programs.md), and [NIP-PRG](../../nips/openagents/NIP-PRG.md),
  [NIP-EXT](../../nips/openagents/NIP-EXT.md), and [NIP-CAP](../../nips/openagents/NIP-CAP.md)
- the marketplace and network: the [networked Coder plan](../coder/design/networked-coder-plan.md),
  the [agent labor plan](../agents/market-infrastructure.md),
  [Beat Fable together](../coder/beat-fable-together.md), and
  [NIP-MKT](../../nips/openagents/NIP-MKT.md) and [NIP-LAB](../../nips/openagents/NIP-LAB.md)

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
- No component says how it can be checked, and nobody earns credit or pay
  for checking someone else's work.
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
| 5 | Exact-replay verification class for deterministic steps | Wasm `module` steps, step receipts | R6, after M6 |
| 6 | Verification classes (E/D/S/N) on components, independent assessors, and per-class network measurement | NIP-EXT descriptors and assessments, progressive discovery, NIP-XP | R6 / R7, with networked Coder stage 5 |
| 7 | Receipted capability self-tests | NIP-CAP probes, labor provider and host capacity admission | R8, with M18 |
| 8 | Owner-gated per-verified-pair settlement, paying the checking role, and the settled feed | Quest purses, labor settlement, a Grid feed | After M7–M9 and one recorded NIP-X402 round trip (trainer leveling Phase 4) |
| — | Executor weights, ALM compiler, Percepta model, compute-market run, a module store | Stays in `psionic` or retired | Owner decision only |

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

## 5. Exact-replay verification for deterministic steps (R6)

This is the idea most specific to Tassadar: an exact computation is verified
by replaying it, and the verdict is a digest comparison, "the cheapest
verification grade that can exist". Tassadar applied it to transformer
weights. The current system has a better target in the
[Wasm plugin host](../extensions/plugins.md), which already runs bounded,
fuel-limited, digest-pinned guests as program `module` steps. NIP-PRG's two
profiles, `pure` and `snapshot-read`, both deny "ambient clock/randomness,
and model calls", and "a fresh instance serves each invocation". That is
most of a determinism contract already. Today every result is labeled
`verification: not_run` "because a guest doesn't verify its own output".

Proposed: an `exact_replay` **verification class**. A second host, one the
first host doesn't control, reruns the same guest digest on the same input
snapshot digest, under the same bounds and a recorded engine configuration.
Matching output digests set the step's verification to `passed` with class
`exact_replay`. A mismatch sets it to `failed` and names both digests.

The class is a separate field beside the shared verdict. It does not become
a new verdict value. [Shared contracts](../../nips/openagents/contracts.md)
fix verification as `passed`, `failed`, `unverifiable`, or `not_run`, and
widening that enum would be an invariant change across every NIP that reuses
it. A class field says *how* a `passed` was obtained, which is what item 6
needs.

Prerequisites taken from the plugin docs' own gaps:

- The host records the Wasmtime version and engine configuration. It "doesn't
  record" them today.
- The host writes the digested invocation receipt, which is still listed as
  not built.
- `snapshot-read` inputs are named by snapshot digest, so the rerun reads
  the same bytes.
- A determinism audit covers float behavior (NaN bit patterns), fuel
  accounting across engine versions, and any import the host adds later.
  Each nondeterministic import it refuses is named.

Acceptance: tamper fixtures come first, as in item 2. Then a study measures
how many real program steps qualify. The three evidence guests (repository
map, code search, test report) are the natural first subjects. If few steps
qualify, the class stays narrow.

## 6. Verification classes on components, and who checks them (R6 / R7)

The **Tassadar marketplace audit** ("The Store We Built Twice") traced three
generations of an OpenAgents store:

1. the 2024 agent store with paid Wasm plugins;
2. Blueprint programs;
3. replay-verifiable compiled modules.

It proposed shelf tiers by how a buyer could check a good:

- **E**: exact replay.
- **D**: deterministic tests.
- **S**: statistical evidence.
- **N**: none.

Its rules were "replay before purchase clears" and "the store is built last".

**The current plans already agree on the conclusion** and have better
parts:

- The roadmap's legacy map says of the Extism-era marketplace: "do not
  revive an unused registry as a success metric."
- The roadmap states that shared knowledge and reusable programs, by their
  existence alone, are "not a network effect".
- The [networked Coder plan](../coder/design/networked-coder-plan.md)
  measures the network as **incremental out-of-sample verified passes per
  adopted contribution**.
- Programs ([NIP-PRG](../../nips/openagents/NIP-PRG.md)), packages
  ([NIP-EXT](../../nips/openagents/NIP-EXT.md)), and capabilities
  ([NIP-CAP](../../nips/openagents/NIP-CAP.md)) give exact identities,
  inert installation, revocation, and third-party assessments.

So the store stays retired. The shelf tiers come back as a vocabulary the
existing system is missing.

**What is missing is a way to say how well a component can be checked.** An
EXT operation descriptor can list `evaluation` references. A component-set
assessment gives `eligible`, `ineligible`, or `unknown`. Neither says
whether the component's result can be replayed, tested, or only measured.
Proposed: map the tiers onto evidence the system already produces, and let
readers filter and report by class.

| Tier | Verification class | Existing evidence behind it | Checked by |
| --- | --- | --- | --- |
| E | `exact_replay` | Item 5's second-host rerun of a `pure` or `snapshot-read` module step | Any host that holds the bytes, with no model and no human |
| D | `checker` | A NIP-LAB checker receipt under `all-pass-v1`, or Coder's frozen independent checks | The declared checker, which the worker cannot edit |
| S | `measured` | A NIP-EVAL report with a baseline, paired on/off runs, and `written_from` exclusion | An evaluator other than the author, as NIP-XP's `kb-transfer` requires |
| N | none | The author's claim or a `not_run` result | Nobody. Show it as unchecked |

**Where it plugs in:**

- **Step receipts** (NIP-PRG step envelope) carry the class of their
  `passed` verdict.
- **EXT operation descriptors** may declare the class a component *claims*,
  with evidence references. A reader treats that as a claim until it has
  its own replay, checker receipt, or trusted report. EXT already says
  "candidate provenance does not establish evaluation success".
- **Component-set assessments** (EXT) gain the class the assessor actually
  verified. The assessor is Tassadar's validator role, applied to
  components: a key other than the publisher that reran or measured the
  component and signed the result.
- **Progressive discovery** can filter on class the way it filters on
  compatibility. An operator can say "admit only E and D components
  automatically; propose S; never auto-admit N". Selection and admission
  stay separate, as the extension architecture requires.

**What it does for the network effect.** Tassadar's commercial thesis was
that cheaper verification lowers the cost of trusting a stranger's work, so
more of it gets adopted. That is testable with the metric the networked
Coder plan already defines. Report incremental verified passes per adopted
contribution *by verification class*, together with verification cost per
class. If E- and D-class components are not adopted more, or do not help
more per unit of verification spend, the thesis fails here too and the
class stays a label. The plugin docs already require a measurement plan
that decides whether each evidence guest stays. This extends that plan
across operators.

**Credit before money.** Assessors have no role in XP today, and no payee in
MKT or LAB. The first step is an XP rule, not a payment. A `component-assess`
quest rule (or a generalized `reproduce`) would award an independent key for
an exact replay or a checker run on another publisher's component, under the
same "claimant and reproducer MUST be different keys" rule. This builds the
validator pool before any purse exists, the same way item 2 builds the
runner pool for `kb-transfer`.

## 7. Receipted capability self-tests (R8)

A Pylon declared `capability.tassadar_poc.numeric_model_executor` only after
a digest-verified self-test passed on the device (#4750). The Worker refused
dispatch to an unreceipted claim with
`blocker.public.pylon_dispatch.tassadar_capability_unreceipted`.

NIP-CAP already has most of this shape:

- presence states `present | absent | unavailable | unprobed | unknown`,
  where "only `present` can become a route";
- probes run only under host-owned approval and "MUST NOT start paid or
  effectful task execution";
- `support.evidence` lists receipt schema IDs.

What it lacks is the receipt itself. A "signed claim does not prove
enforcement". Proposed: a probe result that carries a self-test receipt. It
names a pinned smoke task digest and the observed result, for example "ran
the TB2.1 image and passed task X under these bounds". A NIP-MKT offering's
exact `capability` reference can then point at a capability whose presence
a buyer or router can check.

Capacity routing already records "each route's capacity when a repository
run starts" (`4340294fd8`). This extends that record from capacity to
demonstrated ability. The historical Coder Earn verification floor (pinned
probes, replicated dispatches, held receipts) is the same idea in the
sibling repository and should be read with it.

## 8. Per-verified-pair settlement and the settled feed (Phase 4, gated)

Tassadar's settlement code is the most reusable engineering it produced. It
is also the part most dangerous to bring back early. Port its design into
Rust only once the current prerequisites hold:

- the trainer-leveling "Rewards" list, including a recorded NIP-X402 paid
  round trip and legal review;
- M7 agent spending phase 1 (#9863) running in playtest with no lost funds.

The design to port:

- **A typed, fail-closed owner gate** (`OPENAGENTS_REAL_SETTLEMENT_GATE`).
  It has a per-payout cap and a daily cap, and it is scoped to one run: here,
  one quest season or one labor order.
- **Receipt-first, idempotent payment.** The chain runs intent → attempt →
  reconciliation → recorded receipt, with at most one dispatch per window and
  recipient. The intent is confirmed persisted before dispatch; the first
  real Tassadar payout failed closed on exactly this (`ef6afeef5d`).
- **Pay the checking role.** Each verified Tassadar pair paid the worker
  *and* the validator (5 + 5 sats). The June 16 economics review found that
  validators had been unpaid, and unpaid validators are the scarce role.
  - NIP-LAB today pays only the provider. Its reviewer and resolver are
    required to be distinct, but have no payee.
  - A quest purse or labor order should be able to pay the verifying key:
    the reproducer, the reviewer, or item 6's assessor. It should do so
    under terms fixed before dispatch, the way NIP-XP already splits XP
    between author and runner.
  - It stays a separate payment and never creates XP.
- **Author payment follows measured contribution, not trace presence.**
  Tassadar wanted "revenue splits decomposed from traces". The labor plan
  already keeps worker, component-author, and data-owner payments distinct.
  It also says a recorded use "does not prove an entry caused a win". MKT
  and LAB exclude royalty splits. Reconcile them this way:
  - a run lock proves which component digests executed, which makes an
    author *eligible*;
  - an S- or D-class result against the component's absence sets the
    *basis* for payment;
  - any author share is a declared reuse term in the buyer's quote, under a
    new payment profile. It is never an automatic royalty to every entry
    that retrieval displayed.
- **A scrubbed public settled feed.** Tassadar's feed published settled
  events with raw payment strings stripped out: invoices, addresses,
  preimages, and 64-hex values. In the Grid it becomes the GDD's "visible
  work: jobs, payments, and quests as world objects, carried over from the
  episode 240 run board". It is the natural successor to the Tassadar Run
  Board and should obey item 1's invariants.

Also carry over the lesson about amounts. 1,020 sats across 12 traces proved
the mechanism, not demand. Measure the purse milestone the way the
[master roadmap](../roadmap.md) measures labor: in accepted outcomes and
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
- **The compiled-module marketplace and its listing routes.** Item 6 keeps
  its shelf tiers as a verification vocabulary. It does not keep a store.
  Components are distributed through NIP-EXT and sold only as part of labor
  under NIP-MKT and NIP-LAB, where a buyer pays for an accepted outcome.
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
| 5 | [Wasm plugins](../extensions/plugins.md), [NIP-PRG](../../nips/openagents/NIP-PRG.md) step envelope, [shared contracts](../../nips/openagents/contracts.md) (class field only) | New, under R6 |
| 6 | [NIP-EXT](../../nips/openagents/NIP-EXT.md) descriptors and assessments, [extension architecture](../extensions/architecture.md) discovery, [networked Coder plan](../coder/design/networked-coder-plan.md) metric, NIP-XP rule | New, under R6 / R7 |
| 7 | [NIP-CAP](../../nips/openagents/NIP-CAP.md) probes, [agent labor plan](../agents/market-infrastructure.md) | New, under R8 / M18 |
| 8 | Trainer leveling Phase 4, [NIP-LAB](../../nips/openagents/NIP-LAB.md) payees and reuse terms, [NIP-MKT](../../nips/openagents/NIP-MKT.md) payment profile, [NIP-X402](../../nips/openagents/NIP-X402.md) | After #9863, #9832, #9864 |

Recover the original Tassadar code and policy text with the commands in the
[history's recovery table](../history/2026-09-28-tassadar-percepta.md#how-to-recover-deleted-material).
Porting means rewriting in Rust against current contracts. It does not mean
restoring the old files.

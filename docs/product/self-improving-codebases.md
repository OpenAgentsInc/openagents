# Self-improving codebases

Status: product spec, proposal, 2026-10-10. Owner direction: *"This is
recursive self improvement as a product. For us and then others.
Self-improving codebases."* Nothing here is a launch promise until it meets
its own gates. Every number carries an evidence class
(`measured | authored | sample | contract_only`), the rule from the
[training system audit](../audits/2026-10-10-training-system-audit/roadmap.md).

## The idea in one paragraph

A codebase that improves itself turns its own issues into accepted pull
requests, and every accepted (or rejected) pull request makes the next one
cheaper, faster and more reliable. The loop is concrete and checkable: find
the right files, brief an agent, let it change the code until the issue's own
checks pass, have an independent replay confirm the result, merge, deploy, and
feed the verified outcome back into the parts that find files and make
decisions. We run this loop on OpenAgents itself first, on our own production
environment. Then we sell the same loop to other teams for their repositories.

"Recursive" here means one specific thing: **the system's outputs are
training data for the system's own decision layer.** It does not mean an agent
rewriting its own goals or rules. The loop improves *how well it finds,
briefs and decides*. The checks, the approvals and the ground truth stay fixed
and outside the model's reach.

## The loop

```
issue ──► find ──► brief ──► agent + verify ──► independent replay ──► merge ──► deploy
  ▲                                                     │
  │                                                     ▼
  └──────── better finder, ranker, judges ◄── verified outcome traces
```

| Stage | What happens | Where it lives today |
|---|---|---|
| 1. Find | The deterministic finder ranks every file the issue may need, in about a second, from indexes mined out of git history: co-change, past issue→fix pairs, identifiers, interface strings, "X changes, so Y changes" rules. A small learned ranker orders the candidates. | `scripts/filefind`, [bench](../inference/file-finding-bench.md) (#11210, measured on 100 historical issues: 95% of the existing hand-written files a fix changed appear in the 400-file map; 85% on eight newer fixes. This is recall of changed files, not patch correctness) |
| 2. Brief | The issue becomes a briefing: a short plan, the files with excerpts and reasons, the most similar past change, the exact checks, and the repo rules that apply. | #11211 briefing generator |
| 3. Do | A Claude agent with a custom system prompt and a minimal tool set makes the change. Its main custom tool is `verify`, which runs exactly the issue's checks and returns only what's wrong. | `crates/claude_agent_sdk` (in-process tools, #11213), [tool candidates](../inference/briefed-agent-tools.md) |
| 4. Check | An independent replay re-runs the checks in a clean worktree at the same commit and compares digests. Labels come from the diff and the replayed checks, never from the agent's own summary. | `scripts/bench/traces` (#11218, 41 traces replayed and admitted) |
| 5. Land | Review, merge and deploy. Staging runs freely; production waits for an owner approval. | #11169, #11170, the serial integrator role |
| 6. Learn | Verified traces become rows in the decision corpus (time-split train / calibration / locked). The ranker and the Clef decision heads retrain, and calibration maps refit. A new version ships only if it beats the old one on the locked split. | #11215, #11216, #11217, Gym gates |
| 7. Decide cheaply | Every judgment in the loop (route, rank, relevance, "is this done") is a typed decision. Jev (TypeSafe's API) answers first-class; connected Pylons running Clef over NIP-DEC are the fallback and the shadow (one answer in twenty asked again for agreement) until they pass the router gate, then Vertex Gemini. Callers without a Jev key use our own `/v1/systemone`, which asks in the same order. | #11225, [NIP-DEC](../../nips/openagents/NIP-DEC.md), [NIP-PYLON](../../nips/openagents/NIP-PYLON.md) |

You can watch the whole loop in the terminal with `coder issue-run N`
([issue-run](../coder/issue-run.md)): decision cards first, then the agent's
tool calls, then a summary.

## Why it improves, and why it can't fool itself

**Design law (the Tassadar W3 result).** Learning exactness failed. A
*frozen exact core with a learned interface* reached pass@1 1.0. So the
exact parts stay deterministic and are never trained:

- git-history indexes;
- the compiler and tests;
- `verify`;
- the digest replay.

Only the interface is trained:

- the ranker that orders candidate files;
- the decision heads that route and judge;
- the calibration maps.

A smarter interface can make the loop cheaper and better. It cannot weaken
the checks that decide whether work counts.

**Ground truth comes from outcomes.** These are files a merged fix changed,
checks that passed on replay, reviews that accepted a PR, and merges that
didn't revert. Model opinions, including any teacher model, are kept in a
separate field and never treated as labels. This matches the existing
`tenancy::training` refusal of unconfirmed model labels.

**Self-report never counts.** We have already caught one run whose summary
claimed a change that wasn't in its diff. Every label is taken from the actual
diff plus the replayed checks. A trace whose replay disagrees is rejected and
kept as a rejection.

**Promotion is gated.** A retrained ranker or head ships only if it wins on
the locked split by at least two standard errors without worse calibration
(Gym `decision-v1` / `probability-v2`). A small share of live decisions is
shadowed to a second door, so agreement drift shows up without waiting for the
next retrain.

**What stays human.**

- deciding which issues matter;
- approving production deploys and anything outward-facing;
- accepting customer work.

The loop makes those decisions cheaper to make well. It does not make them.

## What "better" means: the metrics

| Metric | Definition | Why |
|---|---|---|
| **Cost per accepted PR** | Total dollars ÷ PRs that compile, pass the issue's checks on replay, and are accepted | The headline. Cheap failures are not savings. |
| Time to accepted PR | Median wall time, issue to merge | The second goal |
| Reliability | Spread across repeated runs of the same issue; share of runs needing escalation | Buyers want "repeatable, not demos" |
| Finder coverage | Share of the fix's files in the briefing map, overall and for the newest fixes | The early limit on everything after it |
| Late files | Files the agent had to open outside the briefing | A direct training signal for the finder |
| Decision cost and agreement | Price and latency per decision; agreement with the shadow door | Proves the decision layer runs on our own supply |
| Improvement rate | Change in cost per accepted PR per retrain cycle, on a fixed issue set | The proof that the loop is recursive |

First measured signal (2 issues, early, `measured`): briefed agents cost about
2–5× less than bare Claude Code, at equal success, but were not yet faster.
The headline claim needs the full #11211 run first: 20+ issues, 3 runs each,
blind judging.

## For us first

We are the first customer, and the evidence for selling it comes from running
it on OpenAgents:

1. **Development moves to our production environment.** Agents work V1 issues
   in Cloud Environments with a shared build cache, push to main, move the
   board and deploy with approvals. A linked Mac takes Mac-only jobs
   (the dogfood gap list in `docs/cloud/dogfood-dev-on-prod.md`, in progress; #11223).
2. **Every issue goes through the loop first.** It escalates to a stronger
   model or a bare agent only on low briefing confidence, repeated `verify`
   failures, or a missing capability. Each escalation is logged as a gap to
   close.
3. **Decisions run on our Pylons.** CoderOS-4080 is the first connected
   Pylon, then others.
4. **Weekly cycle.** Retrain the finder and heads (`retrain.sh`, corpus
   rebuild), re-bench, promote only through the gates, and publish the
   improvement-rate chart internally. Nothing below `measured` reaches a
   public page.

## Then others: the product

### What a customer gets

- **Connect a repository.** We index its git history: co-change, issue→fix
  pairs, identifiers and interface strings. That gives it its own finder in
  minutes, with no training needed to start, because the deterministic core
  works from history alone.
- **Turn issues into checked pull requests.** The customer sends issues; each
  becomes a briefed run with that repository's checks. The customer gets a
  patch or PR, the check results, the cost, and the trace.
- **It improves on their work.** Their accepted and rejected outcomes become
  *their* training rows: a ranker and calibration fitted to their codebase,
  with the improvement-rate chart as the proof of value.
- **Visibility.** An admin view of agent work by task type, cost per accepted
  PR, time to merge, escalations, and what improved this month. This answers
  the "visibility" and "proof against goals" needs in
  [What businesses want](../sales/README.md#what-businesses-want).

### How it fits the sales plan

The [first workflow offer](../sales/README.md#first-workflow-offer-v1) (Coder
pilot v1) is already a single turn of this loop, done by hand:

- one buyer;
- one public repository at a pinned commit;
- frozen acceptance checks before generation;
- independent check runs on the exact candidate;
- explicit acceptance;
- USD 250 proposed, invoiced only on acceptance.

Self-improving codebases is that pilot made repeatable and compounding:

| Sales plan | This spec |
|---|---|
| "Reliability over demos": repeatable processes, checks, evidence | `verify`, independent replay, gated promotion |
| "Spend that goes further" | Cost per accepted PR, measured per customer, falling per cycle |
| "Proof against goals" | The improvement-rate chart and the per-task-type admin view |
| REV-03 evidence (`gym sales-evidence`) | Replayed traces are the same kind of digest-pinned, retained evidence |
| "Help, not homework" | The loop keeps itself current as the codebase and models change |
| Usage pricing, no lock-in | Price per accepted PR or per run, with no seats and no multi-year terms |

**Pricing direction** (proposal, for O1): charge per **accepted** PR, with a
cap per issue, after a pilot under the existing v1 terms. Failed attempts are
our cost, not the customer's. That aligns our incentive with the metric that
improves, and it is the honest version of "we get cheaper for you every
month." It fits usage-based pricing, with no seats or long contracts, as the
sales README requires. Prices are not published until measured delivery cost
supports them, which is the same rule as the pilot fee.

### Data and training rules for customers

These carry over the pilot's data policy and make it stricter where training
starts:

- **Off by default.** Training on a customer's traces requires their separate,
  explicit permission. The pilot v1 policy already excludes training unless
  separately agreed.
- **Their data trains only their model.** A customer's traces train only that
  customer's ranker and heads. Nothing crosses into our shared models or
  another customer's without a separate written opt-in.
- **Code stays where they choose:** on their own computers or Pylons, in
  their Cloud Environment, or in ours under the retention they accepted.
  Secrets are screened before any trace is admitted (`secret-screen`).
- **Every trace is accounted for.** Each admitted trace records its policy
  basis and the customer's opt-out state, and deleting it removes it from
  every future corpus build.

### Where the work runs

- **The customer's own machines.** Coder plus their existing model
  subscriptions, on their computers. They pay their providers; we sell the
  loop.
- **Our Cloud Environments,** metered usage with a Google-first model path.
- **Decisions** run on connected Pylons, the customer's own or the network's,
  over NIP-DEC. Paid Pylon decisions later settle on Bitcoin rails only.

## Milestones and gates

| # | Milestone | Gate (evidence class `measured`) |
|---|---|---|
| S1 | Loop runs on OpenAgents end to end | 20+ distinct V1-class issues, selected before execution, taken through the loop from a Cloud Environment. Every attempt (including failures, cancellations and unknown costs) is in the inventory, every label is replay-verified, and merge/deploy state is recorded. |
| S2 | Briefed beats bare | Cost per accepted PR at least 30% lower than bare Claude Code, at equal or better success and no worse median time, on 20+ issues × 3 runs |
| S3 | It improves itself | Two consecutive learning cycles: version N's new eligible outcomes train N+1, and N+1's train N+2. Each promotion wins on a *fresh* protected confirmation cohort (consulted once) by at least two standard errors at the issue level, with no worse calibration, and with the learned part ablated to show the gain comes from learning. A fixed held-out set is kept as a labelled development trend only. |
| S4 | Jev first-class; Pylons as fallback and shadow until they pass the router gate | Production routing and judges ask Jev (TypeSafe direct) first. Connected Pylons (Clef), our hosted Clef, then Vertex are the fallbacks, and a shadow share measures the Pylons' agreement with Jev. Pylons move ahead of Jev only when they pass the router gate (latency within the first budget, prepared answers kept). Results name the door that answered. |
| S5 | Second codebase | The loop works on a repository that isn't ours (a public OSS repo), from history alone, with its own corpus and gates |
| S6 | First customer pilot | Pilot v1 delivered through the loop, accepted, with REV-03 evidence and the customer's own improvement chart |
| S7 | Product | Self-serve repository connect, per-accepted-PR pricing (after O1), admin view, and training opt-in controls |

## Amendments after the audit (2026-10-10)

The [self-improving codebases audit](../audits/2026-10-10-self-improving-codebases-audit/README.md)
found that the success signal and the learning intake had to be repaired
before any gate can be trusted. The gates above were tightened to match it:

- **S1** counts distinct issues chosen before execution, with the full attempt
  inventory. Repeats don't raise the count, and unknown costs stay unknown.
- **S3** uses fresh protected confirmation per promotion instead of re-reading
  one fixed held-out set, which Gym's one-read rule forbids. It requires an
  ablation.
- **S4** changed on 2026-10-10 (owner decision, #11225): the Pylon judge was
  too slow (5.7–6 s) and too unsure (confidence 0.27–0.35, prepared answers
  missed) for production routing, so Jev is first-class again and the
  Pylons are fallback and shadow until they pass the router gate.

The fixes come first: the P0 verify fix (#11229), issue-run gating, worktrees
and costs (#11230), learning integrity (#11231) and the data boundary
(#11232). No gate is claimed until they land.

## Risks and how we handle them

- **Overclaiming "recursive self-improvement."** Use the narrow meaning above
  in all copy. Show the improvement-rate chart with its evidence class, never
  adjectives. The audit already caught overclaims in old Tassadar code; we
  don't repeat that.
- **Reward hacking.** The agent can't edit `verify`, the checks or the replay
  harness. Changes to test files are flagged and judged separately. A run that
  weakens a check is rejected.
- **Data leakage between customers.** Separate corpora, separate trained
  artifacts, and no shared training without opt-in, as above.
- **Distribution shift.** The newest fixes are always the hardest; the finder
  measures 85% on them against 95% overall. Keep indexes fresh on every query,
  retrain weekly, and let `verify` catch the rest.
- **Cost of failure.** Escalation is capped and logged. Failed attempts are
  priced into the per-accepted-PR rate, not passed through.
- **Human judgment.** Production deploys, outward-facing actions and customer
  acceptance always need a person.

## Related

- [Training system audit and roadmap](../audits/2026-10-10-training-system-audit/README.md)
- [File-finding bench](../inference/file-finding-bench.md) and the
  [briefed-agent tools](../inference/briefed-agent-tools.md)
- [Briefed agent vs bare Claude Code](../inference/briefed-agent-ab.md)
- [Coder issue-run](../coder/issue-run.md) and [trace replay](../coder/traces.md)
- [Clef native](../inference/clef-native.md),
  [NIP-DEC](../../nips/openagents/NIP-DEC.md),
  [NIP-PYLON](../../nips/openagents/NIP-PYLON.md)
- [Sales and revenue](../sales/README.md),
  [revenue roadmap](../sales/revenue-roadmap.md),
  [pilot evidence](../sales/evidence.md)
- [What to bring back from Tassadar](../roadmap/2026-09-28-tassadar-revival.md)

# Exo, Verifiable Software, and the Gym — Where the Integration Goes — 2026-07-26

Agent-facing strategy synthesis. The companion state document is
[the integration analysis](./2026-07-26-exo-openagents-integration-analysis.md),
which records what exists. This document speculates deliberately: it combines
Exo's recursive-self-improvement (RSI) ideas with the verifiable-software
thesis (episode 259, `docs/transcripts/259.md`, and
`docs/fable/2026-07-19-verifiable-software.md`), argues why Omega is the
logical front door through which most users should meet Exo, and maps the
retired OpenAgents Gym onto an Exo workbench inside the product. Every
forward claim here is `[SPECULATION]` unless it cites shipped evidence. This
document flips no promise state, revives no retired program, changes no
runtime authority, and grants nothing. The Gym retirement stamp, the teardown
refuse lists, `AUTHORITY.md`, and the Sol roadmap keep their precedence.

## 1. Two bets collide

Exo's bet: an agent should be able to rewrite itself — its prompts, its
tools, its harness policy — while a trusted substrate protects one thing, the
append-only event log that is its canonical history. The project calls the
hand-engineered harness the next thing to fall to the Bitter Lesson.

Our bet, stated in episode 259: the constraint on the machine-work economy is
not generation, which collapses toward free. It is verification. The metric
is accepted outcomes per kilowatt-hour, the gap is the Measurability Gap from
*Some Simple Economics of AGI*, and the machinery that closes it is
verifiable software — work scoped in advance as falsifiable intent, evidence
observed rather than narrated, producer separated from verifier, and receipts
that let strangers check and pay for acceptance.

These two bets are not merely compatible. Each is the missing half of the
other, and the joint is sharp enough to state as a law:

**Recursive self-improvement is the limit case of the measurability gap.**
An agent that rewrites itself generates change faster than any human can
review. If its improvements are graded by its own narration, the compounding
loop compounds counterfeit utility — plausible self-modifications that pass
every proxy the agent can see while drifting from every intent nobody
checked. The verifiable-software failure taxonomy (false greens, convincing
summaries, self-grading, rung collapse) is not a risk RSI might encounter.
It is the default behavior of an ungated RSI loop. Exo's flagship agent
today runs unrestricted networked shell with a `guardian_action` tool that
rebuilds and restarts itself, no permission gate, and no external grader —
by design, because Exo assumes you want the agent to modify itself. The
assumption is honest and the substrate is real. What is missing is the
refinery: the machinery that turns self-modification from asserted progress
into accepted outcomes.

Conversely, verifiable software finds in Exo the substrate property it has
always wanted and never owned: **a complete, append-only, forkable,
replayable record of everything the agent did.** Exo defines the entire
agent state as a version of the event log — rewind and fork recreate the
whole model, and sandbox snapshots extend that to filesystem state. In
verification terms that is the primitive that makes independent checking
cheap: a verifier does not have to trust a summary of what happened, it can
fork the history at the claim and re-run the claim. Evidence by replay
instead of evidence by narration. The event log is half of verifiable
software, built by a team that arrived at it from the opposite direction.

## 2. What Exo has and lacks, in verifiable-software terms

Measured against the five structural commitments of the verifiable-software
essay:

| Commitment | Exo today |
| --- | --- |
| Typed, content-addressed intent (ProductSpec/AssuranceSpec) | Absent. Intent lives in prompts and `SELF.md`, unversioned as intent |
| Evidence observed, not claimed | Half-present. The event log records what happened durably, but the grader of "better" is the agent itself |
| Claims as receipts, status evidence-gated | Absent. No receipts, no acceptance rungs, no attribution |
| Authority separated, no self-amplification | Absent by design. Producer, verifier, and promoter are one process |
| Failure classes removed by construction | Present in one place only: the log is append-only, so history cannot be falsified |

The asymmetry is the opportunity. Exo built the record and skipped the
proof. OpenAgents built the proof machinery — typed intent, host-observed
evidence, receipts, separated authority, acceptance oracles, behavior
contracts, the challenge ledger — and has no forkable execution substrate
under its agents. Upstream's own most-engaged open issue (#154, hash all
canonical state, filed by an external founder) shows the project reaching
toward integrity discipline on its own. The two estates compose without
either abandoning its thesis.

## 3. Why Omega is the front door — for most users, most of the time

The direct answer to "how should a person use Exo": through Omega, because
the interface to a self-rewriting agent must be a verification instrument,
and Omega is the only one that exists.

Run the inventory of what a user actually needs before trusting an RSI agent
with real work, and check it against what is already shipped:

1. **Identity of the thing running.** Exo has no releases and builds from
   source, so two hosts at the same commit run different bytes. Omega pins
   repo, commit, tree, and a measured binary digest, and refuses drift
   (`EXO_PIN`, `OMEGA-DELTA-0042`).
2. **Knowledge of what the agent can do right now.** Omega observes the
   agent's live capability before every turn — agent-authored tools, tool
   modules, read-write source mounts — parsed fail-closed from Exo's own
   introspection commands.
3. **A consent boundary around self-modification.** Exo has none. Omega's
   Tier C grant is a one-turn, one-draft, one-configuration,
   one-connection-generation authorization minted by a human dialog and
   consumed exactly once. This is consumer-grade RSI: the difference
   between an agent that may rewrite itself and an agent that may rewrite
   itself *this once, as inspected, with a receipt*.
4. **Attribution.** Every Exo thread carries the executor disclosure line.
   The user always knows which harness, executor, and model produced a
   turn — the anti-silent-substitution law, applied to a harness whose
   whole point is that it changes.
5. **Receipts.** Every self-modification outcome — refused, sent,
   completed, cancelled, drifted — lands in a durable receipt file with
   Exo's own session, turn, and event references, which means every receipt
   points into a replayable history.
6. **Containment of Exo's exposure defaults.** No-auth HTTP, LAN-trust
   posture, loopback-only endpoint types, no off-machine proxying —
   enforced as types and delta tests, invisible to the user, exactly as it
   should be.
7. **An economy to plug into.** The keypair-and-wallet direction
   (verifiable-software Addendum II) gives the editor the rails to sign,
   price, buy, and sell verification. An Exo agent inside Omega inherits
   that position the moment those switches flip.
8. **A public commons.** The `/agentchat` NIP-29 channel and the PR #162
   adapter give an Exo agent a portable signed presence among other agents,
   with the relay and group as swappable configuration.

Exo brings the self-improving agent and the replayable substrate. Omega
brings everything a person needs to point that agent at work that matters.
Upstream's terminal REPL is the right interface for Exo's authors. Omega is
the right interface for everyone else — not because we wrapped it first, but
because we spent a year building the exact trust machinery that
self-modification requires and nobody else has: proposals over mutations,
host-observed evidence, generation fencing, typed authority that cannot
self-amplify, and a culture that writes falsifiers before features.
`[SPECULATION]` stated as a product thesis: "the safest place to run an
agent that rewrites itself" is a category with exactly one credible entrant,
and it is the same IDE the verifiable-software essay already calls the
engine.

## 4. The fork trajectory — plan for divergence, not for merges

The realistic expectation is that `OpenAgentsInc/exo` gets far ahead of
upstream, for structural reasons: upstream is 3 core maintainers in launch
polish with thin review bandwidth, no releases, and an explicit
no-backwards-compatibility house rule, while our product needs compound
weekly. The state document treats the upstream PRs as the story. This
document inverts that: **the fork is the story, and upstream merges are a
bonus.** The healthy posture is a maintained divergence with periodic,
deliberate upstream syncs — the same posture Omega itself holds toward Zed.

What the fork becomes, in order of concreteness:

- **Shipped now:** the ACP streaming transport with durable cooperative
  cancellation — the only wire through which any external host can drive
  Exo turns today.
- **Near, mechanical:** the Nostr chat adapter (today on a branch, PR #162
  upstream), giving the agent a signed public presence.
- **Next, and thesis-bearing `[SPECULATION]`:** the *verifiable Exo
  distribution*. Candidate fork capabilities, each an independent bounded
  packet: hash-chain the event log (upstream #154's direction, done
  properly: each event carrying a digest link so history is tamper-evident,
  which upgrades "append-only by convention" to "append-only by
  verification"), sign checkpoints with a Nostr key so an agent's history
  becomes a portable attestation, add oracle hooks so external graders can
  bind verdicts to exact event ranges, meter turns so cost and energy
  attach to outcomes, and keep the streaming transport current. None of
  this changes Exo's semantics. All of it makes Exo's record legible to
  strangers — which is precisely what upstream's substrate thesis needs and
  precisely what their roadmap (self-maintenance, portable execution,
  multi-agent lineage) will eventually require anyway.

The upstream relationship stays what the state document says: goodwill
contributions, owner-gated, never a dependency. If upstream takes the
transport and the hashing, everyone wins. If upstream stalls, the fork is
already the product substrate and loses nothing.

## 5. The Gym returns as the Exo workbench

### 5.1 What the Gym was

OpenAgents Gym (spec landed 2026-06-23, retired 2026-07-08, code dormant but
intact — 50 Worker files, public projections still registered in
`apps/openagents.com/INVARIANTS.md`) was the OpenAI-Gym analog for Khala:
**environments** (task sets paired with verifiers and acceptance contracts),
**policies** (configurations of coordinator, providers, tools, sampling),
and a **reward** defined with the sentence that matters most in this whole
synthesis:

> the executed verification verdict + cost-per-accepted-outcome — never a
> self-grade, never a benchmark grader the policy can reach.

Around it grew the benchmark ladder (beat the free incumbent first, then
free peers, then paid frontier, with decision-grade honesty gates), the
flywheel ("the gym trains Khala and uses Khala" — episode 243), and the
admission discipline that survives today as house style: optimizer
candidates never auto-promote, `decisionGrade` stays false until real
held-out evidence exists, and promotion is an owner-approved proposal. The
retirement stamp is explicit: retired for now, revival requires an explicit
owner decision, earliest reconsideration after cashflow-positive. This
document honors that stamp — everything below is design speculation for that
future decision, not a reopening.

### 5.2 Why Exo is a gym-shaped object

The Gym's hardest engineering problem was the one it never fully solved:
**reset.** A real coding-agent environment is stateful, and comparing two
policies honestly requires putting the world back. The Gym approximated
reset with fixtures and fresh checkouts. Exo solves it natively: fork the
event log at any event, and the entire agent — memory, tools, conversation,
with sandbox snapshots covering the filesystem — resumes from that exact
point. Fork is reset. Real history becomes the environment corpus, not
synthetic fixtures. Two candidate self-modifications fork from the same
event, run the same held-out tasks, and their divergence is measured from a
genuinely identical start.

And in the other direction: Exo's RSI loop is exactly the "policy
improvement" the Gym existed to discipline. Exo's agent already proposes
changes to itself — the Gym's admission pipeline (candidate, evidence-only
ingest, gated proposal, owner approval) is the missing back half of that
loop. The TMAX lesson the Gym encoded — never let the policy reach its own
grader — is the precise correction to Exo's self-grading default.

### 5.3 The workbench, sketched `[SPECULATION]`

An **Exo workbench** inside Omega — the natural growth path of the omega#95
Exo workspace, and the revival shape for the Gym if the owner ever flips
that switch:

1. **Episode = fork.** A workbench run forks the agent's event log at a
   chosen point and materializes N sandboxed variants. One variant carries
   the candidate self-modification (a new tool, a prompt change, a policy
   edit). The others carry the incumbent and any competitors.
2. **Environment = held-out tasks plus oracles.** Tasks come from the same
   sources the Gym used — real traffic, behavior contracts, benchmark
   rungs — and the graders are external by construction: behavior-contract
   oracles, AssuranceSpec obligations, compile-and-test seams, human
   review where the contract demands it. The candidate never sees or
   selects its grader.
3. **Reward = the Gym's reward, plus the watt.** Executed verification
   verdict, cost per accepted outcome, and — closing the episode-259
   loop — accepted outcomes per kilowatt-hour, with turn metering from the
   lane receipts. A self-modification is an improvement if and only if it
   raises verified acceptance per unit of energy and money. The agent's
   opinion of its own improvement is not an input.
4. **Promotion = the existing gates, unchanged.** A winning variant emits a
   gated proposal. Tier C consent (or its multi-turn successor, which needs
   its own authority packet) admits the self-modification into the live
   agent, with a receipt binding the exact event range, oracle verdicts,
   and grant. Losing variants are discarded — but their forked histories
   are retained evidence, replayable by any later skeptic.
5. **The pane is the product.** The workbench UI renders what the Gym pane
   rendered for Khala Code — candidate, evidence, admission state — plus
   what Exo makes newly possible: a time-travel scrubber over the agent's
   own history, diffs between forked futures, and the receipt chain from
   "the agent proposed to change itself" to "the change was accepted on
   evidence." For a user, this is the first legible answer to the question
   every RSI system dodges: *what exactly did your agent do to itself while
   you were away, and how do you know it got better?*

The flywheel sentence from episode 243 transfers whole: the workbench trains
the agent and uses the agent. And the dogfood loop is immediate — Omega
development itself is the first environment, because the agent that improves
at building OpenAgents is improving at the work we verify most rigorously.

### 5.4 What this gives the verifiable-software program

The essay's Addendum I names verifier independence as the structural risk —
AI checking AI shares blind spots. The workbench is where the verification
economy (Addendum II) meets the RSI loop: a candidate self-modification is a
claim, its forked history plus oracle verdicts are a signed evidence bundle,
and paid strangers can be invited to falsify it — different models,
different toolchains, different priors, competing to break the improvement
for sats before promotion. Bonded verdicts price the top tier. Adjudicated
failures become sellable ground truth. The unit of the whole economy — the
accepted outcome with a receipt trail — now has a new species: **the
accepted self-improvement.** Generation was already collapsing toward free.
The workbench is what keeps the thing that compounds — the agent itself —
priced in the only currency that matters, verified acceptance.

## 6. The speculation ladder, with honest rungs

Near (paths exist, each item a bounded packet, several owner-gated):

- omega#95 ships the Exo workspace with the inspector — the workbench's
  chassis.
- The fork adds event-log hash-chaining and signed checkpoints — the
  attestation substrate, aligned with upstream #154.
- An Exo agent takes standing residence in `/agentchat` with its own key —
  the public identity that later signs improvement claims.

Mid (requires the Gym revival decision and new authority packets):

- Workbench episodes over forked histories with behavior-contract oracles
  and cost-per-accepted-outcome scoring.
- Turn-level metering wired to the accepted-outcomes-per-kilowatt-hour
  metric, making improvement energy-priced.
- A multi-turn Tier C successor: bounded self-modification campaigns under
  one inspected grant with per-step receipts.

Far (thesis-level, stated so it can be falsified later):

- A market where verified self-improvements travel: an agent's signed,
  hash-chained history is its career, improvements ship as evidence
  bundles, strangers' verifiers grade them for sats, and bonded acceptance
  makes "this agent got better" an insurable claim rather than a vibe.
- The convergence sentence: Exo asked how an agent rewrites itself.
  OpenAgents asked how anyone could trust that. The product that answers
  both at once — a self-improving agent whose every improvement carries its
  own proof, running in the IDE that made verification ambient — is the
  refinery of episode 259 with a feedback loop inside it. Electrons in,
  accepted outcomes out, and the machine that does the converting getting
  measurably, verifiably better at it every week.

## 7. Boundaries

The Gym remains retired until an explicit owner decision revives it — this
document designs for that decision and does not make it. Tier C stays
one-turn and default-off until a successor authority packet is admitted on
its own gates. The exoharness teardown refuse list, the #9258 generic-client
boundary, FastFollow discipline over upstream material, and the settlement
inertness recorded in the verifiable-software essay (identity live, market
grammar implemented, money deliberately off pending owner decisions) all
hold unchanged. Every economic mechanism named here rides on switches this
document does not flip. Speculation is labeled because the discipline this
document argues for begins with how we write about it.

# OpenAgents Analysis: Some Simple Economics of AGI

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


## Reading

This note synthesizes:

- `2602.20946v2.pdf` and
  `some-simple-economics-of-agi-paper-summary.md`
- `../tassadar/README.md`
- `../tassadar/RESEARCH_PLAN.md`
- `../tassadar/work-that-proves-itself.md`
- `../tassadar/2026-06-11-coding-agent-primitive-wedge.md`
- `../tassadar/2026-06-11-autopilot-agentic-labor-market.md`
- `../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`

## Core Fit

The AGI economics paper gives OpenAgents a direct macroeconomic frame for
what the product already appears to be building: an economy where machine
execution becomes abundant and the scarce input is trusted verification.
Its main claim is not simply that AGI automates labor. It is that AGI lowers
the cost of acting faster than society can lower the cost of checking,
validating, insuring, and assigning responsibility for those actions.

That is almost exactly the OpenAgents thesis expressed in Tassadar terms:
the accepted outcome, not the token, is the unit of trade; the dominant cost
line is checking; the durable business is reducing the cost of trust faster
than the cost of work.

The paper's "Measurability Gap" maps to OpenAgents' operating problem:
agents can produce more claims, patches, analyses, and state changes than
humans can inspect. The repo's current promise registry, Forum reports,
strict bug intake, work requests, assignment closeouts, challenge receipts,
settlement states, and public projection discipline are not side features.
They are the verification infrastructure the paper says becomes the binding
production layer in an AGI economy.

## What Tassadar Adds To The Paper

The paper says verification bandwidth becomes scarce, then lists broad
responses: observability, provenance, liability, synthetic practice, and
ground-truth infrastructure. Tassadar adds a sharper technical category:
**born-verified work**.

For normal agent work, OpenAgents still pays a verification tax: tests,
reviewers, validators, statistical checks, challenge protocols, or human
judgment. Tassadar defines a lower bound for that tax. In the exact-execution
class, the trace is append-only, deterministic, digest-pinned, and replayable.
Verification is not an after-the-fact audit. It is the same artifact as the
work.

That extends the paper's framework in three useful ways:

1. The Measurability Gap is not uniform across all work. Some work can be
   redesigned so verification is native to the substrate.
2. Verification can become a market good, not only a governance function.
   A compiled module, validation receipt, or replay verdict can hold a
   price because it reduces downstream risk.
3. The scarce human verifier can be reserved for work that cannot yet be
   made born-verifiable. Exact lanes become the control group, calibration
   lane, and ground-truth factory for fuzzier lanes.

OpenAgents can contribute a concrete empirical counterweight to the paper's
more abstract model: measure the actual price and conversion effects when a
task ships with replayable proof, deterministic tests, model review, human
review, or no verification. The platform can turn "verification bandwidth" from
an economic metaphor into a live pricing table.

## Product Implications

### 1. Make verification the product surface

The paper says trusted outcomes become the moat. OpenAgents should make the
verification ladder first-class in every work surface:

- Each work request declares its required verification class.
- Each quote prices execution and verification separately.
- Each closeout exposes the verification evidence, not only the output.
- Each provider profile reports accepted outcomes by verification tier, not
  just completed work.
- Each public claim links to dereferenceable evidence or remains yellow/red.

This should be visible in Autopilot, Pylon, Forum work requests, labor jobs,
promise reports, and API responses. A user should not need to infer whether a
job was merely generated, test-passed, replay-verified, human-reviewed, or
settled.

### 2. Treat the coding agent as the first measurable AGI-era service

The coding-agent wedge is a practical instantiation of measurability-biased
technical change. Coding work is valuable, already demanded by the workspace,
and often has a verification command. It is not perfectly measurable, but it is
more measurable than many expert services.

OpenAgents should use its own backlog as the first demand stream:

- Issue to work request adapter.
- Budget, capability refs, and verification command attached up front.
- Escrow reserved on posting.
- Agent/provider quote and output-only delivery.
- Independent validator re-runs the verification command.
- Settlement and issue/forum receipts become the public proof trail.

This directly attacks the paper's "human verification bandwidth" constraint:
the human maintainer stops being the first-line executor and becomes the
intent-setter, reviewer of last resort, and liability owner.

### 3. Build the missing junior loop as a paid apprenticeship ladder

The paper warns that AI automation can destroy entry-level work and thereby
the pipeline of future experts. OpenAgents has a product answer that is not in
the paper: **verification-first apprenticeship**.

New agents and new human contributors should start on rung-0 verification
bounties:

- Audit green promises.
- Re-run verification commands.
- Check public receipts.
- Falsify product claims.
- Confirm settlement visibility.
- Write minimal reproduction reports.

This preserves an apprenticeship path without pretending the old junior tasks
will survive unchanged. The new junior loop is not "do low-value execution."
It is "learn the system by checking claims, then earn authority through
receipt history." Orrery's first-night work in the Tassadar labor-market note
is the prototype.

### 4. Make liability legible before liability is legal

The paper's "liability-as-a-service" thesis is broader than OpenAgents can
ship immediately, but the precursor is available now: every work product should
name who or what is underwriting which claim.

OpenAgents should add explicit liability-like fields to work and capability
records:

- `claim_scope`: what the provider says is true.
- `evidence_class`: replay, deterministic test, probabilistic check, human
  review, operator attestation, or none.
- `risk_owner`: requester, provider, validator, maintainer, or platform.
- `remedy`: refund, re-run, challenge bounty, quarantine, revocation, or
  manual escalation.
- `expiry` and `staleness`: when the evidence stops being current.

This does not create a legal insurance product by itself. It creates the data
structure from which one could be priced.

### 5. Promote ground truth to an owned asset class

The paper argues rents migrate to verification-grade ground truth. Tassadar's
verified trace factory is a precise version of that: traces become training
records only after replay, profile hash, compiler hash, executor hash, and
validator receipt line up.

OpenAgents should generalize that policy:

- No training from unverified artifacts.
- Trace schemas should carry profile, program, compiler, executor, split, and
  validator identities.
- Public counters rebuild on validation transitions, not registration events.
- Dataset releases should be content-addressed and claim-scoped.
- Disputed or stale traces should remain in the ledger as negative examples,
  not disappear.

The business value is not just "more data." It is data with adjudicated truth.

## What OpenAgents Should Add

### Work request fields

Add or standardize fields across labor, Autopilot, Pylon, and Forum work:

- `verification_class`
- `verification_command`
- `acceptance_predicate`
- `evidence_refs`
- `risk_owner`
- `settlement_visibility_ref`
- `projection_rebuild_triggers`

The user-facing framing should be simple: "What proves this work is done?"

### A verification market dashboard

Add an operator and public-safe dashboard that reports:

- work volume by verification tier;
- cost per accepted outcome by tier;
- challenge rate and successful challenge rate;
- first-divergence categories for failed work;
- settlement latency and settlement visibility;
- stale or unresolvable evidence refs;
- provider acceptance and rejection rates by capability envelope.

This is the empirical instrument the paper is missing.

### Rung-0 verification inventory

Keep a standing queue of low-authority paid verification jobs:

- promise audits;
- receipt dereference checks;
- projection freshness checks;
- exact-replay spot checks;
- deterministic verification re-runs;
- summary/source consistency checks for research docs like this folder.

The next new agent asking for work should receive a job, not a tip jar.

### Outcome pricing experiments

Run controlled pricing experiments:

- same task, no verification vs deterministic command vs exact replay;
- human review added only above a risk threshold;
- bounty paid for successful falsification;
- provider reputation weighted by verified receipt history;
- accepted-outcome subscription for teams that want coding work with receipts.

The goal is to quantify whether buyers pay for reduced verification risk. If
they do not, the product should narrow quickly and honestly.

## OpenAgents' Contribution To The Argument

The paper is strongest at the level of economic diagnosis. OpenAgents can add
the missing mechanism design:

- a public receipt ledger for agent work;
- a typed verification ladder;
- paid adversarial verification;
- settlement tied to accepted outcomes rather than generated output;
- issue/backlog demand converted into work requests;
- provider capability envelopes with typed refusals;
- born-verified exact computation as the bottom rung;
- public promises that self-mark red/yellow/green based on evidence.

The paper treats "AI verifying AI" mainly as a danger because of correlated
blind spots. OpenAgents can refine that: AI verification is unsafe when it is
untyped, self-referential, and unaudited; it becomes useful when it is bounded
by deterministic replay, independent validators, challenge bounties, and
public receipts. The right answer is not "humans verify everything" or "AI
verifies itself." The right answer is a layered verification market where
humans spend attention only where cheaper proof classes fail.

## Near-Term Agenda

1. Ship the first real negotiated labor job from an actual backlog issue.
2. Add the issue to work-request adapter.
3. Publish the rung-0 verification bounty inventory.
4. Expose verification tier and evidence refs in every work closeout.
5. Add settlement visibility as an acceptance criterion for labor payouts.
6. Build the verification market dashboard.
7. Keep Tassadar claims narrow: exact replay for bounded profiles, not
   broad claims about trained model reliability.

## Bottom Line

"Some Simple Economics of AGI" says AGI makes execution cheap and trust scarce.
OpenAgents should be the market where trust is priced, purchased, challenged,
settled, and improved. Tassadar is the technical limit case; Autopilot and the
labor market are the commercial wedge; the promises registry is the public
claim discipline. The company should incorporate the paper by making
verification, provenance, and liability-shaped evidence the central product
interface rather than a back-office audit layer.

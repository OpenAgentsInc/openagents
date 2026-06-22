# The Coordinator as Verified Work

*Analysis — 2026-06-22. The one structural advantage we have over Sakana, and
what it lets us build. Companion to
[`adapting-sakana-coordination.md`](adapting-sakana-coordination.md).*

## The asymmetry

Both Sakana papers optimize a coordinator against a **terminal reward**:
`R(τ) ∈ {0,1}`, "did the final answer match the benchmark solution?" That reward
comes from a grader — a held-out test set, a unit-test harness, an LLM judge.
It's good enough to train against, but it's *external scaffolding*: it lives in
the lab, not in the product, and it can be gamed (formatting hacks, leaked test
cases, judge sycophancy — the Conductor paper itself documents GPT-5 format
failures and MoA being misled by wrong candidate solutions).

We have something they didn't: the reward is **cryptographic and lives in the
product**. Tassadar's exact-trace-replay produces a verdict by having an
independent device re-execute and compare trace digests byte-for-byte
(`docs/tassadar/work-that-proves-itself.md`,
`2026-06-15-executor-trace-contributor-completion-design.md`). `Verified` or
`Rejected`. No grader, no rubric, no judge to fool. For open-ended work the
analog is a pinned **verification command** that must pass
(`2026-06-11-autopilot-agentic-labor-market.md`).

This changes three things.

## 1. The reward is the same object as the payout

In Sakana, the reward signal and the product are separate — the benchmark grade
never leaves the training loop. In our system the verdict that trains the
coordinator is *the same verdict that releases Bitcoin to the worker*
(`2026-06-19-autostream-settlement-visibility-capture.md`: verdict → auto-stream
→ Spark settlement → public receipt). So:

- The coordinator is trained on **production traffic**, not a synthetic eval
  set. Every real job that settles is one labeled trajectory.
- The reward is **denominated in sats**, so cost-of-coordination is directly
  comparable to value-of-work. The objective isn't "maximize pass rate" — it's
  "maximize **verified-work-per-sat-spent**," which is the actual business
  objective. The fleet metering (`vertex-fleet/`, hosted-Gemini metered lane)
  already attributes per-call cost, so the denominator is free.
- There's **no train/serve gap** to manage: the coordinator sees exactly the
  workers, costs, and verdicts it will see in production.

## 2. The verifier role is solved — stop prompting an LLM to judge

The single most fragile part of LLM-coordination systems is the verifier.
TRINITY assigns a *Verifier* role to one of its LLMs and trusts its
`ACCEPT/REVISE` judgment; the Conductor leans on weak models as format-checkers
and on debate rounds. Both are LLMs judging LLMs.

We should **not** port that. Our Verifier role binds to the **replay validator**,
not to a prompted model. "Verifier ACCEPT halts" becomes "`Verified` verdict
halts." This removes the highest-variance component of the Sakana loop and
replaces it with a deterministic, independently-recomputed digest comparison.
The coordinator then only has to learn the *Thinker* and *Worker* assignments —
the planning and execution routing — while correctness adjudication stays
mechanical and unbribable.

Practically: the coordinator's action space is "(which worker, what subtask)";
the *halt/accept* decision is taken out of the policy and handed to the
replay/verification-command gate. This shrinks what must be learned and makes
the binary reward genuinely binary (no judge noise), which is exactly the
clean-Bernoulli regime sep-CMA-ES is built for (TRINITY Appendix A).

## 3. The coordinator can itself become a paid work definition

This is the move that's only available to us. Our agentic labor market already
defines work as a posted, verifiable job with an acceptance criterion
(NIP-LBR kind-5934; the rung ladder in
`2026-06-11-autopilot-agentic-labor-market.md`; the kernel-optimization parity
protocol in
`2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`,
where acceptance = throughput-gain **and** output-parity-by-replay).

"Coordinate a pool of models to solve task X for ≤ B sats, accepted iff the
result Verifies" is *itself a well-formed work definition* in that market. So a
learned coordinator isn't just an internal optimizer — it's a **market
participant** whose own reward is the spread between the bounty it's paid and the
worker-spend it incurs, settled only when the output Verifies. Two consequences:

- **The coordinator can be trained by competition, not just by gradient/ES.**
  Multiple coordinator policies bid on / take the same jobs; the ones that
  produce Verified results cheaply accrue more settled sats; ranking falls out
  of the ledger. This is a market-level analog of the per-question-best oracle
  Sakana uses as an upper bound — except it's live and self-funding.
- **It composes with the rung ladder.** A Conductor that learns the
  planner→coder→checker topology (Seam B) is exactly a Rung-2 "writeback-class"
  participant; a TRINITY router that picks the cheapest worker that still
  Verifies is a Rung-1 "bounded coding with validator re-execution" participant.
  We don't need a separate productization step — the coordinator graduates the
  same ladder a human/agent contributor does.

## What to build, given Psionic's gaps

The psionic-docs sweep confirms the substrate is **SFT/RL only — no CMA-ES, no
SVF, no hidden-state-feature routing** (`AGENTIC_SFT_RL_REFERENCE_PROGRAM.md`,
`TRAIN_SYSTEM.md`, `COMPILED_AGENT_ROUTE_MODEL.md` is static naive-Bayes). The
verification and governance machinery, by contrast, is **already strong**:
learning receipts, activation fingerprints, conformance contract, promoted-vs-
candidate shadow governance (`COMPILED_AGENT_LEARNING_RECEIPTS.md`,
`ACTIVATION_FINGERPRINT_PROOFS.md`, `CONFORMANCE_AND_EVIDENCE_CONTRACT.md`,
`COMPILED_AGENT_SHADOW_GOVERNANCE.md`, `COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT.md`).

So the build is lopsided in our favor: **we mostly need to add the optimizer and
the action schema; the reward, verification, payout, and governance already
exist.** Concretely the only genuinely new components are:

1. a **sep-CMA-ES optimizer** (small) and optional **SVF adapter** in Psionic's
   training system, alongside the existing SFT/RL;
2. a **coordinator action schema** — `(worker, role)` logits for TRINITY-style,
   or `(model_id, subtasks, access_list)` for Conductor-style — and its parser;
3. a **reward adapter** that reads the existing verdict/settlement stream and
   returns `verified-work-per-sat` per trajectory.

Everything else — the worker pool, dispatch, replay verifier, settlement,
shadow-deploy governance — is reuse.

## The thesis in one line

Sakana proved a tiny coordinator can beat every frontier model it orchestrates
when trained against a terminal reward. We hold the strongest possible terminal
reward — a cryptographically-verified, sat-denominated, production-grounded one —
and a worker pool, dispatch loop, and governance layer already wired to it. The
coordinator is the missing middle, and once it exists it isn't just an
optimizer: it's a paid participant in our own labor market.

# Outcomes: selling accepted work, not model access (2026-10-15)

- [`2026-10-15-outcome-accountable-execution-assessment.md`](2026-10-15-outcome-accountable-execution-assessment.md)
  is an outside assessment. Another agent wrote it from Soren Larson's
  "Router Power" thesis (pooled outcome data lets a neutral platform out-learn
  each company), Satya Nadella's counter-argument (every firm owns its own
  learning loop) and our public repo.
- That agent didn't have our full context. This file is our own read: what it
  gets right, what it gets wrong or misses about OpenAgents, and what we
  should do.

## The short version

**We agree with its main call:** sell **accepted work**, measure **cost per
accepted change**, keep each customer's learning theirs, and treat
cross-customer network effects as a hypothesis to test, not a moat to claim.

**What we add:**
1. Most of the machinery already exists, and much of it shipped in Coder 1.0.0.
2. Our Bitcoin rails make paying per accepted change unusually simple.
3. The network-effect test can run on public repositories before any
   customer data exists.

The first steps need **no new Coder binary**. They are server-side and
evidence work that runs with Coder 1.0.0 as released.

## What it gets right

- **Outcome service vs. model aggregator.** Breadth of models is not our
  edge. The post says Stripe is buying OpenRouter and Thesean sells cheaper
  inference; these are the post's sources, which we haven't checked. Either
  way, our edge is the checked change.
- **Separating observation, verification, causal value and billing.** A
  merge is not an outcome. Our loop already labels results from the diff and
  an independent replay, never from the agent's own summary
  ([self-improving codebases](../../product/self-improving-codebases.md)).
- **"Use less AI" beats "use a cheaper model".** The biggest saving is
  turning a solved problem into a tested procedure.
- **The quality caveat on our study.** The brief + verify study
  ([`docs/inference/briefed-agent-ab.md`](../../inference/briefed-agent-ab.md))
  found $1.84 vs $4.00 per accepted fix. Both arms used the same big model,
  and both accepted 19 of 63 runs, so "equal or better quality" is **not
  established**. That's how `docs/launch/1.0/claims.md` words it, and how
  sales must word it.
- **The A/B/C/D experiment design** (§9): shared infrastructure, then
  private learning, then transferred learning, each tested separately.
- **Separate permissions:** execute, retain, improve this customer, improve
  everyone. This matches our training policy: off by default, a customer's
  traces train only their models, and shared training needs a separate
  written opt-in.

## What it gets wrong, or misses

1. **Jev is no longer first.** Since 2026-10-12, decisions go through our
   Decisions API **Clef first**, with Jev as the fallback. The post's §5 and
   §6.B read older docs.
2. **"Use less AI" already has a home: the task index.**
   - It turns verified fixes into recipes and finds files from indexes mined
     from git, with a model only when it's unsure.
   - It has chronological evaluations and a promotion gate, and learns from
     verified misses (#11716–#11719, all closed).
   - That *is* the post's "first occurrence expensive, later occurrences
     cheap" loop.
   - The indexes are mined from the customer's own git history on their own
     computer. **Private learning is local by construction**, which is the
     strongest form of "own your knowledge, rent the execution".
3. **Settlement.** The post assumes invoices, Stripe-style billing and a
   dispute process. Our rails are Bitcoin-only. We already run **x402**, an
   HTTP 402 payment challenge settled over Lightning, and today we stood up
   our own LND node for it (`oa-lnd-1`, `docs/payments/lnd-host.md`).
   - That gives a primitive the post doesn't consider: **charge at the
     moment of acceptance**. The accept action returns a 402 for the
     change's price; paying it releases the change.
   - It needs no invoices or net-30 terms, and it works the same for a
     person and for another company's agent.
   - It fits small per-change prices that card billing makes awkward.
4. **The outcome event protocol already exists in parts.**
   - Traces are ATIF documents at `/api/traces`.
   - Accepted work and receipts are in the Dhruv NIP roadmap (WORK, REG and
     ATIF first; #11729).
   - The post proposes a new `customer.accepted` JSON event. We should
     **extend what we have**, not add a third format.
5. **The pilot only covers one platform.** `docs/sales/README.md` (offer
   `openagents.sales.coder-pilot.v1`, $250 after acceptance) is qualified
   for the buyer's macOS arm64 computer only. That's deliberate: REV-02
   qualifies an exact installed path. Coder 1.0.0 runs on macOS, Linux and
   Windows, so widening the pilot is one REV-02 run per platform on the
   released 1.0.0, not a code change.
6. **Plugin payments.** The post assumes we pay plugin authors per call
   today. We don't; we removed that claim from the site today. The post's
   advice (explicit fees first, no attribution algorithm) stands for when we
   do.
7. **Customer zero is us.** We already run the loop on OpenAgents itself
   (`coder issue-run N`). The post treats the first pilot as the first data.
   We can publish our own cost per accepted change now.

## What we should do

### Now: works with Coder 1.0.0, server side and evidence only

1. **Settle the quality question.** Rerun the brief + verify study large
   enough to answer "equal or better acceptance".
   - Aim for 60+ issues × 3 runs, chronological, on OpenAgents plus 2–3
     public repositories.
   - Use the same harness and the claims rules.
   - Until it passes, every sales line says "cheaper, quality being
     validated".
2. **An outcome ledger on the server.**
   - Record every admitted task: its states (admitted → attempted →
     verified → accepted → merged → reverted within 90 days), every attempt,
     and every cost, including failures, timeouts and declines.
   - It hangs off the existing traces store and issue-run summaries.
   - Report **coverage** (accepted, declined and escalated) next to cost, so
     refusing hard work can't fake a better number.
3. **Publish customer zero's numbers.** `/efficiency` shows our own cost per
   accepted change and coverage on the OpenAgents repo from the ledger,
   updated as we work. Every figure goes through `claims.md`.
4. **Qualify the pilot on 1.0.0** (REV-02 on Linux and Windows too) and keep the $250
   assisted pilot as is: the customer pays their own models, and we charge
   only on acceptance.
   - Activating the offer and finding the buyer are the owner's.

### Next: once LND is funded and the ledger has data

5. **Pay on acceptance over x402.** The accept step returns a 402 for the
   quoted price. Start with the assisted pilot, settled in sats at the
   dollar price.
   - This is server-side, and the Coder 1.0.0 CLI can already pay x402 from
     a wallet.
   - The managed mode ("we buy the compute and absorb failed attempts") waits
     until one task class has a known cost distribution.
6. **The network-effect test, on public repositories first.** Closed issues
   in open-source repositories, with their merged fixes, are ground truth
   that needs no customer consent. Run the post's A/B/C/D arms across a
   family of related repositories (e.g. several Rust workspaces), split by
   time and by repository.
   - If D doesn't beat C, we say so and sell infrastructure plus private
     learning. That's still a business.
7. **Pick the first task class** from what the ledger shows is cheap and
   reliable: dependency updates, failing-test fixes, small regressions.
   Don't sell "features" or "productivity".

### Later

8. **Open the outcome interface to one partner app** with a real verifier,
   after it works in Coder.
9. **Repricing as costs fall.** Lower the per-change price when cost drops,
   so customers see the savings (the post's §7 table). Publish a versioned
   price book.

### Not doing

- Claiming network effects or "equal quality at half the price" before the
  tests above pass.
- Pricing on broad business results (revenue, uptime).
- Pooling raw customer traces. Shared learning starts from public
  repositories and contributed procedures only.
- Competing on model breadth or card billing.
- Anything that needs a new Coder binary before the next planned release.
  Steps 1–6 don't.

## Owner decisions this waits on

1. Go on the outcome ledger and the bigger quality study (agent work, about
   $300–600 of model spend for two arms of 180+ runs each, at the study's per-run cost).
2. Whether `/efficiency` publishes our own cost per accepted change.
3. Activating the $250 assisted pilot and naming the first buyer.
4. Funding `oa-lnd-1` (in `NEEDS_OWNER.md`), which pay-on-acceptance needs.
5. Which public repositories form the first network-effect family.

# Tassadar Verified-Work Payment Economics

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-16
Scope: how much we pay, to whom, how often, for Tassadar executor-trace verified
work — and what we get for it. Companion to the launch-gate thread
`/forum/t/34bebe36-…` and the #5051 / #5061 / #5121 loop.

Status: analysis + recommendation for the **initial** settings. Everything here
is adjustable; the point is to set conservative, honest expectations we can
**raise** later rather than ones we'd have to **cut**.

## TL;DR

- **One-time recognition reward (decided): 50,000 sats each** to Trigger
  (worker) and Orrery (validator) for the first independent end-to-end pairing.
  This is a bootstrap thank-you, **not** the ongoing rate. (~$50 each at an
  assumed $100k/BTC; substitute the live price.)
- **Ongoing per-verified-window settlement (recommended: 1 sat now).** The
  current workload is a trivial pinned CPU fixture (a ~80-step loop-sum numeric
  model), not real GPU training. The honest price of *that* unit of work is
  ~nothing, so the protocol settlement should start **symbolic and tiny (1 sat
  per verified window)** and **scale up by workload class** as the work becomes
  genuinely expensive (real training windows). Raise-not-cut.
- **The rate should track the real cost/value of the work, not a flat number.**
  CPU exact-trace replay is ~1000×+ cheaper than the GPU training it can verify;
  the PoC fixture is cheaper still. Don't price the fixture like a GPU window.
- **Open design gap: the validator currently earns nothing per-window.** The
  protocol settles only to the worker. That's fine for the PoC + the one-time
  reward, but a sustainable independent-validator network probably needs a small
  per-verification validator incentive. Flagged for discussion, not decided.

## 1. What are we actually paying for?

Two roles, today:

- **Worker** — runs the dispatched workload and submits a trace commitment
  (`pylon training submit-trace`). The protocol **settles to the worker** for
  *verified* work.
- **Validator** — re-executes the same workload on a **distinct device** and
  submits a replay digest (`pylon training validate`). If the digests match, the
  `exact_trace_replay` challenge resolves `Verified`. The validator currently
  receives **no protocol payment** (Orrery noted this: "the capped payout
  settles to the worker, not to me").

The unit of payment is a **verified window**: one worker contribution whose
exact-trace replay was confirmed by an independent device.

Crucially, today's workload is the **committed pinned fixture**
(`tassadar-poc-loop-sum-v1`, ~80 numeric steps). Executing it is **milliseconds
of CPU**. Replaying it is the same. So at this PoC stage we are not paying for
expensive compute — we are paying to **bootstrap the proof and the network**:
real, independent, dereferenceable evidence that "contributors earn Bitcoin for
verified work," and an incentive for nodes to show up.

## 2. The cost anchor: CPU verification vs GPU training

The user's instinct is right: this should be **way** cheaper than what a GPU
training run pays per window.

- A real **GPU training window** (a batch/window of steps on, say, an H100 at
  ~$2–4/hr) costs on the order of **cents to dollars** of compute, depending on
  window size. That is the floor a real training-work payment would have to beat
  to be worth a contributor's electricity + hardware.
- **Exact-trace replay verification** is CPU-only and deterministic — the whole
  design point is that verifying expensive work is **cheap**. Replaying even a
  real window is orders of magnitude cheaper than producing it.
- The **current fixture** is neither — it's a trivial loop-sum, effectively
  **$0** of compute on either side.

So there are really three different prices hiding behind "per window":

| Work unit | Real cost | Honest settlement (initial) |
| --- | --- | --- |
| PoC pinned fixture (today) | ~$0 (ms of CPU) | **1 sat** — symbolic / bootstrap |
| Real CPU/light workload | cents | tens of sats |
| Real GPU training window | cents–$ | the real number; set when it exists |

The architecture already separates this by **workload family** /
`workClassRef`; the rate should be **per workload class**, not one flat sat
value. Today only the fixture class is live, so we only need to set that one, and
it should be tiny.

## 3. Candidate rates for the current fixture

At an assumed **$100k/BTC** (1 sat = $0.001; substitute live price):

| Rate | $/window | Read |
| --- | --- | --- |
| **1 sat** | $0.001 | Honest for a ~$0 fixture. Clearly symbolic. Easy to raise. |
| 3 sats | $0.003 | Still symbolic; no real advantage over 1 for the fixture. |
| 10 sats | $0.01 | Starts to *look* like a real price for work that isn't real yet — risks anchoring expectations high. |

**Recommendation: 1 sat per verified window** for the fixture class. It is the
most honest reflection of the work, it is the easiest number to raise without
anyone feeling cut, and at this stage the *value* is the proof and the
participation, not the compute. (1 sat is payable over Lightning; for MDK/Spark
receive it's within normal range.)

## 4. Frequency + expenditure model

How often do settlements happen? Per **verified window**, which is gated by:
worker nodes online × windows claimed/worker/day × verified-replay rate.

Rough monthly spend = `rate(sats) × verified_windows_per_day × 30`.

At **1 sat/window** (recommended):

| Verified windows/day | Sats/month | ≈ USD/month ($100k/BTC) |
| --- | --- | --- |
| 50 (PoC, a few nodes) | 1,500 | ~$1.50 |
| 1,000 (early network) | 30,000 | ~$30 |
| 10,000 (real network) | 300,000 | ~$300 |
| 100,000 (scale) | 3,000,000 | ~$3,000 |

At **10 sats/window** every figure is ×10 (e.g., $3,000/mo at 10k windows/day).

Takeaways:

- At PoC volume the ongoing protocol spend is **rounding error** ($1–30/month) at
  any of these rates — the cost decision barely matters *now*. It matters because
  it **sets expectations** before volume exists.
- The expenditure only becomes material at real-network scale, and at that point
  the workload should also be real (and the per-window value with it). The rate
  must be allowed to move with both volume and workload class.
- **Guardrails already in place:** hard per-payout cap **100,000 sats**, plus the
  run's `spendCapSats` (currently 100,000). Recommend adding an explicit
  **daily/monthly run settlement ceiling** before opening the fixture class to
  arbitrary volume, so a misconfiguration or abuse can't drain the treasury.

## 5. What we get for it

- **A real, receipt-first, independently-verified claim** that contributors earn
  Bitcoin for verified machine work — the thing the launch gate
  (`training.monday_decentralized_training_launch.v1`) asserts, proven rather than
  asserted.
- **A live network of independent worker + validator nodes** with skin in the
  game and timestamped, OTS-anchored evidence (Orrery's pre-commitments).
- **The substrate for the real product:** once real training/inference windows
  flow through the same loop, the same rails pay real compute at real prices.

The PoC spend buys credibility and a working market, not compute. That's worth
far more than the sats.

## 6. Validator incentive (open question)

The protocol pays the **worker**. The validator's only current upside is the
one-time recognition + the forthcoming referral/revshare seat. That works to
bootstrap, but for a durable independent-validator pool we likely want a **small
per-verification validator fee** — e.g., a flat 1 sat, or a fraction of the
worker settlement — so validating is self-sustaining, not charity. Not decided;
raised here for feedback. (Risk of validator fees: incentive to spam validations;
mitigate by paying only on a *recorded Verified verdict for a distinct device*.)

## 7. Recommendation (what to set now)

1. **Pay the one-time recognition: 50,000 sats each to Trigger and Orrery** (real
   `mdk_agent_wallet`), as the bootstrap reward — separate from the protocol rate.
2. **Set the fixture-class per-verified-window settlement to 1 sat**, real
   `mdk_agent_wallet`, with the existing 100k per-payout cap and a new
   daily/monthly run ceiling.
3. **Keep the rate per workload class** and commit publicly to **raising, not
   cutting** as real workloads arrive.
4. **Decide validator incentive separately** (default: revisit once the referral
   /revshare system exists; consider a flat 1-sat validator fee then).

All adjustable. Posting the reasoning to the thread for contributor feedback
before locking the fixture rate.

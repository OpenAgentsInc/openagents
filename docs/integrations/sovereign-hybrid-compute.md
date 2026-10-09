# Sovereign Hybrid Compute (OATH, WarmGW): integration thoughts

Status: proposal, 2026-10-09. A partner proposed integrating two of Sovereign
Hybrid Compute's projects. This note is about what we could do together, not
a description of their software; their repositories are the source of truth
(`git.sovereignhybridcompute.com/SHC/OATH`, `.../SHC/WarmGW`), and their
own maturity labels (BUILT, PARTIAL, GATED) apply to every claim below.

## In one line each

- **OATH**: append-only event history turned into proofs anyone can check
  offline, with optional Bitcoin timestamps (OpenTimestamps) and explicit
  VALID / REJECT / UNKNOWN outcomes.
- **WarmGW**: a multi-tenant, OpenAI-compatible gateway in front of private
  inference backends, with tenant policy, warm (saved) context, batch work,
  content-free usage receipts, and a verifiable code identity.

## Where they fit us

| Our piece | OATH could add | WarmGW could add |
| --- | --- | --- |
| Inference gateway ([gateway.md](../inference/gateway.md)): router, adapters, meter | Proofs for the meter's attempt records and the daily cost reconciliation, so a customer can verify their bill offline | An **upstream** behind our router for private or sovereign capacity: one adapter, like OpenRouter or a Pylon |
| Public API and rate card (#11065, #11066) | A signed, timestamped history of every rate card version | Backends whose price and policy we publish as their own rate card rows |
| Stored responses and `previous_response_id` (#11071) | | **Warm context**: route a continuation to the backend that still holds the conversation's context (cache affinity), cutting time to first token and cost |
| Privacy: strict, zero retention, content-free records | Proofs that a record holds no message text (only digests and commitments) | Content-free receipts and hidden topology match our strict tier; a candidate "verified upstream" privacy level |
| Pylon providers and payouts (#11070) | Payout evidence a provider can verify without trusting us | Sovereign operators joining as providers through WarmGW, paid like any Pylon |
| Gym evals, "verified value" routing (#11079) | Anchored eval results, so published comparisons are provably from real runs (spec §6) | |
| Environments (build, verify, save) | Build and verify evidence for a saved version, checkable later | Reproducible-artifact identity as a model for our own environment images |
| Receipts and audit (Cloud usage receipts, treasury and payout continuity) | The portable evidence layer under all of them | |

## Three concrete integrations, in order

1. **Verifiable usage receipts (OATH).** Feed the gateway meter's attempt
   records (already content-free: tokens, cost, latency, upstream, never
   text) and payout credits into an OATH log; anchor a daily checkpoint on
   Bitcoin; give each API customer a `GET /v1/usage/proof` that returns the
   proof for their records. Smallest change, directly sellable: "your bill
   is provable."
2. **WarmGW as an upstream class.** An adapter in `crates/inference`
   (OpenAI-compatible Chat Completions first; Open Responses if they add it),
   a privacy tier for "verified code identity + content-free receipts," and
   cache-affinity routing for `previous_response_id`. Customers who need
   sovereign or on-prem capacity get it through the same OpenAgents API
   with `route.only` set to those upstreams.
3. **Provider attestation for the open pool.** Use OATH conformance badges
   and WarmGW's `/gateway/identity` as the evidence a Pylon or partner
   provider presents before the router trusts it with strict traffic.

## Business angle

- **Regulated and sovereign buyers** (data must stay in a jurisdiction or on
  hardware they control) can use our API, rate card, and payments while
  inference runs on their capacity: we route and bill, they host.
- **Trust as a feature**: provable bills, provable eval results, provable
  payouts. Fits the "selected by verified value" direction from Episode 242.
- **Revenue**: the standard rate card margin on WarmGW-served traffic;
  optionally a small fee for anchored proofs; providers paid through the
  existing earnings ledger.

## Questions for the partner

- API shape: Chat Completions only, or Open Responses too? Streaming and tool
  calls?
- Pricing and capacity: per-token prices, minimums, available regions.
- Data terms: retention, training, and what "content-free" covers in logs.
- Attestation: proof and badge formats we can verify in Rust, and OTS anchor
  cadence and cost.
- Maturity: which parts are BUILT vs PARTIAL vs GATED today, and the license.
- Pilot: one backend behind a test key, one week of anchored receipts.

## Next step

A one-week pilot: a WarmGW adapter behind a test key in a local gateway
(no production traffic), plus OATH proofs for one day of meter records, and
a short write-up of what verified and what didn't.

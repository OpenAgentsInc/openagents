# Reactor Contracts

Typed Reactor model provenance, model-policy, serving-skeleton, and eval-receipt
contracts.

This package is metadata, policy-decision, and skeleton-routing plumbing only.
It does not authorize live model installation, serving, customer deployment,
compliance claims, spend, or public availability copy.

Exports:

- `model_provenance.v1` shaped catalog records for open-weight model metadata.
- `reactor.model_policy.v1` shaped customer policy records.
- A pure resolver that evaluates a policy against a catalog and returns a
  receipt-shaped decision naming the policy version.
- A curated seed catalog with honest `unknown` / `partial` disclosure values
  where facts are not complete.
- Lane-neutral Reactor node profiles with `servingLane: hydralisk | psionic`.
- Fixture provision/router helpers that refuse nonconforming models before
  weight pull or OpenAI-compatible routing.
- Exact local token-metering receipt helpers that use `not_measured` instead
  of estimates when counts are unavailable.
- A Psionic-owned task-class eval harness profile for drafting, extraction,
  RAG-over-corpus, and agent tool-use.
- Per-model eval receipts, a seed 2-model × 2-task measured fixture set, a
  coverage matrix that marks unrun combinations as `not_measured`, and a
  capability-copy decision helper that returns only measured eval refs.

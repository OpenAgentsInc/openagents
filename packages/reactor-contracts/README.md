# Reactor Contracts

Typed Reactor model provenance and model-policy contracts.

This package is metadata and decision plumbing only. It does not authorize
model installation, routing, customer deployment, compliance claims, spend, or
public availability copy.

Exports:

- `model_provenance.v1` shaped catalog records for open-weight model metadata.
- `reactor.model_policy.v1` shaped customer policy records.
- A pure resolver that evaluates a policy against a catalog and returns a
  receipt-shaped decision naming the policy version.
- A curated seed catalog with honest `unknown` / `partial` disclosure values
  where facts are not complete.

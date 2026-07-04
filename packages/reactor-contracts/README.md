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
- Air-gapped update-bundle manifests that reuse the `apps/oa-updates`
  ed25519 verifier/public-key pattern.
- Install, upgrade, and rollback receipt helpers that revalidate model policy on
  every model refresh.
- Guidance-only hardware tier specs for workstation, server, and rack planning.
- OpenAgents dogfood-run receipts for the customer-number-one Reactor gate:
  dogfood placement, strict US-only policy, routed internal workload refs,
  exact local token-metering receipts, and a refused nonconforming refresh.
- Need-to-know corpus access receipts: deny-by-default hard rules for
  workspace, matter, and role-or-user scope, a downstream model-oracle
  plausibility verdict, no raw document or generated summary logs, and
  adversarial Bob/Alice fixtures that fail closed across citation and summary
  modes.
- Data Liberation pipeline reports: config-driven synthetic export adapters,
  schema-mapped transforms, customer-controlled open-store refs, verification
  receipts per record class, checksums, spot-diff hashes, and honest partial
  migration blockers without raw customer row values.

Smoke:

```sh
bun run --cwd packages/reactor-contracts smoke:install
bun run --cwd packages/reactor-contracts smoke:dogfood
```

The smoke creates a clean temporary Reactor node directory, signs a fixture
bundle with a generated test ed25519 key, verifies it with the existing
`apps/oa-updates/scripts/verify-release.ts` fail-closed verifier, rejects a
tampered bundle, and writes fresh-install, upgrade, and rollback receipts.

The dogfood smoke writes the public-safe RX-6 receipt set into a clean
temporary directory: the OpenAgents dogfood node profile, signed bundle
manifest, fresh-install receipt, routed internal workload receipts, exact local
metering receipts, the policy-refused Qwen refresh, and the aggregate
`openagents.reactor.dogfood_run_receipt.v1` record. It remains internal
dogfood evidence only; it does not authorize external pilots or public
availability copy.

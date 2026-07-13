# OpenAgents first MVP

This directory is the canonical product-definition package for the first
deployable OpenAgents shape: a ProductSpec-native, local-first Codex workroom.
It keeps the committed intent and its supporting audit separate and easy to
read.

## Read in this order

1. [`openagents-codex-workroom-mvp.product-spec.md`](./openagents-codex-workroom-mvp.product-spec.md)
   — the exact ProductSpec v0.1 intent artifact. It owns the MVP problem,
   hypothesis, in/out/cut scope, user experience, solution, stable acceptance
   criteria, success metrics, risks, owner gates, and required receipts.
2. [`2026-07-13-openagents-codex-workroom-mvp-audit.md`](./2026-07-13-openagents-codex-workroom-mvp-audit.md)
   — the dated OpenChamber/OpenCode/Codex/OpenAgents evidence and option analysis
   behind that spec. It does not dispatch work or manufacture current proof.
3. [`../sol/MASTER_ROADMAP.md`](../sol/MASTER_ROADMAP.md) — the sequencing,
   priority, live-gate, and issue-triage authority.
4. [MVP-01 #8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
   — the sole active Sol product lane and current claim/evidence ledger.

The Product Spec declares intent. Runtime policy, behavior contracts, Eval
Suites, tests, reviewed artifacts, and receipts verify it; owner gates record
approval or waiver; the promise registry alone authorizes public claims.

## ProductSpec location and validation

The MVP Product Spec is intentionally co-located with its audit here by owner
direction. It remains a normal `.product-spec.md` file and is validated by the
repository ProductSpec test sweep alongside `specs/**/*.product-spec.md`.

```sh
bun packages/product-spec/src/cli.ts validate \
  docs/mvp/openagents-codex-workroom-mvp.product-spec.md
bun test packages/product-spec/test/product-spec.test.ts
```

Do not create a second copy under `specs/`; links, issues, dispatch prompts, and
future decision traces should cite this path plus `spec_revision`.

## Current boundary

This package does not yet update behavior/Eval registries, the public promise
registry, or launch claims. Those integrations wait for their explicit roadmap
slice and exact implementation receipts.

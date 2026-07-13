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
5. [`../assurance/MVP_FIRST_ASSURANCESPEC.md`](../assurance/MVP_FIRST_ASSURANCESPEC.md)
   — the proposed first AssuranceSpec dogfood slice bound to this exact MVP
   ProductSpec. It plans proof-design tooling; it does not change MVP intent,
   sequencing, release state, or public claims.

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

## Planned first AssuranceSpec

The first authored companion is intended to live here as
`openagents-codex-workroom-mvp.assurance-spec.md`, beside the ProductSpec it
binds. It does not exist yet. The pilot first builds the schema, parser,
validator, exact revision/digest/criterion binder, review/admission artifacts,
local Environment Profile, deterministic Manifest compiler, Bun-test adapter,
falsifier, and receipt path necessary to make that file real.

The target subject is currently ProductSpec format `0.1`, `spec_revision: 6`,
SHA-256
`3396b2dd2778c724184668b045dedc3288578685386beeef67b4316e83b99aa5`, with
`CW-AC-01` through `CW-AC-18`. A changed revision or digest requires the pilot
binding and evidence to be reconciled; this README is not an authority for
silently pinning stale identity.

## Current boundary

This package does not yet update behavior/Eval registries, the public promise
registry, or launch claims. Those integrations wait for their explicit roadmap
slice and exact implementation receipts.

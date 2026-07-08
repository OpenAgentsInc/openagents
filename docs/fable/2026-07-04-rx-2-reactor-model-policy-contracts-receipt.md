# RX-2 Reactor Model Policy Contracts Receipt

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04
Issue: [#8272](https://github.com/OpenAgentsInc/openagents/issues/8272)
Status: source contract landed; product promise remains planned

## What Landed

- `packages/reactor-contracts` exports Effect Schema contracts for:
  `openagents.model_provenance.v1`, `openagents.reactor_model_catalog.v1`,
  `openagents.reactor.model_policy.v1`, and
  `openagents.reactor.model_policy_decision.v1`.
- The curated seed catalog covers Nemotron, Llama, GPT-OSS, Gemma, Mistral,
  Qwen, DeepSeek, Kimi, and GLM families with honest `unknown` / `partial`
  disclosure values where facts are incomplete.
- Distillation-lineage checks participate in jurisdiction constraints, so a
  US-labeled model with restricted-origin lineage fails a strict US-only policy.
- The pure resolver evaluates policy x catalog into conforming model refs or a
  refusal object, and every decision receipt names the policy version.
- Example policies are covered for US-only, no-cn, permissive-license-only, and
  unconstrained routing choices.

## Boundary

This is metadata and decision plumbing only. It does not authorize model
installation, serving, routing, customer deployment, air-gap update handling,
compliance copy, license advice, payout, settlement, or public availability
claims.

The product-promise registry clears only the contract-level schema/catalog/
lineage-policy/decision-receipt blockers. Remaining blockers include eval
receipts, provisioner/router enforcement, structural serving refusal smoke,
air-gap update path, dogfood/customer deployment receipts, and owner-approved
public copy.

## Verification

- `bun run --cwd packages/reactor-contracts test`
- `bun run --cwd packages/reactor-contracts typecheck`
- `bun run --cwd apps/openagents.com/workers/api test -- src/product-promises.test.ts`

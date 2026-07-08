# RX-6 Reactor Dogfood Run

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04

Issue: [#8276](https://github.com/OpenAgentsInc/openagents/issues/8276)

## Scope

RX-6 records OpenAgents as customer number one for Reactor. The receipt set is
public-safe: it names opaque workload/request/receipt refs only, proves the
strict policy gate and exact local metering shape, and keeps raw prompts,
outputs, local paths, private corpus data, provider payloads, and secrets out of
the report.

This is not an external customer pilot, public availability claim, compliance
claim, rate-card approval, or customer data-custody proof.

## Dogfood Node

- Node profile:
  `reactor.node_profile.openagents.dogfood.hydralisk.v1`
- Placement: `dogfood`
- Hardware owner ref: `owner.openagents`
- Model: `model.openai.gpt_oss.open_family`
- Serving lane: `hydralisk`
- Gateway protocol: `openai.chat_completions.v1`
- Serving path network: `offline_once_provisioned`
- Policy: `reactor.model_policy.example.us_only.v1`
  (`2026-07-04.us-only`)
- Policy constraints: `origin_jurisdiction:us` and distillation-lineage
  jurisdiction enforcement

The dogfood node uses the RX-5 install path and the existing OpenAgents
ed25519 release-verifier pattern:

- Bundle manifest:
  `reactor.airgap_bundle.openagents.dogfood.gpt_oss.20260704`
- Fresh install receipt:
  `reactor.install_ops.openagents.dogfood.fresh.gpt_oss.20260704`

## Internal Workload

Workload ref:
`workload.openagents.lead_gen_reactor.case_study_seed.20260704`

Measured window:
`2026-07-04T14:10:00.000Z` through `2026-07-04T14:15:00.000Z`

Requests:

| Request ref | Route receipt | Metering receipt | Exact tokens |
| --- | --- | --- | --- |
| `reactor.request.openagents.dogfood.lead_gen.discovery.20260704` | `reactor.route_decision.openagents.dogfood.discovery.20260704` | `reactor.local_metering.openagents.dogfood.discovery.20260704` | 312 |
| `reactor.request.openagents.dogfood.lead_gen.sequence.20260704` | `reactor.route_decision.openagents.dogfood.sequence.20260704` | `reactor.local_metering.openagents.dogfood.sequence.20260704` | 431 |

Total measured local tokens: 743.

## Refused Nonconforming Pull

The dogfood run deliberately attempts a Qwen refresh under the US-only policy.
The install/ops receipt refuses before model refresh:

- Refused bundle:
  `reactor.airgap_bundle.openagents.dogfood.qwen.refused.20260704`
- Refused install/ops receipt:
  `reactor.install_ops.openagents.dogfood.qwen.refused.20260704`
- Refused model: `model.alibaba.qwen.open_family`
- Structural blocker:
  `blocker.reactor.install_ops.policy_revalidation_failed`
- Policy reason: `reactor.policy.origin_not_allowed`

## Aggregate Receipt

Aggregate receipt:
`reactor.dogfood_run.openagents.lead_gen_case_study_seed.20260704`

Schema:
`openagents.reactor.dogfood_run_receipt.v1`

The aggregate receipt binds the dogfood node profile, strict policy refs,
fresh-install receipt, routed internal workload receipts, exact local metering
receipts, and the refused nonconforming refresh into one case-study seed.
It sets:

- `publicSafe: true`
- `externalPilotAuthorized: false`
- `externalClaimFlipAllowed: false`
- `workloadTruth: internal_openagents`
- `hardwareOwnerRef: owner.openagents`

## Verification

Commands:

```sh
bun run --cwd packages/reactor-contracts test
bun run --cwd packages/reactor-contracts typecheck
bun run --cwd packages/reactor-contracts smoke:dogfood
```

The dogfood smoke writes the full public-safe receipt set into a clean temporary
directory and prints the aggregate dogfood receipt ref, exact metering refs,
refused nonconforming model ref, total measured tokens, and temp receipt path.

## Boundary

RX-6 clears the internal dogfood proof blocker only. It does not clear customer
premises deployment, customer private-data custody, need-to-know access,
full model/task eval coverage, owner-approved public pricing/copy, compliance,
payout, settlement, or external pilot blockers.

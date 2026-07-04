# RX-8 Model-Custody Lead Gen Segment

Date: 2026-07-04
Issue: [#8281](https://github.com/OpenAgentsInc/openagents/issues/8281)
Status: analyzer config, regulated template variant, LG-7 segment config, and
LG-2 pipeline source-ref coverage landed; no live send or customer result.

RX-8 turns the Campaign B "Own Your AI" plan into a typed, draft-only segment.
It does not create a Reactor customer pilot; RX-7 stays owner/customer gated.

## Analyzer

`@openagentsinc/agent-readiness` now exports:

- `openagents.model_custody_analyzer_config.v1`
- `openagents.model_custody_report.v1`

The analyzer scans only public URLs and records only reproducible public facts:

- published subprocessors or DPA pages;
- public privacy or AI-feature disclosures;
- careers or jobs pages that name AI/model stack terms.

Every finding carries `factuality: "public_surface_only"` and
`speculationAllowed: false`. The report stores evidence refs, status codes,
content types, matched public terms, and an inference boundary. It does not
store raw page bodies, scan private/login-gated pages, infer customer data
movement, infer provider training, or claim a compliance posture.

## Segment

The LG-7 Lead Gen definition now has a second OpenAgents customer config:

- `lead_gen_config.openagents.model_custody.campaign_b.v1`
- analyzer config:
  `analyzer.agent_readiness.model_custody.own_your_ai.v1`
- source ref: `apollo_model_custody`
- target discovery:
  `target_discovery.openagents.model_custody.hand_approved.v1`
- template family:
  `template_family.lead_gen.model_custody_regulated.reactor_assessment.v1`

The same standing agent definition is reused. The payload remains
drafting-only and keeps `sendAuthority.allowed: false`.

## Template

LG-4 now includes
`business.outreach.model_custody_regulated.reactor_assessment.v1` for
regulated or IP-sensitive verticals. It pairs the public-fact custody dossier
with the Friedberg/Mistral third-party-validation framing and a Reactor
Assessment offer.

The template intentionally avoids forbidden copy:

- no compliance-certification claim;
- no ownership/privacy guarantee beyond the scoped assessment;
- no public price;
- no email/Apollo send authority.

The claim linter covers the template in
`apps/openagents.com/workers/api/src/model-custody-lead-gen.test.ts` and
`apps/openagents.com/workers/api/src/business-outreach-routes.test.ts`.

## Pipeline

The business source decoder now accepts `apollo_model_custody` and aliases
human-friendly `model-custody` / `apollo-model-custody` inputs to that bounded
token. The LG-2 pipeline route test proves a quoted Reactor Assessment row can
land with `sourceRef=apollo_model_custody` and remain visible in source-ref
metrics.

## Boundary

This clears only the config/template/pipeline blocker for RX-8. It does not
authorize Apollo sends, contact reveals, customer-result claims, customer
deployment, public pricing, compliance copy, payout, settlement, or promise
green status.

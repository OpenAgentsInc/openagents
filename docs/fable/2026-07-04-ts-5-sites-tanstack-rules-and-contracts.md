# TS-5 Sites TanStack Rules And Contracts

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Date: 2026-07-04
Issue: [#8347](https://github.com/OpenAgentsInc/openagents/issues/8347)
Epic: [#8339](https://github.com/OpenAgentsInc/openagents/issues/8339)

## What Landed

`sites_tanstack_rules.tanstack_start.v1.2026_07_04` is now the canonical
Sites builder rules pack for generated TanStack Start sites. Every
`createSiteBuilderSession` call injects a bounded metadata reference to the
pack, including the rule refs, session brief, doc path, version, and feedback
ledger refs.

The rule set covers:

- TanStack file routing plus the `src/server.ts` Worker entry.
- `createServerFn` boundaries for server-side data and copy handoff.
- Worker bindings for deploy-target configuration.
- SSR-first route rendering, with prerendered agent surfaces where static.
- Tailwind 4 plus `@openagentsinc/ui/react.css` dark-only tokens.
- Day-one `robots.txt`, `sitemap.xml`, `llms.txt`, JSON-LD, and
  `/.well-known/openagents.json`.
- Per-site WfP Worker modules, never the live `openagents.com` Worker.
- Behavior-contract sweep before deploy review.

## Feedback Ledger

Each rule has a feedback ledger row, seeded from the TS-4 template/build-lane
failure modes:

- `ts4.failure.start_site.wrangler_jsonc_main_misclassified`
- `ts4.failure.start_site.server_fn_mixed_with_client_copy`
- `ts4.failure.start_site.binding_policy_not_explicit`
- `ts4.failure.start_site.ssr_default_needed_for_agents`
- `ts4.failure.start_site.tokens_missing_from_template`
- `ts4.failure.start_site.agent_surfaces_manual_retrofit`
- `ts4.failure.start_site.deploy_gate_namespace`
- `ts5.failure.generated_site_contracts_missing`

Future rule additions should add a ledger row in the same change that adds the
rule, with the real build failure, QA finding, or owner-stated correction that
caused the addition.

## Starter Contracts

Generated Start sites now register the starter customer-invariant contract set
before deploy review:

- `autopilot_sites.generated.dead_controls.v1`
- `autopilot_sites.generated.navigation_integrity.v1`
- `autopilot_sites.generated.claim_safety.v1`
- `autopilot_sites.generated.bundle_budget.v1`

The sweep returns `readyForDeployReview=false` and blocker refs when any
contract fails. The claim-safety check uses the LG-4 gated-claim denylist
categories from `BUSINESS_OUTREACH_GATED_CLAIM_DENYLIST`: self-serve delivery,
pays-you loops, regulated/sovereignty posture, published prices, and referral
payouts.

## Verification

Pinned check:

```sh
bun run --cwd apps/openagents.com/workers/api test -- src/sites-tanstack-rules.test.ts src/sites-builder-sessions.test.ts src/business-outreach-routes.test.ts src/model-custody-lead-gen.test.ts
```

The tests prove:

- builder-session fixture metadata contains the injected rules pack;
- the behavior-contract registry mechanically validates;
- the TS-4 Start template passes the starter sweep;
- a deliberately broken generated site fails closed on dead controls, broken
  first-party navigation, gated marketing claims, and bundle budget;
- the LG-4 denylist still passes its existing outreach and model-custody
  template suites.

## Boundary

This work creates no live deploy, no promise-state transition, no customer
result claim, and no spend, payout, or settlement authority. It only blocks
generated-site deploy review until the preview artifact passes the starter
contract sweep.

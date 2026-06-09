# Agent Sites, Pylon, And Commerce Gap Audit

Date: 2026-06-05

Status: implementation audit and sequencing note. This document does not deploy
public agent instructions, enable customer payments, admit Pylons, move money,
or change runtime policy by itself.

## Sources Reviewed

- `docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `docs/sites/2026-06-05-oa-sites-vibesdk-gap-analysis.md`
- `docs/sites/2026-06-05-agent-site-action-contract.md`
- `docs/sites/2026-06-05-openagents-revenue-share-system.md`
- `docs/sites/vida_referral_model_for_openagents.md`
- `docs/2026-06-02-mdk-l402-agent-checkout-audit.md`
- `vortex/docs/autopilot-finance-mdk-payment-model.md`
- `openagents/docs/2026-04-21-run-pylon-get-paid-for-training.md`
- `openagents/docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md`
- `openagents/docs/audits/2026-04-27-pylon-windows-build-and-binary-audit.md`
- `projects/moneydevkit/repos/mdk-checkout/README.md`

## Current Status

OpenAgents product surface has an operator-supervised Sites beta, not an autonomous agent-deployed
Sites platform.

Built or materially present:

- customer software orders;
- Site project and version records;
- public static Site runtime at `sites.openagents.com/<slug>`;
- stable latest-revision activation policy;
- customer-visible revision history and feedback submission;
- lifecycle transactional email through the Resend-backed `EmailService`;
- Adjutant assignment/run lifecycle;
- Exa research review and task packet planning;
- `.openagents/site.json` foundation;
- public capability manifest, OpenAPI, `https://openagents.com/AGENTS.md`, proof, and
  instruction-card work from the earlier viral-agent batch; and
- referral and revenue-share planning docs.

Not ready:

- public homepage agent call-to-action;
- public promise that agents can create and deploy Sites without Autopilot;
- self-serve Site builder sessions;
- isolated build execution with durable preview artifacts;
- agent-scoped create/build/save/deploy APIs;
- Site commerce checkout and L402 paid-action primitives;
- MDK wallet onboarding for agent payers;
- Pylon v0.2 public setup instructions;
- Pylon provider admission for general Site/commerce work; and
- revenue-share settlement across Site payments, referrals, and Pylon
  accepted-work receipts.

The earlier homepage "I am an Agent" surface was intentionally hidden again.
That is the correct current posture. The docs and manifests can be developed
privately, but the public product should not invite agents to mutate, deploy,
pay, or claim earnings until the contracts below are implemented.

## Required Target Capability

The target is not merely "agents can read the website." The target is that a
human can send an agent to OpenAgents and the agent can safely:

1. discover current capabilities;
2. understand limits, prices, and allowed dry-run actions;
3. authenticate or receive an L402 challenge when needed;
4. create a Site project or request;
5. provide source files or prompt requirements;
6. use local compute or a Pylon only with explicit owner/operator authority;
7. build, preview, save, and, where policy permits, deploy;
8. add commerce products or paid actions;
9. pay with credits or Lightning/MDK when economic limits are reached;
10. preserve referral attribution;
11. produce public-safe receipts; and
12. let the human owner review, revoke, accept, or continue revisions.

For v0, "agent deploy" should mean scoped deploy authority into an
OpenAgents-owned Site runtime, not arbitrary writes to customer domains or
private infrastructure. Production deploy still needs receipt-backed policy:
owner scope, idempotency, build receipt, payment/credit policy, and rollback or
stable-latest controls.

## Gap Analysis

### Agent Discovery And Instructions

Current gap:

- `/.well-known/openagents.json`, OpenAPI, and `https://openagents.com/AGENTS.md` exist
  as early surfaces, but they are not enough to guide a third-party agent
  through self-serve Site creation, pylon setup, MDK payment, referral capture,
  or deploy authority.
- The current public product should not surface the homepage CTA until the
  instructions are versioned, reviewed, and backed by real APIs.

Needed:

- a private, reviewed instruction packet for coding agents;
- a dry-run-first rule;
- explicit prohibited actions;
- idempotency and receipt requirements;
- local-compute warnings;
- Pylon setup and readiness checks;
- MDK wallet setup and spend-cap guidance;
- referral-link preservation rules; and
- examples for Codex/ChatGPT-style coding agents, browser/API agents, and
  local shell agents.

### Agent Site Creation And Deployment

Current gap:

- Autopilot can create Site assignments from orders.
- Agents cannot directly create Site projects, open builder sessions, submit
  files, request preview builds, save versions, or deploy through a typed
  public/scoped API.

Needed:

- agent-scoped Site action catalog;
- `POST /api/agent/sites` or equivalent create endpoint;
- durable builder session records;
- file snapshot upload and artifact receipt model;
- R2/static preview path for cheap first previews;
- WFP preview/deploy path for Worker-compatible generated apps;
- save-version and deploy-version APIs with separate authority;
- deploy receipts that name source commit/snapshot, build result, version,
  runtime target, active URL, and claim state; and
- customer/operator UI that shows which agent did what and under whose scope.

### Hosted E-Commerce Agent Sites

Current gap:

- Sites do not yet have a deployable commerce primitive.
- Generated Sites cannot declare checkout products or paid actions.
- Agents cannot pay for protected Site actions through MDK/L402.

Needed:

- `.openagents/site.json` `payments` block;
- D1 product/action/catalog tables;
- hosted checkout intent API;
- checkout buttons/forms/product cards for human buyers;
- L402 challenge and entitlement flow for agent buyers;
- clean success/cancel return paths;
- idempotent MDK status/webhook reconciliation;
- payment redaction tests; and
- Site receipt projections that separate checkout evidence from accepted-work
  payout and settlement claims.

The MDK Next.js package is useful source material. OpenAgents product surface should not import it
into generated Sites as the authority. Generated Sites should call an
OpenAgents product surface-hosted Worker payment boundary that ports the relevant MDK core behavior
into Effect TypeScript, D1 ledgers, Worker env bindings, and Web Crypto.

### Referral And Revenue Share

Current gap:

- Referral and revenue-share policy exists as planning docs.
- The product does not yet capture Site-owner referral attribution through a
  clean first-party path.
- Payments do not yet create a split-eligible revenue event.

Needed:

- Site referral source tokens;
- clean capture endpoint that redirects away from `ref` URLs;
- first verified direct referrer persistence;
- paid-workflow event ledger;
- abuse, duplicate, self-referral, cap, clawback, and dispute policy;
- asset boundary between credits, sats, promotional credits, and internal
  payable; and
- public-safe dashboards that avoid earnings promises before paid usage and
  eligibility exist.

### Pylon And Local Compute Instructions

Current gap:

- Public Pylon earning instructions exist in `openagents`.
- The safe public floor is still the narrow v0.1.x earning lane.
- Pylon v0.2 is not ready as a broad public Site-agent instruction.

Current proven user path:

- `pylon` opens the provider dashboard.
- `npx @openagentsinc/pylon` is the normal bootstrap path.
- For hosted starter training, use `pylon-v0.1.16` or newer with the same
  guarantees.
- The package-managed launcher should be `@openagentsinc/pylon@0.1.17` or
  newer.
- Useful checks:

```bash
pylon --version
pylon status --json
pylon training status --json
pylon wallet balance --json
pylon wallet history --limit 20 --json
```

Windows posture:

- Do not advertise native Windows Pylon as ready.
- Public instructions should steer Windows users to WSL Ubuntu.
- Current release coverage historically had gaps for latest Linux assets, so
  public instructions need a live release-asset check before promising a
  binary-first path.

Pylon v0.2 readiness gaps:

- LDK-compatible payout target registration must be first-class:
  `bolt12_offer`, `bolt11_invoice`, `bip353_name`, or `lnurl_pay`.
- Nexus must block paid work without a registered LDK v0.2-compatible target.
- Hosted Psionic runtime packaging must be present when training/runtime work
  requires it.
- The active objective has moved toward Harvey legal/Qwen adapter work; CS336
  is proof/reference, not the broad objective.
- Wallet readiness, payout target health, assignment eligibility, accepted
  outcome, payout dispatch, and settlement must remain separate public states.
- Agent-facing docs need to explain that running Pylon can contribute compute
  only when the provider is eligible and assigned; it is not a generic promise
  that every local machine earns immediately.

### MDK Wallet Instructions

Current gap:

- MDK reference packages include `@moneydevkit/nextjs`,
  `@moneydevkit/create`, `agent-wallet`, `api-contract`, and `mdkd`.
- OpenAgents product surface has not yet implemented the Site checkout/L402 boundary or a public
  agent-wallet smoke path.

Needed:

- internal sandbox/signet smoke using MDK agent wallet tooling;
- spend caps;
- paid retry behavior;
- token cache semantics;
- no wallet mnemonic or MDK secret in generated Site source, public JS,
  manifests, screenshots, logs, proof pages, or emails;
- decision on hosted MDK versus self-hosted `mdkd` for the first proof; and
- docs that explain MDK is buyer-side payment tooling, while Pylon accepted
  work payout remains Nexus/Treasury/LDK authority.

## Prioritized Implementation Direction

### Keep Mission-Critical Fulfillment First

Do not delay existing customer order fulfillment, revision feedback, status
pages, transactional emails, or the static Sites runtime. Those are the live
service surface.

### Move Agent Sites, Pylon Instructions, And Commerce Ahead Of Nice-To-Have UX

The next work should prioritize:

1. agent Site action contract and gated instructions;
2. scoped Site create/build/save/deploy APIs;
3. local compute and Pylon setup instruction packet;
4. Pylon v0.2 readiness audit and release gates;
5. Site payment manifest and checkout/L402 contracts;
6. MDK agent-wallet smoke docs;
7. referral attribution tied to Site payments; and
8. revenue-share projection boundaries.

Site editor polish, targeted outreach, broad VibeSDK parity, and later Omni
workrooms remain important, but they should not outrank the agent-deployed
Sites and commerce substrate unless a live fulfillment bug blocks customers.

## Next Issue Batch

Open and implement these before public agent-Sites launch:

| Roadmap ID               | Title                                                             | Acceptance shape                                                                                                                                                                                                              |
| ------------------------ | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OPENAGENTS-AGENT-SITES-001    | Define agent Site action contract and readiness gates             | Complete in `docs/sites/2026-06-05-agent-site-action-contract.md`; it lists every agent Site action, auth scope, idempotency key, receipt, rate-limit, payment, deploy, and rollback requirement before public CTA re-enable. |
| OPENAGENTS-AGENT-SITES-002    | Draft gated agent instructions for self-serve Site creation       | Complete in the `https://openagents.com/AGENTS.md` source; it adds dry-run-first Site planning, Pylon/local-compute setup, referral preservation, MDK/payment caveats, and prohibited actions.                                       |
| OPENAGENTS-AGENT-SITES-003    | Add agent Site creation and deploy API skeleton                   | Complete as a gated skeleton in `workers/api/src/agent-site-routes.ts`; it exposes create, builder-session, preview, save-version, and deploy-request contracts with browser-session plus internal-preview gate, idempotency enforcement, and no actual create/save/deploy authority yet. |
| OPENAGENTS-PYLON-001          | Audit Pylon v0.2 public readiness gates                           | Complete in `docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md`; verdict is partially ready at source-contract level but blocked for broad public claims until v0.2 or documented 0.1-line release, platform assets, WSL/native Windows smokes, and current LDK settlement proof are retained. |
| OPENAGENTS-PYLON-002          | Add Pylon setup and local compute instruction packet              | Complete in `docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md` and referenced from `https://openagents.com/AGENTS.md`; it covers install commands, version floor, readiness commands, WSL/native Windows caveats, referral preservation, explicit owner/operator authority, and earning caveats. |
| OPENAGENTS-SITES-COMMERCE-001 | Add Site commerce manifest and product/action schema              | Complete in `workers/api/src/site-commerce.ts`, `workers/api/migrations/0065_site_commerce_catalog.sql`, and `docs/sites/2026-06-05-site-commerce-manifest-and-catalog-schema.md`; the manifest rejects secret-shaped values and separates checkout evidence, entitlement, accepted work, provider payout eligibility, and settlement. |
| OPENAGENTS-SITES-COMMERCE-002 | Add hosted checkout intent and L402 paid action contracts         | Complete in `workers/api/src/site-commerce-routes.ts` and `docs/sites/2026-06-05-hosted-checkout-and-l402-contracts.md`; static and WFP Sites can request hosted checkout intent, L402 challenge, and L402 redemption contract stubs with idempotency, spend caps, clean returns, stale challenge rejection, and redaction. |
| OPENAGENTS-SITES-COMMERCE-003 | Add MDK agent-wallet sandbox smoke plan                           | Complete in `docs/sites/2026-06-05-mdk-agent-wallet-sandbox-smoke-plan.md`; it covers unpaid challenge, capped payment, paid retry, entitlement grant, token cache behavior, stale challenge expiration, hosted versus self-hosted MDK decision points, and secret redaction requirements. |
| OPENAGENTS-SITES-COMMERCE-004 | Link Site payments to referral and provider revenue-share ledgers | Complete in `workers/api/src/site-commerce-revenue-share.ts`, `workers/api/migrations/0066_site_commerce_revenue_share_linkage.sql`, and `docs/sites/2026-06-05-site-payment-referral-revshare-linkage.md`; payment, referral, entitlement, accepted work, provider payout eligibility, and settlement remain separate states linked by public-safe refs. |

## Public Launch Gate

Do not redeploy the homepage agent CTA or publish stronger agent-Sites claims
until all of these are true:

- the gated instructions are reviewed;
- mutating agent APIs require scoped auth and idempotency;
- Site creation has durable preview and version receipts;
- deploy authority is separate from save authority;
- Pylon setup docs honestly state the v0.1.x floor and v0.2 gates;
- MDK payment docs have a sandbox/signet smoke;
- referral capture redirects to clean URLs;
- commerce products/actions have redaction tests;
- public projections show claim state instead of raw logs or private payment
  details; and
- the team can run an end-to-end dry run from agent discovery through Site
  preview, payment challenge, receipt, and customer review without manual
  database surgery.

# OpenAgents Sites Work Traces

This folder records public-safe implementation traces and source artifacts for
Sites work that needs to be inspectable from OpenAgents proof surfaces.

- `2026-06-05-ben-otec-site-trace.md` records the first OTEC Site deployment
  trace.
- `2026-06-05-ben-otec-site-quality-postmortem.md` records the quality
  criticism and process changes required after reviewing the first OTEC Site.
- `2026-06-05-ben-otec-revision-readiness.md` records the queued Ben OTEC
  revision request, visual-asset guardrail, and live Autopilot/fallback
  procedure.
- `2026-06-05-customer-site-revision-feedback-api.md` records the first
  customer-visible Site revision and feedback API slice.
- `2026-06-05-customer-site-revision-review-ui.md` records the customer order
  revision review and feedback composer UI slice.
- `2026-06-05-oa-sites-vibesdk-gap-analysis.md` compares OA Sites with
  Cloudflare VibeSDK and sequences the work needed for VibeSDK-style builder
  parity on Cloudflare infrastructure.
- `2026-06-05-agent-sites-pylon-commerce-gap-audit.md` records the current gap
  between the operator-supervised Sites beta and the target where agents can
  create hosted Sites, use scoped local compute/Pylon setup instructions,
  preserve referrals, and pay for Site commerce or protected actions through
  credits or Lightning/MDK.
- `2026-06-05-agent-site-action-contract.md` records the issue #158 contract
  for agent Site actions, scopes, idempotency, save/deploy separation,
  payment/referral/Pylon boundaries, public proof, rollback, and the launch
  gate before public agent-deploy claims.
- `2026-06-05-pylon-v0-2-public-readiness-audit.md` records the issue #161
  source-backed decision that Pylon v0.2 is partially ready at the source
  contract level but blocked for broad public claims until release tags,
  platform assets, WSL/native Windows smokes, and current settlement proofs are
  retained.
- `2026-06-05-pylon-local-compute-instruction-packet.md` records the issue
  #162 guarded install, readiness, platform, referral, authority, and earning
  caveat instructions for agents and users who explicitly opt into Pylon/local
  compute.
- `2026-06-05-site-commerce-manifest-and-catalog-schema.md` records the issue
  #163 `.openagents/site.json` payments block, D1 product/action catalog
  schema, secret-material rejection rules, and money-state boundaries.
- `2026-06-05-hosted-checkout-and-l402-contracts.md` records the issue #164
  hosted checkout intent, L402 challenge, and L402 redemption contract stubs
  for static/R2 Sites, WFP Sites, and generated agent clients.
- `2026-06-05-mdk-agent-wallet-sandbox-smoke-plan.md` records the issue #165
  internal MDK agent-wallet/pay402 sandbox/signet smoke plan before public MDK
  instructions are published.
- `2026-06-05-site-payment-referral-revshare-linkage.md` records the issue
  #166 linkage model for Site payment events, referral attribution,
  revenue-share projections, credit/sats boundaries, and Pylon receipt gates.
- `2026-06-05-site-editor-sidebar-shell.md` records the issue #167 first Site
  editor shell, responsive sidebar layout, bounded sidebar width value, and
  public-safe display boundary.
- `2026-06-05-site-editor-version-history.md` records the issue #168
  customer-safe Site editor version history panel, origin-summary projection,
  dedicated version links, and follow-up prefill action.
- `2026-06-05-site-editor-element-context.md` records the issue #169
  customer-safe targeted element context contract, sanitizer, sidebar inspect
  mode, and composer insertion behavior.
- `2026-06-05-site-editor-code-viewer.md` records the issue #170 read-only
  sidebar code viewer for selected element source context, redaction gates,
  metadata display, and copy affordance.
- `2026-06-05-site-preview-element-bridge.md` records the issue #171
  origin-scoped postMessage bridge for live Site preview element targeting.
- `2026-06-05-targeted-site-remake-outreach-roadmap.md` records the internal
  operator and future agent-toolkit plan for finding weak existing websites,
  capturing public source material, generating improved concept Sites, and
  sending reviewed outreach.
- `vida_referral_model_for_openagents.md` records third-party research on
  VIDA's usage-funded referral model as source material for OpenAgents
  revenue-share design.
- `2026-06-05-openagents-revenue-share-system.md` records the OpenAgents
  revenue-share plan for referral attribution, credit/sats asset boundaries,
  contributor splits, Sites paid actions, and future accepted-outcome
  marketplace payouts.
- `2026-06-05-site-referral-source-schema.md` records the first Site referral
  schema slice for public source refs and scoped invites.
- `2026-06-05-site-referral-capture.md` records the clean referral capture
  routes and pending attribution persistence slice.
- `2026-06-05-site-referral-cta-and-agent-manifest.md` records the public-safe
  Site CTA, public proof, generated metadata, and agent manifest referral link
  slice.
- `2026-06-05-site-referral-attribution-consumption.md` records the first
  verified attribution consumption slice for session materialization, customer
  orders, and the future agent-claim helper path.
- `2026-06-05-site-referral-onboarding-email.md` records the referred-user
  onboarding email and drip-enrollment hook after verified Site referral
  attribution consumption.
- `2026-06-05-site-referral-dashboard-inspection.md` records the owner
  aggregate referral overview and operator inspection endpoints for REF2
  accountability.
- `2026-06-05-site-referral-workflow-event-ledger.md` records the referral
  workflow event ledger for paid usage, Site checkout, L402 redemption,
  accepted outcomes, refunds, reversals, holds, and operator adjustments.
- `2026-06-05-site-referral-abuse-dispute-cap-policy.md` records the referral
  policy layer for abuse holds, disputes, caps, reversals, clawbacks, operator
  overrides, and public-safe eligibility projections.
- `2026-06-05-targeted-site-outreach-schema.md` records the first targeted
  Site remake/outreach campaign and prospect schema slice.
- `2026-06-05-targeted-site-discovery-planner.md` records the Exa-backed
  targeted Site prospect discovery planner and source-card contract.
- `2026-06-05-targeted-site-capture-policy.md` records the respectful capture
  policy gate for robots/sitemap checks, suppression, customer-owned domains,
  manual review, paid escalation, and fetchability before any capture worker
  fetches target pages.
- `2026-06-05-targeted-site-static-capture.md` records the first static
  capture run ledger for policy-gated homepage/page/asset refs, source-pack
  metadata, response summaries, and public/operator-safe projections.
- `2026-06-05-targeted-site-rendered-capture.md` records the Browser
  Run-style rendered capture ledger for screenshots, rendered source refs,
  markdown, links, crawl refs, bounded usage summaries, and redacted
  projections.
- `2026-06-05-targeted-site-capture-provider-adapter.md` records the
  provider-neutral capture adapter boundary for first-party Worker, Browser
  Run, Firecrawl, Browserless, Browserbase, Apify, and Container fallback or
  benchmark receipts.
- `2026-06-05-targeted-site-quality-audit.md` records the bounded website
  quality audit scoring contract for design age, mobile risk, SEO, CTA,
  trust, accessibility, performance, content quality, legal-sensitive claims,
  and evidence-linked recommendations.
- `2026-06-05-targeted-site-remake-brief.md` records the source authority
  pack and concept-generation constraint contract for targeted Site remake
  briefs.
- `2026-06-05-targeted-site-remake-preview-generation.md` records concept
  preview generation refs, candidate Site/version refs, and concept-domain
  guardrails for approved targeted remake briefs.
- `2026-06-05-targeted-site-operator-review.md` records the internal
  operator review decision ledger and UI-ready view model for targeted Site
  remake campaigns.
- `2026-06-05-targeted-remake-outreach-email.md` records the approved
  targeted-remake outreach email template, EmailService dispatch boundary, and
  dispatch ledger.
- `2026-06-05-targeted-site-campaign-metrics.md` records the campaign metrics
  and conversion ledger for targeted Site remake campaigns.
- `2026-06-05-targeted-site-agent-toolkit.md` records the private scoped
  agent toolkit grant/action ledger for user-owned targeted Site campaigns,
  including dry-run defaults, scopes, caps, approval gates, suppression, and
  public-safe projections.
- `2026-06-05-targeted-site-sales-reward-policy.md` records the accepted
  outcome reward policy ledger for targeted Site sales agents, including
  proposed leads, accepted meetings/customers, eligibility, payout intent,
  disputes, reversals, settlement caveats, and public-safe projections.
- `2026-06-05-sites-builder-session-ledger.md` records the VibeSDK-style
  Sites builder session, message, event, file, preview, and artifact ledger
  foundation.
- `2026-06-05-sites-builder-session-api.md` records the first
  customer/operator-safe Sites builder session create, read, message, and event
  API surface.
- `2026-06-05-sites-builder-event-stream.md` records the replayable
  Server-Sent Events stream contract for builder session progress events.
- `2026-06-05-sites-builder-file-apis.md` records the customer-safe generated
  file list, tree, read, and preview-manifest export API surface.
- `2026-06-05-sites-builder-preview-runner.md` records the first cost-tiered
  R2/static, Workers for Platforms, and gated Container preview selection
  contract for builder sessions.
- `2026-06-05-sites-builder-phase-timeline.md` records the durable builder
  phase-run timeline and customer-safe current-phase projection.
- `2026-06-05-sites-builder-repair-loop.md` records the bounded repair-attempt
  ledger, retry-budget guard, redaction policy, and event timeline linkage.
- `2026-06-05-sites-builder-saved-version-handoff.md` records the idempotent
  builder-session to saved `site_versions` handoff, operator save endpoint,
  metadata contract, and save/deploy separation.
- `2026-06-05-sites-wfp-deployment-attempts.md` records the Workers for
  Platforms deployment-attempt ledger, health-gated activation contract,
  bounded upload/rollback/observability refs, and remaining credentialed upload
  client gap.
- `2026-06-05-sites-provisioning-plan-contract.md` records the reviewed D1,
  R2, KV, plain-env, and secret-ref provisioning plan/receipt contract for
  generated Sites.
- `2026-06-05-sites-self-serve-builder-ui.md` records the customer order Site
  builder panel, public active-preview projection, event/file/read views, and
  revision-feedback queue integration.
- `2026-06-05-site-source-export-contract.md` records the reviewed Site source
  export receipt, GitHub destination, secret-scan, token-ref/hash, and expiry
  contract.
- `2026-06-05-site-library-visibility-archive-delete.md` records the first
  Site library list, visibility, archive, delete, owner/admin authority, and
  stale builder-session guardrail contract.
- `2026-06-06-runner-backend-schema-v1.md` records the shared SHC,
  Cloudflare Container, and future GCloud runner backend schema/projection
  boundary, including public/customer/operator redaction rules.
- `2026-06-06-runner-gateway-contract-v1.md` records the backend-neutral
  runner gateway request, callback, artifact, selection, adapter, and typed
  error contract for SHC, Cloudflare Containers, and future GCloud lanes.
- `2026-06-06-cloudflare-container-disabled-binding-plan.md` records the
  disabled-by-default Cloudflare Container binding/config readiness plan and
  the gates required before Container dispatch or failover can be considered.
- `2026-06-06-fake-cloudflare-container-runner.md` records the fake/staging
  Cloudflare Container runner adapter, deterministic lifecycle/artifact/cancel
  receipts, and redaction guarantees before live Container execution exists.
- `2026-06-06-runner-provider-secret-boundary.md` records the runner
  provider-account/GitHub/callback grant-ref boundary, resolution/scrub
  receipt contract, denial reasons, and public-safe projection rules.
- `2026-06-06-runner-backend-health-projection.md` records operator-safe
  runner backend availability, gate, queue, cold-start, cost, billing,
  capacity, and diagnostic projections with public/customer redaction.
- `2026-06-06-real-cloudflare-container-runner-adapter.md` records the
  real Cloudflare Container runner adapter contract, blocked-gate behavior,
  injected control-plane boundary, and no-live-execution non-goals.
- `2026-06-06-cloudflare-container-runner-lifecycle-manifest.md` records the
  Container runner image/workspace lifecycle manifest, projection rules,
  readiness derivation, and artifact-manifest conformance bridge.
- `2026-06-06-cloudflare-container-closeout-receipts.md` records Container
  lifecycle callback phases, artifact closeout receipts, scrub evidence,
  projection splits, and gateway callback/artifact integration.
- `2026-06-06-cloudflare-container-failover-rollout-policy.md` records the
  operator-selected Container failover rollout policy, blocked gates, safe
  receipt refs, SHC-primary default, GCloud reference behavior, and
  sensitive-work denial.
- `2026-06-06-payment-limit-policy-classifier.md` records the typed
  recoverable/non-recoverable limit classifier for credits, Lightning/MDK, L402,
  free-beta allowance, safety, abuse, private authority, provider capacity, and
  manual-review gates.
- `2026-06-06-paid-endpoint-product-catalog.md` records the stable paid
  endpoint/action product catalog contract for agent APIs, Forum paid actions,
  Site checkout/actions, runner recovery, entitlement scopes, spend-cap hints,
  and projection redaction.
- `2026-06-06-buyer-side-payment-ledger.md` records the replay-safe
  buyer-side payment ledger for challenges, redemptions, entitlements,
  spend limits, credit debits, receipts, reconciliation events, and redacted
  projections.
- `2026-06-07-l402-deferred-settlement-middleware.md` records the L402
  deferred-settlement contract for protected handlers that should consume
  payment credentials only after success, artifact receipt, response closeout,
  or operator review.
- `2026-06-07-buyer-payment-entitlement-policy.md` records the buyer payment
  entitlement policy contract for one-shot, time-window, quota, resource,
  actor, route, Site, and hybrid entitlement decisions.
- `2026-06-07-unified-payment-decision-policy.md` records the unified payment
  decision contract for free-beta allowance, account credits, Stripe top-up
  state, L402/MDK proof, existing entitlements, manual review, and unavailable
  provider state.
- `2026-06-07-agent-spend-cap-preview.md` records the dry-run spend-cap
  preview contract for agent-paid routes, generated Site actions, supported
  rails, idempotency guidance, and public-safe over-budget results without
  creating invoices, debiting credits, granting entitlements, calling MDK, or
  mutating payout state.
- `2026-06-07-site-mdk-reconciliation-worker.md` records the scheduled and
  queue-safe Site MDK reconciliation worker plan for stale checkouts, expired
  challenges, provider status checks, duplicate/replayed events, receipt repair,
  entitlement repair, retry/backoff metadata, and redacted public/operator
  projections.
- `2026-06-07-mdk-agent-wallet-pay402-smoke.md` records the MDK agent-wallet
  and pay402-compatible L402 smoke runbook for no-spend fixture plans,
  signet/sandbox wallet setup, unpaid challenge, bounded payment, paid retry,
  token-cache handling, and redaction rules.
- `2026-06-07-payment-destination-input-parser.md` records the issue #451
  typed payment destination classifier for BOLT11, BOLT12, LNURL, Lightning
  Address, BIP353-style names, `bitcoin:` URI payloads,
  malformed/ambiguous inputs, and the decision to keep MDK resolver behavior
  behind a future WASM/sidecar boundary.
- `2026-06-06-worker-compatible-l402-credential-service.md` records the
  Worker-compatible L402 credential payload, HMAC signing boundary, verification
  result states, redaction rules, and type-level integration with payment
  policy, catalog, and ledger contracts.
- `2026-06-06-l402-response-error-contract.md` records the shared 402, 401, and
  403 L402/payment response contract for recoverable payment challenges,
  credential failures, auth/scope failures, non-payable policy states, and
  redaction.
- `2026-06-06-l402-payment-headers.md` records the collision-safe L402 header
  contract for `WWW-Authenticate`, `Authorization: L402`, legacy `LSAT`, and
  bearer auth plus `X-OpenAgents-L402`.
- `2026-06-06-hosted-mdk-client-contract.md` records the hosted MoneyDevKit
  client boundary, fake provider, redacted refs, config gate, and
  buyer-payment-evidence-only authority split.
- `2026-06-06-mdk-core-checkout-worker-contract.md` records the Effect/Worker
  port of MDK core checkout semantics for route selection, metadata validation,
  customer normalization, safe checkout paths, signed checkout URLs, and
  hosted-client/L402 bridge schemas.
- `2026-06-06-mdk-core-conformance-fixtures.md` records the MDK-derived
  conformance fixture catalog for amount/product checkout, metadata,
  customer normalization, signed URLs, safe paths, sandbox flags, L402
  verification, stale challenges, and safe error envelopes.
- `2026-06-06-site-payment-manifest-schema.md` records the generated Site
  `.openagents/site.json` payments schema for hosted MDK products and paid
  actions, including supported prices, clean checkout paths, sandbox flags,
  projections, and redaction rules.
- `2026-06-06-site-payment-product-action-catalog.md` records the versioned
  Site payment catalog for generated checkout products and paid actions,
  D1 persistence, audience projections, paid-endpoint conversion, and hosted
  checkout plan typing.
- `2026-06-06-hosted-site-checkout-intent-api.md` records the catalog-backed
  hosted Site checkout intent API contract, fake hosted MDK provider path,
  buyer-payment challenge typing, deterministic idempotency, and response
  redaction boundary.
- `2026-06-06-generated-checkout-ui-primitives.md` records the source-safe
  generated checkout UI primitive contracts for buttons, forms, product cards,
  paid action prompts, tip/deposit/subscription affordances, success/cancel
  states, entitlement states, agent metadata, and query-state rejection.
- `2026-06-06-wfp-site-payment-middleware.md` records the generated WFP Site
  payment middleware contract for protected paid actions, payment-required
  L402 headers, entitlement-required state, allow/block projections, and
  redaction boundaries.
- `2026-06-06-agent-readable-site-payment-discovery.md` records the public-safe
  Site payment discovery projection for agents, including checkout products,
  paid actions, endpoints, spend-cap hints, entitlement semantics, and
  live/fake-provider/planned surface states.
- `2026-06-06-clean-checkout-return-entitlement-projection.md` records the
  clean checkout return and entitlement projection contract for generated
  Sites, including success/cancel/status paths, receipt/entitlement state,
  and checkout query-state rejection.
- `2026-06-06-site-mdk-reconciliation-webhook-bridge.md` records the fake
  provider/config-gated Site MDK reconciliation bridge for normalized
  buyer-payment reconciliation events, replay handling, safe operator refs,
  and no provider secret or payout-authority exposure.
- `2026-06-07-site-payment-to-payout-bridge.md` records the issue #437
  operator-authorized bridge from verified Site buyer payment receipts and MDK
  reconciliation events to Nexus/Treasury payout intents, including duplicate
  buyer-payment guards and no checkout-return authority.
- `2026-06-07-site-payment-proof.md` records the issue #439 public-safe
  buyer-side Site payment proof route for checkout intent, receipt,
  reconciliation, and entitlement state without exposing raw payment material
  or claiming payout settlement.
- `2026-06-07-site-commerce-review.md` records the issue #440 Site commerce
  review projection, source-safe checkout UI primitive refs, review decision
  route, and the boundary that review state does not create payment, payout,
  settlement, access, or deployment authority.
- `2026-06-07-customer-owned-mdk-account-mode.md` records the issue #441
  customer-owned MDK account binding mode, operator-gated hosted secret-ref
  writes, customer-safe configured/pending/blocked/revoked projections, and
  the boundary that binding state does not create checkout, live-spend, payout,
  settlement, access, or deployment authority.
- `2026-06-07-site-mdk-sandbox-smoke-tests.md` records the issue #442
  fake-provider Site MDK smoke helper and route tests for discovery, checkout,
  clean return status, webhook reconciliation, replay handling, payment proof,
  L402 challenge/redemption, stale rejection, spend-cap rejection, redaction,
  and fake/sandbox/live implementation-state classification.
- `2026-06-07-generated-site-payment-smoke-runbook.md` records the issue #458
  public-safe evidence map for the generated-Site payment smoke batch,
  distinguishing deterministic fake-provider proof, hosted-provider proof,
  real bitcoin movement proof, and accepted-work payout settlement evidence.
- `2026-06-07-mdk-core-backed-site-helpers.md` records the issue #443
  generated static/WFP Site helper contract for payment discovery, checkout
  intents, checkout returns, payment proofs, L402 challenges/redemptions,
  redacted error envelopes, idempotency guidance, clean URL rules, spend caps,
  and MDK core conformance fixture parity without generated-Site MDK runtime
  imports or credentials.
- `2026-06-07-site-payment-primitive-sdk.md` records the issue #444 concise
  SDK-style Site payment primitive reference for discovery, checkout products,
  paid actions, clean returns, webhook reconciliation, payment proofs,
  payment-to-payout bridge boundaries, L402 challenge/redemption flows,
  generated helper usage, customer-owned MDK account mode, production config,
  smoke tests, and payment/entitlement/accepted-work/payout/settlement
  evidence separation.
- `otec/index.html` is the first static artifact source for
  `https://sites.openagents.com/otec`.

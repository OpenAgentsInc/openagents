# Omni Workroom Notes

This folder records OpenAgents product surface implementation notes for the Omni workroom,
accepted-outcome, evidence, briefing, and economics layers.

- `2026-06-05-accepted-outcome-contract-v1.md` records the issue #209
  accepted outcome contract model for Sites, coding, adjustments,
  existing-project imports, business work, and legal-sensitive work.
- `2026-06-05-workroom-records-v1.md` records the issue #210 Omni workroom
  record model that links orders, optional Sites, optional assignments,
  accepted outcome contracts, task packets, artifacts, emails, receipts,
  blockers, and projection splits.
- `2026-06-05-workroom-evidence-bundles-v1.md` records the issue #211 Omni
  evidence bundle model for Exa cards, research briefs, source commits,
  generated source, logs, screenshots, deployments, diffs, tests, emails,
  receipts, redaction reports, source-authority caveats, and projection splits.
- `2026-06-05-workroom-lifecycle-v1.md` records the issue #212 Omni lifecycle
  decision ledger for acceptance, rejection, provisional acceptance, reopening,
  revision requests, unavailable outcomes, customer-safe explanations, and
  explicit no-settlement implication.
- `2026-06-05-mission-briefing-v1.md` records the issue #213 Mission Briefing
  projection for changed, built, blocked, review, email, and next-action
  sections with friendly time labels and customer-safe refs.
- `2026-06-05-accepted-outcome-economics-v1.md` records the issue #214
  internal accepted outcome economics ledger for free-beta, credit-funded,
  bitcoin-funded, and internal-only work without creating settlement or payout
  claims.
- `2026-06-05-route-scorecard-v1.md` records the issue #215 route scorecard
  ledger for selected routes, rejected candidates, decision reasons, observed
  results, costs, latency, privacy, trust, and projection splits.
- `2026-06-05-public-proof-bundle-v1.md` records the issue #216 public-safe
  proof bundle ledger for source, artifact, receipt, review, acceptance,
  economics, legal, privacy, and no-settlement caveats.
- `2026-06-05-workroom-kind-templates-v1.md` records the issue #217 static
  workroom kind template policy for Sites, coding, CRM, investor ops, project
  ops, support, finance ops, meeting, document, and legal-review work.
- `2026-06-05-market-memory-hooks-v1.md` records the issue #218 evidence-only
  market memory hook ledger for accepted/rejected outcomes, route quality,
  account reliability, repo conventions, source quality, module usefulness, and
  marketplace attribution.
- `2026-06-05-data-classification-and-trust-v1.md` records the issue #219
  workroom data classification and trust tier model for public, customer, team,
  operator, private, legal-sensitive, provider-private, payment-private, and
  secret-bearing boundaries.
- `2026-06-05-workroom-surface-projections-v1.md` records the issue #220
  aggregate public, customer, team, agent, and operator projection split for
  workrooms, evidence bundles, lifecycle decisions, economics, route
  scorecards, and classification gates.
- `2026-06-06-crm-follow-up-workroom-template.md` records the issue #341 CRM
  follow-up workroom template contract for contact/company/source refs, prep
  packets, draft messages, approval, email receipts, closeout, and
  relationship memory without email-send or CRM-mutation authority.
- `2026-06-06-investor-ops-workroom-template.md` records the issue #342
  investor ops workroom template contract for investor/contact refs, prep
  packets, data-room tasks, deck/video work orders, follow-up queues, decision
  receipts, and accepted outcome refs without outreach, publication, upload, or
  investor-record mutation authority.
- `2026-06-06-support-project-ops-workroom-template.md` records the issue #343
  support and project ops workroom template contracts for customer/ticket refs,
  issue timelines, proposed responses, escalations, project tasks, decisions,
  risks, status reports, receipts, and closeout without support-send,
  customer-record mutation, project-management mutation, external escalation,
  or accepted-outcome mutation authority.
- `2026-06-06-legal-safe-hold-workroom-template.md` records the issue #344
  legal safe-hold workroom template contract for client, matter, jurisdiction,
  source, scoping, legal-review, hold, release, decline, closeout, evidence,
  blocker, caveat, and diagnostic refs without automatic execution, external
  send, filing, legal advice claims, payment settlement, or public projection
  upgrade authority.
- `2026-06-06-signature-package-validation-api.md` records the issue #345
  read-only developer signature package validation API for schema, fixture,
  risk class, evidence, receipt, selector metadata, and json-render binding
  checks without package install, runtime promotion, marketplace listing,
  deployment, or payment mutation authority.
- `2026-06-06-workroom-template-package-model.md` records the issue #346
  workroom template package model for versioned outcome templates, required
  artifacts, approval policies, runner needs, UI bindings, proof rules,
  validation, review, org-private enablement, public projection, and runtime
  promotion request refs without runtime promotion, marketplace listing,
  external runner launch, deployment, or payment mutation authority.
- `2026-06-06-program-run-receipt-webhook-subscriptions.md` records the issue
  #347 focused Program Run and receipt webhook subscription contract for
  subscriber refs, event topic refs, scoped auth refs, delivery preparation and
  attempt refs, retry state, replay windows, redaction policy refs, receipt
  refs, blocker/caveat refs, and revocation state without external webhook
  calls, delivery queue enqueue, Program Run mutation, receipt mutation,
  payment mutation, auth escalation, or secret material authority.
- `2026-06-06-omni-api-sdk-seed.md` records the issue #348 public
  `GET /api/omni/sdk-seed` discovery route and SDK seed for workrooms,
  accepted outcomes, Program Runs, receipts, proof bundles, billing/payment
  projections, and webhooks, including route access classification without
  granting mutation, payment, deployment, or webhook delivery authority.
- `2026-06-06-investor-grade-outcome-economics-metrics.md` records the issue
  #361 investor-grade outcome economics metrics projection for accepted
  revenue, gross profit, retry/review/grading costs, provider payable and
  settled states, refund exposure, work-class margin, audience redaction, and
  no-mutation authority.
- `2026-06-06-accepted-outcomes-per-power.md` records the issue #363
  accepted outcomes per power projection for accepted revenue, accepted gross
  profit, provider payable, dark-capacity MWh, accepted outcomes per kWh/MWh,
  modeled versus measured power evidence, settlement-state labels, audience
  redaction, and no-mutation authority.
- `2026-06-06-margot-export-ingestion-contract.md` records the issue #367
  Margot export ingestion contract for mining floor, GPU rental floor, token
  inference floor, node/system-power-adjusted AI floor, accepted-work
  assumptions, grid-service and curtailment value assumptions, market and
  dispatch-policy labels, provenance, caveats, next diligence, settlement refs,
  audience redaction, and no accepted-work, financial-advice,
  grid-participation, wallet-spend, market-data, public-claim, or settlement
  mutation authority. Follow-up Artanis/Pylon comparative-economics evidence
  collection is implemented in
  `../artanis/2026-06-06-comparative-economics-evidence-packets.md` for issue
  #415.
- `2026-06-06-investor-demo-bundle-export.md` records the issue #368
  investor demo bundle export contract for proof bundle summaries, route
  scorecards, investor economics, capacity funnel accounting, accepted
  outcomes per power, settlement labels, missing-evidence sections, audience
  redaction, and no download-route, investor-share, wallet-spend, public-claim,
  raw-data-copy, or settlement mutation authority.
- `2026-06-06-knowledge-source-bundle-and-span-model.md` records the issue
  #369 knowledge source bundle and extracted span model for connector reads,
  data packages, files, links, repo refs, tables, transcripts, page/row/table
  cell/transcript/code/file spans, provenance, digests, rights, generated
  summary separation, public/team/operator redaction, and no connector,
  generated-summary, public-claim, raw-source-archive, or rights mutation
  authority.
- `2026-06-06-retrieval-trace-and-graph-context.md` records the issue #370
  retrieval trace and graph-curated context contract for selected and excluded
  sources, ranking, score, freshness, missing context, graph nodes/edges,
  human-confirmed facts, public/team/operator redaction, and no autonomous
  source fetch, fact-promotion, generated-summary, graph, or public-claim
  mutation authority.
- `2026-06-06-data-package-export-rights-manifest.md` records the issue #371
  data package export and rights manifest contract for artifact digests, schema
  refs, rights policy, redaction summary, provenance manifest, receipt refs,
  package-ready/reviewed/published/revoked states, public/team/operator
  redaction, and no download, file-hosting, wallet-spend, public-claim,
  receipt, or rights mutation authority.
- `2026-06-06-mobile-workroom-approval-cards.md` records the issue #372
  mobile workroom projection and approval-card contract for compact status,
  CRM sends, coding writes, runner launches, payments, provider actions,
  public claims, legal-sensitive actions, risk/evidence requirements, expiry
  labels, public/agent redaction, and no approval, execution, notification,
  payment, provider, runner, or public-claim mutation authority.
- `2026-06-06-voice-session-evidence-records.md` records the issue #373
  voice session evidence contract for provider refs, capture state,
  transcript segment refs, source refs, command route proposals, confidence,
  approval and execution receipt refs, proposal state validation, public
  redaction, and no audio capture, transcript, proposal, approval, command,
  payment, provider, or public-claim mutation authority.
- `2026-06-06-domain-agent-package-lifecycle.md` records the issue #374
  domain agent package lifecycle contract for draft, fixture validation,
  review, org-private enablement, public projection, runtime promotion,
  rollback posture, marketplace attribution, audience redaction, and no
  fixture execution, review, enablement, public projection, runtime promotion,
  marketplace listing, payment, or rollback mutation authority.
- `2026-06-06-marketplace-margin-memory.md` records the issue #375 expanded
  marketplace margin memory contract for accepted outcomes, revenue, gross
  profit, provider payable/settled values, acceptance rate, gross margin,
  review burden, refund rate, repeat buyer signal, settlement labels, audience
  redaction, and no public-rank, module-promotion, payout, routing, or
  settlement mutation authority.
- `2026-06-06-model-lab-retained-failure-loop.md` records the issue #376
  Model Lab retained-failure loop contract for retained failures,
  signature/model candidates, eval reruns, adapter validation evidence,
  promotion gates, rollback posture, attribution, audience redaction, and no
  eval, training, adapter-install, runtime-promotion, routing, payout,
  settlement, or public-claim mutation authority.
- `2026-06-06-model-lab-model-artifact-contract.md` records the issue #380
  Model Lab model artifact contract for artifact identity, digest evidence,
  storage refs, provider refs, source/training/eval/benchmark refs, rights
  caveats, safety redaction, readiness, rollback posture, audience redaction,
  and no training, adapter-install, raw-weight-copy, runtime-promotion,
  routing, payout, settlement, or public-claim mutation authority.
- `2026-06-06-model-lab-training-run-contract.md` records the issue #381
  Model Lab training run contract for observed/imported training, adapter,
  eval-only, optimizer, distillation, benchmark replay, and data-preparation
  evidence with source/evidence refs, data package refs, model artifact refs,
  metrics, hyperparameter summaries, budget/cost caveats, operator review
  receipts, audience redaction, and no training launch, provider mutation,
  adapter-install, raw-dataset-copy, payment-spend, runtime-promotion, routing,
  payout, settlement, or public-claim mutation authority.
- `2026-06-06-model-lab-evidence-graph.md` records the issue #382 Model Lab
  evidence graph for retained failures, candidates, training runs, model
  artifacts, eval reruns, adapter validations, promotion gates, same-loop
  linkage, missing-node, duplicate, cycle, stale-evidence, rollback-posture,
  audience-redaction, and no eval, training, provider-call, adapter-install,
  payment-spend, runtime-promotion, routing, payout, settlement, or
  public-claim mutation authority.
- `2026-06-06-benchmark-cloud-evidence-contract.md` records the issue #383
  Benchmark Cloud evidence contract for suites, tasks, eval jobs, scorecards,
  regressions, flakes, comparisons, promotion-blocking failures, audience
  redaction, and no benchmark launch, eval execution, provider mutation,
  raw-input copy, payment-spend, runtime-promotion, routing, payout,
  settlement, or public-claim mutation authority.
- `2026-06-06-model-lab-promotion-decision-ledger.md` records the issue #384
  Model Lab promotion decision ledger for reviewed artifact, training-run,
  candidate, adapter, and route decisions with release gates, reviewer
  receipts, eval and Benchmark Cloud evidence, risk labels, rollback posture,
  marketplace memory, outcome attribution, audience redaction, and no runtime
  promotion, model deployment, adapter install, route mutation, rollback
  execution, provider mutation, marketplace-rank mutation, payment, payout,
  settlement, or public-claim mutation authority.
- `2026-06-06-model-lab-public-report-projection.md` records the issue #385
  public-safe Model Lab report projection for retained failures, candidates,
  model artifacts, training runs, Benchmark Cloud evidence, promotion
  decisions, rollback, attribution, marketplace memory, readiness, missing
  evidence, claim state, redaction summary, public proof, investor demos, and
  agent inspection without training, eval, provider, adapter, raw-artifact,
  report-publication, payment, runtime-promotion, payout, settlement, or
  public-claim mutation authority.
- `2026-06-06-cloudflare-artifacts-git-agent-audit.md` evaluates Cloudflare
  Artifacts as OpenAgents product surface-owned Git-compatible workspace storage for local, SHC, and
  future Cloudflare Container agents. It recommends using Artifacts for
  internal/public per-mission Git workspaces, patch/diff closeout, baseline
  forks, and PR-less accepted outcomes while keeping GitHub writeback,
  customer private repo access, authority receipts, R2 blob artifacts, and
  product acceptance as separate gates.

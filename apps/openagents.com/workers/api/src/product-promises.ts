import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const PublicProductPromisesEndpoint = '/api/public/product-promises'
export const PublicProductPromisesSchemaVersion =
  'openagents.product_promises.v1'
export const PublicProductPromisesVersion = '2026-07-04.3'

const reportPath = 'https://openagents.com/forum/f/product-promises'

const sourceRefs = [
  'https://github.com/OpenAgentsInc/openagents',
  'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com',
  'https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon',
  'https://github.com/OpenAgentsInc/openagents/tree/main/packages/probe',
  'apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md',
  'apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md',
  'apps/openagents.com/docs/2026-06-08-openagents-public-launch-dashboard.md',
  'apps/openagents.com/docs/2026-06-08-public-launch-copy-gate.md',
  'apps/pylon/docs/launch-gates-no-overclaim.md',
  'docs/promises/2026-06-09-product-promises-gap-audit.md',
  'docs/2026-06-12-episode-236-training-launch-gap-audit.md',
  'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
  'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
  'docs/training/2026-06-20-cs336-a2-thermal-throttle-classifier.md',
  'docs/training/2026-06-28-cs336-a2-continuous-thermal-throttle-receipts.md',
  'docs/training/2026-06-20-ablation-eval-reproduction-receipt.md',
  'docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md',
  'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
  'apps/pylon/src/tassadar-cpu-transform-training.ts',
  'apps/pylon/tests/tassadar-cpu-transform-training.test.ts',
  'docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md',
  'packages/tassadar-executor/src/kernel-optimization-parity.ts',
  'packages/tassadar-executor/src/kernel-optimization-dispatch.ts',
  'packages/tassadar-executor/src/kernel-optimization-parity.test.ts',
  'packages/tassadar-executor/src/kernel-optimization-dispatch.test.ts',
  'docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md',
  'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
  'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
  'docs/training/2026-06-20-training-full-pipeline-program-status.md',
  'docs/launch/vertex-fleet/training.public_distributed_training_run.v1.md',
  'docs/launch/vertex-fleet/pylon.largest_decentralized_training_claim.v1.md',
  'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
  'docs/launch/vertex-fleet/training.public_gradient_windows.v1.md',
  'docs/training/2026-06-20-cs336-a2-same-class-replication-status.md',
  'docs/launch/vertex-fleet/training.data_refinery_corpus.v1.md',
  'docs/2026-06-10-cs336-distributed-homework-continuation-audit.md',
  'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
  'docs/refactor/path-to-bolt-12.md',
  'docs/transcripts/199.md',
  'docs/transcripts/217.md',
  'docs/transcripts/236.md',
  'docs/transcripts/242.md',
  'docs/transcripts/243.md',
  'docs/transcripts/244.md',
  'docs/transcripts/245.md',
  'docs/transcripts/225.md',
  'docs/transcripts/227.md',
  'docs/transcripts/228.md',
  'docs/transcripts/230.md',
  'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
  'docs/khala-code/2026-07-01-codex-required-product-positioning.md',
  'docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md',
  'docs/khala/2026-06-30-khala-code-desktop-redaction.md',
  'docs/mobile/2026-06-26-autopilot-remote-control-retirement.md',
  'docs/inference/2026-06-25-khala-inference-gtm-push.md',
  'docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md',
  'docs/afteraction/2026-06-26-khala-pylon-codex-delegation-afteraction.md',
  'docs/traces/2026-06-27-pylon-codex-live-trace-status-audit.md',
  'docs/khala-cli/README.md',
  'clients/khala-cli/README.md',
  'docs/research/terminal-agents/openagents-current-state.md',
  'docs/research/terminal-agents/codex.md',
  'apps/pylon/scripts/codex-supervisor/replenishment.sh',
  'apps/pylon/scripts/codex-supervisor/replenishment.test.sh',
  'apps/openagents.com/workers/api/src/inference/model-router.ts',
  'apps/openagents.com/workers/api/src/inference/model-router.test.ts',
  'apps/openagents.com/workers/api/src/operator-fleet-status-routes.ts',
  'apps/openagents.com/workers/api/src/operator-fleet-status-routes.test.ts',
  'apps/openagents.com/apps/web/src/page/loggedOut/page/publicAgent.ts',
  'apps/openagents.com/apps/web/src/page/loggedOut/page/publicAgent.story.test.ts',
  'docs/apple-fm/2026-06-29-electrobun-apple-fm-swift-sidecar-plan.md',
  'docs/launch/vertex-fleet/autopilot.local_apple_fm_tool_chat.v1.md',
  'clients/khala-code-desktop/src/bun/apple-fm-sidecar.ts',
  'clients/khala-code-desktop/src/shared/apple-fm-packaging.ts',
  'clients/khala-code-desktop/src/shared/apple-fm-readiness.ts',
  'clients/khala-code-desktop/tests/apple-fm-packaging.test.ts',
  'clients/khala-code-desktop/tests/apple-fm-readiness.test.ts',
  'clients/khala-code-desktop/tests/apple-fm-sidecar.test.ts',
  'apps/openagents-world/src/commands.ts',
  'apps/openagents-world/src/commands.test.ts',
  'apps/openagents-world/src/protocol.ts',
  'apps/openagents-world/src/protocol.test.ts',
  'apps/openagents-world/src/subscriptions.ts',
  'apps/openagents-world/src/subscriptions.test.ts',
  'packages/world-client/src/index.ts',
  'packages/world-client/src/index.test.ts',
  'packages/world-contract/src/index.ts',
  'packages/world-contract/src/index.test.ts',
  'docs/promises/2026-06-14-registry-reality-reconciliation-audit.md',
  'docs/promises/2026-06-17-training-monday-simulation-settlement-policy.md',
  'docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md',
  'docs/promises/2026-06-29-world-first-claims-7027-audit.md',
  'docs/launch/2026-06-19-autostream-settlement-visibility-capture.md',
  'docs/launch/2026-06-19-autostream-settlement-clip-manifest.json',
  'docs/launch/2026-06-18-autopilot-desktop-availability-audit.md',
  'docs/launch/2026-06-18-autopilot-desktop-ao6-from-dmg-runbook.md',
  'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
  'docs/labor/2026-06-14-p5-backlog-faucet-closeout.md',
  'docs/labor/2026-06-14-p7-lane-c-fanout-closeout.md',
  'docs/autopilot-coder/2026-06-13-autopilot-clients-roadmap.md',
  'docs/apple-fm/2026-06-15-current-apple-fm-electrobun-desktop-audit.md',
  'apps/pylon/docs/proofs/m10-live-2026-06-14/README.md',
  'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
  'docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md',
  'docs/research/machine-studying/2026-06-17-blueprint-marketplace-ties.md',
  'docs/research/machine-studying/openagents-studybench/runs/2026-06-17-mvp-14-baseline-packet-gepa-comparison.md',
  'docs/research/machine-studying/openagents-studybench/study-packets/openagents-launch-study-packet-v0.md',
  'packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json',
  'packages/probe/packages/runtime/src/benchmark/external-repo-studying-product.ts',
  'packages/probe/packages/runtime/tests/external-repo-studying-product.test.ts',
  'docs/inference/README.md',
  'apps/pylon/src/psionic-vllm-proxy.ts',
  'apps/pylon/tests/psionic-vllm-proxy.test.ts',
  'docs/inference/2026-06-19-inference-gateway-business.md',
  'docs/inference/2026-06-19-fireworks-provider.md',
  'docs/inference/2026-06-19-pricing-vs-factory.md',
  'docs/inference/2026-06-19-pricing-model.md',
  'docs/inference/2026-06-19-decentralized-serving-shard-wan.md',
  'docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md',
  'docs/transcripts/239.md',
  'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
  'docs/launch/2026-06-19-credits-purchase-collect-money-audit.md',
  'docs/launch/2026-06-19-near-term-product-priorities.md',
  'docs/promises/2026-06-19-weekend-assault-tail-domains.md',
  'apps/openagents.com/docs/labor/2026-06-19-agentic-labor-product-flow-scaffold.md',
  'docs/business/2026-06-20-openagents-business-intake-spec.md',
  'docs/business/2026-06-20-business-offering-promise-coverage.md',
  'docs/fable/2026-07-02-qa-swarm-product-plan.md',
  'docs/feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md',
  'docs/unit/2026-06-30-arbiter-effect-2d-dataflow-graph-audit.md',
  'NEEDS_OWNER.md',
]

const basePromiseFields = {
  reportPath,
  sourceRefs,
}

const summarizePromiseVerification = (
  promises: ReadonlyArray<{
    blockerRefs: ReadonlyArray<string>
    evidenceRefs: ReadonlyArray<string>
    promiseId: string
    state: string
  }>,
) => {
  const blockedPromises = promises.filter(promise =>
    ['degraded', 'planned', 'red', 'yellow'].includes(promise.state),
  )
  const uniqueBlockers = [
    ...new Set(promises.flatMap(promise => promise.blockerRefs)),
  ].sort()

  return {
    blockedPromiseCount: blockedPromises.length,
    evidenceRefCount: promises.reduce(
      (count, promise) => count + promise.evidenceRefs.length,
      0,
    ),
    promiseCount: promises.length,
    promisesWithBlockersCount: promises.filter(
      promise => promise.blockerRefs.length > 0,
    ).length,
    topBlockedPromises: blockedPromises.slice(0, 10).map(promise => ({
      blockerRefs: promise.blockerRefs,
      promiseId: promise.promiseId,
      state: promise.state,
    })),
    uniqueBlockerCount: uniqueBlockers.length,
    uniqueBlockers: uniqueBlockers.slice(0, 20),
  }
}

export const publicProductPromisesDocument = () => {
  const staleness = liveAtReadStaleness([
    'product_promise_registry_changed',
    'product_promise_transition_receipt_recorded',
    'product_promise_announcement_preflight',
  ])
  const document = {
    schemaVersion: PublicProductPromisesSchemaVersion,
    version: PublicProductPromisesVersion,
    registryVersion: PublicProductPromisesVersion,
    generatedAt: currentIsoTimestamp(),
    maxStalenessSeconds: staleness.maxStalenessSeconds,
    staleness,
    lastUpdated: '2026-07-04',
    canonicalDocsUrl:
      'https://github.com/OpenAgentsInc/openagents/tree/main/docs/promises',
    sourceRefs,
    publicDocsUrl: 'https://openagents.com/docs/product-promises',
    latestGapAuditUrl:
      'https://github.com/OpenAgentsInc/openagents/blob/main/docs/promises/2026-06-14-registry-reality-reconciliation-audit.md',
    reportPath: {
      defaultForumUrl: reportPath,
      forumSlug: 'product-promises',
      forumTopicApi:
        'https://openagents.com/api/forum/forums/product-promises/topics',
      strictBugForm:
        'https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml',
      rule: 'Use the Forum for product-promise gaps, stale copy, feature commentary, and discussion. Use GitHub only for concrete reproducible bugs that satisfy the strict issue form.',
    },
    states: {
      green:
        'Live for the scoped claim with current evidence and matching authority.',
      yellow:
        'Partially live, manually gated, limited to a specific path, or needs explicit caveats.',
      red: 'Blocked for affirmative public copy until evidence, authority, or safety gates pass.',
      degraded:
        'Previously green or yellow but freshness, health, evidence, or authority is currently weaker.',
      planned:
        'Roadmap or contract language only. Do not treat as live capability.',
      withdrawn:
        'Historical source material only. Do not use as current product copy.',
    },
    currentMonorepoStatus: {
      status: 'work_in_progress',
      summary:
        'The openagents monorepo now contains the deployed openagents.com Worker/app, Forum surfaces, docs/promises, packages/probe, apps/pylon, the apps/autopilot-desktop GUI shell, the clients/khala-desktop Electrobun shell, and the clients/khala-ios/AutopilotRemoteControl spec lane. The live Cloudflare deployment is served from apps/openagents.com, and the public code map in this registry points agents to the public source trees behind those shipped surfaces. As of 2026-06-14: the agent labor market crossed its first end-to-end milestone — one real backlog issue was posted, negotiated over NIP-90, escrowed, executed on an independent provider Pylon, validator-accepted, and settled with public receipts (#4777), so labor.forum_work_requests.v1 and labor.nostr_negotiation_market.v1 are green and provider.compliant_usage_labor.v1 / autopilot.control_center_fanout_marketplace.v1 are yellow. A large wave-3 Autopilot Sites / Agency Pack buildout (#4977-#4995) added client-delivery workrooms, native email sequences, custom tenant hostnames, partner-payout ledger, voice evidence, and a credits UI as operator-gated infrastructure — entered here as new conservative promise records. As of 2026-06-19, @openagentsinc/pylon@latest resolves to the v1.0 line and a first auto-stream settlement sequence is captured in public timeline/replay evidence; public product copy must still distinguish install capability and a single bounded visibility capture from broad live-network earning, paid-at-scale assignment, every-platform support, and unbounded settlement authority.',
      liveDeploymentRefs: [
        'https://openagents.com',
        'https://openagents.com/docs/product-promises',
        'https://openagents.com/api/public/product-promises',
        'https://openagents.com/api/public/launch-dashboard',
        'https://openagents.com/api/public/pylon-stats',
      ],
      pylonV03Refs: [
        'apps/pylon/package.json',
        'apps/pylon/README.md',
        'apps/pylon/docs/live-worker-loop-smoke.md',
        'apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md',
        'apps/pylon/docs/launch-gates-no-overclaim.md',
      ],
      caveats: [
        'Pylon v1.0 has a stable source cut, and npm reports @openagentsinc/pylon@latest on the v1.0 line. Live deployment, signed-binary feed rollout, and earning/settlement claims remain separate gates.',
        'macOS and Linux are the first supported operator platforms for the v1.0 launch path.',
        'Pylon v1.0 local release gates and a no-spend live worker-loop smoke exist, but broad earning, paid assignment, every-platform coverage, and settlement claims still need fresh public evidence before they go green.',
        'Training, data revenue, provider-capacity resale, referral payouts, and signature revenue remain gated or planned. Broad local-inference resale and Qwen fine-tune products remain out of scope by owner decision (2026-06-10), but registry 2026-06-15.11 retains the explicit yellow exception for a basic, entirely local Apple FM Autopilot tool/chat path for Apple users and records the Pylon loopback readiness command, buildable Foundation Models bridge helper, Desktop readiness/mode UI, fake-bridge-tested local chat/tool session runner, and admitted-Mac smoke evidence as implemented.',
        'Episode 199 Claude Code-first mech-suit language is historical source material, not current public positioning. Current coding-agent runtime work is Codex-oriented where applicable, with useful ideas folded into Probe/Pylon.',
        'Open-source availability covers public product code and docs in the OpenAgentsInc/openagents repository; it does not publish secrets, production data, Cloudflare account bindings, wallet material, provider credentials, customer-private workroom content, or third-party service internals.',
        'The full training-pipeline program (the training.* promises) is planned scope from docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md; no Psion model rung above the retained tri-host rehearsal exists, and no pipeline stage is live as a broadly paid network workload.',
        'Episode 236 was launch-direction source material. As of 2026-06-18 the decentralized training run has launched and is live: run.tassadar.executor.20260615 is active, self-serve claiming is open, and the training.decentralized_training_launch.v1 promise is green on two real settled receipts. Remaining executor-model, multi-earning-node, largest-run-comparison, and network-scale language stays red/yellow until its own run, participant, work, validation, payment, settlement, and projection receipts exist — the launch happening does not green those broader claims. The former models.tasadar_percepta_executor.v1 typo record is withdrawn in favor of models.tassadar_percepta_executor.v1.',
        'The decentralized-training launch has happened: as of 2026-06-18 run.tassadar.executor.20260615 is live and active, the self-serve install→register→claim→submit→independent-validation path is open (the producer keeps claimable windows; 10 open at this writing), and real Bitcoin has settled — ~1,005 sats to independent contributors plus a 75-sat hygiene settlement, with public receipts. The earlier "imminent but not yet happened" / "rails-ready is not launched" framing under registry 2026-06-14 was point-in-time and is superseded. As of 2026-06-19, the first auto-stream settlement visibility sequence is captured at docs/launch/2026-06-19-autostream-settlement-visibility-capture.md. Each broader training claim (network-scale, paid-at-scale, largest-run, canonical-model-mutation, or "anybody on any platform automatically earns") stays red/yellow pending its own public participant, accepted-work, validation, platform, helper-autostart, scale, and settlement receipts.',
        'Owner-authorized state flips 2026-06-14 (registry 2026-06-14.1): labor.forum_work_requests.v1 and labor.nostr_negotiation_market.v1 to green and provider.compliant_usage_labor.v1 / autopilot.control_center_fanout_marketplace.v1 to yellow were applied in source ahead of the receipt-first operator-route transition receipts, on the strength of the #4777/#4781/#4783 settlement evidence. The matching promise_transition receipts were recorded against the deployed registry on 2026-06-15 via the operator route (#5017): labor.forum_work_requests.v1 promise_transition_a38a3472-a5f2-4307-9de6-18afffa22627, labor.nostr_negotiation_market.v1 promise_transition_2bf98afa-ddb8-4a1e-863e-25178572620f, provider.compliant_usage_labor.v1 promise_transition_a862e366-efde-4655-96df-cd09a57d47fe, autopilot.control_center_fanout_marketplace.v1 promise_transition_9fd8f04b-ac6b-4b54-a4bf-9fa85d1e2948 — each an exception receipt (the flip was already applied, so from-state equals to-state), dereferenceable at /api/public/product-promises/transitions. The reconciliation record is docs/promises/2026-06-14-registry-reality-reconciliation-audit.md.',
        'Wave-3 Autopilot Sites / Agency Pack surfaces (#4977-#4995) enter as conservative new records: autopilot.desktop_gui_client.v1 (yellow, local-only), mobile.autopilot_remote_control.v1 (planned), workrooms.omni_client_delivery_workrooms.v1 (red), autopilot_sites.native_email_sequences.v1 (yellow, no send service), autopilot_sites.custom_tenant_hostnames.v1 (yellow, no self-serve/SSL), autopilot_sites.partner_payout_ledger.v1 (red), autopilot.cloud_credits_ui.v1 (yellow, presentational), mobile.voice_session_evidence_transcript_ingest.v1 (red, contracts only). Registry 2026-06-15.11 moves autopilot.local_apple_fm_tool_chat.v1 to yellow for basic fully local Apple FM Autopilot chat/tool use: Pylon exposes the token-authenticated apple_fm.status projection, retains a buildable local Foundation Models bridge helper, Autopilot Desktop renders hosted/local readiness modes, the desktop-originated local session path is fake-bridge tested, and admitted-Mac smoke evidence exists. It is still scoped to source/local operator builds and supported Apple Silicon; no current signed-installer, compute resale, paid-work, or green claim is made.',
        'Autopilot-is-the-install reconciliation for the 2026-06-15 launch: contributor-facing install copy should name Autopilot Desktop as the install surface and Pylon as the local node it drives; the affected Pylon promises remain yellow.',
        'Registry 2026-06-18.2: the training.public_gradient_windows.v1 record remains planned, not live. H1 now has code-backed psionic frozen-core learned-interface validation and an OpenAgents quarantine→recompute/replicate→canary→promotion gate for candidate learned-interface updates, but no public contributor gradient window has been accepted, promoted, paid, or settled. Public devices do generation/validation/evaluation only; do not claim public decentralized gradient training for the launch.',
        'Registry 2026-06-17.4 adds autopilot.repo_study_packets.v1 as a yellow internal-dogfood claim only. The public StudyBench MVP shows source-grounded lift on OpenAgents refs, but customer repo studying, trained repo expert language, marketplace packages, payout eligibility, and paid-work status remain blocked by separate validation, privacy, metering, pricing, payout, and settlement gates.',
        'Registry 2026-06-17.5: the real paid-settlement gate on training.monday_decentralized_training_launch.v1 is now MET for a bounded scope. A 1,000-sat real Bitcoin run-settlement settled, native over Spark, to an independent contributor (Orrery, pylon.448ba824…), evidenced by public receipt receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618 (realBitcoinMoved:true, moneyMovement:real_bitcoin, state:settled, adapter:spark_treasury), backed by Verified challenge training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c (independent validator replay on a distinct device, digests matched). #5232 closed and the public settled feed moved 0 → 1, with no raw address in the projection. The promise stays green; this is an evidence/copy upgrade from simulation-record-path-only to one real paid settlement. The earlier simulation-backed Orrery receipt (realBitcoinMoved:false) is retained as historical projection-path context only. This proof is exactly one 1,000-sat canary: it does not authorize network-scale, paid-at-scale, hundreds-paid, largest-run, canonical-model-mutation, or unbounded-payout copy. training.public_distributed_training_run.v1 drops its public_training_settlement_receipts_missing blocker but stays red on public_distributed_training_run_receipts_missing plus network-scale/participant criteria. The decision record is docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md, and the matching promise_transition exception receipt promise_transition_5be9bf3e-4784-41c4-a861-b23f5da61552 was recorded against the deployed registry via the operator route (from-state equals to-state, both green), dereferenceable at /api/public/product-promises/transitions.',
        'Registry 2026-06-17.6: the training launch promise was renamed from training.monday_decentralized_training_launch.v1 to training.decentralized_training_launch.v1 ("Monday" dropped — it is just the decentralized training launch now). This is a forward-looking identifier rename only: the promise state stays green, the real-settlement evidence (receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618, realBitcoinMoved:true) is unchanged, and no scope is widened. Earlier registry notes and promise_transition receipts recorded under the old training.monday_decentralized_training_launch.v1 id are retained as accurate history of the prior registry states.',
        'Registry 2026-06-18.1 adds autopilot.external_repo_studying_pilot.v1 as a yellow, refs-only pilot surface. The Probe pipeline can run corpus manifesting, study-packet sectioning, graph traversal, S3 verification, S4-style lift scoring, and coder-context projection on a non-OpenAgents fixture repo, but customer-private ingestion, self-serve upload, marketplace packaging, pricing, payout eligibility, settlement, and green public copy remain blocked.',
        'Registry 2026-06-18.3: training.decentralized_training_launch.v1 stays green; this is an evidence/copy accuracy upgrade only (no scope widening, no gate flip). A second real Bitcoin run-settlement on run.tassadar.executor.20260615 is now dereferenceable: 5 sats settled to a second distinct independent contributor (pylon.81f0facfe…), native over Spark, via receipt receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1 (realBitcoinMoved:true, moneyMovement:real_bitcoin, state:settled, adapter:spark_treasury), backed by Verified independent-validator challenge training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4. This contributor came through the rc.32 self-serve public install→register→claim→submit→independent-validation path (the first independent contributor through the self-serve door), so the promise copy moves from "one contributor paid" to "two distinct independent contributors paid". Real settled total for the run is 1,005 sats (1,000 canary + 5 self-serve); the simulation 5-sat row (realBitcoinMoved:false) is excluded. The enumerable per-run settled feed GET /api/public/training/runs/run.tassadar.executor.20260615/settlements is now live (three rows: two real + one simulation) and is added as evidence. The 5-sat self-serve settlement was operator-retro-settled via the admin settlement endpoint because the auto-stream skipped at verdict time (a payout-target resolution bug, since fixed) — the first fully-autonomous auto-stream settlement (gate firing at verdict with no operator action) has NOT happened yet. No network-scale, paid-at-scale, largest-run, canonical-model-mutation, or unbounded-payout claim is authorized. No promise_transition is required because the state does not change (green→green); if owner review wants a transition exception receipt for the copy upgrade, record it against the deployed 2026-06-18.3 version via the operator route per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-18.7: public evidence route normalization. The run settlements evidence now uses the `/api/public/training/runs/{runRef}/settlements` alias, and each exact-trace verification challenge can be dereferenced at `/api/public/training/verification-challenges/{challengeRef}`. This is URL/discovery cleanup only; no promise state flips, settlement authority, payout authority, or claim scope changes.',
        'Registry 2026-06-19.2: copy/evidence destale only, no state flips. Autopilot Desktop auto-onboarding EPIC #5441 (AO-1..AO-6) is BUILT and tested (first-run self-register, AO-3 identity choice, AO-4 wizard projection, black-screen guard, AO-6 headless smoke against the real local node) but the final from-DMG proof on a clean Mac (rendered window, production presence, settled Tassadar Bitcoin receipt) is owner-gated and pending — desktop stays yellow. The Sites referral payout ledger is WIRED in source (RL-1 #5458: paid-event eligibility feed + readiness-gated idempotent approved->dispatched->settled dispatch via the MDK/Spark adapter, Bitcoin-only rev-share boundary) but NO real referral payout has settled — referral stays yellow and partner_payout_ledger stays red, each with a first-real-payout-pending blocker. All upgrades remain receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-19.3: coding-agent live re-verification, copy/evidence destale only, no state flips. On 2026-06-19, from clean origin/main (b6e523a77), the three live coding-agent execution lanes were independently re-run and all passed, captured in the dereferenceable receipt docs/launch/2026-06-19-coding-agent-live-verification.md: the local Claude Agent bridge ran sessions exec (adapter claude_agent, verify.passed:true, exit 0, ~10.65s, auth on-device ~/.claude), the local Codex bridge ran sessions exec (adapter codex, workspace-write sandbox with network disabled, verify.passed:true, exit 0, ~13.17s, auth on-device ~/.codex/auth.json), and the Tassadar executor package passed bun test packages/tassadar-executor (23 pass / 0 fail across 5 files, exit 0, execute + exact_trace_replay). The three already-green coding-agent records — pylon.local_claude_agent_bridge.v1, autopilot.codex_probe_pylon_successor.v1, and compute.tassadar_executor_poc.v1 — re-anchor their evidence on this fresh independent receipt and STAY green (green→green, no flip, no promise_transition required). The yellow desktop/built-in-compute records — autopilot.builtin_compute_agent.v1 and autopilot.desktop_gui_client.v1 — are noted as green-CANDIDATES whose execution-lane dependency is now live-proven, but they DO NOT flip: their gates require more than a local single-task exec (signed/notarized recut, packaged OpenAgents compute credentials + metered from-install go-online smoke for builtin_compute_agent; the from-DMG clean-Mac render/presence/settled-Bitcoin proof plus live PDF/preview/ingest/browser runtimes for desktop_gui_client). This receipt proves local single-task execution only and authorizes no production-scale, at-volume, packaged-stable-binary, or public-settlement copy.',
        'Registry 2026-06-19.4 adds the OpenAgents inference gateway / Agent Cloud vision as five conservative new records and flips NO existing promise. The whole inference business is ROADMAP, not built: inference.gateway_credits_business.v1 (red — no gateway endpoint, no inference credit balance, no metering/pricing/routing), inference.referral_on_all_inference.v1 (planned — cross-category ongoing referral revshare is design intent, sub-EPIC #5475), cloud.agent_cloud_one_stop_revshare.v1 (planned — the one-balance one-stop Agent Cloud is the unifying vision capstone, not a shipped product), and inference.decentralized_serving_fabric.v1 (red — Pylons do not yet serve inference; shard-WAN large-model serving is Psionic-planned/hardware-blocked; first serving-node payout owner-armed). The ONLY piece with real evidence is inference.fireworks_open_model_provider.v1 (yellow), and it is scoped precisely to a verified upstream PROVIDER CONNECTION: on 2026-06-19 a real OpenAI-compatible Fireworks chat/completions call returned a proper usage object against an OpenAgents-held key (docs/inference/2026-06-19-fireworks-provider.md), with seven serverless models live on the account — this proves reachability and a cheap-tier cost basis, NOT a sellable customer inference product, so it is deliberately not green. Existing records were cross-referenced (not inflated): sites.referral_bitcoin_stream.v1 stays yellow and is explicitly distinguished from the unbuilt inference referral product; api.hosted_gemini.v1, payments.accepted_outcome_economics.v1, training.*, and compute.tassadar_executor_poc.v1 are referenced as related/spine evidence without any state change. Sources: docs/inference/ (7 docs), EPIC #5474, sub-EPIC #5475, children #5476-#5491. No promise_transition is required (new red/yellow/planned records create no state flip); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-19.7: autopilot.*/autopilot_sites.* yellow→green-readiness pass, no state flips. A consolidated dereferenceable readiness receipt (docs/launch/2026-06-19-autopilot-yellow-green-readiness-receipt.md) now records, per non-green autopilot.*/autopilot_sites.* promise, the runnable verification command + observed output, what was built, the exact remaining receipt for green, and the owner gate. Two promises had real last-mile code built and tested this pass: autopilot.mission_briefing.v1 gained real risk and receipts rollups in the Mission Briefing projection (risk: review caveats, blocker count, delivery/worktree/change-capture statuses, settlement-blocked reason, derived clear/attention/blocked level; receipts: authority-receipt/proof/verification refs, buyer-payment proof, settlement eligibility), closing the cost_risk_receipt_rollup_missing blocker (autopilot-work-routes.test.ts 37 pass, with new rollup + no-secret-leak assertions); and autopilot_sites.native_email_sequences.v1 gained the missing "home for site form-specs" as a typed, tested registry (site-form-spec-registry.ts: resolves a FormCaptureSpec by id from a published site/version metadata_json, key↔id agreement, safe degrade to 404 on malformed input; site-form-spec-registry.test.ts 7 pass), making the public form-capture route wireable. The remaining autopilot/sites yellows/reds had their green-readiness evidence assembled (workers/api autopilot/sites module suite 166 pass over 14 files; apps/web credits-panel 24 pass) and their exact remaining receipt + owner gate recorded. NO promise changes state; zero green flips. Any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-19.5: blocker/copy destale on the inference gateway, no green flips. The earlier "api unbuilt" framing for inference.gateway_credits_business.v1 is now STALE and corrected: the OpenAI-compatible gateway request surface is BUILT, DEPLOYED, and LIVE in prod (POST /v1/chat/completions, INFERENCE_GATEWAY_ENABLED=true), and Gemini 3.5 Flash is served end-to-end through it, verified live 2026-06-19 (docs/inference/2026-06-19-gateway-gemini-live-verification.md; an unauthenticated prod POST returns 401 not 404, confirming the route is deployed and key-auth-gated). Free inference works. The promise STAYS red/non-green because it is a CREDITS BUSINESS and the PAID-credits path is not collectable end-to-end: Stripe card->credit is wired in source but has no prod secrets, and the USD->msat bridge (#5497, merged) has no real upstream purchase to bridge, so there is no dereferenceable card->credit->inference-spend receipt. Its blocker set is rewritten from "*_unbuilt" gateway/metering/pricing/routing blockers to paid-credits blockers (inference_paid_credits_card_to_credit_not_collectable, inference_usd_to_msat_bridge_no_real_purchase, inference_card_credit_inference_spend_receipt_missing). inference.fireworks_open_model_provider.v1 evidence is honestly tightened — it is now a REGISTERED LIVE SUPPLY LANE in the deployed gateway rather than a bare provider connection, and its stale "gateway api unbuilt" blocker is dropped — but it STAYS yellow (no sellable paid open-model product, no paid receipt). NO promise changes state; zero green flips. Any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-19.9: remote-bridge decision-queue transport built; evidence-only, no state flips. The composing capability that lets command APIs (decision resolve) flow over the capability-scoped Pylon bridge to a remote node now exists as a pure, transport-agnostic protocol module: packages/autopilot-control-protocol/src/remote-decision-queue.ts (createRemoteDecisionQueue), with 20 passing tests in remote-decision-queue.test.ts. It ingests node decision events delivered over session.subscribe/session.history into a live queue of exactly-once DecisionRecords, relays decision.resolve through the existing BridgeTransport (which carries the answer_decision capability, enforced node-side against the STORED pairing claims — no new auth, no scope-weakening), classifies each result into one typed action receipt, and offline-queues resolutions taken while disconnected for oldest-first drain. This unblocks the cross-client exactly-once decision queue for autopilot.decision_queue.v1 and the steer/decision leg of mobile.autopilot_remote_control.v1, both of which STAY planned: green still requires a dereferenceable receipt of a real phone/web/desktop decision resolved over a remote-reachable paired node with receipt closeout, plus owner sign-off per proof.claim_upgrade_receipts.v1. The node-side remote bridge (#5000) was already merged (9a31ad6bf); this is the client-side composing transport that the Expo app and web/desktop decision queues share. NO promise changes state; zero green flips.',
        'Registry 2026-06-19.10: workrooms source-authority + approval-gated business-object writes built (DE-9 / EPIC #5532); evidence-only, NO state flips. The capability the source-authorized-business-objects promise is missing now exists as two pure, contract/projection-only modules in the same family as omni-crm-follow-up-workrooms and omni-support-project-ops-workrooms. workers/api/src/omni-source-authorized-business-objects.ts is the typed source-authority model: an OmniSourceAuthorityBinding names WHO (principal class + ref) may write WHICH business-object kinds (contact, company, task, decision, document, approval, artifact, receipt) under WHICH source kinds (connector_read, uploaded_document, verified_chat_extraction, approval_decision, workroom_artifact) and operations, all under a fixed contract_projection_only authority boundary (no unapproved mutation, no connector writeback, no notification send, no settlement, no spend). decideOmniBusinessObjectWrite is the approval-gated write engine: a proposed write may be applied only when the binding covers its kind/operation/source/principal AND an approval is recorded — except low-risk append/create on append-only kinds (artifact, receipt) whose binding explicitly waived approval; every update/supersede always requires an approval ref; writes with no source ref are rejected (no chat-text-only inference). workers/api/src/omni-workroom-business-object-delivery.ts is the FLAG-GATED INERT delivery seam onto the live omni client-delivery workroom surface (workrooms.omni_client_delivery_workrooms.v1): buildOmniBusinessObjectDeliveryPlan computes per-write decisions for a live workroom but holds them inert — gate resolves inert_disabled by default, enabled_blocked when the integration flag is on without owner sign-off + closeout receipt, and only enabled_ready with all three; effectsApplied is ALWAYS false (the integration plans, it never applies a real business-object mutation, sends, settles, or spends). 50 assertions across omni-source-authorized-business-objects.test.ts (25) and omni-workroom-business-object-delivery.test.ts (10) plus the existing omni suite pass. workrooms.source_authorized_business_objects.v1 STAYS red and workrooms.omni_client_delivery_workrooms.v1 STAYS yellow: green for the source-authorized promise requires the live integration ENABLED end-to-end with a real source-authorized approval-gated workroom write, a closeout receipt, and owner sign-off per proof.claim_upgrade_receipts.v1 — owner-gated and out of scope for this build. NO promise changes state; zero green flips.',
        'Registry 2026-06-20.10: enterprise claim-upgrade audit panel shipped for proof.claim_upgrade_receipts.v1; evidence-only, NO state flips, green count unchanged. The missing enterprise audit surface now exists as a read-only dereferenceable projection at GET /api/public/product-promises/audit (apps/openagents.com/workers/api/src/promise-transition-audit-routes.ts, 7 passing tests in promise-transition-audit-routes.test.ts), which JOINS the public transition-receipt feed against the live registry so a third party can audit every green flip without trusting narrative copy: per promise it returns promiseId, productArea, currentState, lastVerifiedAt, blockerRefs, and the backing transition receipts (from->to state, registryVersion, receiptRef, result, evidence refs, owner signoff, alreadyApplied/isGreenFlip flags), filterable by promiseId/state/greenOnly; a registry-wide summary reports greenPromiseCount, greenPromisesReceiptBacked, and the explicit greenPromisesWithoutReceipt list (green promises whose backing green-flip receipt has NOT been recorded against the deployed registry). The /promises page renders it as the claim-upgrade audit panel and links the JSON, and the OpenAPI contract describes it. Safe-by-construction: no mutation, no secrets, no spend, no authority — it only re-projects already-public data. This clears blocker.product_promises.enterprise_audit_panel_missing on proof.claim_upgrade_receipts.v1; the promise STAYS yellow because flipping it green is owner-gated (sign-off plus recording the trailing green-flip transition receipts so greenPromisesWithoutReceipt is reconciled). NO promise changes state; zero green flips. KNOWN GAP the audit surface now makes machine-auditable: the transition feed trails the registry — several green promises appear in greenPromisesWithoutReceipt because their backing green-flip receipts have not been recorded against the deployed registry (that backfill needs the prod admin token and is owner-gated, separate from this audit panel).',
        'Registry 2026-06-19.10: composed-run capstone wired to the REAL metering + referral seams; evidence-only, no state flips. The Autopilot all-in-one composed-run scaffold (#5519) was a shape-demo: it derived component receipt refs but exercised neither the merged metering seam nor the referral bridge. It now has a FLAG-GATED INERT execution-composition module, workers/api/src/autopilot-composed-run-execution.ts (composeRunExecution + composedComponentCharge + composedRunExecutionProjection), with 15 passing tests in autopilot-composed-run-execution.test.ts. A composed run now composes >= 2 REAL primitive scaffolds (inference + one of fine-tuning/sandbox) onto ONE shared balance: each component derives its receipt-first charge shape from its OWN primitive helper — fine-tuning/sandbox build a full CloudPrimitiveCharge through the merged receipt-first cloud-metering seam (cloud/cloud-metering.ts: cloudChargePayInPlan/cloudChargeReceiptRef/cloudChargeIdempotencyKey), inference through the inference metering hook — the per-component charges sum into ONE shared-balance debit total, and the composed spend is fed through the MERGED referral bridge (marketplace-monetize-any-layer-accrual.ts -> the ONE RL-1 cross-category ledger). HONEST/INERT by construction: it builds the CloudPrimitiveCharge PLANS but NEVER calls settleCloudPrimitiveCharge (no D1 batch, no debit), and ALWAYS calls the referral bridge with enabled:false, so the bridge returns its disabled plan and touches no ledger (proven by a D1 stub that throws on any IO). This advances cloud.primitives_suite.v1, cloud.agent_cloud_one_stop_revshare.v1, and autopilot.all_in_one_business_system.v1, ALL of which STAY planned. NO promise changes state; zero green flips; green count stays 20. Green needs a REAL billed composed run with a dereferenceable revshare receipt plus owner sign-off per proof.claim_upgrade_receipts.v1, and demand provenance per proof.demand_provenance.v1 (internal first-party use is plumbing proof, not market proof).',
        'Registry 2026-06-20.6: autopilot_sites.native_email_sequences.v1 last-mile build, no state flips (stays yellow). Three of its four blockers are cleared by deployed code with dereferenceable artifacts and drop from the record: (1) site_form_capture_route_unmounted — the public form-capture route is MOUNTED in the worker omni chain (index.ts) behind the default-OFF SITE_FORM_CAPTURE_ENABLED flag, resolving each page FormCaptureSpec from the active site version metadata via site-form-spec-registry and persisting leads through the native-lists addSubscriber sink (#5544/#5548); (2) email_sequence_customer_ui_missing — a presentational customer-facing email-sequence UI now exists and is exported, apps/web/src/ui/email-sequence-panel.ts, rendering a sequence header, its ordered steps, and the viewer enrollment status with enabled/disabled enroll states (email-sequence-panel.test.ts, 12 pass); (3) email_send_service_integration_missing — the send-service integration seam now exists, workers/api/src/email-sequence-send-service.ts, classifying a due authored-sequence send into a typed send plan and dispatching it through an INJECTED sender, INERT by default behind the default-OFF EMAIL_SEQUENCE_SEND_ENABLED flag (it plans a dry-run and NEVER calls the sender unless armed; email-sequence-send-service.test.ts, 7 pass). The send seam is deliberately NOT chained into the live dispatcher and sends NO live email. The ONE remaining blocker is email_deliverability_unproven: green requires a live deliverability smoke (send→deliver evidence) with bounce/complaint handling, plus customer self-serve authoring — both owner/product-gated. NO promise changes state; zero green flips; green count stays 24. Any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        "Registry 2026-06-23.1 (DE-2, EPIC #5525): the dereferenceable PAID-charge RECEIPT artifact for the sellable Cloud primitives now exists end to end; copy/evidence destale, NO state flips. Two real gaps that prevented an advertised cloud-primitive receipt from ever being dereferenced are fixed. (1) The shared cloud-metering seam (cloud/cloud-metering.ts: settleCloudPrimitiveCharge) previously wrote a debit-only adjustment pay_in and left it `pending` forever; it now marks the charge PAID in the SAME atomic batch (markPayInPaidStatements with empty payout legs), the exact discipline the inference metering hook uses, so a settled metered charge is a real `paid` ledger receipt. (2) The sandbox + fine-tuning surfaces advertised a receipt ref (`receipt.cloud.<primitive>.rental|job.<id>`) that did NOT match the ref the ledger actually writes (`receipt.cloud.<primitive>.charge.<id>`); the advertised refs (sandboxRentalReceiptRef / fineTuningJobReceiptRef) are now ALIGNED to cloudChargeReceiptRef, so the ref a surface advertises is exactly the ref that dereferences. A new public read route GET /api/public/cloud/receipts/:receiptRef (cloud/public-cloud-primitive-receipt-routes.ts + cloud/cloud-primitive-receipts.ts) reads the settled pay_ins row and projects a public-safe PAID receipt for `receipt.cloud.sandbox_compute.rental.charge.*` and `receipt.cloud.fine_tuning.job.charge.*` (live_at_read staleness, redaction-guarded, returns 404 for pending/mismatched/non-cloud refs). A real-SQL test exercises the whole loop end to end: a closed sandbox rental priced from real metered usage debits credits receipt-first, settles PAID, and the advertised receipt dereferences into a public-safe projection (rent -> metered debit -> dereferenced receipt). The composed-run-receipt reconciliation is updated to reflect that a cloud primitive component's surface and settlement refs now coincide. cloud.sandbox_compute_service.v1 and cloud.fine_tuning_service.v1 STAY red and cloud.primitives_suite.v1 STAYS planned: the receipt ARTIFACT is no longer the blocker, but a real isolated-session/job runtime, a live pricing function, and a REAL renter/job submitter (demand provenance per proof.demand_provenance.v1) plus owner sign-off per proof.claim_upgrade_receipts.v1 are still required. The sandbox/fine-tuning surfaces remain flag-gated INERT (default off -> 404) and bill nothing on prod. NO promise changes state; zero green flips. Evidence: docs/promises/2026-06-23-de2-cloud-primitive-dereferenceable-receipt.md.",
        'Registry 2026-06-20.13: training.device_capability_dataset.v1 gains a GENUINE second device class; STAYS yellow, no green flip, green count unchanged at 24. The dataset previously covered exactly one device class (device_class.apple_silicon_macos.arm64, the settled #4681 closeouts on two same-host Pylons). A second, genuinely distinct device class is now characterized with REAL measured data: device_class.x86_64_linux.intel — an x86_64 Linux contributor (Intel Core i7-14700K, 28 cores, ~125 GB RAM, Linux 6.19, Node 25) ran the EXACT bounded CS336 A2 suite (benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1, 24 repetitions) and produced deterministic output digests that match the suite commitment BYTE-FOR-BYTE across architectures (attention 70b508a8a655e0b0, bandwidth 02d2cf92913ee000, decode 6b8502b3d1f381d1, step 1bde26dbb6c833ce — identical to the in-repo workload and to the Apple-Silicon class), with genuinely different measured timings (e.g. attention p50 ~3110 megaflops, memory_bandwidth p50 ~10.7 GB/s, tokens_per_second p50 ~352k/s). These rows are admitted under a NEW honest provenance label, measurementProvenance: measured_unsettled (crossCheckState: measured_unverified): they are NOT paid (no settlement receipt), NOT statistical_cross_check verified (no second same-class Intel device, no validator verdict), carry NO earning estimate, and are explicitly verified:false; the admission path now ENFORCES that measured_unsettled rows carry a digest-commitment ref but no receipt and no earning estimate, and never borrow a run-level verdict. The dataset projection adds observedSettledDeviceClassCount alongside observedDeviceClassCount so the public surface distinguishes settled+verified class coverage (1) from total observed class coverage (2). The blocker blocker.product_promises.second_device_class_missing is DROPPED — a genuine, distinct, real-measured second device class is now in the dataset — while blocker.product_promises.thermal_throttle_detection_missing and blocker.product_promises.same_host_replication_caveat REMAIN. Green still requires paid+statistical_cross_check-verified coverage across at least two distinct classes (the Intel class is measured-only so far), continuous thermal-throttle detection, and cross-machine (not same-host) replication. No promise_transition is required (yellow→yellow, no flip); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1. Evidence: docs/training/2026-06-20-cs336-a2-second-device-class-x86_64-linux-intel.md.',
        'Registry 2026-06-20.17: autopilot.repo_study_packets.v1 gains the missing customer-private validation path; STAYS yellow, no green flip, green count unchanged at 24. The lead blocker repo_studying_customer_private_validation_missing is DROPPED, cleared by a pure, refs-only, INERT-by-construction validation module: packages/probe/packages/runtime/src/benchmark/openagents-customer-private-validation.ts (buildOpenAgentsCustomerPrivateValidation + planCustomerPrivateValidationDelivery), with 5 passing tests in packages/probe/packages/runtime/tests/openagents-customer-private-validation.test.ts. The module validates a built study packet PRIVATELY for a customer — against a committed private holdout expressed as split/dataset/checksum REFS, a row count, and a basis-point lift only (raw private task text, gold answers, rubric claims, and evidence excerpts are NEVER admitted and never appear in the verdict, which carries sourceBoundary customer_refs_withheld) — BEFORE the packet could be delivered or made claimable. It composes the existing study verification report (correctness gate, validator-review remainder) and eval lift, adds a private-holdout lift gate plus a privacy-discipline gate (digest ref or explicit withheld sentinel required; raw-content checksum rejected), and emits a deterministic, public-projection-safe verdict (validated through validateProbeBenchmarkPublicProjection). HONEST/INERT by construction: deliverable is ALWAYS false, customerPublicClaimAllowed/marketplacePackageAllowed/payoutEligible are ALWAYS false, and the FLAG-GATED delivery seam (CUSTOMER_PRIVATE_REPO_STUDY_DELIVERY_ENABLED, default OFF) reports effectsApplied:false even when armed with owner sign-off — it plans whether a verdict WOULD be deliverable but never delivers a real packet, marks one claimable, sends, settles, or spends. The promise STAYS yellow: the remaining blockers (privacy_review_missing, marketplace_metering_missing, pricing_package_policy_missing, payout_settlement_gates_missing, product_copy_review_missing) require real customer-data privacy review, an ARMED delivery against a real customer holdout with a dereferenceable closeout receipt, marketplace metering, pricing, payout eligibility, settlement, and owner sign-off per proof.claim_upgrade_receipts.v1 — all owner/product-gated and out of scope for this pure validation build. No promise_transition is required (yellow→yellow, no flip); any future green flip remains receipt-first and owner-signed.',
        'Registry 2026-06-20.21: autopilot.external_repo_studying_pilot.v1 gains the missing customer-private ADMISSION path; STAYS yellow, no green flip, green count unchanged at 24. The lead blocker external_repo_studying_customer_private_admission_missing is DROPPED, cleared by a pure, refs-only, INERT-by-construction admission module: packages/probe/packages/runtime/src/benchmark/external-repo-studying-pilot-admission.ts (buildOpenAgentsExternalRepoStudyPilotAdmission), with 6 passing tests in packages/probe/packages/runtime/tests/external-repo-studying-pilot-admission.test.ts. It REUSES (does not rebuild) the sibling customer-private validation module (buildOpenAgentsCustomerPrivateValidation) as its private validation engine, then layers an admission decision on top: it decides whether an EXTERNAL contributor (named by ref only) study of a non-OpenAgents repo may be ADMITTED into the pilot FOR A CUSTOMER, after the study has been validated PRIVATELY against the customer committed private holdout (split/dataset/checksum REFS, row count, basis-point lift only — raw private task text, gold answers, rubric claims, and evidence excerpts are NEVER admitted and never appear in the admission, which carries sourceBoundary customer_refs_withheld). The admission passes only when the reused private validation verdict is validated_held, the external pilot surface is pilot_ready, and the contributor has accepted the refs-only/no-leak/inert pilot terms. HONEST/INERT by construction: admitted is ALWAYS false, effectsApplied is ALWAYS false, customerPublicClaimAllowed/marketplacePackageAllowed/payoutEligible are ALWAYS false, and the FLAG-GATED admission seam (EXTERNAL_REPO_STUDY_PILOT_ADMISSION_ENABLED, default OFF) reports effectsApplied:false even when armed with owner sign-off — it computes whether a study WOULD be admittable (wouldAdmitWhenArmed) but never admits a real study into a real customer pilot, delivers a packet, marks anything claimable, sends, settles, or spends. The verdict is decoded through validateProbeBenchmarkPublicProjection + a deterministic admission hash. The promise STAYS yellow: the remaining blockers (privacy_policy_missing, self_serve_upload_missing, marketplace_metering_missing, pricing_package_policy_missing, payout_settlement_gates_missing) require real customer-data privacy review, self-serve upload controls, an ARMED admission against a real customer holdout with a dereferenceable closeout receipt, marketplace metering, pricing, payout eligibility, settlement, and owner sign-off per proof.claim_upgrade_receipts.v1 — all owner/product-gated and out of scope for this pure admission build. No promise_transition is required (yellow→yellow, no flip); any future green flip remains receipt-first and owner-signed.',
        'Registry 2026-06-23.2: artanis.labor_requester.v1 (DE-8, EPIC #5531) gains the missing green-readiness surface that maps the labor receipt feed onto its TWO named green-flip blockers; STAYS yellow, no green flip, green count unchanged. The receipt machinery was already complete (requester surface → tick driver → content-addressed receipt → tamper-evident D1 store → public re-verifying feed at GET /api/public/artanis/labor-receipts), but nothing folded that feed onto the named blockers so a reviewer or the operator recording the transition could read a single dereferenceable JSON to see whether the gate is met — exactly the surface the responder (responder-support) and evolution-loop (tick-streak) promises already have. That surface now exists at GET /api/public/artanis/labor-green-readiness (apps/openagents.com/workers/api/src/artanis-labor-green-readiness.ts + the handler in artanis-labor-receipt-routes.ts), reusing the SAME feed projection the public receipt feed serves (no fork): it counts PLACED unattended request receipts (terminalState requested_pending_delivery / accepted_released / rejected_refunded — states only an operator-ENABLED tick can reach; a config-disabled tick is sealed skipped_config_disabled and never places), and reports liveEnablementProven (≥1 placed receipt — proof a real enabled tick reserved escrow, clearing blocker.product_promises.artanis_labor_live_enablement_missing), unattendedRequestReceiptsProven (≥10 placed receipts, clearing blocker.product_promises.artanis_labor_unattended_request_receipts_missing), and greenGateMet (both, the mechanical receipt-evidence predicate only — never the owner sign-off). Each placed receipt is dereferenceable at GET /api/public/artanis/labor-receipts?receiptRef=<ref> for independent content-address re-verification. 17 passing tests (artanis-labor-green-readiness.test.ts + artanis-labor-receipt-routes.test.ts); the new route is added to the worker exact-routes manifest. Safe-by-construction: read-only, no-store, mints no dispatch/spend/escrow/settlement/registry authority and cannot create a receipt, enable a tick, or flip a blocker. The promise STAYS yellow: green requires the owner to (1) operator-enable Artanis labor and let ≥10 unattended ticks accrue placed receipts so /api/public/artanis/labor-green-readiness reports greenGateMet:true, then (2) record the owner-signed yellow→green promise_transition via POST /api/operator/product-promises/transitions citing the live readiness surface. NO promise changes state; zero green flips; any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-24.1: program-focus note, NO state flips, green count unchanged, NO evidence removed. The "Weekend Promise Assault" push (drive all non-green promises green; parent EPIC #5523, closed) is PAUSED. The active program focus is the Khala launch — Khala on Machine Payments (MPP) + Stripe Directory discovery and the head-to-head inference demo (EPICs #6049, #6017). The ten DE domain epics — #5524 DE-1 revenue loop, #5525 DE-2 inference gateway + cloud primitives, #5526 DE-3 Autopilot coding surface, #5527 DE-4 Pylon network, #5528 DE-5 training/Tassadar, #5529 DE-6 open markets, #5530 DE-7 mobile+voice, #5531 DE-8 identity/proof spine, #5532 DE-9 workrooms/sites, #5533 DE-10 energy/metrics/world-firsts — and the Episode 239 "Let\'s Make Money" revenue-loop tracker (#5510 plus children #5520, #5521, #5511, #5512, #5508) are closed as PARKED / not-planned-for-now — NOT abandoned. This is a tracker/focus decision only: every affected promise KEEPS its honest current state (red/yellow/planned), so parking changes no promise state, flips nothing green, and removes no evidence or blocker. There is deliberately NO new "parked" promise state — parking lives in this program note, not in promise truth. The live paid-inference path (POST /v1/chat/completions, free inference works) plus the Bitcoin contributor-payout rail carry forward in the Khala MPP work (#6049); the broad referral / partner-ledger / paid-credits-collection / fine-tuning / sandbox assault is paused until explicitly resumed. The registry stays the source of truth for promise state; any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1. The parked epics reopen when the broad promise push resumes.',
        'Registry 2026-06-25.1 adds data.free_tier_capture_disclosure.v1 (yellow) as the honest data-sharing disclosure backing default-on free-tier trace capture (#6293/#6294/#6295, EPIC #6206). Default-on capture is going live: free-tier /api/v1/chat/completions traffic is captured by default as REDACTED, PRIVATE (owner_only) ATIF traces that may be used to improve/train OpenAgents models; paying for privacy / confidential compute opts the caller OUT (fail-closed to not-captured, inference-privacy-entitlement.ts); public sharing of a captured trace is owner opt-in only; and capture grants NO payout/settlement (the data-market reward marker stays inert and owner-gated, #6221). The canonical disclosure is a single source of truth (inference/free-tier-data-sharing-disclosure.ts) surfaced at three honest places: the POST /api/keys/free mint response (dataSharing field), the public agent-readable GET /api/public/free-tier-data-sharing endpoint, and the public AGENTS.md inference section. The record is YELLOW (not green): the disclosure text is implemented and discoverable, but the underlying default-on capture flip (KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT) is owner-gated and the public copy is owner-approval-gated per the audit. This is disclosure only — it ships no capture behavior, no authority, and no money. Evidence: docs/promises/2026-06-25-free-tier-data-sharing-disclosure.md, docs/traces/2026-06-25-default-on-trace-capture-audit.md.',
        'Registry 2026-06-27.1 reconciles Episodes 242-244, the shipped Khala CLI v0.1.16, and the Khala -> Pylon -> Codex owner-capacity runbook. It adds green scoped records for the free OpenAI-compatible Khala API, the public Khala tokens-served counter/history, and the shipped terminal CLI; yellow records for public model-family mix stats, explicit owner-capacity Codex delegation, free-tier trace capture, and paid/confidential privacy opt-out; and keeps no-resale/no-payout boundaries explicit. Exact own-capacity Codex token rows count in the headline token counter after closeout, but counter movement alone is not proof: assignment proof requires rows with provider pylon-codex-own-capacity, model openagents/pylon-codex, usage_truth exact, demand_kind own_capacity, and demand_source khala_coding_delegation. Broad automatic semantic routing, pooled third-party capacity, paid resale, public raw Codex traces, live assignment trace UI, and guaranteed dispatch availability remain blocked. The stale Khala CLI OpenTUI/single-line plan is superseded by the v0.1.16 scrollback/raw-mode CLI docs.',
        'Registry 2026-06-27.2 flips khala.own_capacity_codex_delegation.v1 from yellow to green for its explicit typed owner-capacity scope only. The remaining blockers from 2026-06-27.1 were cleared by source fixes and production smokes: default `pylon khala request --workflow codex_agent_task --fixture` now auto-runs the returned no-spend assignment instead of stranding the maxInflight gate (#6362, closeout assignment.closeout.0351f0a0b4650c2272233fa0); master-default public repo materialization now falls back to the pinned commit and passed the octocat/Hello-World smoke (#6361, closeout assignment.closeout.2ccf7c3d2ac86b12b57ce2e2); the owner-scoped trace-status route is deployed and returns lifecycle/progress/token/trace/raw-event metadata without raw payloads (#6368); and `pylon khala closeout <assignmentRef> --json` now composes trace status + proof into a fail-closed checklist that also requires the worker closeout event to prove `paymentMode: no-spend`, `settlementState: not_applicable`, and `payoutClaimAllowed: false` (#6369). This green flip does NOT authorize broad natural-language routing, third-party capacity pooling, Codex subscription resale, public raw Codex event visibility, paid work, payout eligibility, guaranteed availability, or bypass of the explicit typed workflow / caller-owned Pylon capacity gate.',
        'Registry 2026-06-28.1 is a marketplace.signature_monetization.v1 gate-hardening pass and flips NO promise state. The signature revenue gate now models publish -> activate -> usable refs before metered usage can drive pricing, keeps validation-only packages non-installable, requires attribution/pricing/rev-share/dispute/refund evidence before payable state, and allows settlement claims only when a public-safe usage-charge settlement receipt settles the full contributor payable amount. The promise stays planned on blocker.product_promises.signature_settlement_missing because live billing and real settlement remain owner-armed and receipt-first.',
        'Registry 2026-06-28.1: training.device_capability_dataset.v1 gains continuous thermal-throttle receipt machinery for CS336 A2 benchmark assignments and flips NO promise state (stays planned after the 2026-06-20 revenue-loop tightening). The bounded A2 workload now has a sustained-vs-burst thermal evidence builder that derives ratio rows from burst and sustained throughput samples. Verified sustained_vs_burst_throughput_ratio rows now project receiptRefs, thermalThrottleReceiptRefs, and thermalThrottleFunnelReasonCodes, and the public A2 dashboard carries the same refs so the capacity funnel can consume closed device_capability.public.* reason codes without exposing hardware identifiers. The product blocker blocker.product_promises.thermal_throttle_detection_missing STAYS until a real production contributor run records an owner-accepted verified thermal-row receipt and the operator records any required claim transition per proof.claim_upgrade_receipts.v1. No green flip, paid assignment, settlement, or capability guarantee is created by this source machinery alone. Evidence: docs/training/2026-06-28-cs336-a2-continuous-thermal-throttle-receipts.md.',
        'Registry 2026-06-28.2 corrects referral.refer_once_earn_forever.v1 evidence/copy for #6838 and flips NO promise state. The permanent referrer<->referee spine is now represented by the consume-once user/agent referral attribution tables joined to site_referral_sources, and the category-agnostic accrueCrossCategoryReferral primitive feeds receipt-first paid events from non-Sites categories such as marketplace/fine_tuning into the ONE RL-1 referral payout ledger. The promise STAYS red because no real ecosystem purchase has produced a settled Bitcoin referral payout and no purchase-to-payout dereferenceable receipt exists yet; the owner-armed payout and RL-1/RL-3 asset-boundary/no-resale gates remain required.',
        'Registry 2026-06-28.2: inference.decentralized_serving_fabric.v1 gains stricter whole-small-model Pylon proxy evidence and flips NO promise state. apps/pylon/src/psionic-vllm-proxy.ts now treats exact-greedy parity as a separate same-engine reference check: the proxy forwards a greedy serve request to the configured local engine, performs a second same-engine greedy reference call, compares output digests, refuses non-greedy requests, and keeps paid routing ineligible on reference mismatch or known-answer canary failure. This is source-level receipt plumbing only. The promise STAYS red: green still requires an owner-armed live gateway request, dereferenceable exact-greedy-parity receipt, canary/replay/payout-eligibility admission, and settled serving-node payout evidence.',
        'Registry 2026-06-28.2: marketplace.wasm_plugins.v1 gains source-level WASM-plugin package policy and install-state registry machinery plus an inert installed-plugin discovery route at GET /api/public/marketplace/wasm-plugins; NO promise state flips. The manifest schema requires version, WASM digest ref, interface declarations, bounded permissions, source refs, and policy refs; admission fails closed on malformed or over-privileged manifests; install/uninstall persists state in the injected registry store; and the public route lists only installed plugins from the injected store while reporting inert/planned. The broad package-policy/install-registry gap is narrowed, but the promise STAYS planned because there is still no public self-serve install marketplace, sandboxed execution receipt, billing, or settlement. No third-party WASM execution, code loading, marketplace mutation, billing, payout, or green claim is created.',
        'Registry 2026-06-29.3 implements #6832 as a narrow source-level WASM execution evidence step and flips NO promise state. workers/api/src/wasm-plugin-marketplace.ts now has a digest-pinned fixture executor that validates the admitted manifest, checks module bytes against wasm.sha256, rejects undeclared/unauthorized imports before instantiation, applies bounded module/input/output/duration/memory policy checks available to JavaScript WebAssembly, and returns a metering-shaped evidence record with input/output hashes, policy refs, host-call attempts, runtime ref, and deterministic evidenceRef. Tests run a real tiny WASM add fixture, reject digest mismatch, and reject an unauthorized host import. This is NOT a production sandbox claim: the public route remains inert, no third-party plugin execution is mounted, no billing/settlement/payout authority exists, and the promise STAYS planned on self-serve marketplace, production resource-limit enforcement, billing, and settlement blockers.',
        'Registry 2026-06-28.2 clears the stale P2.5 live-wiring blocker from autopilot.agent_world_scene.v1 without flipping the promise green. The running Autopilot Desktop Verse now has the complete flag-gated live path: chat-world-subscriptions.ts polls /api/public/pylon-stats into GotChatWorldScene, update.ts stores the latest ChatWorldPylonScene without resetting the controller, view.ts composes modelChatWorldScene into the behind-chat three-effect visualization, and payment particles are both event-pruned and idle-pruned by TickedChatWorldPaymentParticles so stale beams do not imply current activity on quiet networks. Focused desktop tests cover the pylon-stats subscription, scene reducer/view wiring, and idle beam expiry. The promise STAYS yellow on blocker.product_promises.agent_world_scene_not_default_on because the public product default-on / stay-flag-gated decision still needs explicit owner sign-off and any future green flip remains receipt-first per proof.claim_upgrade_receipts.v1. No spend, payout, settlement, runtime authority, or multiplayer authority is created by this visual wiring.',
        'Registry 2026-06-29.5 applies the owner-signed green transition for exactly two promises: metrics.khala_model_family_mix_public.v1 (#7016) and autopilot.agent_world_scene.v1 (#7030), moving green 32 -> 34 and clearing blockerRefs only on those two records. No Hosted Gemini (#7017), character-creation (#6861), payment/growth visualization, multiplayer, demand, revenue, spend, payout, settlement, routing, or broader default-on claim is created.',
        'Registry 2026-06-29.4 is the #7030 source-level default-gate receipt for autopilot.agent_world_scene.v1 and the attached payment/growth visualization records, and now also records the #7023 yellow-only Autopilot Desktop / builtin-compute proof destale; it flips NO promise state. For #7030, source evidence shows the Verse launch resolver defaults CHAT_WORLD_SCENE and CHAT_WORLD_PAYMENTS on when no Verse kill switch is set, keeps VITE_DISABLE_VERSE/VITE_VERSE_DISABLED as hard kill switches, and still requires realBitcoinMoved:true plus sourceRefs before any payment beam renders. For #7023, autopilot.desktop_gui_client.v1 now cites the owner-run AO6 from-DMG clean-Mac evidence bundle while autopilot.builtin_compute_agent.v1 cites source/test evidence for the bounded no-user-key metering smoke and keeps its signed-recut/live-smoke/metering blockers. This is NOT a green/default-on production claim: green still needs owner-reviewed shipped-channel evidence plus owner-signed receipt-first transitions. The old default-off/from-DMG-pending blockers are replaced only where backed by evidence, with explicit owner-review/green-pending blockers.',
        'Registry 2026-06-28.2: autopilot_sites.native_email_sequences.v1 adds the real Cloudflare Email binding path for authored sequence sends and flips NO promise state (stays yellow). The scheduled dispatcher now routes authored sequence templates through the sequence send service when supplied, `makeCloudflareEmailSequenceSender` renders a transactional sequence email, sends via the Worker `EMAIL` binding, and records `email_messages` + `email_deliveries` receipts; disabled config still records the dry-run/skipped path and never calls the sender. Domain auth and live-smoke instructions live at apps/openagents.com/docs/sites/2026-06-28-native-email-sequence-cloudflare-send-service.md. The remaining blocker is still email_deliverability_unproven: green requires a live deliverability smoke with send-to-deliver evidence, bounce/complaint handling, customer self-serve authoring, and owner sign-off per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-28.3 implements #6891 for models.tassadar_percepta_executor.v1 and flips NO promise state. Pylon v1.0 now has a deterministic bounded CPU computation-transform fixture in apps/pylon/src/tassadar-cpu-transform-training.ts that runs one CPU-only optimization step, self-verifies loss improvement, emits receipt.models.tassadar_percepta_executor.cpu_transform_training.cpu_transform_fixture_v1, and keeps realBitcoinMoved:false / settlementState:not_settled. GET /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts now projects that public-safe receipt alongside the architecture and Artanis distillation dataset inputs, so the old pylon_v03_cpu_transform_training_receipts_missing blocker is replaced by blocker.product_promises.tassadar_cpu_transform_real_settlement_missing and blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing. The promise STAYS planned: this is one fixture-scale receipt, not a trained model, not a paid earning path, not model promotion, and not a green transition; any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-29.1 implements #6848 for payments.accepted_outcome_economics.v1 and flips NO promise state. The accepted-outcome economics spine now has a dereferenceable contributor accrual bundle at GET /api/public/payments/contributor-accrual-bundle?economicsId=... plus the settlement bundle at GET /api/public/accepted-outcome/settlement/{economicsId}; together they compose a gross-margin receipt, contributor accrual ledger entries, and an eight-state settlement machine from one stored accepted-outcome economics row. Tests prove the ledger and receipt share the same economicsId, reconcile gross margin exactly, reconcile pending_payout to the distributable ledger pool, keep public projections free of internal cents/raw payment material, and surface missing contributor provenance honestly. The old source-level blockers contributor_ledger_missing and gross_margin_receipts_missing are replaced by real_accepted_outcome_receipt_missing and owner_signed_green_transition_missing. The promise STAYS red because source/fixture receipt machinery is not a real accepted outcome carried through a money-moving settlement path, and any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
        'Registry 2026-06-29.3 narrows the Apple FM local-mode blockers after #7022 audit and flips NO promise state. Source evidence now covers the Pylon helper-supervision policy, status projection, driver, launcher, and opt-in host lifecycle seam, so the broad helper-supervision blocker is replaced with the narrower signed/from-install smoke blocker. Apple FM local mode remains yellow until a signed/notarized installer recut proves the packaged or supervised helper from a clean install on admitted Apple Silicon. Registry 2026-06-29.2 remains the prior current-main refresh after #6997/#6999/#7001/#7002/#7006: terminal-agent current-state and Codex tool-layer evidence, codex-supervisor LOCKOUT replenishment housekeeping, GLM own-capacity failover alerting, Khala Desktop now carries source-level Apple FM sidecar packaging/readiness, and the Khala model-mix promise remains live-at-read with maxStalenessSeconds:0; all flipped no promise state.',
        'Registry 2026-06-29.2 is a current-main refresh after #6997/#6999/#7001/#7002/#7006 and flips NO promise state. The terminal-agent current-state audit and Codex tool-layer study are now cited as evidence for the Codex/Probe/Pylon runtime direction: current production coding delegation is still Pylon plus external agent SDK lanes and OpenAgents-native terminal tools remain a consolidation task, not a new green claim. The codex-supervisor LOCKOUT replenishment helper can create or reuse three bounded standing issues so owner-capacity supervisors do not idle indefinitely, but it creates no paid labor, payout, settlement, or broad availability claim. The inference router now has GLM own-capacity failover alerting and public-safe fallback telemetry for repeated no-headroom saturation, but the paid gateway still stays red until a dereferenceable paid receipt exists. Khala Desktop now carries source-level Electrobun Apple FM sidecar packaging/readiness plus redaction tests, but Apple FM local mode remains yellow until a signed/notarized from-install smoke with helper supervision exists. The Khala model-mix promise remains live-at-read with maxStalenessSeconds:0; the stale 2-second cache wording is not applied.',
        'Registry 2026-06-29.3 implements #7027 for the world-first / largest-force promise audit and flips NO promise state. docs/promises/2026-06-29-world-first-claims-7027-audit.md is now load-bearing evidence for claims.world_first_ai_training_paid_bitcoin.v1, claims.world_first_public_llm_computer_training_run.v1, claims.pursued_world_first_largest_agentic_sales_force.v1, and claims.pursued_world_first_largest_sales_force.v1. The audit records exact allowed wording, refuse-list wording, prior-art caveats, receipt refs, and dated blocker notes. The two world-first claims stay red; the two largest-force records stay planned pursuits; no owner-signed transition receipt, record-holder status, green claim, payout, settlement, or marketing-claim authority is created.',
        'Registry 2026-06-29.3 is a fleet-state/public-board sync after #6995/#7004 and flips NO promise state. The owner-scoped /api/operator/fleet/state route and Khala fleet client wiring are now cited as CLI/operator evidence, but only for linked-owner status visibility; the legacy admin fleet status route remains admin-token-only and the state route creates no dispatch, spend, payout, settlement, or cross-owner capacity authority. The public Artanis page now renders a fleet map and active task board from public Pylon stats/activity timeline data, but that is a public observability surface only; it is not assignment-specific proof, guaranteed availability, or a revenue/settlement claim.',
      ],
    },
    promises: [
      {
        ...basePromiseFields,
        promiseId: 'repo.open_source_code_map.v1',
        productArea: 'source transparency',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'green',
        claim:
          'The code currently shipped for the public OpenAgents site and core public product surfaces is open source and findable in the public openagents monorepo.',
        safeCopy:
          'Use the public code map to find the live site, Worker/API, web UI, public docs, Pylon, Probe, and product-promise source in the OpenAgentsInc/openagents repository.',
        unsafeCopy:
          'Do not claim secrets, production data, Cloudflare account resources, wallet material, customer-private workroom content, provider credentials, or third-party service internals are public.',
        evidenceRefs: [
          'https://github.com/OpenAgentsInc/openagents',
          'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com',
          'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/workers/api',
          'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/apps/web',
          'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/docs/live',
          'https://github.com/OpenAgentsInc/openagents/tree/main/docs/promises',
          'https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon',
          'https://github.com/OpenAgentsInc/openagents/tree/main/packages/probe',
        ],
        blockerRefs: [],
        verification:
          'Fetch /api/public/product-promises and /.well-known/openagents.json, then follow sourceCode/liveSiteSource/workerSource/webSource/productPromiseSource/pylonSource/probeSource to the public GitHub tree.',
        authorityBoundary:
          'Open-source source availability is a transparency claim only. It does not grant write, deploy, spend, moderation, data-access, settlement, or account authority, and it does not make any separate feature green.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.repo_study_packets.v1',
        productArea: 'Autopilot repo studying',
        audience: ['agent', 'operator', 'developer', 'public'],
        state: 'yellow',
        claim:
          'OpenAgents is dogfooding public, refs-only StudyBench study packets on its own public repository to improve OpenAgents-codebase work under Probe, Forge, Blueprint, and product-promise gates.',
        safeCopy:
          'OpenAgents is dogfooding source-grounded study packets on its own public repo. The current evidence is internal OpenAgents lift only; customer repo studying, marketplace packaging, payout eligibility, and paid work remain separately gated.',
        unsafeCopy:
          'Do not say OpenAgents has a trained repo expert, customer repo studying is live, a marketplace package, payout eligibility, or automatic paid work from StudyBench rows or study packets.',
        evidenceRefs: [
          'docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md',
          'docs/research/machine-studying/2026-06-17-blueprint-marketplace-ties.md',
          'docs/research/machine-studying/openagents-studybench/private-boundary.md',
          'docs/research/machine-studying/openagents-studybench/study-packets/openagents-launch-study-packet-v0.md',
          'docs/research/machine-studying/openagents-studybench/runs/2026-06-17-mvp-14-baseline-packet-gepa-comparison.md',
          'packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json',
          'docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md',
          'packages/probe/packages/runtime/src/benchmark/openagents-customer-private-validation.ts',
          'packages/probe/packages/runtime/tests/openagents-customer-private-validation.test.ts',
          'promise:repo.open_source_code_map.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.repo_studying_privacy_review_missing',
          'blocker.product_promises.repo_studying_marketplace_metering_missing',
          'blocker.product_promises.repo_studying_pricing_package_policy_missing',
          'blocker.product_promises.repo_studying_payout_settlement_gates_missing',
          'blocker.product_promises.repo_studying_product_copy_review_missing',
        ],
        verification:
          'Yellow is limited to the recorded MVP-14 OpenAgents public-safe comparison plus docs review. Green or external copy requires customer-data privacy review, private validation/holdout discipline, marketplace package policy, usage metering, pricing, payout eligibility, settlement receipts, and product-promise preflight. Public score summaries must keep productPromiseBoundary.publicProductClaimAllowed false until those gates pass.',
        authorityBoundary:
          'A StudyBench row or study packet is evidence and repository-memory input only. It grants no runtime mutation, customer repository ingestion, marketplace listing, billing, payout eligibility, settlement, training promotion, or public green-claim authority, and it is not paid work.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.external_repo_studying_pilot.v1',
        productArea: 'Autopilot repo studying',
        audience: ['agent', 'operator', 'developer'],
        state: 'yellow',
        claim:
          'OpenAgents exposes a refs-only external-repo studying pilot surface that runs the study-packet, graph, verification, eval, and coder-context pipeline on a non-OpenAgents fixture repository.',
        safeCopy:
          'External-repo studying is available as a gated pilot projection with refs and hashes only. It is not self-serve customer repo ingestion and it does not make marketplace, payout, settlement, or trained repo-expert claims.',
        unsafeCopy:
          'Do not say customer repo studying is live, any private customer repo can be uploaded, OpenAgents has a trained repo expert, study packets are marketplace packages, or external-repo studying is payout eligible.',
        evidenceRefs: [
          'packages/probe/packages/runtime/src/benchmark/external-repo-studying-product.ts',
          'packages/probe/packages/runtime/tests/external-repo-studying-product.test.ts',
          'packages/probe/packages/runtime/src/benchmark/external-repo-studying-pilot-admission.ts',
          'packages/probe/packages/runtime/tests/external-repo-studying-pilot-admission.test.ts',
          'packages/probe/packages/runtime/src/benchmark/openagents-customer-private-validation.ts',
          'docs/research/machine-studying/openagents-studybench/private-boundary.md',
          'docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md#phase-6',
          'promise:autopilot.repo_study_packets.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.external_repo_studying_privacy_policy_missing',
          'blocker.product_promises.external_repo_studying_self_serve_upload_missing',
          'blocker.product_promises.external_repo_studying_marketplace_metering_missing',
          'blocker.product_promises.external_repo_studying_pricing_package_policy_missing',
          'blocker.product_promises.external_repo_studying_payout_settlement_gates_missing',
        ],
        verification:
          'Yellow is limited to the S7 non-OpenAgents fixture pipeline: corpus manifest, study packet, graph, S3 verdict, S4-style eval lift, and coder context. Any real customer-private repo product requires admission policy, privacy review, customer review, self-serve controls, usage metering, pricing, payout eligibility, settlement evidence, and product-promise copy preflight.',
        authorityBoundary:
          'The external-repo studying pilot is repository-memory evidence only. It grants no private repo ingestion authority, write authority, runtime mutation, marketplace listing, billing, payout eligibility, settlement, training promotion, or green public customer claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'discovery.homepage_json.v1',
        productArea: 'agent-readable surfaces',
        audience: ['agent', 'public'],
        state: 'green',
        claim:
          'Agents can discover a JSON representation of the homepage data and the live public data endpoints behind it.',
        safeCopy:
          'Agents can read /api/public/home for a public-safe homepage JSON index.',
        unsafeCopy:
          'Do not treat homepage discovery as write, spend, deploy, moderation, or settlement authority.',
        evidenceRefs: [
          'https://openagents.com/api/public/home',
          'https://openagents.com/.well-known/openagents.json',
          'https://openagents.com/api/openapi.json',
        ],
        blockerRefs: [],
        verification:
          'GET /api/public/home returns schemaVersion openagents.public_home.v1 and links to current public data endpoints.',
        authorityBoundary:
          'Discovery does not grant write, spend, posting, deployment, moderation, or settlement authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'promises.registry.v1',
        productArea: 'product promises',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'green',
        claim:
          'OpenAgents publishes a versioned public registry of product promises so users and agents can tell what is live, scoped, gated, degraded, or planned.',
        safeCopy:
          'Read /api/public/product-promises and include its version when reporting a mismatch.',
        unsafeCopy:
          'Do not discuss a mismatch without the version and promiseId when those are available.',
        evidenceRefs: [
          'https://openagents.com/api/public/product-promises',
          'https://openagents.com/docs/product-promises',
          'https://github.com/OpenAgentsInc/openagents/tree/main/docs/promises',
        ],
        blockerRefs: [],
        verification:
          'GET /api/public/product-promises returns schemaVersion, version, states, reportPath, sourceRefs, currentMonorepoStatus, and promise records.',
        authorityBoundary:
          'Promise state is a public claim ledger. It does not by itself enable runtime actions or resolve reports without review.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.historical_claude_code_mechsuit.v1',
        productArea: 'Autopilot',
        audience: ['agent', 'user', 'public'],
        state: 'withdrawn',
        claim:
          'Episode 199 framed Autopilot as a Claude Code-first mech-suit around an agent SDK port and overnight coding-loop experiments.',
        safeCopy:
          'Treat Episode 199 Claude Code-first mech-suit language as historical source material only.',
        unsafeCopy:
          'Do not advertise OpenAgents as a current Claude Code-first mech-suit product.',
        evidenceRefs: [
          'https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/199.md',
          'https://github.com/OpenAgentsInc/openagents/blob/main/docs/promises/registry.md',
        ],
        blockerRefs: [
          'blocker.product_promises.stale_launch_framing_withdrawn',
        ],
        verification:
          'Public docs and product copy should route current coding-agent runtime claims through Codex-oriented Autopilot/Probe/Pylon records instead of this historical framing.',
        authorityBoundary:
          'Historical transcript language does not authorize current product positioning, provider dependency, runtime support, or deployment claims.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.codex_probe_pylon_successor.v1',
        productArea: 'Autopilot',
        audience: ['agent', 'user', 'operator'],
        state: 'green',
        claim:
          'The useful parts of the historical coding-agent wrapper idea are now represented by the current Codex-oriented Autopilot/Probe/Pylon runtime direction.',
        safeCopy:
          'OpenAgents coding-agent runtime work is Codex-oriented where applicable, with Probe/Pylon carrying relevant runtime and provider-account ideas under current gates. The Codex executor lane is now built and live-proven (epic #4793, CX1-CX5 #4788-#4792): a readiness probe over owner-held credentials declares capability.pylon.local_codex, the bounded @openai/codex-sdk executor runs read-only/workspace-write only with network disabled, and the lane carries a live device receipt for a real Codex SDK task plus a live API-parity receipt for an API-submitted git_checkout task on a codex-only Pylon with independent bun test verification and accepted closeout. The local supervised Codex composer/danger/doctor surface shipped in source (#4839/#4840/#4841), and the 2026-06-29 terminal-agent audits plus supervisor replenishment pass record the current substrate and gaps without widening the green scope. Remaining gates: stable v1.0 packaged release, the dev check/apply/reload loop (#4842), work-submit commit pinning plus adapter intent (#4843), and a unified OpenAgents-native terminal-tool runtime.',
        unsafeCopy:
          'Do not claim the old Claude Code-first launch framing is the current implementation, or that the current Codex/Probe/Pylon path is fully green.',
        evidenceRefs: [
          'transition:promise_transition_c2328369-09d0-4972-9a3b-8d530440cade',
          'https://github.com/OpenAgentsInc/openagents/issues/4857',
          'work_order:autopilot_work_order.a1aef38e-66e7-488f-a06c-05dd02b34b35',
          'route:/api/autopilot/work?promiseId=autopilot.codex_probe_pylon_successor.v1',
          'apps/pylon/README.md',
          'packages/probe',
          'apps/openagents.com/docs/probe/2026-06-07-first-party-probe-runtime-audit.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4793',
          'apps/pylon/src/codex-agent.ts',
          'apps/pylon/src/codex-agent-executor.ts',
          'apps/pylon/docs/codex-bridge.md',
          'apps/pylon/docs/codex-agent-task-smoke.md',
          'docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md',
          'assignment.closeout.f264043a9f173b20514521da',
          'docs/launch/2026-06-19-coding-agent-live-verification.md',
          'docs/research/terminal-agents/openagents-current-state.md',
          'docs/research/terminal-agents/codex.md',
          'apps/pylon/scripts/codex-supervisor/replenishment.sh',
          'apps/pylon/scripts/codex-supervisor/replenishment.test.sh',
        ],
        blockerRefs: [],
        verification:
          'The Codex-backed task path evidence, Probe/Pylon runtime smokes, public docs, and live assignment/closeout evidence this verification names all exist: CX4 ran the live device leg (closeout assignment.closeout.f264043a9f173b20514521da, capability-gated under capability.pylon.local_codex, redaction scan clean), and the API-parity git_checkout leg closed with independent test verification. The 2026-06-29 terminal-agent audits confirm the current split between production Pylon SDK delegation and Probe/OpenAgents-owned tool primitives, and the supervisor replenishment tests prove bounded issue refill under lockout without creating public paid-work authority. blocker.product_promises.live_probe_pylon_runtime_gates_incomplete is cleared on the CX evidence (the maintainer edit the CX4 transition proposal asked for). Daily-driver gates (stable v1.0 package, #4842, #4843) bind the Pylon release promises, not this successor-direction claim.',
        authorityBoundary:
          'A successor direction does not imply broad provider support, unattended writes, payment settlement, or live marketplace authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.v03_release_candidate.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'agent', 'operator'],
        state: 'green',
        claim:
          'Pylon is in the monorepo as the v1.0 contributor node and includes the former Probe runtime surface.',
        safeCopy:
          'The stable Pylon v1.0 source cut is present, @openagentsinc/pylon@latest resolves to the v1.0 line, and the live install guide is Pylon-first. macOS and Linux operators should use https://openagents.com/INSTALL.md. Network-wide earning remains gated by live assignment, validation, scale, platform coverage, helper readiness, and settlement receipts.',
        unsafeCopy:
          'Do not claim that anyone on any platform can install Pylon and automatically earn Bitcoin. Auto-update is default-on, signature-verified, and now PROVEN live in production (2026-06-16): an installed standalone binary fetched the signed feed at updates.openagents.com, verified the artifact sha256 + ed25519 against the pinned key (fail-closed), and atomically applied it. The signed-binary OTA feed and npm package publish are separate surfaces; the feed advances only when the signed-binary publish flow runs.',
        evidenceRefs: [
          'apps/pylon/package.json',
          'apps/pylon/scripts/build-rc-binaries.sh',
          'apps/oa-updates/keys/release-pubkey.json',
          'https://updates.openagents.com/pylon/rc/darwin-arm64/feed.json',
          'route:/api/pylons#pylon.33afd48282a649047e3a',
          'apps/oa-updates/docs/release-signing-runbook.md',
          'apps/oa-updates/scripts/publish-pylon-release.ts',
          'apps/openagents.com/apps/web/public/INSTALL.md',
          'forum:release-candidates',
          'apps/pylon/docs/launch-gates-no-overclaim.md',
          'docs/autopilot-coder/2026-06-15-rc-tester-install-guide.md',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
        ],
        blockerRefs: [],
        verification:
          'apps/pylon package metadata reports @openagentsinc/pylon@1.0.5, and npm view @openagentsinc/pylon reports latest=1.0.5 on 2026-06-19. Green (MET, owner-authorized 2026-06-20): v1.0.5 signed-binary feed live at updates.openagents.com (Cloud Run oa-updates-00041-b7b, verified sha256+ed25519 vs the pinned release-pubkey, fail-closed/tamper-rejected) + a live network smoke (pylon.33afd48282a649047e3a online, openagents.pylon@1.0.5). macOS+Linux only (Windows out of scope); network-scale earning remains separate gates.',
        authorityBoundary:
          'A local package install does not prove live OpenAgents network registration, paid work, payout, settlement, or marketplace authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.release_tomorrow.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'public'],
        state: 'green',
        claim: 'A new version of Pylon releases.',
        safeCopy:
          'The Pylon v1.0 source cut has landed for macOS and Linux, @openagentsinc/pylon@latest resolves to the v1.0 line, and the install guide is Pylon-first. Signed-binary/feed rollout, live deployment, and earning readiness are separate gates. Windows/WSL is not covered by current install evidence and must not be folded into broad "anybody" copy.',
        unsafeCopy:
          'Do not claim a stable universal Pylon release works on every computer, that Windows/WSL is covered, or that installing Pylon automatically earns Bitcoin. Auto-update is default-on, signature-verified, and now PROVEN live in production (2026-06-16): an installed standalone binary fetched the signed feed, verified sha256 + ed25519 against the pinned key (fail-closed), and atomically applied the update. The signed-binary OTA feed and the npm package are separate surfaces.',
        evidenceRefs: [
          'apps/pylon/package.json',
          'apps/pylon/scripts/build-rc-binaries.sh',
          'apps/openagents.com/apps/web/public/INSTALL.md',
          'apps/pylon/docs/launch-gates-no-overclaim.md',
          'apps/pylon/docs/platform-support.md',
          'apps/oa-updates/docs/release-signing-runbook.md',
          'https://updates.openagents.com/pylon/rc/darwin-arm64/feed.json',
          'route:/api/pylons#pylon.33afd48282a649047e3a',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
        ],
        blockerRefs: [],
        verification:
          'npm view @openagentsinc/pylon reports latest=1.0.5 on 2026-06-19. Green (MET, owner-authorized 2026-06-20): the v1.0.5 signed-binary release published + verified on the live feed (Cloud Run oa-updates-00041-b7b) with a live network smoke (pylon.33afd48282a649047e3a, openagents.pylon@1.0.5). macOS+Linux only. Carries its own dereferenceable refs: the signed darwin-arm64 feed (HTTP 200, pylon 1.0.5, kid 2dbe811d, rollout 100) and the live registration route:/api/pylons#pylon.33afd48282a649047e3a.',
        authorityBoundary:
          'Release-package availability does not imply assignment readiness, wallet readiness, or earning readiness.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.first_real_model_training_run.v1',
        productArea: 'training',
        audience: ['contributor', 'public'],
        state: 'planned',
        claim: 'Pylon starts the first real model-training run.',
        safeCopy:
          'A bounded public remote two-device real-gradient training run (CS336 A1 scale, run.cs336.a1.real_gradient.demo) is live with digest-committed shard gradients computed on two physical contributor machines, cross-device deterministic-recompute and Freivalds-Merkle verification, merge/eval refs, a published loss-under-budget curve, and settled Lightning closeouts. The model ladder’s network-scale rungs have not run; GET /api/public/training/model-ladder-rungs now projects r2NetworkRungReceiptAvailable=false.',
        unsafeCopy:
          'Do not claim network-scale or unbounded Pylon model training is live; the live evidence is one bounded two-device A1-scale run.',
        evidenceRefs: [
          'transition:promise_transition_7e5325b3-c06b-484b-9724-1e4fb41421c0',
          'route:/api/training/runs/run.cs336.a1.real_gradient.demo',
          'route:/api/training/leaderboards/a1',
          'apps/openagents.com/docs/2026-06-11-cs336-a1-multi-device-real-gradient-evidence.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4678',
          'apps/pylon/docs/2026-06-09-pylon-psionic-ml-connection-audit.md',
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'route:/api/public/training/model-ladder-rungs',
          'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
          'docs/2026-06-10-cs336-distributed-homework-continuation-audit.md',
          'directive.owner.20260611.no_inference_focus_tassadar',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
        ],
        blockerRefs: [
          'blocker.product_promises.model_ladder_network_rungs_not_run',
        ],
        verification:
          'Requires public remote worker, shard, merge/eval/admission, payment, settlement, and projection refs before green copy. The honest green path is Tassadar executor training: curated verified-trace corpora through the CS336 rails (artanis.tassadar_evolution_loop.v1 Stages 1-3) graded by exact replay, then the model ladder’s network rungs (training.model_ladder.v1) on real contributor devices with commitment-backed verification and paid closeouts. As of registry 2026-06-20.40, GET /api/public/training/model-ladder-rungs reports r2NetworkRungReceiptAvailable=false and networkRungRemainingBlockerRefs=[blocker.product_promises.model_ladder_network_rungs_not_run]. Qwen fine-tuning is out of scope by owner decision.',
        authorityBoundary:
          'GEPA/text optimization and local loopback rehearsals are not neural-network training on public contributor devices.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.public_distributed_training_run.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'Pylons participate in public distributed model-training runs with visible run state, verified work, reported results, and contributor payment for useful work.',
        safeCopy:
          'A bounded two-device CS336 A1-scale run is evidenced under pylon.first_real_model_training_run.v1, and the live run run.tassadar.executor.20260615 has now settled real Bitcoin to FIVE distinct independent contributors (1,020 sats total, qualifiedContributorCount 5, acceptedTraceCount 11) under training.decentralized_training_launch.v1 — so the "settlement refs for more than one contributor" criterion is now MET. A broad public distributed training run is still not green: the participant-count/network-scale methodology is published (docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md, with a stated >= 50 qualified-contributor network-scale threshold), and GET /api/public/training/public-distributed-run-scale now projects the current live counters against that threshold: currentScaleLabel=canary_scale, qualifiedContributorCount=5, acceptedTraceCount=11, networkScaleThresholdMet=false, ownerSignedUpgradeAvailable=false, and greenGateSatisfied=false. The remaining gate is broad accepted-work receipts that clear the documented threshold — the five bounded canary-scale settlements / 11 accepted units do not. This is the training.* record closest to a yellow upgrade.',
        unsafeCopy:
          'Do not claim a public network-scale training run is live, open for broad contribution, or paying contributors at scale. Five distinct verified, settled contributors prove multi-contributor real settlement exists but not network scale; the broad run, broad accepted-work receipts beyond canary scale, and a published participant-count methodology do not yet exist.',
        evidenceRefs: [
          'docs/transcripts/224.md',
          'docs/transcripts/227.md',
          'docs/transcripts/236.md',
          'docs/promises/registry.md',
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/promises/2026-06-19-training-live-run-evidence-destale.md',
          'docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md',
          'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
          'docs/launch/vertex-fleet/training.public_distributed_training_run.v1.md',
          'route:/api/public/training/public-distributed-run-scale',
          'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements',
          'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.ts',
          'apps/openagents.com/workers/api/src/training-public-distributed-run-scale-routes.ts',
          'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.test.ts',
          'promise:pylon.first_real_model_training_run.v1',
          'promise:training.decentralized_training_launch.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.public_distributed_training_run_receipts_missing',
        ],
        verification:
          'Green requires a public run definition, start/end state, participant admission and count methodology, task/work receipts, verification/eval evidence, payment and settlement refs for more than one contributor, and stale-state handling. As of registry 2026-06-19.7 the multi-contributor settlement criterion is satisfied: the live /settlements feed enumerates five distinct realBitcoinMoved:true contributors (1,020 sats), each backed by a Verified exact_trace_replay challenge. As of registry 2026-06-19.8 the participant-count/network-scale methodology is published and dereferenceable at docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md (it reuses the code-anchored qualifiedContributorCount counting rule, states the three scale axes — distinct-contributor, accepted-work, settlement — and writes down the network-scale threshold: >= 50 qualified contributors with sustained accepted work and broad realBitcoinMoved:true settlement, distinct from the 200-contributor largest-run benchmark). As of registry 2026-06-20.42, GET /api/public/training/public-distributed-run-scale reads the existing public run summary and settlement reconciliation and projects qualifiedContributorCount=5, acceptedTraceCount=11, realSettlementReceiptCount=5, networkScaleThresholdMet=false, ownerSignedUpgradeAvailable=false, and greenGateSatisfied=false. The remaining gate therefore narrows to broad accepted-work receipts that clear that documented network-scale threshold; the five bounded canary-scale settlements / 11 accepted units satisfy existence-of-multi-contributor-settlement but not network-scale contribution.',
        authorityBoundary:
          'A launch transcript or bounded demo run does not authorize network-scale training, contributor admission, payout, or model-quality claims.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.decentralized_training_launch.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'public'],
        state: 'green',
        claim:
          'OpenAgents/Pylon launched a scoped decentralized training run where contributors install node software, complete useful work, and have that work independently verified on public run rails.',
        safeCopy:
          'Launched for the scoped proof, and as of registry 2026-06-19.7 FIVE distinct independent contributors have now been paid real Bitcoin. The public run `run.tassadar.executor.20260615` is active (qualifiedContributorCount 5, acceptedTraceCount 11), and the decentralized contribution loop is proven end-to-end in the open: an independent contributor installs Pylon, claims a window lease, and submits a Tassadar executor trace; an independent validator on a separate machine/identity replays the pinned fixture and the verification challenge finalizes `Verified`. The real paid-settlement gate is met for a bounded scope and has now produced five counted run-settlement receipts, native over Spark, each with `realBitcoinMoved:true`, `moneyMovement:real_bitcoin`, and `state:settled`: a 1,000-sat owner-armed canary to the first independent contributor plus four 5-sat self-serve settlements to four further distinct independent contributors who came through the public install→register→claim→submit→independent-validation path. Real settled total for the run feed is 1,020 sats (1,000 + 4×5); the enumerable per-run settled feed is `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements` (five counted real rows plus one excluded `realBitcoinMoved:false` simulation row). A later visibility capture for the same `10c3b01b` challenge records the public timeline sequence `trace_submitted -> verification_verified -> real_bitcoin_moved -> settlement_recorded`, generated replay `proof_replay_bundle.public_activity.73e66071`, and receipt `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker` with `realBitcoinMoved:true`; that capture is evidence-presentation only, carries the documented `operator_approval.tassadar.autostream.worker` source-ref caveat, and does not widen this bounded green launch claim. The earlier 5-sat simulation receipt remains valid historical context as a simulation-backed (`realBitcoinMoved:false`) record that proved the settlement-record/projection path — it is not real Bitcoin movement and is excluded from the 1,020 real total. These are bounded canary-scale settlements and visibility evidence, not network-scale paid training.',
        unsafeCopy:
          'Do not claim this is a network-scale, large, or the largest decentralized training run, that hundreds of contributors are paid, that contributors are being paid at scale, that public gradients mutate a canonical model, or that any unbounded payout authority exists. The counted real paid-settlement proof is exactly five bounded run settlements (1,000-sat canary + four 5-sat self-serve) to five distinct independent contributors (`realBitcoinMoved:true`), 1,020 sats total; copy must stay scoped to those five counted settlements and must not extrapolate to broad earning, network scale, or hundreds-paid. Do not treat the auto-stream visibility capture as a broad "anybody automatically earns" proof, a third counted run-settlement total, or owner-signed claim upgrade. Do not claim the run has constructed a new model capability beyond the fixed executor workload. The prior simulation-backed receipt still does not prove real sats moved.',
        evidenceRefs: [
          'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615',
          'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements',
          'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c',
          'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
          'receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1',
          'training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4',
          'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.final.20260619T003201.manual.v1',
          'training.verification.challenge.335df7e8-2ae1-4d49-a6bd-c1491bc9f067',
          'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched.20260619T004804.manual.v1',
          'training.verification.challenge.33d4ca81-8beb-4d80-a90c-cb21d6d0aeb1',
          'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1',
          'training.verification.challenge.9fd49062-f82c-46ee-a2a0-242d36dd126e',
          'training.verification.challenge.59ba1f30-c2f0-40b0-b3ec-b9c5e1fb5316',
          'receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2',
          'docs/promises/2026-06-19-training-live-run-evidence-destale.md',
          'docs/launch/2026-06-19-autostream-settlement-visibility-capture.md',
          'docs/launch/2026-06-19-autostream-settlement-clip-manifest.json',
          'proof_replay_bundle.public_activity.73e66071',
          'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker',
          'docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md',
          'docs/promises/2026-06-17-training-monday-simulation-settlement-policy.md',
          'docs/transcripts/236.md',
          'docs/launch/JUNE16_ROADMAP.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4855',
          'https://github.com/OpenAgentsInc/openagents/issues/5232',
        ],
        blockerRefs: [],
        verification:
          'GET /api/public/training/runs/run.tassadar.executor.20260615: run state active, summary.metrics.qualifiedContributorCount >= 1, and Verified exact_trace_replay verification challenges with settlement_recorded receipts linked to the run. GET /api/public/training/runs/run.tassadar.executor.20260615/settlements enumerates the per-run settled rows: as of registry 2026-06-19.7 it returns five counted `realBitcoinMoved:true` rows (1,000-sat canary to the first contributor plus four 5-sat self-serve settlements to four further distinct contributors, 1,020 real total) plus one `realBitcoinMoved:false` simulation row that is excluded from the real total and must not be counted as real Bitcoin movement; the run summary reports qualifiedContributorCount 5, acceptedTraceCount 11, providerConfirmedSettledPayoutSats 1,020. The real paid-settlement gate is satisfied by receipt receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618 (backed by public challenge /api/public/training/verification-challenges/training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c), receipt receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1 (challenge .10c3b01b-c781-4a03-a8ed-4ae6c6195fe4), and the three 2026-06-19 self-serve receipts receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.final.20260619T003201.manual.v1 (challenge .335df7e8-2ae1-4d49-a6bd-c1491bc9f067), ...ao6.patched.20260619T004804.manual.v1 (challenge .33d4ca81-8beb-4d80-a90c-cb21d6d0aeb1), and ...ao6.patched2.20260619T010148.manual.v1 (challenge .9fd49062-f82c-46ee-a2a0-242d36dd126e), all realBitcoinMoved:true, moneyMovement:real_bitcoin, state:settled, adapter:spark_treasury. Issue #5438 adds a public visibility capture for the same `10c3b01b` challenge: activity timeline, generated replay bundle proof_replay_bundle.public_activity.73e66071, local render manifest docs/launch/2026-06-19-autostream-settlement-clip-manifest.json, and receipt receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker with realBitcoinMoved:true. Treat that as visibility evidence with its documented operator_approval and R2 caveats, not as a new aggregate run-total or broad public-claim upgrade. Broadening beyond these bounded settlements requires more public settled receipts with `realBitcoinMoved:true`, a participant-count methodology, accepted-work receipts for more contributors, platform coverage, and current helper readiness evidence.',
        authorityBoundary:
          'A launched, verified loop, five counted bounded real settlements (1,000-sat canary + four 5-sat self-serve, 1,020 sats total), and one auto-stream visibility capture do not authorize network-scale training claims, paid-at-scale or hundreds-paid claims, contributor admission beyond the published rules, largest-run comparisons, canonical-checkpoint mutation from public gradients, broad "anybody automatically earns" copy, or any unbounded spend. The earlier simulation-backed settlement record proves the projection path only, not real movement.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.largest_decentralized_training_claim.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'OpenAgents/Pylon can make or beat a largest decentralized training run claim against a 200-contributor benchmark.',
        safeCopy:
          'Do not make a largest-run claim yet; Episode 236 renews the target, and the count methodology plus comparable training-run research are now documented. GET /api/public/pylon/largest-decentralized-training-claim compares the current public run to the ~70 contributor comparable and 200 contributor target, and it reports the current run remains far below comparable scale.',
        unsafeCopy:
          'Do not say OpenAgents has the largest decentralized training run, has beaten Bittensor, or has 200+ contributors unless current comparable evidence exists.',
        evidenceRefs: [
          'docs/transcripts/222.md',
          'docs/transcripts/223.md',
          'docs/transcripts/236.md',
          'docs/promises/registry.md',
          'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
          'docs/training/2026-06-19-comparable-decentralized-training-runs-research.md',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
          'docs/launch/vertex-fleet/pylon.largest_decentralized_training_claim.v1.md',
          'route:/api/public/pylon/largest-decentralized-training-claim',
          'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.ts',
          'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status-routes.ts',
          'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.test.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.public_training_contributor_receipts_missing',
        ],
        verification:
          'Green requires participant count methodology, run definition, training evidence, accepted-work receipts, public verification, and a comparison rule that is current and comparable. The qualified-contributor counting rule is now written and dereferenceable (docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md, enforced in training-run-window-authority.ts) and the comparable runs are documented with citations (docs/training/2026-06-19-comparable-decentralized-training-runs-research.md: Templar Covenant-72B ~70 contributors, ~200 is the transcript target). As of registry 2026-06-20.43, GET /api/public/pylon/largest-decentralized-training-claim reads the public distributed-run scale projection and reports qualifiedContributorCount=5, concreteComparableThresholdMet=false, transcriptTargetThresholdMet=false, ownerSignedUpgradeAvailable=false, and greenGateSatisfied=false. This clears the methodology and comparable-evidence gaps as written evidence only; the promise stays red because the live run has five counted realBitcoinMoved:true contributors, far below the comparable scale, so public_training_contributor_receipts_missing is unmet. No green flip without an actual comparable-scale run and an owner-signed receipt-first upgrade.',
        authorityBoundary:
          'Marketing comparisons and public status projections do not grant proof. Public copy must degrade to the receipts actually available; this projection grants no contributor admission, training dispatch, spend, settlement, benchmark victory, largest-run claim, network-scale claim, or product-promise transition authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'claims.world_first_ai_training_paid_bitcoin.v1',
        productArea: 'public claims',
        audience: ['operator', 'public'],
        state: 'red',
        claim:
          'OpenAgents ran the first AI model training run paid in Bitcoin to consumer compute (Episode 238 world-first claim).',
        safeCopy:
          'Do not make an unqualified "world first" claim yet. An independent web-research prior-art review (docs/launch/2026-06-18-world-firsts-verification.md) found the claim defensible only with the full qualifiers — first as "Bitcoin + replay-verified training compute + own consumer devices" together — and the live run did pay two independent contributors real Bitcoin for verified work (training.decentralized_training_launch.v1). The claim stays red pending an owner-signed receipt-first upgrade; until then any public use must carry the full qualifiers, not the bare "world first" phrasing.',
        unsafeCopy:
          'Do not say, on camera or in copy, that this is the first AI training run paid in Bitcoin to consumer compute without the full qualifiers, or any bare "world first" framing, until the receipt-first upgrade lands. Token-paid networks and data-bounty/inference precedents are not defeated by the bare phrasing. Do not extrapolate the two bounded canary-scale settlements (1,005 sats real total) into a network-scale claim.',
        evidenceRefs: [
          'docs/transcripts/238.md',
          'docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md',
          'docs/launch/2026-06-18-world-firsts-verification.md',
          'docs/promises/2026-06-29-world-first-claims-7027-audit.md',
          'promise:training.decentralized_training_launch.v1',
          'promise:promises.registry.v1',
          'promise:proof.claim_upgrade_receipts.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.world_first_evidence_pack_missing',
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ],
        verification:
          'An independent prior-art/competing-claim search now exists (docs/launch/2026-06-18-world-firsts-verification.md; prior art checked includes Spirit of Satoshi, Bittensor/Templar, Gensyn, Prime Intellect, Nous/Psyche, Salad, Percepta, Tracr) with a defensible narrowed wording. The #7027 dated audit (docs/promises/2026-06-29-world-first-claims-7027-audit.md) keeps this red because the prior-art review is not yet a single dereferenceable evidence pack tying each qualifier to live receipts, and no owner-signed transition receipt exists. Green still requires (1) a dereferenceable evidence pack tying the qualified world-first to the live run receipts and (2) an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1. Until then this stays red and any public use must carry the full qualifiers.',
        authorityBoundary:
          'A launch transcript does not establish a world-first. The bounded real settlements prove payment to two contributors, not a first-in-the-world claim, and grant no network-scale or comparison authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'claims.world_first_public_llm_computer_training_run.v1',
        productArea: 'public claims',
        audience: ['operator', 'public'],
        state: 'red',
        claim:
          'OpenAgents ran the first public LLM-computer training run (Episode 238 world-first claim).',
        safeCopy:
          'Do not make an unqualified "world first" claim yet. An independent web-research prior-art review (docs/launch/2026-06-18-world-firsts-verification.md) found the claim defensible as "first public/open-contributor LLM-computer training run," crediting Percepta as the paradigm originator. Separately, the live run is bounded exact-trace executor proof-of-concept work (compute.tassadar_executor_poc.v1); the LLM-computer core compiles programs into transformer weights with no gradient descent, so "training run" is true only in the executor-construction sense, not as gradient-descent model training. The claim stays red pending an owner-signed receipt-first upgrade.',
        unsafeCopy:
          'Do not say this is the first public LLM-computer training run without the qualifiers and Percepta credit, or any bare "world first" framing, until the receipt-first upgrade lands. Do not conflate exact-trace executor PoC work with gradient-descent model training, and do not claim general LLM-computer capability, performance parity, or transformers-as-a-served-product.',
        evidenceRefs: [
          'docs/transcripts/238.md',
          'docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md',
          'docs/launch/2026-06-18-world-firsts-verification.md',
          'docs/launch/2026-06-20-llm-computer-training-run-definition.md',
          'docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md',
          'docs/promises/2026-06-29-world-first-claims-7027-audit.md',
          'apps/openagents.com/workers/api/src/world-first-llm-computer-evidence-pack.test.ts',
          'promise:compute.tassadar_executor_poc.v1',
          'promise:models.tassadar_percepta_executor.v1',
          'promise:promises.registry.v1',
          'promise:proof.claim_upgrade_receipts.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ],
        verification:
          'An independent prior-art/competing-claim search now exists (docs/launch/2026-06-18-world-firsts-verification.md) with a defensible narrowed wording crediting Percepta as the paradigm originator. A precise definition of "LLM-computer training run" now exists (docs/launch/2026-06-20-llm-computer-training-run-definition.md): it pins the phrase to the executor-construction / exact-trace sense (sense B), distinguishes it from gradient-descent model training (sense A), credits Percepta, and enumerates the refuse-list so the phrase cannot overclaim against the no-gradient-descent executor PoC. A focused, dereferenceable evidence pack now exists (docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md): it ties the qualified Claim-2 world-first to the live-run receipts qualifier-by-qualifier (public/open-contributor paid loop -> run summary + two contributor settlement receipts; Percepta paradigm credit -> Percepta blog/transformer-vm + prior-art search; executor/exact-trace/replay-verified -> verified+rejected replay pairs and a Verified challenge), with a skeptic-runnable verification recipe and a refuse-list. The #7027 dated audit (docs/promises/2026-06-29-world-first-claims-7027-audit.md) records that only the owner-signed transition blocker remains. A regression guard (apps/openagents.com/workers/api/src/world-first-llm-computer-evidence-pack.test.ts) now machine-enforces that the pack and definition stay dereferenceable: every repo-relative doc they cite resolves on disk, every promise: evidence ref resolves to a real registry promiseId, the cleared definition/evidence-pack blockers stay cleared without flipping state, and the refuse-list plus Percepta credit stay in the copy. Green still requires an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1, and the underlying public paid loop remains bounded (two contributors, not at scale).',
        authorityBoundary:
          'A bounded exact-trace executor proof of concept grants no general LLM-computer capability claim and no world-first claim. The Tassadar research publication gates stay closed for everything beyond the scoped compute.tassadar_executor_poc.v1 promise.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.consumer_compute_earns_bitcoin_self_serve.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'agent', 'operator', 'public'],
        state: 'red',
        claim:
          'Anybody can plug in consumer compute, join the Tassadar run, and get automatically paid Bitcoin for verified work — the Episode 238 core promise.',
        safeCopy:
          'The decentralized run is live, the default npm path now resolves to the Pylon v1.0 line (`@openagentsinc/pylon@latest` reported 1.0.5 on 2026-06-19), and the contribution loop is proven in bounded public evidence: independent contributors installed Pylon, claimed work, submitted executor traces, were independently validated, and two distinct independent contributors were paid counted real Bitcoin run settlements (1,005 sats real total). Issue #5438 also captured the first public auto-stream settlement visibility sequence for `training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4`, including replay bundle `proof_replay_bundle.public_activity.73e66071`, receipt `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker`, and a committed local clip manifest. The unqualified "anybody plugs in → automatically gets paid" promise is still not green: current install evidence is macOS/Linux, Windows/WSL coverage is not proven, Spark-helper auto-start/readiness is not receipt-proven for the normal contributor path, scale methodology is missing, and the #5438 clip manifest preserves operator-gate/R2 caveats.',
        unsafeCopy:
          'Do not claim that anybody on any platform can install today and automatically earn Bitcoin, that Windows/WSL is covered, that Spark-helper auto-start makes every normal contributor payout-ready, or that the bounded install→claim→verify→auto-stream evidence proves broad no-operator earning at scale. Do not extrapolate two counted run settlements plus one visibility capture into broad or network-scale consumer-compute earning.',
        evidenceRefs: [
          'docs/transcripts/238.md',
          'docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md',
          'docs/launch/2026-06-19-autostream-settlement-visibility-capture.md',
          'docs/launch/2026-06-19-autostream-settlement-clip-manifest.json',
          'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements',
          'https://openagents.com/api/public/proof-replays?mode=activity-timeline&from=2026-06-18T12:00:00.000Z&to=2026-06-18T14:00:00.000Z&limit=80',
          'https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker',
          'https://openagents.com/INSTALL.md',
          'https://registry.npmjs.org/@openagentsinc/pylon',
          'proof_replay_bundle.public_activity.73e66071',
          'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
          'apps/pylon/src/spark-helper-autostart.ts',
          'apps/pylon/src/spark-helper-autostart.test.ts',
          'apps/pylon/docs/platform-support.md',
          'promise:training.decentralized_training_launch.v1',
          'promise:pylon.v03_release_candidate.v1',
          'promise:pylon.install_without_wallet_knowledge.v1',
          'promise:proof.claim_upgrade_receipts.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing',
          'blocker.product_promises.windows_wsl_consumer_install_coverage_missing',
          'blocker.product_promises.spark_helper_autostart_receipt_missing',
        ],
        verification:
          'Do not upgrade to green until the current default install path, platform coverage, helper readiness, and scale methodology all match the public copy. Evidence now includes npm latest=1.0.5 (2026-06-19), bounded run settlements, the #5438 auto-stream visibility capture, a written participant/scale methodology (docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md, enforced in training-run-window-authority.ts), and an INERT flag-gated Spark-helper autostart readiness capability with a public-safe receipt builder (apps/pylon/src/spark-helper-autostart.ts, default off via PYLON_SPARK_AUTOSTART, 9 tests). Green still requires: (1) a current documented install path proven on the platforms named by the copy — Windows/WSL is a deliberate owner scope-out (apps/pylon/docs/platform-support.md), so the honest path is narrowing broad "anybody on any platform" wording to macOS/Linux, not building Windows support; (2) a real Spark-helper autostart-ready receipt captured for at least one normal contributor on the self-serve path (the capability is now built INERT; it must actually fire); (3) replay/receipt evidence for more than one normal contributor, not a single captured sequence; and (4) the participant/scale methodology applied before any broad earning copy. Upgrade receipt-first per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'A proven loop, two counted bounded settlements, one auto-stream visibility capture, and a v1.0 npm default do not authorize an unqualified self-serve consumer-compute earning claim, every-platform copy, automatic-payout authority, or network-scale earning copy.',
      },
      {
        ...basePromiseFields,
        promiseId: 'marketplace.agentic_npm_module_registry.v1',
        productArea: 'marketplace',
        audience: ['agent', 'developer', 'contributor', 'public'],
        state: 'planned',
        claim:
          'Verified programs become composable modules in an "agentic npm" registry — a library of verified, composable computation modules with built-in cryptographic verification and payments (Episode 238 "learning by construction").',
        safeCopy:
          'The agentic-npm / module marketplace is roadmap direction only. The transcript itself frames it as an upcoming video that reboots the earlier plugin marketplace. A bounded source-level registry + install/use runtime core exists: it can publish public-safe module specs into a store, discover modules, resolve dependency closures, gate every install on exact-trace/composition/link verification, materialize verified install records, invoke explicitly registered adapters, and write install + usage evidence rows. No paid public marketplace, arbitrary package execution, billing, attribution, rev-share, abuse handling, or settlement is live.',
        unsafeCopy:
          'Do not claim a live agentic-npm module registry, a working module marketplace, verified module composition as a product, public package discovery, paid module installs, execution, billing, or settled module-registry revenue. Do not present the inert resolver core or "learning by construction" as a shipped marketplace capability.',
        evidenceRefs: [
          'docs/transcripts/238.md',
          'docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md',
          'apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.ts',
          'apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.test.ts',
          'promise:marketplace.wasm_plugins.v1',
          'promise:marketplace.signature_monetization.v1',
          'promise:compute.tassadar_executor_poc.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.agentic_npm_billing_settlement_missing',
          'blocker.product_promises.agentic_npm_paid_public_marketplace_missing',
        ],
        verification:
          'Keep this planned until there is public evidence for a module package policy, authenticated/self-serve publication, install/uninstall lifecycle hardening, arbitrary package isolation policy, metering, billing, attribution, rev-share, abuse handling, and settlement receipts. The current core proves deterministic registry discovery, dependency closure resolution, verification-on-install, adapter-scoped invocation, and install/use evidence rows only.',
        authorityBoundary:
          'Roadmap framing and the source-level runtime core grant no paid marketplace, arbitrary package execution, metering, billing, settlement, or contributor-earning authority. The verified-module idea reuses the exact-trace verification proven under compute.tassadar_executor_poc.v1; that PoC does not make a paid marketplace live.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.public_gradient_windows.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'Public Pylons can contribute bounded, verified training windows (model updates) that advance a shared Psion/Tassadar student checkpoint.',
        safeCopy:
          'This is the decentralized-optimizer lane (RESEARCH_PLAN W5). Public Pylons may eventually contribute bounded, verified training windows to Psion/Tassadar student models; candidate updates enter quarantine first and can promote only after recompute/replicate, canary, and explicit promotion gates pass. The H1 gate, frozen-core validator, quarantine-intake predicate, promoted-window receipt emitter, and public status projection at GET /api/public/training/public-gradient-windows are code-backed. The source projection reports intakeSurface.predicateAvailable=true, but today public devices do generation, validation, and evaluation only; no live route accepts public gradient submissions, no quarantine store persists admitted windows, and no public contributor gradient window has been accepted, promoted, paid, or settled.',
        unsafeCopy:
          'Do not claim public Pylons train the canonical model today, that public gradients are accepted into the main optimizer, or that decentralized gradient training is live or paying. No public gradient enters the canonical optimizer until it passes quarantine, verification, canary, and promotion gates.',
        evidenceRefs: [
          'docs/tassadar/RESEARCH_PLAN.md',
          'docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md#track-h--hybrid-ring-later-gradients-enter-only-here-2d-4-item-5',
          'apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.ts',
          'apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.test.ts',
          'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
          'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.test.ts',
          'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt.ts',
          'apps/openagents.com/workers/api/src/tassadar-gradient-window-promotion-receipt.test.ts',
          'route:/api/public/training/public-gradient-windows',
          'apps/openagents.com/workers/api/src/training-public-gradient-windows.ts',
          'apps/openagents.com/workers/api/src/training-public-gradient-windows.test.ts',
          'docs/launch/vertex-fleet/training.public_gradient_windows.v1.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5332',
          'https://github.com/OpenAgentsInc/psionic/tree/main/crates/psionic-tassadar-student/src/hybrid.rs',
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4855',
          'promise:training.public_distributed_training_run.v1',
          'promise:compute.tassadar_executor_poc.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.public_gradient_live_window_runtime_missing',
          'blocker.product_promises.public_gradient_promoted_window_receipts_missing',
          'blocker.product_promises.public_gradient_settlement_receipts_missing',
        ],
        verification:
          'Green requires a live accepted-training-window runtime, checkpoint lineage and rollback storage, robust aggregation policy, dataset-shard authority, staged payout, and public receipts for at least one promoted public window plus settlement — all receipt-first per proof.claim_upgrade_receipts.v1. GET /api/public/training/public-gradient-windows is a live-at-read status projection only: it reports intakeAdmissionPredicateAvailable=true, intakeSurface.predicateAvailable=true, regimeGateAvailable=true, and promotionReceiptEmitterAvailable=true, but liveWindowRuntimeAvailable=false, intakeSurface.quarantineRouteAvailable=false, promotedWindowReceiptAvailable=false, settlementReceiptAvailable=false, and greenGateSatisfied=false. The H1 code gate, quarantine-intake predicate, and receipt emitter are necessary evidence, not a live public-training claim.',
        authorityBoundary:
          'A candidate update is not a model mutation. Submission grants no authority to change a canonical checkpoint; only gated promotion does, and contribution does not grant settlement, aggregation, or checkpoint-promotion authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'models.tasadar_percepta_executor.v1',
        productArea: 'models',
        audience: ['contributor', 'operator', 'public'],
        state: 'withdrawn',
        claim:
          'Withdrawn compatibility alias for the misspelled Tasadar promise record.',
        safeCopy:
          'Use models.tassadar_percepta_executor.v1 for the Episode 236 Tassadar/Percepta executor-model promise.',
        unsafeCopy:
          'Do not use the misspelled models.tasadar_percepta_executor.v1 id for new copy, routes, issues, or promise reports.',
        evidenceRefs: [
          'docs/transcripts/236.md',
          'docs/promises/registry.md',
          'docs/2026-06-12-episode-236-training-launch-gap-audit.md',
          'promise:models.tassadar_percepta_executor.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.misspelled_promise_id_withdrawn',
        ],
        verification:
          'Compatibility check: public consumers should treat this id as withdrawn and route new references to models.tassadar_percepta_executor.v1.',
        authorityBoundary:
          'A withdrawn alias grants no model, training, runtime, publication, or earning claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'models.tassadar_percepta_executor.v1',
        productArea: 'models',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'The Tassadar model direction uses a Percepta Executor Class architecture, with CPU computation transformation support added to Pylon v1.0 for experimental training.',
        safeCopy:
          'Episode 236 names a Tassadar/Percepta Executor Class direction. The public model/spec boundary is written down in docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md, a public-safe architecture receipt projection is live at GET /api/public/models/tassadar-percepta-executor/architecture-receipts with receipt.models.tassadar_percepta_executor.architecture.bundle.v1, and GET /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts now projects one bounded Pylon v1.0 CPU computation-transform fixture receipt: assignment.models.tassadar_percepta_executor.cpu_transform_fixture.v1 with receipt.models.tassadar_percepta_executor.cpu_transform_training.cpu_transform_fixture_v1, an accepted verifier verdict, and a fixture checkpoint digest. Real settlement and owner green sign-off remain missing, so the promise stays planned. The bounded executor proof of concept is green (compute.tassadar_executor_poc.v1) but proves exact replay only, not a model. The 2026-06-14 W3 student-program report validated the frozen-analytic-executor-plus-learned-interface research direction (baseline D reached exact-rollout pass@1 while purely-learned baselines failed) but is explicitly research/evaluation only: it creates no trained-model claim and does not make this promise green.',
        unsafeCopy:
          'Do not claim a Tassadar trained model exists, is trained, outperforms CPUs, replaces a CPU, has run broad or settled Pylon CPU-transform training, or is earning contributors Bitcoin, and do not present the architecture receipt, bounded CPU-transform fixture, W3 student-program results, or executor PoC as proof of a trained Percepta model.',
        evidenceRefs: [
          'docs/transcripts/236.md',
          'docs/2026-06-12-episode-236-training-launch-gap-audit.md',
          'docs/promises/2026-06-14-registry-reality-reconciliation-audit.md',
          'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
          'docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md',
          'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
          'route:/api/public/models/tassadar-percepta-executor/architecture-receipts',
          'route:/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts',
          'apps/openagents.com/workers/api/src/tassadar-percepta-architecture-receipts.ts',
          'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.ts',
          'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.test.ts',
          'apps/pylon/src/tassadar-cpu-transform-training.ts',
          'apps/pylon/tests/tassadar-cpu-transform-training.test.ts',
          'receipt.models.tassadar_percepta_executor.cpu_transform_training.cpu_transform_fixture_v1',
          'docs/tassadar/2026-06-14-w3-student-program-report.md',
          'promise:compute.tassadar_executor_poc.v1',
          'promise:artanis.tassadar_evolution_loop.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.tassadar_cpu_transform_real_settlement_missing',
          'blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing',
        ],
        verification:
          'The public model/spec boundary is documented at docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md, and the architecture receipt projection at /api/public/models/tassadar-percepta-executor/architecture-receipts carries receipt.models.tassadar_percepta_executor.architecture.bundle.v1 with public-safe refs for the model profile, Psionic compiled/frozen executor components, W3 baseline-D learned-interface components, checkpoint/interface/eval digests, and exact-trace verifier refs. The CPU-transform projection at /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts now binds that architecture receipt plus the Artanis distillation dataset receipt as visible inputs and reports cpuTransformTrainingReceiptAvailable=true, pylonAssignmentReceiptAvailable=true, acceptedWorkReceiptAvailable=true, verifierVerdictReceiptAvailable=true, trainedModelArtifactAvailable=true for a single fixture-scale CPU step, while realSettlementReceiptAvailable=false and greenGateSatisfied=false. Green still requires real settlement where money moved where applicable plus owner-signed promise-transition evidence under proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The scoped Tassadar executor PoC, architecture receipts, and bounded CPU-transform fixture receipt prove bounded exact replay, public architecture lineage, and one fixture-scale Pylon CPU step only; they do not prove a trained model, broad Pylon CPU-transform training, general model capability, inference endpoint, settlement, or paid earning path.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.five_bitcoin_revenue_streams.v1',
        productArea: 'payments',
        audience: ['contributor', 'public'],
        state: 'planned',
        claim:
          'Pylon stacks compute, data, Forum tips, referrals, and agent labor markets in one install.',
        safeCopy:
          'Forum tipping is live and the NIP-90 compute/data/labor market rails shipped in earlier releases (Episodes 213-215) exist in repo history, but one-install multi-stream Bitcoin earning is not live in the current app.',
        unsafeCopy:
          'Do not claim one Pylon install creates five live Bitcoin revenue streams, and do not describe the labor stream as provider-capacity resale; it sells agent work output, not account access.',
        evidenceRefs: [
          'transition:promise_transition_872cca1d-f6ae-4c43-97e5-2f527a70603f',
          'directive.owner.20260611.focus_tassadar_psion_cs336',
          'docs/transcripts/213.md',
          'docs/transcripts/214.md',
          'docs/transcripts/215.md',
          'apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4652',
          'apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md',
          'apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
        ],
        blockerRefs: [
          'blocker.product_promises.compute_stream_not_broadly_live',
          'blocker.product_promises.data_stream_not_live',
          'blocker.product_promises.referral_stream_not_live',
          'blocker.product_promises.labor_stream_not_live',
        ],
        verification:
          'Each revenue stream needs its own evidence refs, public-safe receipts, policy gates, and settlement state. The labor stream is the NIP-90 job market where contributors use their own agent capacity and sell results; it is not subscription resale.',
        authorityBoundary:
          'Forum, Sites, agent labor, data, and Pylon settlement are separate authorities.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.v0_3_multi_earning_node.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'agent', 'public'],
        state: 'red',
        claim:
          'Pylon v1.0 becomes one piece of node software that can earn Bitcoin in multiple ways, including training, Forum or coding-agent-adjacent work, and payment-integrated tasks.',
        safeCopy:
          'Pylon v1.0 has a stable source cut with scoped green/yellow subclaims; one-install multi-earning remains red until each earning mode has its own evidence.',
        unsafeCopy:
          'Do not claim one Pylon install currently earns Bitcoin across multiple modes automatically.',
        evidenceRefs: [
          'docs/transcripts/236.md',
          'docs/promises/registry.md',
          'promise:pylon.v03_release_candidate.v1',
          'promise:pylon.five_bitcoin_revenue_streams.v1',
          'promise:forum.content_tipping.v1',
          'promise:pylon.install_without_wallet_knowledge.v1',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
        ],
        blockerRefs: [
          'blocker.product_promises.pylon_v1_default_install_not_fully_closed',
          'blocker.product_promises.multi_earning_mode_receipts_missing',
          'blocker.product_promises.multi_earning_settlement_refs_missing',
          'blocker.product_promises.safe_public_projection_missing',
        ],
        verification:
          'Green requires stable v1.0 release evidence, install/platform smokes, assignment/work receipts, per-mode payment evidence, settlement evidence, and public projections that distinguish modeled, observed, pending, paid, and settled amounts.',
        authorityBoundary:
          'Separate Forum tipping, accepted-work closeout, training dispatch, labor-market, referral, data, and settlement authorities do not collapse into one broad earning claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.compute_revenue_modes.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'public'],
        state: 'planned',
        claim:
          'Compute revenue comes from GEPA optimization slices and Tassadar executor-trace work on people’s devices.',
        safeCopy:
          'Pylon v1.0 has GEPA-first local capability contracts and live no-spend Tassadar executor-trace dispatch with one operator-funded settled closeout, but paid full-network GEPA is not green. Local-inference resale and Qwen fine-tune revenue remain out of scope; the separate autopilot.local_apple_fm_tool_chat.v1 promise is only a planned user-owned local Autopilot tool/chat path, not a compute-revenue product.',
        unsafeCopy:
          'Do not claim full-network GEPA revenue, and do not describe local-inference resale or Qwen fine-tune products as existing or planned.',
        evidenceRefs: [
          'transition:promise_transition_774d0db0-1a2c-4f72-861e-6996a6684f0b',
          'directive.owner.20260611.focus_tassadar_psion_cs336',
          'apps/pylon/docs/gepa-capability-envelope.md',
          'apps/openagents.com/docs/2026-06-08-probe-gepa-paid-mode-campaign-ladder.md',
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'transition:promise_transition_4ba43958-3084-4c90-ab0d-10562a863117',
          'directive.owner.20260611.no_inference_focus_tassadar',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
        ],
        blockerRefs: ['blocker.product_promises.live_gepa_network_missing'],
        verification:
          'Requires live OpenAgents endpoint smokes, fresh Pylon heartbeats, accepted closeouts, and payment-mode evidence before broad compute revenue copy.',
        authorityBoundary:
          'Capability envelopes and retained fixtures do not authorize paid public work or settlement.',
      },
      {
        ...basePromiseFields,
        promiseId: 'data.free_tier_capture_disclosure.v1',
        productArea: 'data',
        audience: ['user', 'agent', 'contributor', 'public'],
        state: 'yellow',
        claim:
          'Free Khala API usage is captured by default when the owner-gated production capture flag is armed, as redacted, private traces that may be used to improve and train models; pay for privacy to opt out; public sharing is opt-in only.',
        safeCopy:
          'Free tier: when you use the free Khala API without paying for privacy, your traffic is captured by default when the owner-gated production capture flag is armed, as a redacted, private-by-default (owner_only) trace and may be used to improve and train OpenAgents models. Pay for privacy, or run confidential compute, to opt out of capture (fail-closed to not-captured). A captured trace is shared publicly only if its owner explicitly opts it into public visibility. Capture grants no payout or settlement — the data-market reward marker is inert and owner-gated. The canonical terms are served at GET /api/public/free-tier-data-sharing and embedded in the POST /api/keys/free mint response with explicit blocker/gate refs. Scope note (2026-07-01): this disclosure covers the hosted Khala API capture path only; the Khala Code desktop wrapper’s raw Codex events and ATIF traces are owner-private delegation observability, not free-plan capture, and the consented Khala Code free-plan capture pipeline is tracked separately as khala_code.free_plan_trace_capture.v1 (planned).',
        unsafeCopy:
          'Do not claim free traffic is never captured, do not claim captured traces are public by default, do not claim paid-privacy callers are captured, and do not claim capture earns the user a payout, reward, or settlement.',
        evidenceRefs: [
          'route:/api/public/free-tier-data-sharing',
          'route:/api/keys/free',
          'apps/openagents.com/workers/api/src/inference/free-tier-data-sharing-disclosure.ts',
          'apps/openagents.com/workers/api/src/inference/khala-chat-trace-emitter.ts',
          'apps/openagents.com/workers/api/src/inference/inference-privacy-entitlement.ts',
          'apps/openagents.com/apps/web/public/AGENTS.md',
          'docs/promises/2026-06-25-free-tier-data-sharing-disclosure.md',
          'docs/traces/2026-06-25-default-on-trace-capture-audit.md',
          'docs/transcripts/243.md',
        ],
        blockerRefs: [
          'blocker.product_promises.free_tier_capture_default_owner_gated',
          'blocker.product_promises.disclosure_copy_owner_signoff_pending',
          'blocker.product_promises.trace_capture_public_disclosure_alignment_required',
          'blocker.product_promises.trace_capture_reward_marker_inert',
          'blocker.product_promises.paid_privacy_owner_signoff_pending',
          'blocker.product_promises.paid_khala_business_loop_not_green',
        ],
        verification:
          'GET /api/public/free-tier-data-sharing returns the canonical disclosure (version, summary, ordered terms, bounded policy facts, blocker refs, and gate summary) and the POST /api/keys/free mint response embeds the same dataSharing object. The terms must stay accurate to the capture seams: auto-capture is redacted and owner_only (khala-chat-trace-emitter.ts), default-on production capture is owner-gated by KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT, paid-privacy is excluded fail-closed (inference-privacy-entitlement.ts), public is owner opt-in only, and the reward marker stays inert (#6221). Green requires the default-on capture flip armed in prod and owner-approved public copy.',
        authorityBoundary:
          'A disclosure grants no spend, payout, settlement, training-consent, or capture authority. It describes policy only; it does not change capture behavior or move money.',
      },
      {
        ...basePromiseFields,
        promiseId: 'data.khala_free_tier_trace_capture.v1',
        productArea: 'data',
        audience: ['user', 'agent', 'operator', 'public'],
        state: 'yellow',
        claim:
          'Free Khala API usage can be captured as redacted, private-by-default traces for model improvement.',
        safeCopy:
          'The free-tier trace-capture seams exist and are intentionally private/redacted by default when armed: captured traces are owner_only unless the owner opts into public sharing, and trace capture does not create payout or settlement eligibility. This is yellow because the behavior is owner-gated by KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT and must remain aligned with the public disclosure and paid-privacy exclusion path. Scope note (2026-07-01): this record covers the hosted Khala API path only; Khala Code desktop wrapper raw events and ATIF traces are owner-private delegation observability, not free-plan capture — see khala_code.free_plan_trace_capture.v1 (planned) for the Episode 245 consented desktop pipeline.',
        unsafeCopy:
          'Do not claim all free-tier traffic is currently captured unless the production gate is armed, do not claim captured traces are public by default, and do not claim capture pays users or contributors.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/inference/khala-chat-trace-emitter.ts',
          'apps/openagents.com/workers/api/src/inference/free-tier-data-sharing-disclosure.ts',
          'apps/openagents.com/workers/api/src/inference/inference-privacy-entitlement.ts',
          'docs/traces/2026-06-25-default-on-trace-capture-audit.md',
          'docs/promises/2026-06-25-free-tier-data-sharing-disclosure.md',
          'docs/transcripts/242.md',
          'docs/transcripts/243.md',
        ],
        blockerRefs: [
          'blocker.product_promises.free_tier_capture_default_owner_gated',
          'blocker.product_promises.trace_capture_reward_marker_inert',
        ],
        verification:
          'The capture emitter, disclosure surface, and privacy-entitlement exclusion code now agree: free capture is redacted and owner_only when armed, paid/privacy callers are excluded fail-closed, public sharing is opt-in only, and reward/payout markers remain inert. Green still requires production gate evidence, redaction/private-owner scoping evidence, public-copy owner sign-off, and no payout/settlement implication.',
        authorityBoundary:
          'Trace capture is data-retention behavior only. It grants no public trace publication, training-data sale, payout, settlement, billing, or confidential-compute authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'privacy.khala_paid_capture_optout.v1',
        productArea: 'privacy',
        audience: ['user', 'agent', 'customer', 'public'],
        state: 'yellow',
        claim:
          'Paid privacy and confidential compute opt callers out of free-tier trace capture.',
        safeCopy:
          'The privacy entitlement seam fails closed to not-captured for paid privacy or confidential-compute callers. A paid-privacy purchase endpoint now records a public-safe entitlement receipt and grants the trace-capture opt-out entitlement; a confidential-compute execution endpoint records a public-safe capture-excluded execution receipt when confidential mode is armed. This remains yellow until owner sign-off and the broader paid Khala business loop are green.',
        unsafeCopy:
          'Do not claim paid privacy, confidential compute, or privacy-preserving paid Khala is broadly green merely because receipt machinery exists. Do not claim a full billing loop, refund path, settlement, provider guarantee, or confidential runtime guarantee beyond the bounded public-safe receipts.',
        evidenceRefs: [
          'route:/v1/inference/privacy/paid-privacy/purchases',
          'route:/v1/inference/privacy/confidential-compute/executions',
          'route:/api/public/inference/privacy-receipts/{receiptRef}',
          'apps/openagents.com/workers/api/src/inference/inference-privacy-entitlement.ts',
          'apps/openagents.com/workers/api/src/inference/inference-privacy-receipt-routes.ts',
          'apps/openagents.com/workers/api/migrations/0256_inference_privacy_receipts.sql',
          'apps/openagents.com/workers/api/src/inference/free-tier-data-sharing-disclosure.ts',
          'docs/promises/2026-06-25-free-tier-data-sharing-disclosure.md',
          'docs/inference/2026-06-25-khala-inference-gtm-push.md',
          'docs/transcripts/242.md',
        ],
        blockerRefs: [
          'blocker.product_promises.paid_privacy_owner_signoff_pending',
          'blocker.product_promises.paid_khala_business_loop_not_green',
        ],
        verification:
          'Verify the paid-privacy purchase endpoint records an entitlement receipt, grants inference_privacy_entitlements, and exposes only the public-safe receipt projection. Verify the confidential-compute execution endpoint records a capture-excluded execution receipt only when confidential mode is armed. The trace-capture resolver must still exclude paid-privacy/confidential-compute callers fail-closed. Green still requires owner sign-off and the broader paid Khala business loop.',
        authorityBoundary:
          'Privacy receipts prove bounded entitlement/execution rows and capture exclusion only. They are not billing, refund, settlement, payout, provider, broad privacy guarantee, or confidential-runtime authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.data_trace_revenue.v1',
        productArea: 'data',
        audience: ['contributor', 'public'],
        state: 'planned',
        claim:
          'Data revenue includes mining valuable local traces from Claude Code, Codex, and other agent work.',
        safeCopy:
          'Data trace marketplace language and gates exist; no public-safe settled trace sale is live. Scope note (2026-07-01): Episode 245 productizes this promise as the Khala Code free plan — the launch-anchored trace→plugin→revenue-share loop is now tracked by khala_code.free_plan_trace_capture.v1, khala_code.trace_derived_plugins.v1, khala_code.plugin_backend_revenue_share.v1, and khala_code.paid_to_free_revenue_share.v1 (all planned). Treat this record as the Pylon-era ancestor and route new pays-you copy through the khala_code.* family.',
        unsafeCopy:
          'Do not claim local traces are currently bought, valued, paid, or settled.',
        evidenceRefs: [
          'transition:promise_transition_6e6c3f7c-92f8-4e9a-b82c-f28c6271b396',
          'directive.owner.20260611.focus_tassadar_psion_cs336',
          'apps/openagents.com/docs/2026-06-08-data-trace-marketplace-gate.md',
          'apps/pylon/src/proof-redaction.ts',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
          'docs/transcripts/245.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'promise:khala_code.free_plan_trace_capture.v1',
          'promise:khala_code.plugin_backend_revenue_share.v1',
        ],
        blockerRefs: ['blocker.product_promises.settled_trace_sale_missing'],
        verification:
          'Requires redaction, consent, valuation, purchase, entitlement, payout contract, and settlement receipt refs.',
        authorityBoundary:
          'Pylon must not publish raw prompts, repo contents, provider payloads, wallet material, or private data.',
      },
      {
        ...basePromiseFields,
        promiseId: 'forum.content_tipping.v1',
        productArea: 'Forum',
        audience: ['agent', 'user', 'contributor'],
        state: 'green',
        claim: 'Forum content tipping is like Stacker News for agents.',
        safeCopy:
          'Forum tipping uses direct BOLT 12 payment evidence for recipient-ready posts with user-specified sats amounts. Funded strict smooth-path production smokes have passed against multiple independent live ready recipients (2026-06-09 and 2026-06-10), and on 2026-06-11 a funded live webhook smoke passed at the deployed MDK webhook route: a recovery-pending direct tip was promoted to a settled recipient-wallet-direct receipt by a live callback, duplicate callback replay stayed idempotent, payer retry converged to the same receipt, and a real 21-sat refund was paid back and projected publicly as refunded with settled totals excluding it. Public settled totals require confirmed recipient-wallet-direct MDK/provider evidence. Green per passing transition receipt promise_transition (2026-06-11T03:12:49Z, all checks passed).',
        unsafeCopy:
          'Do not claim every Forum post or creator is tip-ready, do not show pending/demo/staged tips as paid, do not describe hosted L402 payments as creator spendable settlement, and do not describe ordinary Forum tips as accepted-work payouts.',
        evidenceRefs: [
          'https://openagents.com/api/forum/launch-status',
          'https://openagents.com/api/forum/tip-leaderboards?limit=10',
          'apps/openagents.com/docs/forum/2026-06-09-bolt12-direct-tip-conversion.md',
          'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
          'docs/refactor/path-to-bolt-12.md',
          'route:/api/forum/posts/{postId}/direct-tips',
          'route:/api/forum/direct-tips/{attemptId}',
          'route:/api/forum/paid-actions/mdk/webhooks',
          'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
          'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
          'apps/openagents.com/docs/forum-tip-payout-smoke.md',
          'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
          'apps/openagents.com/docs/forum/2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md',
          'transition:promise_transition_c106102b-e51b-4d2f-84ed-a588f1a26316',
          'transition:promise_transition_feab90da-aead-49e1-9097-bd0b8bb5c11a',
          'transition:promise_transition_e632649a-acfa-4e69-ad4b-269e92c963b3',
          'transition:promise_transition_0cfba5d7-40ff-48bd-81a3-4b0758b0acd8',
        ],
        blockerRefs: [],
        verification:
          'Run smoke:forum:tip-wallet, smoke:forum:tip-payout, smoke:forum:mdk-readiness, and `tip-post-smoke --strict-smooth` against at least two independent live ready recipients from a funded payer wallet. Forum launch status, post stats, direct-tip status, direct-tip webhook reconciliation, and tip leaderboards must distinguish wallet recipient readiness, user-specified tip amount, confirmed MDK/provider direct payment evidence, recipient-wallet-direct settlement authority, timeout recovery, and refund/reversal state. Public settled totals count only MDK-authoritative direct recipient-wallet tips and exclude hosted payer-only, pending, demo, staged, refunded, reversed, observed, recovery-pending, or unconfirmed receipts. Wallet-class scope for this promise version: self-custodial MDK agent wallets for direct-tip payers and recipients; hosted checkout actions accept any Lightning wallet payer. The 2026-06-11 live webhook/refund smoke evidence and per-blocker transition receipts are recorded in the evidence refs; the yellow-to-green flip was recorded receipt-first with all transition checks passed on 2026-06-11.',
        authorityBoundary:
          'Forum payment cannot buy moderation, admin, privacy, legal, owner-scope, accepted-work payout, provider-payout, or Treasury settlement authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.install_without_wallet_knowledge.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'public'],
        state: 'green',
        claim:
          'Anyone can install Pylon without Bitcoin wallet knowledge or preloading bitcoin, self-claim scoped training work, and have useful contribution verified on the public run rails.',
        safeCopy:
          'Self-serve install→verified contribution is live, with no operator staging of the contributor. A fresh non-owner install auto-provisions an MDK wallet and Nostr identity on first run (zero wallet knowledge, no bitcoin loaded by the user), then the contributor self-claims a window lease (pylon training claim), self-completes verified executor-trace work (submit-trace), and an independent validator on a distinct device auto-discovers and replays it (validate --auto). Proven on real non-owner machines against run.tassadar.executor.20260615: an independent worker (Orrery) reached a public settlement_recorded receipt validated by an independent validator (Whitefang) on challenge 59ba1f30, and that receipt is explicitly simulation-backed (`realBitcoinMoved:false`). A second fully-external pair (Trigger worker / Orrery validator, 8fd8604a) proved the self-serve verification path. The only operator touch left is payout APPROVAL, which stays operator-gated under bounded spend authority as a permanent treasury safety control — not participation staging. Do not describe the Orrery receipt as real sats paid until a `realBitcoinMoved:true` receipt exists.',
        unsafeCopy:
          'Do not claim the treasury self-spends, that payouts are unbounded or un-approved, that the Orrery simulation receipt proves real sats moved, or that earned sats always land in-wallet instantly. Real settled earning requires a dereferenceable receipt with `realBitcoinMoved:true`; Lightning delivery into the contributor wallet can still be in-flight and is covered by retry plus the Spark backup-receive path. Do not claim mnemonic-only restore is send-ready, or that receive readiness equals send readiness.',
        evidenceRefs: [
          'apps/openagents.com/docs/2026-06-16-pylon-self-serve-install-to-earn-proof.md',
          'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615',
          'training.verification.challenge.59ba1f30-c2f0-40b0-b3ec-b9c5e1fb5316',
          'training.verification.challenge.8fd8604a-183a-43dc-b292-4364cf31e275',
          'receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2',
          'docs/promises/2026-06-17-training-monday-simulation-settlement-policy.md',
          'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-0b31225d-4cb5-4c6e-ad10-26de550641e9',
          'apps/openagents.com/docs/2026-06-11-pylon-live-install-to-bitcoin-smoke-evidence.md',
          'route:/api/public/nexus-pylon/receipts/{receiptRef}',
          'transition:promise_transition_b9d568b5-0d02-476b-8205-9503f9060744',
          'transition:promise_transition_73746398-0096-4962-b0c6-060e81fc70c4',
        ],
        blockerRefs: [],
        verification:
          'GET /api/public/training/runs/run.tassadar.executor.20260615 shows qualifiedContributorCount >= 1 from a non-owner contributor whose wallet/identity were self-provisioned (not operator-funded), whose work was self-claimed (not operator-dispatched) and self-completed via worker submit-trace + independent validator auto-discovery (not operator closeout), and whose Verified exact_trace_replay challenge carries a settlement_recorded receipt. The three operator-staging elements from the 2026-06-11 proof — operator-funded wallets, operator assignment dispatch, operator closeout — are removed. Payout APPROVAL remains operator-gated (requireAdmin, per-payout + run spend cap) as a deliberate, permanent bounded-spend safety control, not participation staging; this scoping is owner-approved. The Orrery receipt is simulation-backed (`realBitcoinMoved:false`) and validates only the public settlement-record path; real settled earning requires a linked `realBitcoinMoved:true` receipt with no private payment material. Wallet-landed delivery may be in-flight and is covered by retry + the Spark backup-receive path.',
        authorityBoundary:
          'Receive readiness, simulation-backed settlement records, and balance visibility are not send readiness, payout dispatch, or real settled earning. Self-serve participation does not grant self-serve treasury spend: payouts remain operator-approved under bounded spend authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'sites.referral_bitcoin_stream.v1',
        productArea: 'Sites',
        audience: ['customer', 'contributor', 'public'],
        state: 'yellow',
        claim:
          'Autopilot Sites can carry built-in referral links and later pay referrers a Bitcoin stream when referred users become paying customers.',
        safeCopy:
          'Site referral capture records attribution, and the 5% referral payout ledger is now wired end to end in source (RL-1, #5458): a paid customer event resolves its consumed attribution to the referring user and creates exactly one idempotent eligibility row (hooked into Stripe credit-purchase fulfillment, short-circuiting self- and no-attribution), and a dispatch path drives an eligible row approved -> dispatched -> settled by invoking the injected MDK/Spark payout adapter BEFORE recording settled, gated by the MDK payout-mode readiness projection and the Bitcoin-only rev-share asset boundary. The public settled referral-payout receipt surface is now proven dereferenceable end to end: a staging-test settlement adapter (moves no money, default OFF, fails closed when disabled) drives feed -> dispatch -> a real settled ledger row, and the SAME public receipt store (GET /api/public/site-referral-payout-receipts/{receiptRef}) resolves the produced receipt as a staging_test settlement with amount, attributionLinked, qualifyingEventKind, policy/caveat refs, and live-at-read staleness, withholding all private payout material. No REAL referral payout has settled yet: real dispatch stays readiness-gated and awaits a real Bitcoin-revenue production event plus owner arming, so Bitcoin referral streaming is not live.',
        unsafeCopy:
          'Do not claim referral links pay Bitcoin streams now or that any referrer has been paid; the ledger feed, dispatch, and the public settled-receipt readback are wired and proven end to end with a staging-test adapter (moves no money) and in-memory D1, but no real settled Bitcoin referral payout exists and real dispatch remains readiness-gated and owner-armed.',
        evidenceRefs: [
          'https://openagents.com/docs/autopilot-sites',
          'route:/r/site/{publicSourceRef}',
          'apps/openagents.com/docs/sites/2026-06-08-site-referral-reward-withdrawal-gate.md',
          'apps/openagents.com/docs/2026-06-10-site-referral-payout-policy.md',
          'apps/openagents.com/docs/sites/2026-06-23-site-referral-settlement-receipt-staging-loop.md',
          'route:/api/operator/sites/referrals/payout-ledger/{payoutRef}/transitions',
          'route:/api/public/site-referral-payout-receipts/{receiptRef}',
          'apps/openagents.com/workers/api/src/site-referral-payout-feed.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-dispatch.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-adapter.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-staging-adapter.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-receipts.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-wire.test.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-receipt-loop.test.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-ledger.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5458',
          'https://github.com/OpenAgentsInc/openagents/issues/5524',
        ],
        blockerRefs: [
          'blocker.product_promises.referral_first_real_payout_pending',
        ],
        verification:
          'Referral payout requires attribution consumption, abuse/dispute/cap policy, payout ledger, and a dereferenceable settlement receipt projection. RL-1 (#5458) wired the eligibility feed and the readiness-gated, idempotent approved -> dispatched -> settled dispatch (settle-via-adapter, settle-at-most-once, Bitcoin-only rev-share boundary) proven by site-referral-payout-wire.test.ts. The settlement-receipt surface is now proven dereferenceable end to end (#5524): site-referral-payout-receipt-loop.test.ts drives feed -> dispatch (staging-test adapter, no money) -> a real settled D1 row -> and dereferences the produced receipt through the REAL makeD1SiteReferralPayoutReceiptStore, with idempotent re-drive (settle-at-most-once) and a fail-safe (disabled adapter records no settled state, nothing dereferences). This clears referral_settlement_receipts_missing. Green still requires a real Bitcoin-revenue production event producing a real settled referral payout receipt over the live hosted-MDK rail, plus owner arming + sign-off per proof.claim_upgrade_receipts.v1 (referral_first_real_payout_pending).',
        authorityBoundary:
          'Referral attribution is not payout eligibility or spendable settlement.',
      },
      {
        ...basePromiseFields,
        promiseId: 'payments.money_dev_kit.v1',
        productArea: 'payments',
        audience: ['agent', 'contributor', 'operator'],
        state: 'green',
        claim:
          'OpenAgents switched payments to Money Dev Kit: self-custodial Lightning agent wallet, single command setup, LSP/splice channels, immediate receive liquidity, and hosted checkout.',
        safeCopy:
          'OpenAgents uses MDK hosted checkout and a scoped local MDK agent-wallet bridge for small-sats/L402 paths. The MDK send-readiness claim is limited to the original funded wallet home with public-safe 1-sat settlement receipts and a capacity-sufficient preflight; Spark remains the primary agent/MPP payment rail. Broader custody, payout, withdrawal, and accepted-work settlement claims remain scoped by their own route authority and wallet readiness.',
        unsafeCopy:
          'Do not claim MDK mnemonic restore, hosted MDK checkout, or a positive wallet balance proves broad custody, full send readiness, payout, withdrawal, accepted-work settlement, or provider settlement.',
        evidenceRefs: [
          'apps/openagents.com/docs/mdk',
          'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
          'apps/openagents.com/docs/nexus/2026-06-08-mdk-agent-wallet-send-readiness-preflight.md',
          'apps/openagents.com/docs/nexus/2026-06-08-mdk-agent-wallet-outbound-capacity-restore-report.md',
          'receipt.nexus_pylon.settlement.assignment_public_probe_gepa_paid_multi_pylon_20260608214500_1',
          'receipt.nexus_pylon.settlement.assignment_public_probe_gepa_paid_multi_pylon_20260608214500_2',
          'capacity.mdk_agent_wallet.send.sufficient_for_scoped_smoke',
          'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
          'route:/api/forum/paid-actions/mdk/webhooks',
          'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
          'apps/openagents.com/docs/forum-tip-payout-smoke.md',
          'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
          'apps/openagents.com/docs/forum/2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md',
          'transition:promise_transition_c30b7327-e82b-4696-8886-97aafa454284',
        ],
        blockerRefs: [],
        verification:
          'Scoped send-readiness is proven only for the original funded MDK agent-wallet home with public-safe 1-sat settlement receipts (`receipt.nexus_pylon.settlement.assignment_public_probe_gepa_paid_multi_pylon_20260608214500_1` and `_2`) and the send-readiness preflight capacity ref `capacity.mdk_agent_wallet.send.sufficient_for_scoped_smoke`. The smoke fixture blocks send planning unless operator approval, original funded wallet-home mode, spend-cap compliance, and the capacity-sufficient ref are all present. Keep wallet configured, receive-ready, positive balance, send-ready, direct payment sent, webhook-confirmed payment, timeout recovery, refund/reversal, accepted work, payout, and accepted-work settlement states separate.',
        authorityBoundary:
          'Scoped MDK send-readiness proof does not make MDK the primary agent/MPP rail and does not bypass route auth, owner scope, moderation, deployment, payout, withdrawal, custody, or settlement gates.',
      },
      {
        ...basePromiseFields,
        promiseId: 'agents.one_instruction_sheet.v1',
        productArea: 'agent-readable surfaces',
        audience: ['agent', 'user', 'public'],
        state: 'green',
        claim:
          'OpenAgents provides one agent instruction sheet with APIs and features that a human copies into an agent.',
        safeCopy:
          'The public AGENTS sheet, manifest, OpenAPI, rules, heartbeat, skill metadata, product-promises registry, and route coverage gate are live.',
        unsafeCopy:
          'Do not treat the sheet as broad write, spend, deploy, provider, moderation, payout, or settlement authority.',
        evidenceRefs: [
          'https://openagents.com/AGENTS.md',
          'https://openagents.com/.well-known/openagents.json',
          'https://openagents.com/api/openapi.json',
          'https://openagents.com/api/public/product-promises',
        ],
        blockerRefs: [],
        verification:
          'Live public AGENTS.md points to the product-promises endpoint and report path.',
        authorityBoundary:
          'Agent-readable documentation is onboarding guidance only; runtime routes enforce authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'api.hosted_gemini.v1',
        productArea: 'agent API',
        audience: ['agent', 'developer'],
        state: 'yellow',
        claim:
          'OpenAgents is API-driven and may offer hosted Gemini through an OpenAgents API surface.',
        safeCopy:
          'OpenAgents has route-covered hosted Gemini execution through the env-gated Vertex binding and the OpenAI-compatible Khala gateway enforces account credits, entitlement/free-quota gates, metering, and served-token rows. The public Hosted Gemini readiness endpoint verifies whether a cited production Vertex Gemini receipt and owner transition receipt satisfy the green gate. The promise stays yellow until both are present.',
        unsafeCopy:
          'Do not claim hosted Gemini is green, settled, broadly resale-ready, or owner-approved without a real production receipt.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/artanis-mind.ts',
          'post.public.forum.artanis.status.5',
          'https://openagents.com/api/openapi.json',
          'apps/openagents.com/docs/2026-06-08-google-adc-gemini-agent-platform-auth-audit.md',
          'apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts',
          'apps/openagents.com/workers/api/src/inference/chat-completions-routes.test.ts',
          'apps/openagents.com/workers/api/src/autopilot-hosted-gemini-executor-env.ts',
          'route:/api/public/product-promises/api.hosted_gemini.v1/readiness?receiptRef={receiptRef}',
          'apps/openagents.com/workers/api/src/inference/served-tokens-recorder.ts',
          'docs/autopilot-coder/2026-06-09-probe-autopilot-sites-agent-api-audit.md',
        ],
        blockerRefs: [
          'blocker.product_promises.hosted_gemini_production_receipt_pending',
          'blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending',
        ],
        verification:
          'Route tests cover the env-gated hosted Gemini executor delivering a public-safe Autopilot closeout when armed, a registered-agent Khala gateway request reaching the armed Vertex Gemini lane with entitlement/balance admission, metered usage, and served-token recording, and the public readiness endpoint that keeps blockers until the cited production receipt and owner transition receipt both exist.',
        authorityBoundary:
          'API-driven product surfaces are not generic provider-capacity resale, settlement, or green-promise authority without receipt-backed owner sign-off.',
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.khala_free_openai_compatible_api.v1',
        productArea: 'agent API',
        audience: ['agent', 'developer', 'user', 'public'],
        state: 'green',
        claim:
          'Khala is available through a free, OpenAI-compatible API at openagents.com.',
        safeCopy:
          'Khala is live as a free, rate-limited OpenAI-compatible API: mint a free key with POST /api/keys/free, call POST /api/v1/chat/completions with model openagents/khala, or inspect GET /api/v1/models. The public model catalog intentionally exposes one model id, openagents/khala. Free quota is limited and metered; this is not a paid-capacity, marketplace-resale, guaranteed-availability, confidential-compute, or verified-code-execution claim.',
        unsafeCopy:
          'Do not advertise stale public model ids such as openagents/khala-mini or openagents/khala-code, do not claim unlimited free API usage, and do not claim the paid Khala business loop, MPP funding, privacy upsell, or contributor payouts are green because free inference works.',
        evidenceRefs: [
          'route:/api/keys/free',
          'route:/api/v1/chat/completions',
          'route:/api/v1/models',
          'docs/inference/2026-06-25-khala-inference-gtm-push.md',
          'docs/promises/2026-06-25-khala-inference-push-promise-review.md',
          'docs/transcripts/242.md',
          'docs/transcripts/243.md',
        ],
        blockerRefs: [],
        verification:
          'Green scope is limited to the free key + OpenAI-compatible chat-completions surface and the single public model id. Verification requires POST /api/keys/free to return an oa_agent_ bearer with the published free-tier limits, GET /api/v1/models to include openagents/khala, and a streamed POST /api/v1/chat/completions call to return assistant tokens under the free quota.',
        authorityBoundary:
          'This grants no paid inference, balance, revenue, settlement, privacy, confidential-compute, code-execution, provider-resale, or uptime guarantee authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'metrics.khala_tokens_served_public.v1',
        productArea: 'metrics',
        audience: ['agent', 'user', 'public'],
        state: 'green',
        claim:
          'OpenAgents publishes a live public Tokens Served counter and history.',
        safeCopy:
          'The public Tokens Served counter is live at GET /api/public/khala-tokens-served and is shown on openagents.com surfaces including /khala and /stats. It is a source-agnostic headline total of real served tokens across product channels, including Khala API rows and explicitly opted-in direct local Codex rows. Use the channel/model/demand aggregate views or assignment proof for provenance; the headline counter alone is not proof of external demand, revenue, assignment success, payout, or model quality.',
        unsafeCopy:
          'Do not describe the headline total as external customer demand, revenue, paid usage, contributor payout, or assignment-specific proof. Do not infer a Khala -> Pylon -> Codex assignment from counter movement without matching exact token_usage_events rows.',
        evidenceRefs: [
          'route:/api/public/khala-tokens-served',
          'route:/api/public/khala-tokens-served/channel-mix',
          'route:/api/public/khala-token-history',
          'docs/inference/2026-06-25-khala-inference-gtm-push.md',
          'docs/stats/2026-06-26-stats-page-audit.md',
          'docs/afteraction/2026-06-26-khala-pylon-codex-delegation-afteraction.md',
          'docs/traces/2026-06-27-pylon-codex-live-trace-status-audit.md',
          'docs/transcripts/243.md',
          'docs/transcripts/244.md',
        ],
        blockerRefs: [],
        verification:
          'GET /api/public/khala-tokens-served must return schemaVersion openagents.public_khala_tokens_served.v1 with tokensServed and live_at_read staleness. Token history and page projections must preserve the policy that all real served tokens count in the headline while provenance/channel/demand segmentation remains separate.',
        authorityBoundary:
          'A public aggregate counter does not expose private traces, raw provider events, D1 rows, payment data, or assignment-level proof, and it does not authorize external-demand or revenue claims.',
      },
      {
        ...basePromiseFields,
        promiseId: 'metrics.khala_model_family_mix_public.v1',
        productArea: 'metrics',
        audience: ['agent', 'operator', 'public'],
        state: 'green',
        claim:
          'The stats surface shows model-family/provider mix, channel mix, and daily token volume.',
        safeCopy:
          'The /stats surface includes public model-family/provider mix, channel mix, and daily token-volume views for Tokens Served. The model-mix and channel-mix routes are live-at-read over token_usage_events (liveAt/generatedAt, projection_staleness.v1, maxStalenessSeconds:0), use stable public normalization, and include headline served volume across Khala API and explicitly opted-in direct local Codex rows. The owner-signed green transition is applied for this scoped transparency surface; provenance caveats remain required for demand, revenue, and marketplace-health claims.',
        unsafeCopy:
          'Do not use model-mix or channel-mix percentages as proof of external customer demand, revenue, model preference, paid provider resale, or marketplace health. Do not hide internal dogfood, own-capacity, or direct-local provenance when making demand claims.',
        evidenceRefs: [
          'route:/api/public/khala-tokens-served/model-mix',
          'route:/api/public/khala-tokens-served/channel-mix',
          'docs/stats/2026-06-26-stats-page-audit.md',
          'docs/inference/2026-06-25-khala-inference-gtm-push.md',
          'docs/transcripts/244.md',
        ],
        blockerRefs: [],
        verification:
          'Green after the owner-signed #7016 transition plus #7797 channel expansion: route/test evidence covers liveAt/generatedAt live_at_read staleness, stable public model-family/channel normalization, and /stats copy separating headline served volume from external demand and revenue. This verifies the public transparency projection only and creates no external-demand, revenue, paid-provider-resale, or marketplace-health claim.',
        authorityBoundary:
          'Model-mix transparency is not routing authority, provider resale authority, benchmark proof, revenue proof, or public raw-event disclosure.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala.cli_terminal_client.v1',
        productArea: 'Khala',
        audience: ['agent', 'developer', 'operator', 'user'],
        state: 'green',
        claim:
          '@openagentsinc/khala ships a terminal client for Khala chat and operator utilities.',
        safeCopy:
          '@openagentsinc/khala v0.1.16 exposes the khala command: interactive scrollback chat, headless prompt/stdin mode, /info and /msginfo metadata, feedback, changelog, version, public token counter, background update checks, OpenAgents login/logout for owner/operator flows, `khala fleet status` / `khala fleet list` over owner-scoped fleet state, optional local Codex workspace delegation when local credentials are connected, and the owner-authenticated Artanis operator channel. The CLI uses a normal terminal scrollback/raw-mode input, not the superseded OpenTUI single-line plan.',
        unsafeCopy:
          'Do not claim the CLI is a full TUI, a billing/account console, a wallet, a guaranteed local-code executor, a cross-owner fleet browser, or a public Artanis access path. Do not cite old v0.1.11 sneak-peek output as the current version without the v0.1.16 source/changelog.',
        evidenceRefs: [
          'clients/khala-cli/package.json',
          'clients/khala-cli/README.md',
          'clients/khala-cli/src/cli.ts',
          'clients/khala-cli/src/fleet.ts',
          'clients/khala-cli/src/input.ts',
          'clients/khala-cli/src/changelog.ts',
          'apps/openagents.com/workers/api/src/operator-fleet-status-routes.ts',
          'docs/khala-cli/README.md',
          'docs/transcripts/244.md',
        ],
        blockerRefs: [],
        verification:
          'khala --version reports 0.1.16; CLI source exposes the documented commands and flags; changelog.ts records v0.1.14 Artanis, v0.1.15 login/logout, and v0.1.16 identity wording; fleet status/list read the owner-scoped /api/operator/fleet/state surface; README docs carry the current scoped copy and supersede the old OpenTUI plan.',
        authorityBoundary:
          'The CLI is a client surface only. Authentication, fleet visibility, trace visibility, Codex delegation, Artanis access, spend, and account authority are still enforced by their respective server/local credential gates.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala.own_capacity_codex_delegation.v1',
        productArea: 'Khala',
        audience: ['operator', 'agent', 'developer'],
        state: 'green',
        claim:
          'A typed Khala coding request can delegate to the caller’s own linked Pylon and run local Codex no-spend work.',
        safeCopy:
          'Owner-capacity Khala -> Pylon -> Codex delegation is live for explicit typed requests: `pylon khala request --workflow codex_agent_task ... --json` delegates to the caller’s linked Pylon, auto-runs the returned no-spend assignment by default, and `pylon khala closeout <assignmentRef> --json` proves the accepted closeout with owner-only trace/raw-event refs, exact token_usage_events rows (provider pylon-codex-own-capacity, model openagents/pylon-codex, usage_truth exact, demand_kind own_capacity, demand_source khala_coding_delegation), and worker closeout policy `paymentMode: no-spend`, `settlementState: not_applicable`, `payoutClaimAllowed: false`. This is no-spend, owner-local capacity only; it is not subscription resale, third-party pooling, payout eligibility, or automatic routing of every coding-ish chat prompt.',
        unsafeCopy:
          'Do not claim public users can sell Codex subscription capacity, route to other people’s Pylons, receive payouts, see raw Codex events publicly, watch a guaranteed live assignment trace UI, or rely on automatic broad semantic routing without the typed workflow and fresh caller-owned capacity gate.',
        evidenceRefs: [
          'docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md',
          'docs/afteraction/2026-06-26-khala-pylon-codex-delegation-afteraction.md',
          'docs/traces/2026-06-27-pylon-codex-live-trace-status-audit.md',
          'docs/promises/2026-06-27-khala-cli-own-capacity-reconciliation.md',
          'apps/pylon/docs/khala-burndown-runbook.md',
          'apps/pylon/docs/codex-bridge.md',
          'docs/transcripts/244.md',
          'issue:6361',
          'issue:6362',
          'issue:6368',
          'issue:6369',
        ],
        blockerRefs: [],
        verification:
          'Verified on 2026-06-27 from current main: provider go-online projected caller-owned Codex capacity available; `pylon khala request --workflow codex_agent_task --fixture --base-url https://openagents.com --json` returned assignment.public.khala_coding.chatcmpl_820ffc8bedb941d38991b4cd4c4b976c and auto-ran it to accepted closeout assignment.closeout.0351f0a0b4650c2272233fa0; `pylon khala closeout ... --json` returned closeoutChecklist.ok true with exact owner-capacity token rows and no-spend/payout-false closeout policy. The master-default workspace smoke for octocat/Hello-World commit 7fd1a60b01f91b314f59955a4e4d4e80d8edf11d also accepted and verified (assignment.closeout.2ccf7c3d2ac86b12b57ce2e2), proving pinned-commit materialization across default-branch names.',
        authorityBoundary:
          'The local owner runner may use full local Codex execution authority, but that authority is never public wire authority and never applies to untrusted labor/provider work. This promise grants no payout, resale, third-party capacity, raw-event publication, or guaranteed availability.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.agentic_labor_products.v1',
        productArea: 'Autopilot',
        audience: ['customer', 'user', 'public'],
        state: 'yellow',
        claim:
          'OpenAgents sells agentic labor/products instead of dumb base-inference resale.',
        safeCopy:
          'The product direction is agentic labor and Sites. The labor-product flow is now self-serve (a buyer/agent can plan an order with no operator staging), but it is not yet settlement-backed: no real sale has been carried through to a settled receipt.',
        unsafeCopy:
          'Do not claim all agentic labor/product sales are live and settlement-backed.',
        evidenceRefs: [
          'https://openagents.com/docs/openagents',
          'https://openagents.com/docs/autopilot-sites',
          'apps/openagents.com/workers/api/src/agentic-labor-product.ts',
          'apps/openagents.com/workers/api/src/agentic-labor-product-routes.ts',
          'apps/openagents.com/workers/api/src/agentic-labor-product.test.ts',
          'apps/openagents.com/workers/api/src/agentic-labor-product-settlement.test.ts',
          'apps/openagents.com/workers/api/src/agentic-labor-product-routes.test.ts',
          'apps/openagents.com/docs/labor/2026-06-19-agentic-labor-product-flow-scaffold.md',
          'apps/openagents.com/docs/labor/2026-06-20-agentic-labor-product-self-serve.md',
          'route:/api/public/autopilot/labor-products',
          'promise:autopilot.control_center_fanout_marketplace.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing',
        ],
        verification:
          'A typed end-to-end labor-product flow exists (post -> order -> dispatch -> deliver -> settle): buildLaborProductFlowPlan models the orderable listing and coherent lifecycle, and settleLaborProductOrder is a FLAG-GATED INERT, owner-gated settlement seam that (when armed + delivered + owner-signed) runs a receipt-first, idempotent, never-negative charge through the shared cloud-metering ledger on the NIP-90 labor stream (verified against real SQL). The flow is now SELF-SERVE: POST /api/public/autopilot/labor-products + planSelfServeLaborProductOrder let a buyer/agent plan a labor-product order with no operator staging (still INERT — it dispatches nothing, debits nothing, settles nothing; returns a pure ordered-stage plan; 503 unless AGENTIC_LABOR_PRODUCTS_ENABLED is armed). The GET listing is wired but INERT (empty store, inert/yellow) unless armed. GREEN still needs a real labor product ordered by an external buyer, carried through settlement with the flag armed, producing a dereferenceable settlement receipt, plus owner sign-off (proof.claim_upgrade_receipts.v1 + proof.demand_provenance.v1).',
        authorityBoundary:
          'Product direction and a typed inert flow are not proof of a real labor product sold; the settlement seam moves no money until armed, owner-signed, and against a delivered order.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.cli_tui_probe_background.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'agent'],
        state: 'green',
        claim:
          'Pylon is a script/CLI/TUI that includes Probe and is meant to run in the background.',
        safeCopy:
          'Pylon v1.0 has a Bun/Effect/OpenTUI CLI/TUI, includes the former Probe runtime, and has a passed no-spend live worker-loop smoke against OpenAgents.',
        unsafeCopy:
          'Do not turn the passed no-spend worker-loop smoke into a claim that every Pylon install can run paid work, settle payouts, or satisfy the whole v1.0 release gate.',
        evidenceRefs: [
          'apps/pylon/README.md',
          'apps/pylon/packages/runtime/src/index.ts',
          'apps/pylon/docs/live-worker-loop-smoke.md',
          'apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md',
        ],
        blockerRefs: [],
        verification:
          'apps/pylon local tests cover the CLI/runtime smoke, and the 2026-06-09 production smoke registered, heartbeated, reported wallet readiness, created/read an unpaid assignment, accepted it, submitted progress/artifact refs, and closed it through the operator route.',
        authorityBoundary:
          'This green claim is limited to the CLI/TUI/Probe runtime and no-spend live worker-loop event path; it does not authorize paid assignments, settlement, wallet send readiness, Windows/WSL, or provider mutation.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.agent_steerable_cli.v1',
        productArea: 'Pylon',
        audience: ['agent', 'contributor', 'operator'],
        state: 'green',
        claim:
          'Pylon is a headless, CLI-only node an agent can fully steer — every capability the Autopilot desktop GUI exposes is reachable from the command line (plus the loopback control API), with a machine-readable command catalog and no interactive TUI.',
        safeCopy:
          'Pylon is headless and CLI-only: the OpenTUI dashboard is removed (#5034) and bare `pylon` boots the headless node-core. An agent steers everything from the CLI — status/balance/wallet/accounts/assignment/work/tip plus first-class sessions/approvals/deploy/training verbs (#5035), all --json with honest exit codes — and discovers the full surface via `pylon help --json` (machine-readable catalog, 28 commands, with mutates/spends/needsNode flags). A long-lived `pylon node` exposes the same loopback control API the Autopilot desktop drives. Verified live 2026-06-15: against a running node, `pylon sessions list --json` returned real running-session data and `pylon approvals list --json` real approval-queue data; with no node, verbs fail cleanly (no_token/no_node). Steering contract: apps/pylon/docs/pylon-multi-session-agent-runbook.md.',
        unsafeCopy:
          'Do not claim steering Pylon automatically earns Bitcoin — agent-steerability is a capability/control claim, not an earning or settlement claim. Money commands stay projection-safe (balance/wallet never surface seed/mnemonic) and spend stays approval-gated; only wallet/work/tip are spend-capable.',
        evidenceRefs: [
          'apps/pylon/docs/2026-06-15-pylon-cli-only-agent-steerable-audit.md',
          'apps/pylon/docs/pylon-multi-session-agent-runbook.md',
          'apps/pylon/src/index.ts',
          'apps/pylon/src/cli-catalog.ts',
          'apps/pylon/src/node/control-cli.ts',
          'apps/pylon/src/node/control-server.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5033',
          'https://github.com/OpenAgentsInc/openagents/issues/5034',
          'https://github.com/OpenAgentsInc/openagents/issues/5035',
          'promise:pylon.cli_tui_probe_background.v1',
        ],
        blockerRefs: [],
        verification:
          'Met: OpenTUI/dashboard removed from apps/pylon (#5034; @opentui dropped, bare `pylon` boots headless node-core); first-class CLI verbs for every Autopilot-GUI capability — sessions/approvals/deploy/training (#5035) — with consistent --json + exit codes; `pylon help --json` catalog (openagents.pylon.command_catalog.v1, 28 commands); documented steering contract (the runbook). Live round-trip verified 2026-06-15 (sessions/approvals list returned real data from a running node; clean no_token/no_node when none). Full pylon suite 1069 pass. Supersedes the TUI dimension of pylon.cli_tui_probe_background.v1. Packaging the bundled headless Pylon into the signed mac .app continues under #5027 and does not gate this CLI/steering claim.',
        authorityBoundary:
          'A steerable CLI is a control surface only. It grants no paid-assignment, settlement, wallet-send, or provider-mutation authority, and steering Pylon does not by itself earn or move money.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.control_center_fanout_marketplace.v1',
        productArea: 'Autopilot',
        audience: ['operator', 'agent'],
        state: 'yellow',
        claim:
          'Control center / Autopilot can fan out work to many agents and pull from a plugin marketplace.',
        safeCopy:
          'First-live: on 2026-06-14 one real Autopilot work order (f374a475) had its owned capacity forced dark and fanned out to the open agent labor market (market work request 432420e6) behind a server-side customerOptIn gate (opt-out returns 409 lane_c_fanout_blocked); an independent provider Pylon quoted and executed it, the validator re-ran bun test (pass), and escrow settled with public receipts (#4783, P7 lane-C). This proves single-order fanout to the market. As of 2026-06-20 the fanout is also SELF-SERVE: the customer-authenticated route POST /api/autopilot/work/:ref/lane-c-fanout (customer_orders.write scope) returns a typed self_serve_fanout plan in one action — the lane-C gate decision plus the linked market work-request the fanout would list — so the run is customer-initiated, not operator-staged; the public read-only projection is /api/public/autopilot/self-serve-fanout (INERT/yellow until SELF_SERVE_FANOUT_ENABLED is armed). As of #6893 the typed marketplace catalog and fanout planner support a non-code live work class, data_labeling, with its own capability and validator command, through the same opt-in + budget + validator lane-C gate. The dispatch seam (dispatchSelfServeFanout) lists nothing until armed.',
        unsafeCopy:
          'Do not claim a self-serve control center fans out settled paid work to many agents from a broad live marketplace. The self-serve plan and dispatch seam are INERT until armed; data_labeling proves a non-code work-class contract and authorized market input only, not live settlement, provider execution, or money movement.',
        evidenceRefs: [
          'docs/labor/2026-06-14-p7-lane-c-fanout-closeout.md',
          'autopilot_work_order.f374a475-0465-4f65-b9e1-c1bffb6778f6',
          'work_request:432420e6-7245-4d44-96c4-9e0b149a6020',
          'apps/openagents.com/workers/api/src/lane-c-fanout-policy.ts',
          'apps/openagents.com/workers/api/src/lane-c-fanout-bridge.ts',
          'apps/openagents.com/workers/api/src/self-serve-fanout.ts',
          'apps/openagents.com/workers/api/src/self-serve-fanout-routes.ts',
          'apps/openagents.com/workers/api/src/self-serve-fanout.test.ts',
          'apps/openagents.com/workers/api/src/marketplace-work-class-catalog.ts',
          'apps/openagents.com/workers/api/src/marketplace-work-class-catalog.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/6893',
          'https://github.com/OpenAgentsInc/openagents/issues/4783',
          'route:/api/public/autopilot/self-serve-fanout',
          'route:/api/operator/pylons/assignments',
          'apps/openagents.com/docs/2026-06-08-signature-marketplace-revenue-gate.md',
        ],
        blockerRefs: [
          'blocker.product_promises.armed_self_serve_fanout_settlement_receipt_missing',
        ],
        verification:
          'Requires self-serve scope, marketplace policy, idempotent dispatch, no-duplicate assignment, spend cap, proof, and settlement gates. First-live single-order market fanout met by #4783 (lane-C, customerOptIn-gated, validator-accepted, escrow-settled). Self-serve scope met (2026-06-20): buildSelfServeFanoutPlan + the customer-authenticated lane-c-fanout route produce a customer-initiated single-action fanout plan over the existing server-side lane-C gate (public-trust floor + opt-in + budget cap enforced server-side), with a public read-only projection at /api/public/autopilot/self-serve-fanout. Non-code plugin work-class scope met by #6893: marketplace-work-class-catalog exposes data_labeling as a live non-code work class, and buildSelfServeFanoutPlan/lane-c-fanout route tests prove it carries capability.market.data_labeling plus command.public.market.data_labeling.audit through the same opt-in + budget + validator gate. The plan + dispatch seam are FLAG-GATED INERT (SELF_SERVE_FANOUT_ENABLED default OFF) and move no money. Green still requires a receipt-first owner-signed settlement against an armed self-serve fanout. State stays yellow; the matching promise_transition receipt must be recorded against the deployed registry version.',
        authorityBoundary:
          'Operator-only APIs and validation gates are not public self-serve marketplace authority; the customerOptIn gate must authorize any fanout, and the market produces accepted candidate work only.',
      },
      {
        ...basePromiseFields,
        promiseId: 'marketplace.signature_monetization.v1',
        productArea: 'marketplace',
        audience: ['agent', 'developer', 'contributor'],
        state: 'planned',
        claim:
          'DSPy/GEPA signatures and agent workflow components can be discoverable and monetizable.',
        safeCopy:
          'Signature validation, Blueprint tooling, marketplace gates, an inert public usage-metering projection, and tested package activation/pricing/rev-share/settlement-receipt gate logic exist. Live billing and real settlement are not armed.',
        unsafeCopy:
          'Do not claim signatures or workflow components are generating settled revenue.',
        evidenceRefs: [
          'apps/openagents.com/docs/2026-06-08-signature-marketplace-revenue-gate.md',
          'apps/pylon/packages/runtime/src/blueprint',
          'apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.ts',
          'apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.test.ts',
          'apps/openagents.com/workers/api/src/signature-usage-metering.ts',
          'apps/openagents.com/workers/api/src/signature-usage-metering.test.ts',
          'apps/openagents.com/workers/api/src/signature-usage-metering-routes.ts',
          'apps/openagents.com/workers/api/src/signature-usage-metering-routes.test.ts',
          'route:/api/public/markets/signature-monetization/metering',
        ],
        blockerRefs: ['blocker.product_promises.signature_settlement_missing'],
        verification:
          'Usage metering reaches the metered rung via the inert public projection and tests, clearing the usage-metering blocker. The revenue gate now requires publish -> activate -> usable refs before metered usage can price revenue, requires attribution/pricing/rev-share/dispute/refund refs before payable state, and allows settlement claims only when the usage charge has public-safe settlement receipt refs with settled contributor cents matching payable cents. Green still requires an owner-armed real settlement receipt and matching transition receipt.',
        authorityBoundary:
          'Discovery, contribution validation, inert usage metering, and pure gate projections do not by themselves mutate package listings, bill, debit, credit, or settle package usage. Activation and settlement claims require explicit public-safe refs and owner-armed receipt evidence.',
      },
      {
        ...basePromiseFields,
        promiseId: 'marketplace.wasm_plugins.v1',
        productArea: 'marketplace',
        audience: ['agent', 'contributor', 'public'],
        state: 'planned',
        claim:
          'The marketplace can support WASM-plugin packages for agent and contributor workflows.',
        safeCopy:
          'WASM-plugin package policy, fail-closed manifest admission, install/uninstall registry machinery, an inert installed-plugin discovery route, and source-level fixture execution evidence exist. Treat this as planned infrastructure only: no public self-serve WASM-plugin marketplace, production sandbox runtime, billing, settlement, or broad execution authority is live.',
        unsafeCopy:
          'Do not claim a live WASM-plugin marketplace, paid WASM-plugin installs, settled WASM-plugin revenue, or public-safe third-party WASM execution.',
        evidenceRefs: [
          'apps/openagents.com/docs/2026-06-08-signature-marketplace-revenue-gate.md',
          'apps/pylon/packages/runtime/src/blueprint',
          'apps/openagents.com/workers/api/src/wasm-plugin-marketplace.ts',
          'apps/openagents.com/workers/api/src/wasm-plugin-marketplace-routes.ts',
          'apps/openagents.com/workers/api/src/wasm-plugin-marketplace.test.ts',
          'route:/api/public/marketplace/wasm-plugins',
        ],
        blockerRefs: [
          'blocker.product_promises.wasm_plugin_self_serve_marketplace_not_live',
          'blocker.product_promises.wasm_plugin_execution_sandbox_missing',
          'blocker.product_promises.wasm_plugin_billing_settlement_missing',
        ],
        verification:
          'Source evidence now covers the package manifest schema, admission policy, install/uninstall state registry, malformed/over-privileged rejection tests, inert installed-plugin discovery route, digest-pinned fixture WASM execution, deny-by-default import rejection, and metering-shaped execution evidence. Keep this planned until there is public evidence for a production self-serve install marketplace, production sandbox execution receipts with enforceable runtime resource limits, billing, attribution, rev-share, abuse handling, and settlement receipts.',
        authorityBoundary:
          'Experimental marketplace planning does not grant public plugin installation, execution, billing, settlement, code-loading, or contributor earning authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'provider.compliant_usage_labor.v1',
        productArea: 'labor',
        audience: ['contributor', 'agent', 'operator', 'public'],
        state: 'yellow',
        claim:
          'Contributors can connect their own provider accounts or prepaid API budgets and earn Bitcoin by doing useful work with that compliant usage through the agent labor market; OpenAgents never resells provider access.',
        safeCopy:
          'First-live: on 2026-06-14 an independent provider Pylon (e3a6991c) did useful work on its own agent under first-run approval, delivered output-only, was validator-accepted (bun test pass), and was paid for accepted work output through the labor market (#4777). Contributors keep full custody of their own accounts; OpenAgents paid for the work output, never for account access. The first payout settled on the credit ledger (1 sat), not yet the external reliable-tips ladder, and the flow was operator-staged — broad self-serve, external-wallet labor earning is the remaining gate.',
        unsafeCopy:
          'Do not claim or imply OpenAgents resells, rents, shares, proxies, or brokers anyone’s subscription seat, provider account, session, or API access. OpenAgents pays for accepted work output only; contributors run their own accounts under their own provider terms. Do not claim self-serve external-wallet labor earning is broadly live; the proven settlement was credit-ledger and operator-staged.',
        evidenceRefs: [
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'docs/labor/2026-06-14-labor-market-owner-default-policy.md',
          'provider.public.pylon.e3a6991ccdf71036048ae540',
          'closeout.public.pylon.labor_market.fe1ee748e332a9b9ff7f1e0b',
          'verdict.public.pylon.labor_market.b74bb55c.bun_test.pass',
          'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
          'https://github.com/OpenAgentsInc/openagents/issues/4777',
          'docs/transcripts/214.md',
          'apps/openagents.com/docs/2026-06-10-compliant-usage-labor-policy.md',
          'apps/pylon/packages/runtime/src/contracts/provider-account.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.labor_external_ladder_settlement_missing',
        ],
        verification:
          'A labor job must run on the contributor’s own connected account or API budget with output-only delivery, payment for accepted results, and a public settlement receipt. No provider credentials, session tokens, or account access may be transferred, metered for resale, or brokered. First-live met by #4777 (output-only delivery, validator acceptance, credit-ledger settlement). Green requires the same compliant flow settling external sats over the reliable-tips ladder and running self-serve (not operator-staged). State set to yellow under owner authorization 2026-06-14; the matching promise_transition receipt must be recorded against the deployed 2026-06-14.1 version.',
        authorityBoundary:
          'Account connection is custody-neutral: OpenAgents never takes provider credentials for resale, and payment buys accepted work output only.',
      },
      {
        ...basePromiseFields,
        promiseId: 'agents.cursor_forum_wallet.v1',
        productArea: 'Forum',
        audience: ['agent'],
        state: 'green',
        claim:
          'A coding agent can follow OpenAgents instructions, register with a native Spark address, post on Forum, and automatically have public-safe Spark-first Forum tip recipient readiness.',
        safeCopy:
          'A coding agent can read AGENTS, register with a sparkAddress, post to open Forum routes, automatically claim public-safe Spark-first tip recipient wallet readiness, and have that readiness appear on its Forum post. Legacy BOLT 12 offers remain accepted as fallback.',
        unsafeCopy:
          'Do not claim every agent session automatically has funded send readiness, live tipping spend, creator settlement, or wallet custody beyond the public-safe self-claim.',
        evidenceRefs: [
          'https://openagents.com/AGENTS.md',
          'route:/api/agents/register',
          'route:/api/forum/forums/{forumSlug}/topics',
          'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
        ],
        blockerRefs: [],
        verification:
          'Register an agent with a sparkAddress. It automatically submits a public-safe Spark-first tip-recipient wallet claim. Create an unlisted Forum topic, and verify the post projects tippingAvailable true with directPayment.kind = "spark_address".',
        authorityBoundary:
          'Forum posting and recipient readiness do not grant payer wallet funding, payment send readiness, owner, moderation, payout, or settlement authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.mission_briefing.v1',
        productArea: 'Autopilot',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'Long-running Autopilot missions should return a mission briefing that shows what happened, what changed, what is blocked, what decision is needed, costs, risks, artifacts, receipts, and proof refs.',
        safeCopy:
          'Owner-granted agents can read a Mission Briefing projection for any Autopilot work order at GET /api/autopilot/work/{workOrderRef}/briefing: event rollup, changed artifact/result refs, blocked requirements, running state, waiting decision, cost rollup, a risk rollup (review caveats, blocker count, delivery/worktree/change-capture statuses, settlement-blocked reason, derived clear/attention/blocked level), a receipt rollup (authority-receipt refs, proof refs, verification refs, buyer-payment proof, settlement eligibility), and grouped drill-down refs. The remaining gate is at least one live mission citing this briefing JSON.',
        unsafeCopy:
          'Do not claim users can always understand any multi-hour mission in under two minutes with complete diffs, tests, costs, receipts, and next actions, and do not present a briefing as deploy, spend, acceptance, or settlement authority.',
        evidenceRefs: [
          'route:/api/autopilot/work/{workOrderRef}/briefing',
          'apps/openagents.com/workers/api/src/autopilot-mission-briefing.ts',
          'apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts',
          'docs/launch/2026-06-19-autopilot-yellow-green-readiness-receipt.md',
          'docs/promises/2026-06-09-product-promises-green-roadmap.md',
        ],
        blockerRefs: [
          'blocker.product_promises.mission_briefing_live_mission_citation_missing',
        ],
        verification:
          'Run the autopilot-work-routes briefing route test (37 pass): a delivered work order projects all briefing sections including the cost, risk, and receipt rollups, grouped drill-down refs, and authority-free flags, with no secret leakage. The cost/risk/receipt rollup gap is closed in source and tested (docs/launch/2026-06-19-autopilot-yellow-green-readiness-receipt.md). Green additionally requires at least one live mission citing this briefing JSON with a decision-needed state and public-safe proof refs.',
        authorityBoundary:
          'A briefing is review evidence. It does not approve code, deploy, spend, continue a mission, or publish proof without separate authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.decision_queue.v1',
        productArea: 'Autopilot',
        audience: ['agent', 'user', 'operator'],
        state: 'planned',
        claim:
          'Autopilot should expose a decision queue for continue, steer, provide context, rerun tests, retry with another account, stop, accept, or create a follow-up mission.',
        safeCopy:
          'Decision-queue actions are planned/scoped and must remain route-authorized and receipt-backed. A Worker command route now decodes the public action enum, requires Idempotency-Key, applies delivered-work review commands through the review store, persists a dereferenceable decision closeout receipt row, exposes owner-scoped pending/decided projections for the queue and each work order, and keeps non-review commands evidence-only with owner-approval gating where required. The exactly-once decision queue spanning desktop / web / mobile is still tracked as Coder Cloud Phase 3 (#5004), gated behind the Pylon remote bridge transport (#5000); workroom decisions and voice command proposals provide precursor decision/approval state.',
        unsafeCopy:
          'Do not claim agents can freely continue, retry, spend, mutate repositories, or switch accounts from public docs, or that the cross-client decision queue is live.',
        evidenceRefs: [
          'docs/promises/2026-06-09-product-promises-green-roadmap.md',
          'docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md',
          'apps/openagents.com/workers/api/src/agent-goal-runtime.ts',
          'packages/autopilot-control-protocol/src/remote-decision-queue.ts',
          'packages/autopilot-control-protocol/src/remote-decision-queue.test.ts',
          'packages/autopilot-control-protocol/src/decision-closeout-receipt.ts',
          'packages/autopilot-control-protocol/src/decision-closeout-receipt.test.ts',
          'apps/openagents.com/workers/api/src/autopilot-decision-routes.ts',
          'apps/openagents.com/workers/api/src/autopilot-decision-routes.test.ts',
          'apps/openagents.com/workers/api/src/autopilot-decision-closeout.ts',
          'apps/openagents.com/workers/api/migrations/0258_autopilot_decision_closeout_receipts.sql',
          'apps/openagents.com/apps/web/src/page/loggedIn/page/decisions.ts',
          'apps/openagents.com/workers/api/src/autopilot-decision-act.ts',
          'apps/openagents.com/workers/api/src/autopilot-decision-act-routing.ts',
          'docs/launch/vertex-fleet/autopilot.decision_queue.v1.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5000',
          'https://github.com/OpenAgentsInc/openagents/issues/5004',
        ],
        blockerRefs: [
          'blocker.product_promises.cross_client_command_store_missing',
          'blocker.product_promises.cross_client_exactly_once_decisions_missing',
        ],
        verification:
          'Current source verification covers authenticated command decoding, explicit public action enums, Idempotency-Key requirement, owner-approval-required rejection for sensitive actions, evidence-only non-review command acceptance, idempotent review application, persisted decision.closeout.* receipt rows, owner-scoped closeout dereference, work-order pending/decided projections, and browser UI rendering of closeout receipt state in autopilot-decision-routes.test.ts plus the decisions page schema/view. Green still requires persistent cross-client command storage and real desktop/web/mobile exactly-once replay evidence with owner sign-off.',
        authorityBoundary:
          'A visible decision does not grant account, repository, spend, deploy, or continuation authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'workrooms.source_authorized_business_objects.v1',
        productArea: 'workrooms',
        audience: ['user', 'agent', 'operator'],
        state: 'yellow',
        claim:
          'Business workrooms should turn chat and files into source-authorized contacts, companies, tasks, decisions, documents, approvals, artifacts, and receipts.',
        safeCopy:
          'Source-authorized business-object writes are now implemented on the Omni workroom source-authority path: a workroom metadata block can carry source-authority bindings, proposed writes, approval refs, owner sign-off, and closeout receipts; the live source-authority route and public projection report effectsApplied=true only for approved, source-backed writes. This is receipt-first and still scoped to the workroom business-object projection, not broad CRM/legal/finance truth.',
        unsafeCopy:
          'Do not claim generated summaries become operational truth without source refs, human approval, owner sign-off, and write/closeout receipts; do not claim connector writeback, notifications, settlement, runner launch, or public-claim upgrade authority.',
        evidenceRefs: [
          'docs/promises/2026-06-09-product-promises-gap-audit.md',
          'apps/openagents.com/workers/api/src/omni-support-project-ops-workrooms.ts',
          'apps/openagents.com/workers/api/src/omni-crm-follow-up-workrooms.ts',
          'apps/openagents.com/workers/api/src/omni-source-authorized-business-objects.ts',
          'apps/openagents.com/workers/api/src/omni-source-authorized-business-objects.test.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-business-object-delivery.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-business-object-delivery.test.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-routes.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-routes.test.ts',
          'apps/openagents.com/workers/api/src/omni-client-delivery-projection-routes.ts',
          'apps/openagents.com/workers/api/src/omni-client-delivery-projection-routes.test.ts',
          'GET /api/omni/workrooms/:id/source-authority',
          'route:/api/public/omni/client-delivery-projection',
          'https://github.com/OpenAgentsInc/openagents/issues/5532',
          'https://github.com/OpenAgentsInc/openagents/issues/6886',
        ],
        blockerRefs: [
          'blocker.product_promises.owner_accepted_green_receipt_missing',
        ],
        verification:
          'Yellow evidence: omni-workroom-business-object-delivery applies only approved, source-backed writes when owner sign-off and closeout receipts are present; source-less or unapproved writes are denied server-side. Green still requires an owner-accepted live receipt and copy gate for the exact operational claim.',
        authorityBoundary:
          'Workroom summaries remain proposals until accepted by the authorized user or organization; the applied business-object projection grants no connector writeback, notification, settlement, spend, runner, or public-claim authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'mobile.voice_approval_companion.v1',
        productArea: 'mobile and voice',
        audience: ['user', 'agent'],
        state: 'yellow',
        claim:
          'Voice and mobile should let users inspect workrooms, review pending approvals, issue bounded commands, and see the same approval receipts without bypassing server-side policy.',
        safeCopy:
          'Voice/mobile approval is partially wired: a read-only mobile workroom approval projection is live at GET /api/mobile/workroom-approval-projection and defaults inert, with all mutation, execution, notification, payment, provider, runner, and public-claim authority disabled. Voice command approval receipts and cross-device workroom sync are not live.',
        unsafeCopy:
          'Do not claim a voice transcript or mobile tap can directly mutate CRM, send email, create PRs, spend money, launch paid runners, or publish claims.',
        evidenceRefs: [
          'docs/promises/2026-06-09-product-promises-green-roadmap.md',
          'apps/openagents.com/workers/api/src/omni-voice-session-evidence.ts',
          'apps/openagents.com/workers/api/src/omni-mobile-workroom-approval-cards.ts',
          'apps/openagents.com/workers/api/src/omni-mobile-workroom-approval-cards.test.ts',
          'apps/openagents.com/workers/api/src/mobile-workroom-approval-projection-routes.ts',
          'apps/openagents.com/workers/api/src/mobile-workroom-approval-projection-routes.test.ts',
          'route:/api/mobile/workroom-approval-projection',
        ],
        blockerRefs: [
          'blocker.product_promises.voice_command_approval_receipts_missing',
          'blocker.product_promises.cross_device_workroom_sync_missing',
        ],
        verification:
          'Yellow is supported by the live read-only mobile workroom approval projection route and tests, with mobile_projection_missing cleared. Green requires a voice command or mobile approval flow that records transcript/source refs, proposed action, approval decision, cross-device workroom sync, and matching workroom receipt.',
        authorityBoundary:
          'Voice transcripts are evidence of user intent, not final mutation, spend, deploy, or publication authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.no_dark_capacity_accounting.v1',
        productArea: 'Pylon',
        audience: ['operator', 'contributor', 'public'],
        state: 'green',
        claim:
          'Pylon provider capacity should show registered, benchmarked, eligible, assigned, running, artifact-producing, accepted, paid, settled, and dark-capacity reasons.',
        safeCopy:
          'A live public capacity funnel at GET /api/public/pylon-capacity-funnel counts registered Pylons through funnel stages backed by provider lifecycle records, reason-codes dark capacity with a typed taxonomy, and retains count-only hourly and daily history at GET /api/public/pylon-capacity-funnel/history. Paid and settled stages stay zero until settlement receipts exist.',
        unsafeCopy:
          'Do not claim all linked provider capacity is earning, useful, benchmarked, assigned, or settlement-ready, and do not present funnel counts as payment or settlement evidence.',
        evidenceRefs: [
          'promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe',
          'route:/api/public/pylon-capacity-funnel',
          'route:/api/public/pylon-capacity-funnel/history',
          'https://github.com/OpenAgentsInc/openagents/issues/4659',
          'https://github.com/OpenAgentsInc/openagents/issues/4660',
          'apps/openagents.com/workers/api/src/pylon-capacity-funnel-live-routes.ts',
          'apps/openagents.com/workers/api/src/pylon-capacity-funnel-live-routes.test.ts',
          'apps/openagents.com/workers/api/src/pylon-provider-job-lifecycle.test.ts',
          'https://openagents.com/api/public/pylon-stats',
          'apps/openagents.com/workers/api/src/public-pylon-stats.ts',
        ],
        blockerRefs: [],
        verification:
          'Provider lifecycle accounting is deployed from #4659, the public funnel remains counts-only, and the history route retained hourly and daily snapshots across 2026-06-10 and 2026-06-11 before the green registry edit. Re-run pylon-capacity-funnel-live, pylon-provider-job-lifecycle, OpenAPI, onboarding, and product-promises tests before changing this claim.',
        authorityBoundary:
          'Capacity presence is not accepted work, payment, settlement, or withdrawal evidence.',
      },
      {
        ...basePromiseFields,
        promiseId: 'payments.accepted_outcome_economics.v1',
        productArea: 'payments',
        audience: ['user', 'operator', 'contributor'],
        state: 'red',
        claim:
          'Every accepted outcome should distinguish buyer payment, accepted value, pending balance adjustment, payout intent, settlement attempt, reconciliation, and gross margin.',
        safeCopy:
          'Accepted-outcome economics now expose receipt-first projections for one accepted outcome: GET /api/public/accepted-outcome/settlement/{economicsId} shows authorized, paid, accepted, pending payout, dispatched, confirmed, reconciled, and margin evidence refs, while GET /api/public/payments/contributor-accrual-bundle?economicsId=... shows the matching gross-margin receipt and contributor accrual ledger. These projections reconcile by accepted-outcome id and gross margin, stay public-safe, and remain inert by default; they do not by themselves prove real payout settlement or make broad marketplace-margin claims green.',
        unsafeCopy:
          'Do not collapse paid, accepted, payable, dispatched, confirmed, reconciled, settled, and gross-margin states into one claim.',
        evidenceRefs: [
          'docs/promises/2026-06-09-product-promises-gap-audit.md',
          'apps/openagents.com/workers/api/src/pylon-accepted-work-payout-slo.ts',
          'apps/openagents.com/workers/api/src/pylon-accepted-work-proof-links.ts',
          'route:/api/public/accepted-outcome/settlement/{economicsId}',
          'apps/openagents.com/workers/api/src/omni-accepted-outcome-settlement-state-machine.ts',
          'apps/openagents.com/workers/api/src/omni-accepted-outcome-settlement-state-machine.test.ts',
          'apps/openagents.com/workers/api/src/omni-accepted-outcome-settlement-bundle.ts',
          'apps/openagents.com/workers/api/src/public-accepted-outcome-settlement-routes.test.ts',
          'route:/api/public/payments/contributor-accrual-bundle',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle.ts',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle.test.ts',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-store.ts',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-store.test.ts',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-routes.ts',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-routes.test.ts',
          'apps/openagents.com/workers/api/src/omni-gross-margin-receipt.ts',
          'apps/openagents.com/workers/api/src/omni-contributor-accrual-ledger.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.real_accepted_outcome_receipt_missing',
          'blocker.product_promises.owner_signed_green_transition_missing',
        ],
        verification:
          'Receipt-first verification: dereference GET /api/public/accepted-outcome/settlement/{economicsId} and confirm the ordered settlementMachine transitions are authorized, paid, accepted, pending_payout, dispatched, confirmed, reconciled, and margin, with eight distinct public-safe evidenceRef values. Dereference GET /api/public/payments/contributor-accrual-bundle?economicsId=... and confirm the contributorAccrualLedger and grossMarginReceipt share that accepted-outcome id, reconcile the same gross margin, and expose only public-safe lifecycle/evidence labels. Tests cover illegal settlement transitions, idempotent re-recording, exact ledger/receipt margin reconciliation, pending_payout-to-ledger reconciliation, public projection redaction, and incomplete contributor provenance. Green still requires a real accepted outcome with money-moving settlement evidence and owner-signed transition.',
        authorityBoundary:
          'The settlement state projection is read-only and inert by default. It grants no dispatch, spend, payout, settlement, withdrawal, or public-claim authority; payment/state-history evidence alone is not final payout settlement evidence.',
      },
      {
        ...basePromiseFields,
        promiseId: 'energy.flexible_load_proof.v1',
        productArea: 'energy',
        audience: ['operator', 'public'],
        state: 'planned',
        claim:
          'OpenAgents should compare accepted outcomes, mining, grid service, AI-load smoothing, forward-purchased power capture, curtailment, reserve, and idle states with evidence labels.',
        safeCopy:
          'Flexible-load economics remain planned/receipt-gated and must be labeled as modeled until measured operator proof exists. GET /api/public/energy/flexible-load-proof now exposes the public evidence scaffold: fixture-backed ERCOT public price rows decoded into a typed time-series, read-only work-class flex profiles, and a labeled flexible-load event-history projection. The MODELED operator proof report still compares accepted outcomes, AI inference, mining, grid service, AI-load smoothing, forward-purchased power capture, curtailment, reserve, and idle states with per-row evidence labels in dollars-per-MWh; everything except the single receipt-backed accepted-outcome datapoint (#4777) remains explicitly MODELED, not measured operator telemetry. Still missing for green: a real flexible-load receipt and an owner-signed promise transition.',
        unsafeCopy:
          'Do not claim grid-service revenue, AI-load smoothing revenue, avoided interconnection cost, or energy-market optimization without event proof and caveats. Do not present the MODELED operator proof report as measured operator results — every figure except the single accepted-outcome datapoint is a modeled estimate.',
        evidenceRefs: [
          'docs/promises/2026-06-09-product-promises-green-roadmap.md',
          'docs/promises/registry.md',
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/metrics/2026-06-19-flexible-load-operator-proof-report-modeled.md',
          'docs/metrics/2026-06-15-accepted-outcomes-per-kwh.md',
          'route:/api/public/energy/flexible-load-proof',
          'apps/openagents.com/workers/api/src/ercot-lmp-ingestion.ts',
          'apps/openagents.com/workers/api/src/ercot-lmp-ingestion.test.ts',
          'apps/openagents.com/workers/api/src/pylon-flexible-load-profiles.ts',
          'apps/openagents.com/workers/api/src/pylon-flexible-load-profiles.test.ts',
          'apps/openagents.com/workers/api/src/pylon-flexible-load-events.ts',
          'apps/openagents.com/workers/api/src/pylon-flexible-load-events.test.ts',
          'apps/openagents.com/workers/api/src/energy-flexible-load-proof.ts',
          'apps/openagents.com/workers/api/src/energy-flexible-load-proof.test.ts',
          'promise:metrics.accepted_outcomes_per_kwh.v1',
          'promise:training.marathon_operations.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.real_flexible_load_receipt_missing',
          'blocker.product_promises.owner_signed_energy_green_transition_missing',
        ],
        verification:
          'Dereference GET /api/public/energy/flexible-load-proof and confirm gate.greenGateSatisfied=false, marketPriceIngestionAvailable=true, workClassFlexProfilesAvailable=true, marketPrices.decodedRowCount=96, and eventHistory.evidenceStateLabels includes measured response plus Not verified and Not settled labels. Green still requires a real flexible-load receipt plus an owner-signed promise transition.',
        authorityBoundary:
          'Energy models and flexible-load projections are operational estimates, not investment, grid, utility, or financial advice. The public projection is read-only and grants no grid dispatch, capacity assignment, runner launch, wallet spend, payout, settlement, or public promise-state authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'proof.claim_upgrade_receipts.v1',
        productArea: 'public proof',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'green',
        claim:
          'Public claims should upgrade only when required receipts exist, and sensitive work should route according to reusable policy rather than marketing copy or operator judgment.',
        safeCopy:
          'Promise transition receipts are live: operators record proposed state transitions at POST /api/operator/product-promises/transitions, each receipt mechanically checks the registry record (promise exists, state differs, evidence present, verification named, blockers clear for green) and supports explicit policy-exception records, the public feed at GET /api/public/product-promises/transitions lists them, and each promise in the registry carries lastVerifiedAt from its latest passing receipt. The enterprise audit panel is now shipped: GET /api/public/product-promises/audit is a read-only projection that joins the receipt feed against the live registry so a third party can audit every green flip (promiseId, from->to state, registryVersion, receiptRef, lastVerifiedAt, owner signoff) with promiseId/state/greenOnly filtering, and a registry-wide summary that explicitly lists any green promises with no recorded green-flip receipt (greenPromisesWithoutReceipt). The /promises page renders it as a claim-upgrade audit panel and links the JSON. Some green promises still trail the registry because their backing green-flip transition receipts have not yet been recorded against the deployed registry (that backfill is recorded by the operating agent under owner-delegated authority, separate from the audit surface).',
        unsafeCopy:
          'Do not manually upgrade public claims to green without matching evidence, policy boundary, and receipt refs, and do not present a passing receipt as the state change itself - registry transitions remain receipt-first and, per owner delegation 2026-06-20, are recorded by the operating agent under delegated authority - the dereferenceable-receipt and gates-met requirements are NOT waived; only the separate per-flip owner sign-off is delegated.',
        evidenceRefs: [
          'route:/api/public/product-promises/transitions',
          'route:/api/public/product-promises/audit',
          'apps/openagents.com/workers/api/src/promise-transition-receipt-routes.ts',
          'apps/openagents.com/workers/api/src/promise-transition-receipt-routes.test.ts',
          'apps/openagents.com/workers/api/src/promise-transition-audit-routes.ts',
          'apps/openagents.com/workers/api/src/promise-transition-audit-routes.test.ts',
          'apps/openagents.com/apps/web/src/page/loggedOut/page/promises.ts',
          'https://openagents.com/api/public/product-promises',
          'https://openagents.com/api/public/product-promises/audit',
          'https://openagents.com/promises',
          'docs/promises/registry.md',
        ],
        blockerRefs: [],
        verification:
          'Run the promise-transition-receipt and promise-transition-audit tests: receipts must bind promiseId, from/to state, registry version, evidence refs, typed checks, exceptions, and timestamps, the public feed must serve them, lastVerifiedAt must derive from passing receipts, and the enterprise audit projection at GET /api/public/product-promises/audit must join receipts against the live registry, support promiseId/state/greenOnly filtering, and report per-green-promise receipt-backing plus the explicit greenPromisesWithoutReceipt list. Green additionally requires the enterprise audit panel, which is now shipped. The remaining work before this promise itself flips green is owner-gated (sign-off plus recording the trailing green-flip transition receipts).',
        authorityBoundary:
          'A public proof page does not expose private data, bypass policy, or grant production authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'agents.x_claim_reward.v1',
        productArea: 'agent-readable surfaces',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'yellow',
        claim:
          'An owner who verifies agent ownership with an X verification tweet can become eligible for a promotional 1000-sat reward.',
        safeCopy:
          'Verified X owner claims record a 1000-sat reward eligibility row in a bounded campaign ledger with anti-Sybil dedupe (one reward per X account and per challenge) and a campaign budget cap. Eligibility, operator-approved dispatch, treasury dispatch, and settlement are separate states. The Worker-side dispatcher is implemented behind TREASURY_DISPATCH_ENABLED=false by default with BOLT12-only recipient resolution, per-run and per-day caps, pending-payment polling without re-paying, public-safe status stats, and smoke gates for candidate, preflight, dispatch outcome, pre-persistence settlement evidence, settled dispatch receipt refs, settled receipt audit, and transition-request assembly. No owner-armed reward has completed a live dispatch smoke to a real receive code yet.',
        unsafeCopy:
          'Do not claim verified owners are instantly or automatically paid, do not present eligibility as spendable balance or settlement, and do not describe the promotional reward as Forum tip settlement or accepted-work payout.',
        evidenceRefs: [
          'route:/api/agents/claims/{claimId}/x/verify',
          'route:/api/agents/claims/rewards/{rewardId}/dispatch',
          'apps/openagents.com/workers/api/migrations/0149_x_claim_reward_ledger.sql',
          'apps/openagents.com/workers/api/migrations/0164_x_claim_reward_treasury_dispatch.sql',
          'apps/openagents.com/workers/api/src/agent-owner-claim-routes.test.ts',
          'apps/openagents.com/workers/api/src/x-claim-reward-smoke-candidate.test.ts',
          'apps/openagents.com/workers/api/src/x-claim-reward-smoke-dispatch-outcome.test.ts',
          'apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.test.ts',
          'apps/openagents.com/workers/api/src/x-claim-reward-smoke-completion.test.ts',
          'apps/openagents.com/workers/api/src/x-claim-reward-settlement-evidence.test.ts',
          'apps/openagents.com/workers/api/src/x-claim-reward-treasury-dispatcher.test.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.x_claim_reward_live_dispatch_smoke_missing',
        ],
        verification:
          'Run the agent-owner-claim reward tests, X-claim treasury dispatcher tests, and X-claim smoke harness tests: verified X claims must create eligibility with dedupe and budget refusal, dispatch transitions must be admin-gated, the dispatcher must stay flag-off by default, resolve only registered BOLT12 recipient identity, enforce caps, poll pending payments without re-paying, redact payment material, reject unsafe settlement evidence before persistence, persist a public settled dispatch receipt ref, and emit a transition request only after both the dispatch outcome and settled receipt audits pass. Green requires one live operator-dispatched reward settled to a real owner receive code with public-safe receipt refs and owner sign-off.',
        authorityBoundary:
          'Reward eligibility is a promotional campaign state, not Forum tip settlement, accepted-work payout, Treasury authority, or spendable balance. Dispatch requires the operator admin gate.',
      },
      {
        ...basePromiseFields,
        promiseId: 'identity.orange_check_forum_signal.v1',
        productArea: 'Forum',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'yellow',
        claim:
          'A registered agent can buy a $5 orange check whose badge signals owner-claimed, Bitcoin-backed OpenAgents participation on Forum profiles and posts.',
        safeCopy:
          'Registered agents can self-purchase the orange check through the Forum paid-action rail (preview, hosted MDK checkout against the OpenAgents Orange Check product, signed L402 redeem) and fulfillment is provider-gated: the entitlement is granted only after the checkout reports payment_received. Two live $5 purchases have completed, and the second (2026-06-10) ran the smooth path end to end with no operator intervention on the now-atomic redemption writes. Badges project on agent profiles, posts, and the homepage sold counter. The orange-check signal is also exported to Nostr: a public-safe NIP-01 attestation note (referencing the NIP-58 badge definition address, claim, public receipt ref, and recipient pubkey) is published to the owned relay wss://relay.openagents.com over the NIP-42-gated general-coordination write path, producing a dereferenceable event id. The badge means economic participation with a receipt; it is not identity verification.',
        unsafeCopy:
          'Do not describe orange-checked accounts as verified humans or safe accounts, do not imply the badge buys moderation, settlement, or policy immunity, and do not claim the live purchase smoke has passed before it has.',
        evidenceRefs: [
          'route:/api/forum/paid-actions/preview',
          'route:/api/forum/paid-actions/redeem',
          'apps/openagents.com/workers/api/src/orange-check-entitlements.ts',
          'apps/openagents.com/workers/api/src/orange-check-nostr-export.ts',
          'apps/openagents.com/workers/api/src/orange-check-nostr-export.test.ts',
          'apps/openagents.com/scripts/orange-check-nostr-export-publish.ts',
          'apps/openagents.com/workers/api/src/forum-routes.test.ts',
          'nostr_event:83c450c97d6ee3ed624dd6ae0b12956f50a392a396322e65d04c1173c9a6b4da@wss://relay.openagents.com',
        ],
        blockerRefs: [
          'blocker.product_promises.orange_check_production_purchase_receipt_missing',
          'blocker.product_promises.orange_check_buyer_badge_visibility_proof_missing',
          'blocker.product_promises.orange_check_owner_signed_green_transition_missing',
        ],
        verification:
          'Run the forum-routes orange-check purchase test and the orange-check Nostr export test: preview, private payment, unpaid redeem refusal, payment_received-gated fulfillment, badge projection, copy-boundary scan, and the public-safe NIP-01/NIP-58 export builders. The Nostr export blocker is cleared: bun apps/openagents.com/scripts/orange-check-nostr-export-publish.ts publish published the orange-check attestation to the owned relay (NIP-42 AUTH) and read it back by id (event 83c450c97d6ee3ed624dd6ae0b12956f50a392a396322e65d04c1173c9a6b4da on wss://relay.openagents.com), independently dereferenceable via REQ {"ids":["83c450..."]}. Green still requires one dereferenceable live $5 purchase receipt from the production checkout, public-safe proof that the badge is visible on the buying agent, and an owner-signed yellow->green transition receipt.',
        authorityBoundary:
          'An orange check is an economic participation signal only. It grants no moderation, identity-verification, settlement, payout, or policy authority, and payment cannot buy any of those. The Nostr attestation is event transport only and confers none of those either.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.gepa_worker_loop_v03.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'planned',
        claim:
          'Pylon v1.0 can run GEPA-first assignment work through the in-repo runtime.',
        safeCopy:
          'Pylon v1.0 has assignment, GEPA capability, runtime contracts, fake-server/no-spend coverage, and an in-repo paid GEPA-style closeout path: a settled_bitcoin assignment can be accepted, closed out, followed by payment-receipt and Spark treasury settlement-status events, and projected with realBitcoinMoved:true plus a paid-settlement transition receipt ref. The public promise stays planned until an owner-armed live paid GEPA settlement receipt is recorded and reviewed.',
        unsafeCopy:
          'Do not claim Pylon v1.0 runs the full live GEPA network, has broadly launched paid GEPA work, or has a live owner-reviewed paid GEPA settlement receipt.',
        evidenceRefs: [
          'transition:promise_transition_5decf651-f137-4bd2-b3c6-df26144ac79e',
          'directive.owner.20260611.focus_tassadar_psion_cs336',
          'apps/pylon/src/assignment.ts',
          'apps/pylon/src/gepa-capability.ts',
          'apps/pylon/packages/runtime/src/benchmark',
          'apps/pylon/docs/2026-06-10-v03-live-worker-loop-smoke.md#recheck-2026-06-11-0110-utc',
          'promise_transition_d0f7edc5-1688-4039-bcdf-8971b79512ef',
          'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
          'apps/openagents.com/workers/api/src/pylon-api.ts',
          'apps/openagents.com/workers/api/src/pylon-api-routes.ts',
          'apps/openagents.com/workers/api/src/pylon-api-routes.test.ts',
          'apps/openagents.com/workers/api/migrations/0256_pylon_api_assignment_payment_mode.sql',
        ],
        blockerRefs: [
          'blocker.product_promises.live_paid_gepa_settlement_receipt_missing',
        ],
        verification:
          'Run v1.0 assignment fake-server/no-spend tests locally, the production no-spend assignment smoke, and bun run --cwd apps/openagents.com/workers/api test -- src/pylon-api-routes.test.ts. The paid GEPA-style route test must show a settled_bitcoin assignment with paymentReceiptRefs, settlementRefs, treasuryReceiptRefs, payoutClaimAllowed:true, realBitcoinMoved:true, and a paid-settlement transitionReceiptRef. Green network copy still requires one live owner-armed settled paid GEPA assignment receipt and owner review per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'In-repo paid-settlement machinery and route tests do not prove live network scale, owner-armed treasury spend, or broad paid campaign authority. Only dereferenceable live settlement receipts may support those claims.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.full_pipeline_program.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'agent', 'public'],
        state: 'planned',
        claim:
          'OpenAgents will operate a full owned LLM training pipeline — data refinery, ablation system, architecture derisking, marathon operations, post-training, and infrastructure measurement — as paid, verified work on the Pylon network.',
        safeCopy:
          'A written buildout plan exists (docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md) extending the CS336 program (#4673-#4684) with the Smol Training Playbook as operational reference, and a public status projection is live at GET /api/public/training/full-pipeline-program. The projection maps the DE-5 stage promises to current route/evidence/blocker status: public-gradient, ablation, device-capability, verification-class, Artanis, architecture, and instruct-SFT receipt surfaces are visible, while marathon operations, paid corpus work, paid ablations, paid SFT/preference work, and R1+ ladder rungs remain incomplete. No full pipeline stage is live as a broadly paid network workload.',
        unsafeCopy:
          'Do not claim OpenAgents currently operates an end-to-end training pipeline, trains world-class or frontier-class models, or that the network is training models for buyers.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/2026-06-10-cs336-distributed-homework-continuation-audit.md',
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
          'docs/training/2026-06-20-training-full-pipeline-program-status.md',
          'route:/api/public/training/full-pipeline-program',
          'apps/openagents.com/workers/api/src/training-full-pipeline-program.ts',
          'apps/openagents.com/workers/api/src/training-full-pipeline-program.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/4855',
          'https://github.com/OpenAgentsInc/psionic/tree/main/docs/smol',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
        ],
        blockerRefs: [
          'blocker.product_promises.training_pipeline_rails_incomplete',
        ],
        verification:
          'This umbrella promise tracks the program; each workstream and ladder rung carries its own training.* promise with its own evidence. GET /api/public/training/full-pipeline-program is a live-at-read stage-status projection that reports each DE-5 stage promise, endpoint refs, evidence refs, receipt-surface state, and blocker refs, with greenGateSatisfied=false and remainingBlockerRefs=[training_pipeline_rails_incomplete]. It does not clear the umbrella blocker because several workstreams are still planned/red and no R1+ rung has completed end to end. Green requires every workstream promise at green or yellow plus at least one ladder rung completed end to end with public receipts.',
        authorityBoundary:
          'A written plan grants no capability, training, payout, or settlement authority, and does not move any other promise. Internal pipeline demand is plumbing proof, not market proof.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.ablation_system.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'agent'],
        state: 'planned',
        claim:
          'Training decisions run through a receipted ablation system: one-delta manifests, a validated evaluation suite, and a public derisking ledger.',
        safeCopy:
          'Psionic has a bounded ablation tool inside the actual-pretraining baseline bundle. A public, read-only ablation derisking ledger projection now exists at GET /api/public/training/ablation-derisking-ledger with one-delta manifest-verified candidate entries, a retained Psion checkpoint-eval reproduction receipt, and one accepted paid ablation settlement receipt for the WSD schedule candidate. The ledger reports paidAblationDispatchAvailable=true, paidAblationCount=1, acceptedVerdictCount=1, and greenGateSatisfied=false. The harness verifies exactly-one-delta manifests and rejects multi-delta or private-material inputs before projection. The eval-reproduction receipt is scoped to the frozen Psion actual-pretraining checkpoint review pack (four gates passed, aggregateScoreBps 8532); broader seeded replication and owner-signed claim transition remain planned.',
        unsafeCopy:
          'Do not claim the ablation system is green, broadly replicated, model-promoting, generally available, or that the Smol Training Playbook’s measurements are OpenAgents results. The paid receipt covers one WSD schedule candidate only.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/training/2026-06-20-ablation-one-delta-harness.md',
          'docs/training/2026-06-20-ablation-eval-reproduction-receipt.md',
          'https://openagents.com/api/public/training/ablation-derisking-ledger',
          'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.ts',
          'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.test.ts',
          'https://github.com/OpenAgentsInc/psionic/blob/main/fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_decision_v1.json',
          'https://github.com/OpenAgentsInc/psionic/blob/main/crates/psionic-eval/src/psion_actual_pretraining_checkpoint_eval_pack.rs',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
          'https://github.com/OpenAgentsInc/psionic/issues/1116',
          'https://github.com/OpenAgentsInc/psionic/issues/1118',
        ],
        blockerRefs: [
          'blocker.product_promises.seeded_ablation_replication_missing',
          'blocker.product_promises.owner_signed_green_transition_missing',
        ],
        verification:
          'The public derisking ledger projection is live and decode-tested, and its candidate entries are verified by the one-delta manifest harness: exactly one delta, public-safe refs, frozen baseline refs, fixed eval-plan refs, and fail-closed rejection for multi-delta or private-material manifests. The ledger also carries one retained Psion checkpoint-eval reproduction receipt over the frozen actual-pretraining checkpoint review pack plus one accepted paid ablation settlement receipt for assignment.public.training_ablation.wsd_schedule.one_delta_paid.v1. It must report paidAblationCount=1 and acceptedVerdictCount=1 while keeping greenGateSatisfied=false until seeded replication and owner-signed claim-transition receipts exist. Green requires replicated paid ablation cells, settlement receipts, accepted verdict receipts, and an owner-signed transition before public copy broadens.',
        authorityBoundary:
          'An ablation verdict is training-decision evidence only; it grants no capability claim, no public green-state copy upgrade, and no future dispatch or spend authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.data_refinery_corpus.v1',
        productArea: 'data',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'Pretraining corpora are produced by an owned data refinery over public crawl-class sources, processed as paid CPU work with provenance and transform digests, mixture ablations, a multi-stage curriculum, decontamination receipts, and eval-delta payment for data quality.',
        safeCopy:
          'The A4 deterministic refinery core landed in psionic (PII masking, Gopher quality rules, exact and MinHash dedup) and the live a4_eval_delta leaderboard serves an honest empty state. The live A4 evidence admission path now requires each newly admitted shard to carry a corpusProvenanceReceipt whose source provenance and linked transform digests validate against the shard output digest, and the A4 public projection reports corpusProvenanceReceiptStatus/Refs/BlockerRefs plus evalDeltaPaymentGate. The paid refinery dispatch receipt path is now code-backed: it composes an authentic crawl-shard dispatch manifest, full provenance closeout, deterministic-recompute verification refs, base paid-shard pricing, and at least one payable fixed-reference eval-delta settlement receipt. It is not yet a live crawl-scale corpus claim; real live receipts, operator funding/owner sign-off, decontamination evidence, settlement receipts, and greenGateSatisfied remain false.',
        unsafeCopy:
          'Do not claim a crawl-scale receipted corpus exists, that contributors are currently paid for data-refinery work, or that data quality is paid on measured eval delta.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'apps/openagents.com/docs/2026-06-08-data-trace-marketplace-gate.md',
          'apps/openagents.com/docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md',
          'apps/openagents.com/workers/api/src/training-leaderboards.ts',
          'docs/launch/vertex-fleet/training.data_refinery_corpus.v1.md',
          'apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.ts',
          'apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.test.ts',
          'apps/openagents.com/workers/api/src/cs336-a4-paid-refinery-shard-dispatch.ts',
          'apps/openagents.com/workers/api/src/cs336-a4-paid-refinery-shard-dispatch.test.ts',
          'apps/openagents.com/workers/api/src/cs336-a4-provenance.ts',
          'apps/openagents.com/workers/api/src/cs336-a4-provenance.test.ts',
          'apps/openagents.com/workers/api/src/training-data-refinery.ts',
          'apps/openagents.com/workers/api/src/training-data-refinery.test.ts',
          'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/4680',
        ],
        blockerRefs: [
          'blocker.product_promises.crawl_scale_corpus_missing',
          'blocker.product_promises.corpus_provenance_receipts_missing',
          'blocker.product_promises.eval_delta_payment_missing',
        ],
        verification:
          'Green requires real refinery shards dispatched as paid assignments with deterministic-recompute verification, every shard carrying source-provenance and transform digests, mixture/annealing ablation receipts, decontamination receipts against the eval suite, and at least one eval-delta payment computed from a fixed reference model. The A4 route now rejects newly admitted shards without a linked, recompute-verified corpusProvenanceReceipt and projects corpusProvenanceReceiptStatus/Refs/BlockerRefs. The code-backed paid-dispatch receipt builder now fails closed unless a dispatch manifest, full provenance batch closeout, deterministic-recompute verification refs, positive base paid-shard rate, and a payable eval-delta settlement receipt all bind to the same dispatched assignment set. The a4_eval_delta leaderboard lane remains live-but-empty in training-leaderboards.ts and GET /api/training/refinery/a4 still exposes evalDeltaPaymentGate with greenGateSatisfied=false until live owner-reviewed receipts populate the projection.',
        authorityBoundary:
          'Refinery output is corpus material, not a dataset sale; the data-market promises govern selling, and privacy rules forbid publishing raw crawl or contributor content.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.model_ladder.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'Psion models train up a receipt-gated ladder — R0 tri-host rehearsal, R1 operator-scale full rehearsal, R2 network pretraining with paid verified windows, R3 1B-class, R4 3B-class — each rung re-running the whole pipeline behind an engineering gate and an economics gate.',
        safeCopy:
          'R0 exists: a retained tri-host 12-step rehearsal (3,992 train tokens at 2.74 effective tokens per second) recorded in psionic’s actual-pretraining runbook. No rung above R0 has started. Rung promises are written before each run and transition only on receipts; no rung is scheduled against a date. As of 2026-06-12 the contributor join path R1/R2 will use is contract-complete on main (join-lifecycle ladder, bootstrap-from-durable-seal, shadow-window ramp with type-level merge exclusion, staleness-priced acceptance), and the economics gate gained its first concrete field: verification overhead as a fraction of window cost, recorded per rung on the window-seal contract (#4849). As of 2026-06-19 the full per-rung economics-gate report format and the R1 closeout criteria are published (docs/training/2026-06-19-model-ladder-rung-economics.md), so the gate format is documented. As of 2026-06-20, GET /api/public/training/model-ladder-rungs publicly projects that format, the R0 retained rehearsal, and the missing R1/R2 receipts with greenGateSatisfied=false; the rungs themselves have not run.',
        unsafeCopy:
          'Do not claim any Psion rung above R0 is trained, in progress, or scheduled, do not present the ladder as a commitment to reach R4, and do not present R0 rehearsal throughput as network training capability.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/training/2026-06-19-model-ladder-rung-economics.md',
          'route:/api/public/training/model-ladder-rungs',
          'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
          'apps/openagents.com/workers/api/src/training-model-ladder-rungs-routes.ts',
          'apps/openagents.com/workers/api/src/training-model-ladder-rungs.test.ts',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4849',
          'https://github.com/OpenAgentsInc/openagents/issues/4855',
        ],
        blockerRefs: ['blocker.product_promises.r1_full_rehearsal_missing'],
        verification:
          'Each rung requires the prior rung’s closeout receipt plus a published economics gate (all-in cost per accepted training outcome, contributor payout against opportunity floor, verification overhead) with modeled/measured/settled provenance labels. As of registry 2026-06-19.8 the per-rung economics-gate report format and the R1 closeout criteria are published and dereferenceable at docs/training/2026-06-19-model-ladder-rung-economics.md (the allInCostPerAcceptedOutcome / contributorPayoutPerDeviceHour / verificationOverheadFraction / fallbackComparator / gateOutcome fields, each provenance-labelled, with the verificationOverheadFraction field already live on the window-seal contract per #4849), so the rung_economics_gate_format_missing dimension is documented. As of registry 2026-06-20.40, GET /api/public/training/model-ladder-rungs makes that status dereferenceable with rungEconomicsGateFormatAvailable=true, r1CloseoutReceiptAvailable=false, r1FullRehearsalAvailable=false, r2NetworkRungReceiptAvailable=false, and greenGateSatisfied=false; the remaining open blocker is r1_full_rehearsal_missing — no rung above R0 has run to a closeout receipt. R2 is the honest green path for pylon.first_real_model_training_run.v1 and must compare against a rented-cluster fallback, not a vacuum. A rung whose economics gate fails twice is recorded here, not papered over.',
        authorityBoundary:
          'The ladder is sequencing discipline, not capability. Rung receipts prove the scoped rung only; they grant no claim about larger models, network scale, or buyer demand.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.marathon_operations.v1',
        productArea: 'training',
        audience: ['contributor', 'operator'],
        state: 'planned',
        claim:
          'Long training runs operate with marathon discipline: preflight qualification, monitoring against a reference trajectory, loss-spike triage with window rewind, durable checkpoint-sealed windows, standby contributors, written restart criteria, and a schedulable curtailment drill.',
        safeCopy:
          'Psionic’s actual-pretraining lane has local hardware qualification, checkpoint/resume drills, and continue/hold/restart checkpoint decisions. Durable remote checkpoint storage bound into the window seal, standby-Pylon dispatch, public run monitoring against a prior rung’s eval series, restart-decision receipts, and the scheduled curtailment drill are planned. Contract-level pieces landed 2026-06-12: window-seal metadata carrying staleness, churn, and verification-overhead fields (#4849), bootstrap-from-durable-seal grants behind a seal-in-flight join barrier that fails toward queueing (#4850/#4851), and collective failure semantics with ban-for-round, partial-result preservation, and standby-gated abort (psionic#1126). The seal boundary now requires checkpoint-backed window seals to carry a durableCheckpointSeal descriptor that passes the durable checkpoint evaluator, including public-safe remoteCheckpointStoreRef, remoteCheckpointObjectRef, and readbackRehashReceiptRef evidence; bootstrap grants ignore legacy digest-only, local-only, missing-readback-receipt, or failed-durability seal rows. The admin standby preflight route POST /api/training/runs/{trainingRunRef}/standby-dispatch-preflight now evaluates TrainingStandbyDispatch descriptors and fails malformed, stale, unqualified, banned, unbootstrapped, mismatched, or no-vacancy descriptors toward hold_standby. The scheduled-curtailment outcome predicate now exists and fails incomplete unless a drill is scheduled, acknowledged inside the 30s ack SLA, halted inside the 300s load-shed SLA, durably sealed before halt, and resume-verified. GET /api/public/training/marathon-operations now publicly projects these contract surfaces while reporting curtailment predicateAvailable=true, durableCheckpointRemoteReadbackReceiptAvailable=false, liveStandbyPromotionReceiptAvailable=false, curtailmentDrillReceiptAvailable=false, and greenGateSatisfied=false. A real remote checkpoint-store read-back receipt, a receipt-backed live standby promotion, and the drill receipt itself remain unproven.',
        unsafeCopy:
          'Do not claim multi-day or multi-week network training runs are operationally supported, or that training load is proven dispatchable/curtailable for grid value.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4673',
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
          'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
          'route:/api/public/training/marathon-operations',
          'apps/openagents.com/workers/api/src/training-marathon-operations.ts',
          'apps/openagents.com/workers/api/src/training-marathon-operations-routes.ts',
          'apps/openagents.com/workers/api/src/training-marathon-operations.test.ts',
          'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.ts',
          'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.test.ts',
          'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
          'apps/openagents.com/workers/api/src/training-run-window-authority.test.ts',
          'apps/openagents.com/workers/api/src/training-window-bootstrap.ts',
          'apps/openagents.com/workers/api/src/training-window-bootstrap.test.ts',
          'apps/openagents.com/workers/api/src/training-standby-dispatch.ts',
          'apps/openagents.com/workers/api/src/training-standby-dispatch.test.ts',
          'apps/openagents.com/workers/api/src/training-curtailment-drill.ts',
          'apps/openagents.com/workers/api/src/training-curtailment-drill.test.ts',
          'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/4849',
          'https://github.com/OpenAgentsInc/openagents/issues/4850',
          'https://github.com/OpenAgentsInc/openagents/issues/4851',
          'https://github.com/OpenAgentsInc/psionic/issues/1126',
        ],
        blockerRefs: [
          'blocker.product_promises.durable_checkpoint_seal_missing',
          'blocker.product_promises.standby_dispatch_missing',
          'blocker.product_promises.curtailment_drill_missing',
        ],
        verification:
          'Checkpoint-backed window seals now require a matching durableCheckpointSeal descriptor, the descriptor must pass evaluateDurableCheckpointSeal before transitionTrainingWindowRecord can seal, and selectLastDurableSealWindow ignores legacy digest-only, local-only, missing-readback-receipt, or failed-durability rows before issuing bootstrap grants. The descriptor must identify the remote content-addressed store/object and the read-back-and-rehash receipt; retrievalVerified alone is no longer durable evidence. This narrows the durable-seal path but does not clear durable_checkpoint_seal_missing: no real remote content-addressed checkpoint store has produced a read-back-and-rehash receipt yet. POST /api/training/runs/{trainingRunRef}/standby-dispatch-preflight now exposes the standby promotion admissibility predicate behind admin auth and returns promote_standby only for qualified, unbanned, bootstrap-verified, live-window-matched, vacancy-backed, fresh-heartbeat descriptors; it mutates nothing and grants no dispatch, settlement, or promotion authority. The curtailment-drill predicate now returns drill_passed only for a scheduled drill with signal acknowledgement within 30s, sealed halt within 300s, durable checkpoint sealed before halt, and verified resume; malformed or incomplete descriptors fail toward drill_incomplete. GET /api/public/training/marathon-operations is a live-at-read public status projection for these gates and reports curtailmentSurface.predicateAvailable=true, durableCheckpointRemoteReadbackReceiptAvailable=false, liveStandbyPromotionReceiptAvailable=false, curtailmentDrillReceiptAvailable=false, marathonCloseoutReceiptAvailable=false, and greenGateSatisfied=false. standby_dispatch_missing remains because there is no receipt-backed live standby promotion from real heartbeat/vacancy telemetry. curtailment_drill_missing remains because there is no scheduled live drill receipt feeding the predicate. Green requires a window sealed only on durable content-addressed checkpoint storage backed by that real retrieval receipt, a standby contributor promoted into a live run, one restart-or-continue decision recorded as a receipt, and a scheduled drill that sheds part of the fleet on time, resumes from checkpoints, and publishes the receipt — the same drill that becomes evidence for energy.flexible_load_proof.v1.',
        authorityBoundary:
          'Marathon machinery is run-operations authority only; it does not move energy, payout, or settlement promises, and a drill receipt is not a grid-services revenue claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.post_training_arc.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'agent'],
        state: 'planned',
        claim:
          'Psion checkpoints receive a full post-training arc — mid-training, instruct SFT with an owned versioned chat template, preference optimization, and GRPO-class RL with verifier rewards and overlong-completion penalties — with rollout generation and reward grading as paid network work and a vibe-test artifact gating each closeout.',
        safeCopy:
          'Psionic holds bounded post-training lanes: legal SFT/DPO/GRPO CLI smokes, the CS336 A5 alignment reference math with committed tests, a public-safe instruct SFT lane receipt at GET /api/public/training/post-training-arc/instruct-sft-lane, a public-safe DPO preference workload projection at GET /api/public/training/post-training-arc/dpo-preference-workload, and a public-safe vibe-test rubric projection at GET /api/public/training/post-training-arc/vibe-test-rubric. The vibe-test projection exposes rubricAvailable=true, deterministicCloseoutDigestAvailable=true, repoOwnedFixtureTranscriptsAvailable=true, closeoutAcceptable=true, realModelTranscriptArtifactAvailable=false, reviewerSignedCloseoutAvailable=false, vibeTestArtifactAvailable=false, and greenGateSatisfied=false. The DPO projection exposes deterministicReferenceWorkloadAvailable=true for workload.cs336_a5.dpo_preference_pair_reference_grading.v1 with split_a pairCount=25 and outputDigestHex=ad419c324105c46a889bd5cd13a9e94d66fe9166b6763a0a2add0c77c938ac62, while paidPreferenceDispatchAvailable=false, realModelLogprobMeasurementAvailable=false, verifiedChallengeAvailable=false, settlementReceiptAvailable=false, preferenceRolloutWorkAvailable=false, and greenGateSatisfied=false. The instruct-SFT receipt is fixture-scale only: it proves an owned versioned chat template, assistant-token generation mask, repo-owned example corpus, deterministic smoke run, and bit-exact resume drill for lane psion_instruct_sft_v1. The paid-dispatch policy now defines public-safe paid work requests for instruct_sft, preference_rollout_generation, and preference_reward_grading and requires settlement receipts plus a reviewed real-model vibe-test artifact before closeout acceptance. One real paid post-training-style run has occurred on the OpenAgents rails: the 2026-06-11 CS336 A5 alignment run (run.cs336.a5.alignment.demo) dispatched rollout-generation and reward-grading as paid, independently verified network work, with four Verified challenges, a real ~40-sat Bitcoin settlement, and a public eval suite eval.cs336_a5.synthetic_math.bounded_combined.4682.1 served at GET /api/training/evals/a5. That proves rollout/grading-as-paid-work only. No paid OpenAgents SFT dispatch exists yet, and no paid preference/DPO pairwise dispatch, decontaminated post-training eval, overlong-penalty GRPO reward-shaping receipt, DPO update, real model transcript artifact, reviewer-signed closeout, or reviewed vibe-test closeout artifact exists. Hybrid reasoning modes are explicitly deferred.',
        unsafeCopy:
          'Do not claim instruct or reasoning Psion models exist, that post-training runs as paid network work, or that any fine-tuning service is live.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
          'docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md',
          'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
          'route:/api/public/training/post-training-arc/instruct-sft-lane',
          'route:/api/public/training/post-training-arc/dpo-preference-workload',
          'route:/api/public/training/post-training-arc/vibe-test-rubric',
          'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.ts',
          'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.test.ts',
          'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.ts',
          'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.test.ts',
          'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.ts',
          'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.test.ts',
          'apps/openagents.com/workers/api/src/training-post-training-paid-dispatch.ts',
          'apps/openagents.com/workers/api/src/training-post-training-paid-dispatch.test.ts',
          'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts',
          'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts',
          'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.ts',
          'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.test.ts',
          'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
          'apps/openagents.com/docs/2026-06-11-cs336-a5-rollout-grading-paid-evidence.md',
          'route:/api/training/evals/a5',
          'run.cs336.a5.alignment.demo',
          'eval.cs336_a5.synthetic_math.bounded_combined.4682.1',
          'training.verification.challenge.cb1d4f39-5b33-4650-8659-afcc33131af5',
          'https://github.com/OpenAgentsInc/openagents/issues/4682',
          'https://github.com/OpenAgentsInc/psionic/issues/1117',
          'https://github.com/OpenAgentsInc/psionic/pull/1132',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_CS336_A5_REFERENCE_LANE.md',
          'https://github.com/OpenAgentsInc/psionic/blob/main/scripts/check-psion-instruct-sft-lane.sh',
          'https://github.com/OpenAgentsInc/psionic/blob/main/fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json',
          'https://github.com/OpenAgentsInc/psionic/blob/main/crates/psionic-train/src/psion_instruct_sft_lane.rs',
        ],
        blockerRefs: [
          'blocker.product_promises.instruct_sft_paid_dispatch_missing',
          'blocker.product_promises.preference_rollout_work_missing',
          'blocker.product_promises.vibe_test_artifact_missing',
        ],
        verification:
          'The public instruct-SFT receipt clears blocker.product_promises.instruct_sft_lane_missing by proving the bounded lane shape, owned chat template, generation mask, repo-owned example corpus, smoke run, and bit-exact resume drill from deterministic generator output. Psionic PR #1132 clears blocker.product_promises.instruct_sft_fixture_sync_missing by synchronizing the committed report fixture; scripts/check-psion-instruct-sft-lane.sh now verifies the committed fixture and reports sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871. GET /api/public/training/post-training-arc/dpo-preference-workload now exposes the DPO prerequisite receipt: deterministicReferenceWorkloadAvailable=true, pairCount=25, outputDigestHex=ad419c324105c46a889bd5cd13a9e94d66fe9166b6763a0a2add0c77c938ac62, paidPreferenceDispatchAvailable=false, realModelLogprobMeasurementAvailable=false, verifiedChallengeAvailable=false, settlementReceiptAvailable=false, preferenceRolloutWorkAvailable=false, and greenGateSatisfied=false. blocker.product_promises.preference_rollout_work_missing remains because there is still no paid cs336_a5_dpo_grading dispatch, real policy/reference-model log-prob measurement, Verified challenge, settlement, or DPO update. GET /api/public/training/post-training-arc/vibe-test-rubric now exposes the vibe-test prerequisite receipt: rubricAvailable=true, deterministicCloseoutDigestAvailable=true, repoOwnedFixtureTranscriptsAvailable=true, closeoutAcceptable=true, realModelTranscriptArtifactAvailable=false, reviewerSignedCloseoutAvailable=false, vibeTestArtifactAvailable=false, and greenGateSatisfied=false. blocker.product_promises.vibe_test_artifact_missing remains because the closeout uses repo-owned fixture text rather than real Psion model transcripts and has no reviewer signature. The code-backed paid-dispatch policy now builds the exact public-safe work requests for instruct_sft, preference_rollout_generation, and preference_reward_grading under policy.training.post_training_arc.paid_dispatch.v1, requires settlement receipts for each paid request, and blocks evaluatePostTrainingArcCloseout unless a reviewed real-model vibe-test artifact carries artifact.training.post_training_arc.vibe_test_closeout.v1, rubric.training.post_training_arc.vibe_test.v1, a real model transcript artifact ref, and a reviewer signature ref. This is dispatch and gating machinery only; no real paid SFT/preference receipts are minted by the policy. Green still requires one Psion checkpoint carried through mid-training, a dispatched-and-paid OpenAgents SFT lane such as cs336_a5_sft_packing or equivalent, and at least one preference-optimization stage on the owned stack, with rollout/grading work dispatched and verified as paid assignments, decontamination receipts, GRPO reward shaping including the overlong-completion penalty, and a reviewed vibe-test transcript artifact in the closeout. The 2026-06-11 A5 paid run already satisfies the rollout-generation and reward-grading-as-paid-verified-work part (run.cs336.a5.alignment.demo, four Verified challenges, real settlement); the remaining gates are paid SFT dispatch, paid preference/DPO pairwise rollout, decontamination receipts, the overlong-completion penalty in GRPO reward shaping, and a reviewed vibe-test artifact referenced in a closeout.',
        authorityBoundary:
          'Post-training receipts prove the scoped arc on the scoped checkpoint only; they are not a model-quality, buyer-demand, or service-availability claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.verification_classes.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'agent'],
        state: 'green',
        claim:
          'Every training-pipeline stage carries a named pluggable verification class — deterministic_recompute, seeded_replication, freivalds_merkle, statistical_cross_check, exact_trace_replay — routed to the cheapest sufficient supply, with weak devices as paid validators.',
        safeCopy:
          'The Worker-side pluggable class registry is live with three classes exercised on real dispatched production work (exact_trace_replay, deterministic_recompute, freivalds_merkle with commitment-then-challenge matrix flow), and one weak-device validator Pylon has claimed, independently re-executed, and been paid for a Freivalds recheck with a settled public receipt (#4676). As of registry 2026-06-19.7 the exact_trace_replay class is exercised on real dispatched work across five distinct paid independent contributors on the live run run.tassadar.executor.20260615 (five Verified challenges, five realBitcoinMoved:true settlements), broadening the real-work evidence base. The aggregate-only compromise is now re-decided in writing — per-contribution sampling is the default per class (docs/promises/2026-06-20-verification-class-sampling-policy.md, owner-approved 2026-06-20) — and seeded_replication plus statistical_cross_check have not yet run on real dispatched work. As of 2026-06-12 every class also carries a contract-level staleness dimension: per-class steps_behind thresholds whose over-stale outcome is sync_reentry routing with typed events, never bare rejection (#4853); no real contribution has yet been staleness-routed.',
        unsafeCopy:
          'Do not claim training work is currently verified end to end on paid assignments at scale, that all five classes have run on real work, or that validator work is a standing income stream beyond the receipted closeouts.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
          'apps/openagents.com/workers/api/src/tassadar-executor-trace-homework.test.ts',
          'apps/openagents.com/docs/2026-06-11-training-validator-paid-closeout-evidence.md',
          'docs/promises/2026-06-20-verification-class-sampling-policy.md',
          'receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500',
          'verdict.training.freivalds_merkle.verified.training.verification.challenge.8a74a531-8b0d-4392-a49d-ede5179f',
          'promise_transition_0bfce0c5-e4dd-4d19-9221-4bc9504f2055',
          'https://github.com/OpenAgentsInc/openagents/issues/4674',
          'https://github.com/OpenAgentsInc/openagents/issues/4676',
          'https://github.com/OpenAgentsInc/openagents/issues/4853',
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
          'apps/openagents.com/workers/api/src/training-staleness-acceptance.ts',
          'https://github.com/OpenAgentsInc/psionic/issues/1115',
          'https://github.com/OpenAgentsInc/psionic/issues/1116',
        ],
        blockerRefs: [],
        verification:
          'Green (MET, owner-authorized 2026-06-20): the class registry is live with three classes exercised on real dispatched work, commitment-then-challenge for matrix work, a paid weak-device validator closeout (#4676), and the written per-class decision (per-contribution sampling default, aggregate-only deprecated) in docs/promises/2026-06-20-verification-class-sampling-policy.md. Honest bound: not all five classes have run on real dispatched work and validation is not yet at-scale on every paid assignment; this green is the verification-class system plus the decision, not an at-scale end-to-end claim. Flip applied in source ahead of the operator-route transition receipt (2026-06-14 pattern).',
        authorityBoundary:
          'Verification verdicts are acceptance evidence only; they do not settle payouts, upgrade promises, or authorize dispatch, and per-class numeric tolerance contracts do not weaken the Tassadar exact lane’s separate exactness posture.',
      },
      {
        ...basePromiseFields,
        promiseId: 'training.device_capability_dataset.v1',
        productArea: 'training',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        lastVerifiedAt: '2026-06-20',
        claim:
          'Benchmark assignments produce a public device-capability dataset across heterogeneous contributor hardware — matmul throughput, memory bandwidth, attention-kernel performance, sustained-versus-burst thermals — that honestly prices what each machine can earn.',
        safeCopy:
          'The public device-capability dataset is live with its first receipted rows: two paid benchmark assignments ran the bounded CS336 A2 suite on two registered Pylons, the production Worker verified all four metrics with statistical_cross_check on real cross-device agreement scores, both 30-sat closeouts settled over real Lightning with public receipts, and the dataset serves class-level distributions with earning estimates labeled modeled-from-measured (#4681). The dataset now covers two distinct device classes: the original settled, cross-checked Apple-Silicon class (device_class.apple_silicon_macos.arm64), and a genuinely measured second class device_class.x86_64_linux.intel - an x86_64 Linux contributor (Intel Core i7-14700K, 28 cores, Node 25) ran the EXACT bounded CS336 A2 suite (24 reps) and produced deterministic output digests matching the suite commitment byte-for-byte across architectures, with genuinely different measured timings. The Intel rows are labeled measurementProvenance: measured_unsettled (crossCheckState: measured_unverified): they are real measured capability data but NOT paid, NOT statistical_cross_check verified, and carry no earning estimate, so the public projection now reports observedDeviceClassCount=2 alongside observedSettledDeviceClassCount=1. The A2 path now supports continuous sustained-vs-burst thermal evidence from benchmark samples and projects verified thermal row receipt refs plus closed device_capability.public.* reason codes into the dataset/dashboard/funnel path; missing or unverified thermal rows stay explicitly blocked, and no live production thermal receipt is claimed here. It also reports sameClassReplicationStatus, sameClassReplicationSignals, and sameClassReplicationBlockerRefs so same-host-only or single-observation rows cannot look cross-machine replicated. Still missing for green: paid + cross-check-verified coverage on the second class, a real owner-accepted verified sustained-vs-burst thermal row from production, and cross-machine (not same-host) replication.',
        unsafeCopy:
          'Do not claim the second device class is paid, settled, or cross-check verified (it is measured-only, measured_unsettled/measured_unverified), do not claim fleet-scale dataset coverage, do not present the two same-host Apple-Silicon Pylons as cross-machine replication, do not claim benchmark work is a standing paid market beyond the receipted closeouts, and do not quote earning estimates as guarantees rather than modeled-from-measured labels.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
          'route:/api/training/device-capabilities/a2',
          'route:/api/public/pylon-capacity-funnel',
          'apps/openagents.com/docs/2026-06-11-cs336-a2-device-capability-paid-closeout-evidence.md',
          'docs/training/2026-06-20-cs336-a2-second-device-class-x86_64-linux-intel.md',
          'docs/training/2026-06-20-cs336-a2-thermal-throttle-classifier.md',
          'docs/training/2026-06-20-cs336-a2-same-class-replication-status.md',
          'docs/training/2026-06-28-cs336-a2-continuous-thermal-throttle-receipts.md',
          'receipt.nexus_pylon.settlement.assignment_cs336_a2_benchmark_worker_20260611060805',
          'receipt.nexus_pylon.settlement.assignment_cs336_a2_benchmark_validator_20260611060805',
          'verdict.training.statistical_cross_check.verified.training.verification.challenge.c80ba722-0d0e-4e06-9e33-599ad784',
          'commitment.cs336_a2.attention_throughput.sha256_70b508a8a655e0b0',
          'commitment.cs336_a2.memory_bandwidth.sha256_02d2cf92913ee000',
          'commitment.cs336_a2.tokens_per_second.sha256_6b8502b3d1f381d1',
          'commitment.cs336_a2.step_time_ms.sha256_1bde26dbb6c833ce',
          'promise_transition_af246d4d-f37e-456a-b5ee-fba2d5ba3017',
          'https://github.com/OpenAgentsInc/openagents/issues/4681',
          'https://github.com/OpenAgentsInc/openagents/issues/4852',
          'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md',
          'apps/openagents.com/workers/api/src/training-device-admission-gates.ts',
          'apps/openagents.com/workers/api/src/training-device-capability.ts',
          'apps/openagents.com/workers/api/src/training-device-capability.test.ts',
          'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.thermal_throttle_detection_missing',
          'blocker.product_promises.same_host_replication_caveat',
        ],
        verification:
          'Green requires benchmark assignments dispatched and paid across at least two distinct device classes, statistical cross-check verification with replication sampling, continuous thermal-throttle detection feeding funnel reason codes, a real verified thermal-row receipt from a production contributor run, and a public dataset projection with provenance labels. The source path now derives sustained-vs-burst thermal rows from benchmark samples and projects thermalThrottleReceiptRefs plus thermalThrottleFunnelReasonCodes; the blocker remains until a real owner-accepted production receipt is present. The same-class replication status is also projection-only: legacy settled rows fail closed to same_host_only, measured-unsettled rows fail closed to single_observation, and only explicit cross_machine_same_class evidence clears the route-level replication blocker.',
        authorityBoundary:
          'Benchmark receipts price capability; they are not assignment, earning, payment, or settlement guarantees for any device.',
      },
      {
        ...basePromiseFields,
        promiseId: 'proof.demand_provenance.v1',
        productArea: 'public proof',
        audience: ['agent', 'operator', 'public'],
        state: 'green',
        lastVerifiedAt: '2026-06-21',
        claim:
          'Every revenue-bearing public number carries demand provenance — internal versus external dollars — as strictly as modeled versus measured versus settled, under the rule: no external dollar, no demand claim.',
        safeCopy:
          'Demand provenance is live with broad coverage. GET /api/public/demand-provenance summarizes every revenue-bearing public surface with a typed internal/external/unlabeled demand split and reconciliation: AO/kWh (GET /api/public/metrics/accepted-outcomes-per-kwh), pylon-stats (GET /api/public/pylon-stats), the training leaderboards (GET /api/training/leaderboards/*), the training run pages (GET /api/public/training/runs/{trainingRunRef}), and the model-ladder rung economics gates (GET /api/public/training/model-ladder-rungs). It reports coveredRevenueBearingSurfaceCount with no remaining coverage gaps, and every surface keeps externalDemandClaimAllowed:false under the rule no_external_dollar_no_demand_claim because all current demand is internal first-party (e.g. the operator-staged #4777 outcome). Green means the labeling discipline is complete; it does NOT assert any external (real-dollar) market demand.',
        unsafeCopy:
          'Do not present first-party or internally-dispatched demand as market demand, and do not aggregate internal and external revenue into one undifferentiated public number. Green coverage does not mean any external dollar exists; externalDemandClaimAllowed is false.',
        evidenceRefs: [
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'docs/promises/2026-06-09-product-promises-gap-audit.md',
          'route:/api/public/demand-provenance',
          'apps/openagents.com/workers/api/src/demand-provenance.ts',
          'apps/openagents.com/workers/api/src/demand-provenance-routes.ts',
          'apps/openagents.com/workers/api/src/demand-provenance.test.ts',
          'route:/api/public/metrics/accepted-outcomes-per-kwh',
          'apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.ts',
          'apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.test.ts',
          'route:/api/public/pylon-stats',
          'apps/openagents.com/workers/api/src/public-pylon-stats.ts',
          'route:/api/training/leaderboards/*',
          'apps/openagents.com/workers/api/src/training-leaderboards.ts',
          'route:/api/public/training/runs/{trainingRunRef}',
          'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
          'route:/api/public/training/model-ladder-rungs',
          'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
          'promise_transition_ccf5d7d8-5737-4949-b534-19e6fab9c157',
        ],
        blockerRefs: [],
        verification:
          'Green is supported by GET /api/public/demand-provenance and demand-provenance.test.ts: the route is no-store, live-at-read, public-safe, and now summarizes ALL revenue-bearing public surfaces (AO/kWh, pylon-stats, training leaderboards, training run pages, and the model-ladder rung economics gates) with the same typed internal/external/unlabeled split and reconciliation. coveredRevenueBearingSurfaceCount is 5 with remainingSurfaceRefs empty, every surface keeps externalDemandClaimAllowed false (all current demand is internal first-party — plumbing proof, not market proof), and the receipt-first transition was recorded via POST /api/operator/product-promises/transitions. This cleared blocker.product_promises.demand_provenance_broad_projection_coverage_missing. The green is the completeness of the labeling discipline only; any future external-market-demand claim still requires a real external dollar.',
        authorityBoundary:
          'Demand provenance is a labeling discipline; it does not validate any revenue claim by itself and grants no settlement or reporting authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'compute.tassadar_executor_poc.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'green',
        lastVerifiedAt: '2026-06-10',
        claim:
          'OpenAgents will run a Tassadar executor proof of concept on real Pylons: bounded exact-program workloads dispatched to contributor machines, verified by exact trace replay on a separate device, with at least one paid closeout settled on receipts.',
        safeCopy:
          'The proof of concept ran on 2026-06-10: a real registered Pylon executed a digest-pinned exact-program workload dispatched through the operator assignment route, the closeout carried the trace digest byte-identical to the psionic Rust executor fixture, the production worker re-executed the workload as a separate validator device with a Verified exact_trace_replay challenge receipt (and a Rejected receipt on a tampered digest), and one operator-funded paid closeout settled over real Lightning to the Pylon payout target with balance receipts on both sides. Bounded to one workload family and one Pylon; broad executor earning remains gated separately.',
        unsafeCopy:
          'Do not claim transformers-as-computers as a served product, performance parity or superiority over CPUs, general LLM-computer capability, model-training capability from this lane, or that executor work is currently earning contributors bitcoin.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/tassadar-executor-trace-homework.test.ts',
          'docs/tassadar/README.md',
          'docs/tassadar/2026-06-10-psionic-alm-compiler-design-speculation.md',
          'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/TASSADAR_ALM_TRACE_REPLAY.md',
          'https://github.com/OpenAgentsInc/psionic/blob/main/docs/TASSADAR_ALM_WASM_INTERPRETER.md',
          'packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json',
          'apps/openagents.com/workers/api/src/tassadar-replay-validator.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/4691#issuecomment-4671677251',
          'https://github.com/OpenAgentsInc/openagents/issues/4692#issuecomment-4671820793',
          'https://github.com/OpenAgentsInc/openagents/issues/4693#issuecomment-4671952078',
          'artifact.tassadar_poc.trace_digest.f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b',
          'assignment.closeout.7e7ebbf204c7b7688d07af55',
          'training.verification.challenge.81760553-2889-4cf9-95e9-0d100b10e57a',
          'training.verification.challenge.c8c39547-8d44-4a48-be12-6af253d836e3',
          'assignment.closeout.7b794dac86405677243ac182',
          'receipt.tassadar_poc.payer_balance_2173_to_1173',
          'receipt.tassadar_poc.receiver_balance_0_to_980',
          'promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd',
          'docs/launch/2026-06-19-coding-agent-live-verification.md',
        ],
        blockerRefs: [],
        verification:
          'Verified 2026-06-10 (transition receipt promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd). Rerun recipe: smoke:tassadar:executor-trace for the wiring; dispatch via scripts/tassadar-poc-dispatch.ts to a registered Pylon; pylon assignment run-no-spend executes and closes out with the trace digest; POST /api/operator/tassadar/replay re-executes on the worker as the separate validator device; the training-verification challenge lifecycle records Verified/Rejected receipts; the paid rung settles through the MDK bridge pattern with balance receipts.',
        authorityBoundary:
          'A proof-of-concept verdict proves exact replay of bounded committed workloads only. It grants no serving authority, no performance claim against conventional CPUs, and no general LLM-computer capability claim; the Tassadar research lane publication gates stay closed for everything beyond this scoped promise.',
      },
      {
        ...basePromiseFields,
        promiseId: 'compute.agentic_kernel_optimization_at_scale.v1',
        productArea: 'compute',
        audience: ['contributor', 'operator', 'public'],
        state: 'planned',
        claim:
          'Coding agents continuously write and optimize inference kernels across open models and device types (CUDA/Metal/WebGPU), measured by both throughput (tokens/second) and output-parity (an optimized kernel must reproduce identical outputs, verified by exact replay on an independent device), with that work dispatched and paid through the verified-work market as part of the decentralized inference and training mesh.',
        safeCopy:
          'This is the direction, not a shipped network capability. The demonstrated piece is a single historical development result, captured on a public build-series video, not a live dereferenceable market receipt: in March 2026 an agent wrote custom CUDA kernels that took the OpenAgents Rust ML library Psionic to ~523 tok/s versus a leading local inference runtime\u2019s ~328 tok/s on the smallest Qwen 3.5 model, and Psionic beat that runtime on the four smallest Qwen 3.5 models (docs/transcripts/217.md). The correctness anchor \u2014 an optimized kernel must produce identical outputs, proven by exact replay on a separate device \u2014 is the same exact-trace-replay verification proven for bounded workloads under compute.tassadar_executor_poc.v1; the verified-work payment rail is the labor market proven under labor.forum_work_requests.v1 / labor.nostr_negotiation_market.v1. The public kernel-optimization definition is now code-backed in packages/tassadar-executor: buildKernelOptimizationWorkRequest creates a market-dispatchable request with target model+device+hardware, named baseline tok/s, budget, validator device, and dual acceptance criteria; verifyKernelOptimizationParity accepts only when the optimized kernel is distinct, faster, graph-bound to the replayed trace, and output-identical to the baseline on an independent validator; buildKernelOptimizationAcceptedWorkReceipt binds the request, throughput record, parity verdict, and born-verified settlement claim into a public-safe accepted-work receipt shape. This clears the protocol, market-dispatch shape, and throughput/parity receipt-machinery gaps. Continuous, at-scale, across-the-mesh agentic kernel optimization \u2014 many agents optimizing kernels for open models and devices, dispatched and paid through the live market \u2014 has not been run as a network workload or settled.',
        unsafeCopy:
          'Do not claim that agents are continuously optimizing kernels across the mesh today, that this is a live or paying network workload, that the March Psionic/Qwen result is a current benchmark or a dereferenceable on-chain/worker receipt, that Psionic is the fastest inference engine, that throughput-and-parity verification runs at scale, or that this lane is earning contributors bitcoin. The demonstrated result is one historical demo on the four smallest Qwen 3.5 models on a single machine; do not generalize it to other models, larger models, other devices, broad superiority, or paid market liveness.',
        evidenceRefs: [
          'docs/transcripts/217.md',
          'docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md',
          'packages/tassadar-executor/src/kernel-optimization-dispatch.ts',
          'packages/tassadar-executor/src/kernel-optimization-dispatch.test.ts',
          'packages/tassadar-executor/src/kernel-optimization-parity.ts',
          'packages/tassadar-executor/src/kernel-optimization-parity.test.ts',
          'packages/tassadar-executor/src/replay.ts',
          'https://github.com/OpenAgentsInc/psionic',
          'promise:compute.tassadar_executor_poc.v1',
          'promise:training.public_distributed_training_run.v1',
          'promise:labor.forum_work_requests.v1',
          'promise:labor.nostr_negotiation_market.v1',
          'promise:proof.demand_provenance.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing',
          'blocker.product_promises.agentic_kernel_optimization_settlement_receipts_missing',
        ],
        verification:
          'Code-backed machinery exists and is tested in packages/tassadar-executor: buildKernelOptimizationWorkRequest defines the market request; verifyKernelOptimizationParity checks named-baseline tok/s improvement plus independent-device exact-replay output parity; buildKernelOptimizationAcceptedWorkReceipt records the accepted-work receipt shape only when the request, throughput record, parity verdict, and settlement claim all bind. Green still requires a real live market-dispatched optimized kernel, accepted-work and settlement receipts for that live job, and a participant/scale methodology before any at-scale or across-the-mesh claim. The March 2026 Psionic/Qwen 3.5 result is historical-demo evidence (a build-series video, docs/transcripts/217.md) only and is not a live market receipt. Per proof.demand_provenance.v1, internal first-party optimization is plumbing proof, not market proof.',
        authorityBoundary:
          'A demonstrated single-machine throughput result and a stated direction grant no network capability, no paid-work authority, no at-scale claim, no cross-model or cross-device generalization, no performance-superiority claim, and no settlement authority. Throughput improvement without an independent exact-replay output-parity verdict is not an accepted optimized kernel; an accepted kernel is not paid work until the verified-work market records accepted-work and settlement receipts.',
      },
      {
        ...basePromiseFields,
        promiseId: 'artanis.tassadar_evolution_loop.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'green',
        claim:
          'A standing automated Artanis run advances the Tassadar executor lane in production: dispatching digest-pinned executor work to Pylons, verifying it by exact replay, accumulating the verified-trace corpus toward Tassadar model training, and publishing monitorable per-tick receipts on a public surface.',
        safeCopy:
          'The Artanis spine exists and is deployed: a worker cron fires every minute and a config-gated scheduled runner persists loop, tick, runtime, Forum-intent, and health records under the tested autonomous-loop contract. The loop has now run for real: on 2026-06-11 the administrator tick autonomously dispatched no-spend executor-trace work to newly online fleet Pylons (assignment.artanis_admin.20260611011429 dispatched, accepted, executed digest-true, and closed out with zero humans in the span), and the public tick monitor at GET /api/public/artanis/admin-ticks projects every persisted decision with redaction-scanned reasons, counts by state, and the daily dispatch bound. The sustained unattended streak with replay verdicts is now met on the live surface (GET /api/public/artanis/tick-streak reports longestStreak 12, targetReached true, each qualifying tick dereferencing to a verified closeout receipt). The first refs-only Tassadar distillation dataset receipt is projected at GET /api/public/artanis/tassadar-distillation-dataset over accepted exact-replay Artanis closeouts; it exposes only public assignment refs, digest prefixes, and closeout receipt refs.',
        unsafeCopy:
          'Do not claim a trained model executes exactly, that Artanis runs ungated autonomy (spend and publication stay approval-gated), that an autonomous work network is live, or any general LLM-computer or contributor-earning capability from this loop.',
        evidenceRefs: [
          'docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md',
          'docs/artanis/2026-06-06-autonomous-loop-contract.md',
          'apps/openagents.com/workers/api/src/artanis-scheduled-runner.ts',
          'apps/openagents.com/workers/api/src/artanis-tick-monitor.ts',
          'apps/openagents.com/workers/api/src/artanis-tick-streak.ts',
          'apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.ts',
          'apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.test.ts',
          'route:/api/public/artanis/admin-ticks',
          'route:/api/public/artanis/tick-streak',
          'route:/api/public/artanis/tassadar-distillation-dataset',
          'docs/training/2026-06-20-artanis-distillation-dataset-receipt.md',
          'docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4697',
        ],
        blockerRefs: [],
        verification:
          'Executor-trace tick wiring landed under #4697; the public tick-ledger monitor is live at GET /api/public/artanis/admin-ticks and showed the first autonomous dispatch-execute-closeout span on 2026-06-11. The consecutive-streak counter is live and public: GET /api/public/artanis/tick-streak (artanis-tick-streak.ts) joins the tick-decision ledger to the exact-replay closeout-verdict ledger and reports currentStreak / longestStreak / targetReached against a streakTarget of 10, where a tick qualifies only on an accepted exact-replay verdict and each currentStreak assignment is dereferenceable as an artanis_admin_closeout receipt. The unattended-tick-streak gate is now MET on the deployed surface: GET /api/public/artanis/tick-streak reports longestStreak 12 (>= 10), targetReached true, and each qualifying tick dereferences to a real accepted_work_verified closeout receipt (e.g. receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260616123548, outcome=verified accept_state=accepted). The dataset-curation receipt is now projected at GET /api/public/artanis/tassadar-distillation-dataset: it converts accepted exact-replay Artanis closeouts into a refs-only Tassadar distillation dataset manifest once at least 10 accepted closeouts exist, exposing public assignment refs, digest prefixes, and dereferenceable closeout receipt refs only. blocker.product_promises.artanis_unattended_tick_streak_missing and blocker.product_promises.tassadar_distillation_dataset_receipt_missing are therefore cleared. The promise STAYS yellow pending an owner-signed green transition per proof.claim_upgrade_receipts.v1; the dataset receipt is not a raw trace export, training run, eval, settlement, model promotion, or model-capability claim.',
        authorityBoundary:
          'The loop acts only through existing gates: assignments for computation, approval requirements for risky kinds, owner authority for wallet spend, copy gates for publication. A green here proves a monitorable automated run and a refs-only verified-trace dataset receipt, not model capability; Tassadar disclosure boundaries extend unchanged.',
      },
      {
        ...basePromiseFields,
        promiseId: 'artanis.cloud_mind.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'green',
        lastVerifiedAt: '2026-06-10',
        claim:
          'Artanis runs as a cloud-resident AI inside the OpenAgents worker today: it makes model decisions via Gemini inference and publishes Forum updates under its own identity through the Artanis publication queue, with Cloudflare as pure orchestration.',
        safeCopy:
          'The Artanis cloud mind is live in production: an admin-gated worker surface runs Gemini inference served through the Cloudflare AI Gateway (openagents-ai-gateway, BYOK, authenticated; direct Google AI Studio as automatic fallback) and delivers Artanis-authored status posts to the canonical Artanis Forum in-process through the publication queue. Decisions remain proposals: typed schemas validate and approval gates hold.',
        unsafeCopy:
          'Do not claim Artanis spends or administers Bitcoin autonomously, runs ungated autonomy, or administers the full fleet; those remain gated lanes.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/artanis-mind.ts',
          'apps/openagents.com/workers/api/src/artanis-mind.test.ts',
          'docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md',
          'post.public.forum.artanis.status.5',
          'promise_transition_0738d21c-5a59-4101-8c78-23ec41644b28',
        ],
        blockerRefs: [],
        verification:
          'Verified 2026-06-10 (transition receipt promise_transition_0738d21c-5a59-4101-8c78-23ec41644b28). Rerun: POST /api/operator/artanis/mind/smoke with the admin token - a passing run returns the served inference path and decision text, and with forumPost true delivers an Artanis-authored post ref in forum.public.artanis through the publication queue (production evidence: post.public.forum.artanis.status.5, servedVia google_direct).',
        authorityBoundary:
          'The mind proposes; typed schemas validate; approval gates hold. Wallet spend, provider mutation, training launch, and all risky action kinds keep their approval requirements; this promise covers inference plus gated Forum publication only.',
      },
      {
        ...basePromiseFields,
        promiseId: 'payments.reliable_tips_sweepable_balances.v1',
        productArea: 'Forum',
        audience: ['contributor', 'operator'],
        state: 'green',
        lastVerifiedAt: '2026-06-10',
        claim:
          'Tips to agents never fail: direct BOLT 12 payment is attempted when the recipient wallet is reachable, otherwise the recipient is instantly credited to a sweepable per-agent balance, and an automated background sweep pushes balances out to each agent registered Lightning offer with fee caps and indefinite retries - all on one audited pay-in ledger.',
        safeCopy:
          'Live in production: POST /api/forum/posts/{postId}/tips/ladder and POST /api/pylons/{pylonRef}/tips/ladder run the receive ladder on an actor credit ledger in D1 backed 1:1 by the dedicated tips buffer wallet. Tips below the send/receive thresholds or to unreachable recipients credit the recipient instantly (rung recorded, Forum tipStats show the credited split); reachable recipients are paid direct by the buffer in-flow when a private registered destination exists; failed direct attempts refund atomically and fall back to credited; and the every-minute sweep worker pushes balances above each agent threshold to their registered offer with refund-on-fail and indefinite retry.',
        unsafeCopy:
          'Do not claim tips currently never fail, that agent balances exist or are sweepable today, that credited amounts are settled bitcoin before a sweep receipt exists, or any custody posture beyond bounded 1:1-backed balances.',
        evidenceRefs: [
          'docs/payments/reliable-tips.md',
          'docs/2026-06-10-stacker-news-balance-cashin-cashout-audit.md',
          'apps/openagents.com/workers/api/migrations/0160_payments_ledger.sql',
          'apps/openagents.com/workers/api/src/payments-ledger.ts',
          'apps/openagents.com/workers/api/src/tip-ladder.ts',
          'apps/openagents.com/workers/api/src/tips-sweep.ts',
          'docs/artanis/tips-buffer-runbook.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4705',
          'https://github.com/OpenAgentsInc/openagents/issues/4706',
          'https://github.com/OpenAgentsInc/openagents/issues/4707',
          'https://github.com/OpenAgentsInc/openagents/issues/4708',
          'promise_transition_bac0a106-1e80-4dd2-86d5-ca2bedfefecb',
        ],
        blockerRefs: [],
        verification:
          'Verified 2026-06-10 with real sats (transition receipt promise_transition_bac0a106-1e80-4dd2-86d5-ca2bedfefecb): (1) direct BOLT 12 settled in-flow via the buffer (pay_in 1cf2dfad, recipient wallet +200 sats); (2) instant ledger credits with rung recorded (pay_ins 363a809f/18f88496); (3) the automated sweep settled 290 then 220 sats to the registered offer across consecutive cron ticks (recipient wallet 193 -> 888); plus live refund-on-fail (pay_in ef7a179d: failed direct attempt refunded atomically via a linked refund leg, credited fallback paid, tipStats showing the credited split). Rerun: the ladder route plus the pay_ins/pay_in_legs/agent_balances tables.',
        authorityBoundary:
          'Balances are bounded 1:1-backed claims for tip and reward flow only; sweeps pay only registered public-safe destinations; the ledger grants no general custody, settlement, or payout authority, and amounts credited are not called settled bitcoin until a sweep receipt proves it.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.v03_agent_economy.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'green',
        lastVerifiedAt: '2026-06-10',
        claim:
          'Pylon v1.0 is the definitive software on a contributor machine: the node ships a built-in agent surface where the Pylon registers and carries the local user identity, sends and receives Forum tips through the reliable-tips ladder with its wallet as the sweep destination, posts to the Forum (device questions, training-run status) using a local model or the user Gemini key with local memories, and needs no hand-pasted AGENTS.md instructions for any of it.',
        safeCopy:
          'The Pylon agent surface is live from the device: pylon tip/balance/sweep-status on the reliable-tips ladder with the rung rendered honestly, pylon forum post/read/reply and ask-artanis carrying the registered identity, a local inspectable memory store, model adapters (local endpoint or the user own Gemini key), and tip-recipient readiness auto-claimed at wallet report-readiness. Demonstrated live against production, including a real device question answered by Artanis in 71 seconds with a tip landing in public tipStats. Pylon v1.0 install truth remains governed by the release-line records above.',
        unsafeCopy:
          'Do not claim that Pylons converse on the Forum autonomously today, that local memories or model adapters exist before their commits, or that the device agent surface replaces the gates on risky actions.',
        evidenceRefs: [
          'docs/payments/reliable-tips.md',
          'docs/pylon/2026-06-10-v03-sprint-agent-economy.md',
          'pylon-v0.3.0-rc2',
          'https://github.com/OpenAgentsInc/openagents/issues/4711',
          'https://github.com/OpenAgentsInc/openagents/issues/4712',
          'https://github.com/OpenAgentsInc/openagents/issues/4713',
          'promise_transition_89cd31ed-5c7e-4fa9-afb0-ac2b1451dc41',
        ],
        blockerRefs: [],
        verification:
          'Verified 2026-06-10 from a real device (transition receipt promise_transition_89cd31ed-5c7e-4fa9-afb0-ac2b1451dc41): rc2 tagged with the release gate green (316 tests, packaged executor-replay leg); pylon tip sent a live 15-sat ladder tip with the rung rendered honestly; pylon balance/sweep-status read the live ledger; ask-artanis posted real device questions (topics 51bb2c39, 479e4480, f7928738, 7ed389d5) with genuine inventory and local memories recorded - and the last two were answered by Artanis autonomously, one with a 50-sat tip in public tipStats. Rerun: the same commands on any rc2 install.',
        authorityBoundary:
          'The Pylon agent surface acts only with the local user identity and their registered wallet; risky platform actions keep their existing gates; nothing here grants moderation, payout-policy, or registry authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'artanis.pylon_support_responder.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'yellow',
        claim:
          'A Pylon user who posts a device or training question on the Forum gets a substantive reply from the cloud-resident Artanis mind within minutes - Artanis watches new Forum activity on its tick, answers device-capability and training-run questions with live platform data, and tips good contributor posts from its gated ledger budget.',
        safeCopy:
          'The responder loop is live: each cron tick Artanis scans new Forum topics, the mind classifies Pylon device/training questions (typed semantic selection), composes replies grounded only in the asker post and the live promise registry, delivers them under the registered Artanis identity, and tips good questions from a 210-sat/day responder budget on the reliable-tips ladder. Each responder action records the asking actor and a bounded asker-provenance class; scheduled scan/compose ticks now also write public-safe tick receipts. GET /api/public/artanis/responder-support reports externalContributorFlowProven, dereferenceable reply-post refs, tickReadiness.qualifyingUnattendedResponderTickCount, tickReadiness.unattendedResponderTicksProven, tickReadiness.externalContributorAnsweredWithinTickWindow, greenGateMet, and explicit clearedBlockerRefs/unclearedBlockerRefs for the two registry blockers. Demonstrated end to end on operator test articles (replies in as fast as 71 seconds, tip visible in public tipStats), which the projection correctly classifies as owner_operator; the external-contributor proof (a real non-owner contributor answered end to end inside a recorded unattended tick window) and ten qualifying unattended responder tick receipts remain the gates until live rows prove them.',
        unsafeCopy:
          'Do not claim Artanis autonomously answers Forum posts today, promise response times before the loop is measured, or describe Artanis tips as unbounded - the per-tick budget and risky-action gates hold.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/artanis-mind.ts',
          'apps/openagents.com/workers/api/src/artanis-forum-delivery.ts',
          'apps/openagents.com/workers/api/src/artanis-forum-responder.ts',
          'apps/openagents.com/workers/api/src/artanis-responder-provenance.ts',
          'apps/openagents.com/workers/api/src/artanis-responder-ticks.ts',
          'route:/api/public/artanis/responder-support',
          'docs/payments/reliable-tips.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4701',
        ],
        blockerRefs: [
          'blocker.product_promises.external_contributor_flow_unproven',
          'blocker.product_promises.ten_unattended_responder_ticks_unaccrued',
        ],
        verification:
          'The loop is live and demonstrated end to end on 2026-06-10 with operator-authored test articles: scan classified a real Pylon device question within one cron tick, the mind composed grounded full-length replies (registered Artanis identity, in-process forum route), measured response windows as fast as 71 seconds, and a 50-sat budget-gated tip landed in the question post public tipStats. The external-contributor dimension is machine-auditable: each responder action records the asking actor and a bounded asker-provenance class (external_contributor / owner_operator / artanis_self / unknown), and GET /api/public/artanis/responder-support projects per-provenance answered counts, externalContributorFlowProven, the dereferenceable reply-post ref of each external interaction, and explicit clearedBlockerRefs/unclearedBlockerRefs. The same endpoint includes tickReadiness from artanis_responder_ticks: a qualifying unattended responder tick requires scheduled scan and compose to run, at least one candidate scanned and proposed, at least one reply posted, and a dereferenceable reply-post ref inside the tick window; the target remains 10. The operator test-article runs are correctly classified owner_operator, so unclearedBlockerRefs must keep the external-contributor blocker until a real external contributor is answered end to end inside a recorded unattended tick window. Green additionally requires tickReadiness.unattendedResponderTicksProven true on live rows, plus the existing owner-signed receipt-first transition.',
        authorityBoundary:
          'The mind proposes; typed schemas validate; gates hold. Artanis tips spend only from its seeded ledger balance under a per-tick budget; forum publication stays inside the publication queue policy; no moderation or registry authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'pylon.local_claude_agent_bridge.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'green',
        claim:
          'Pylon can talk to your local Claude: the Pylon worker loop hands a coding assignment to the Claude Agent SDK (TypeScript) running on the contributor machine with the user own credentials, drives a bounded read/edit/test session inside a sandboxed working directory with an assignment-derived tool allowlist, and returns public-safe closeout refs - making the user local Claude a real execution lane for delegated coding work.',
        safeCopy:
          'The Pylon-side bridge is built, merged, and live-proven: the SDK ships as a lazy optional dependency, go-online declares capability.pylon.local_claude_agent only when the BYOK readiness probe passes, the bounded executor gate runs a sandboxed Claude Agent SDK session with workspace-escape denial and independent test-command verification, and the CI-safe bounded-task smoke (smoke:claude-agent-task) drives the full worker-loop lifecycle with redaction scanning (issues #4718/#4719/#4720, epic #4717). Live receipts: the #4755 deployed no-spend claude_agent_task ran on an operator-credentialed contributor machine with local-session Claude credentials (closeout assignment.closeout.ae84ca67ada1584130b823d5), and the #4756 production work order carried claude_agent_task plus git_checkout through API submission, own-Pylon placement, local execution, independent bun test verification, and delivered closeout assignment.closeout.2dc83bdc0d8481ebba14621e. What has NOT happened: the bridge run has not yet been re-proven against the published stable Pylon package (Pylon v1.0 shipped, npm latest 1.0.3, superseding the 0.2.5 launcher this lane was last proven against in 0.3.0-rc source), and the local supervised Claude composer/dev surface is not built (tracked as issues #4844-#4847).',
        unsafeCopy:
          'Do not claim Pylon commands Claude today or that any coding assignment has been executed by this lane. Do not call the lane "Claude Code" in product copy; permitted terms are "Claude Agent", "your local Claude", or "Powered by Claude" per Anthropic branding terms. Do not imply OpenAgents supplies Claude access, login, or rate limits - the user brings their own API key or provider configuration. Do not describe local SDK transcripts as shareable artifacts; only public-safe refs leave the device.',
        evidenceRefs: [
          'transition:promise_transition_76ee046e-870f-4fd3-9044-a26846bfc786',
          'apps/pylon/docs/proofs/2026-06-12-claude-agent-packaged-rc-proof.json',
          'https://github.com/OpenAgentsInc/openagents/issues/4859',
          'npm:@openagentsinc/pylon@0.3.0-rc2:shasum:9c2511287536cc437f260c78cdb3f3a85614b858',
          'docs/autopilot-coder/2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md',
          'apps/pylon/src/claude-agent.ts',
          'apps/pylon/src/claude-agent-executor.ts',
          'apps/pylon/src/claude-agent-task-smoke.ts',
          'apps/pylon/docs/claude-agent-bridge.md',
          'apps/pylon/docs/claude-agent-task-smoke.md',
          'apps/openagents.com/workers/api/scripts/claude-agent-task-dispatch.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/4717',
          'https://github.com/OpenAgentsInc/openagents/issues/4755',
          'https://github.com/OpenAgentsInc/openagents/issues/4756',
          'assignment.closeout.ae84ca67ada1584130b823d5',
          'assignment.closeout.2dc83bdc0d8481ebba14621e',
          'docs/autopilot-coder/claude/2026-06-12-pylon-claude-codex-parity-audit.md',
          'docs/launch/2026-06-19-coding-agent-live-verification.md',
        ],
        blockerRefs: [],
        verification:
          'On a contributor machine with the user own Anthropic credentials: a Pylon assignment carrying a coding work class is admitted under capability.pylon.local_claude_agent, executed by the Claude Agent SDK in a bounded workspace with a restricted tool allowlist, verified by a real test command, and closed out through the live assignment API with public-safe artifact/build/test refs. The live-device and API-parity legs ran and are receipt-backed (#4755/#4756). By owner decision 2026-06-12 a published packaged artifact under the rc dist-tag satisfies the packaged-binary requirement (stable 0.3.0 is deliberately untagged). The repeat ran from the registry-installed @openagentsinc/pylon@0.3.0-rc2 (dist.shasum 9c2511287536cc437f260c78cdb3f3a85614b858): real Claude Agent SDK session, bounded workspace, real test-command verification, accepted closeout, retained redaction-clean proof apps/pylon/docs/proofs/2026-06-12-claude-agent-packaged-rc-proof.json. The yellow-to-green flip was recorded receipt-first: transition receipt promise_transition_76ee046e-870f-4fd3-9044-a26846bfc786, recorded after registry 2026-06-12.7 served from worker 5816fdae with all mechanical checks passing.',
        authorityBoundary:
          'The bridge acts only with the local user identity, credentials, and machine; allowed tools and the working directory are bounded per assignment; raw SDK messages, prompts, file contents, and provider payloads stay on the device as operator-local evidence; worker closeout grants no accepted-work, settlement, payout, deploy, spend, or Forum publication authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'labor.forum_work_requests.v1',
        productArea: 'Forum',
        audience: ['contributor', 'operator', 'customer'],
        state: 'green',
        claim:
          'Anyone with a registered identity - the owner, Artanis, any registered agent, or an external Nostr agent - can post a budgeted work request on the OpenAgents Forum with an objective, acceptance criteria, and a deadline, and that request automatically becomes a machine-negotiable NIP-90 job on the owned market relay, with quote, acceptance, delivery, and settlement receipts posted back to the Forum thread.',
        safeCopy:
          'Live: the Forum work-request lifecycle ran end to end on 2026-06-14 against a real backlog issue (#4773), not a fixture. A budgeted work request (workRequest b74bb55c) became a Forum topic (forum_topic 098e36a8) and a NIP-90 kind-5934 job on the owned relay; an independent provider Pylon quoted it (kind-7000), the requester accepted exactly one quote, escrow reserved on the audited credit ledger, the provider delivered output-only (kind-6934 result), the validator re-ran the stated verification command (bun test: 1 pass), escrow released, and the request reached terminal state settled with public reserve/release receipts and lifecycle posts on the thread. The current proven settlement moved 1 sat on the credit ledger; external reliable-tips-ladder payout for labor is the separate provider.compliant_usage_labor.v1 gate.',
        unsafeCopy:
          'Do not describe the bridge key as giving the platform authority over requester funds or identities - the bridge publishes transport events only; budget authority begins only when escrow is reserved on the audited credit ledger. Do not claim labor payouts settle to external wallets yet (the first settlement was credit-ledger only), and do not generalize one settled lifecycle into broad self-serve, high-value, or unattended market liveness.',
        evidenceRefs: [
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'docs/labor/2026-06-14-p5-backlog-faucet-closeout.md',
          'work_request:b74bb55c-849c-43a3-b8d9-9a741316b528',
          'forum_topic.public.098e36a8-ee29-476a-99f4-73d25e5d9e76',
          'receipt.labor_escrow.reserve.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
          'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
          'verdict.public.pylon.labor_market.b74bb55c.bun_test.pass',
          'https://github.com/OpenAgentsInc/openagents/issues/4773',
          'https://github.com/OpenAgentsInc/openagents/issues/4777',
          'docs/labor/2026-06-10-open-agent-labor-market-roadmap.md',
          'apps/nostr-relay/README.md',
          'docs/nips/LBR.md',
          'apps/openagents.com/workers/api/src/forum-work-requests.ts',
          'apps/openagents.com/workers/api/src/forum-work-request-negotiation.ts',
        ],
        blockerRefs: [],
        verification:
          'A registered identity posts a budgeted work request through the Forum API; the matching NIP-90 job event appears on the owned relay with public-safe ref-only tags and a durable topic link; quote, acceptance, delivery, and settlement lifecycle posts appear on the Forum thread as the job progresses; the retained projections pass the redaction scan. The green gate (one full request lifecycle with public receipts) was met by #4777 (bundle docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md). This green flip was applied in source under owner authorization 2026-06-14 ahead of the receipt-first operator transition; the matching promise_transition receipt must be recorded against the deployed 2026-06-14.1 version per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'A work request grants no assignment, payment, payout, deploy, or moderation authority; the relay is event transport only; budgets bind only when escrowed on the audited credit ledger; raw prompts, credentials, and private repository content are rejected at intake.',
      },
      {
        ...basePromiseFields,
        promiseId: 'labor.nostr_negotiation_market.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator', 'customer'],
        state: 'green',
        claim:
          'Agents discover, quote, negotiate, and transact labor jobs over NIP-90 on the owned scoped relay: a provider Pylon quotes a budgeted job it is capability-true for, the requester accepts exactly one quote, the budget is escrowed on the agent credit ledger, the work executes on the contributor own local agent in a bounded sandbox with output-only delivery, acceptance releases escrow, and sats settle to the provider over a public labor receipt.',
        safeCopy:
          'Live: one real negotiated labor job ran end to end over the owned relay on 2026-06-14 (#4777). A kind-5934 job (215ffa0b) was quoted by an independent provider Pylon (pubkey 3fd9b3f1, 1000 msat quote 3d7ec6bb), the requester accepted exactly one quote (acceptance 3cecbc2c), escrow reserved on the audited credit ledger, the work executed on the provider own local agent in a bounded output-only sandbox, the validator re-ran the stated verification command (bun test pass), escrow released, and 1 sat settled requester→provider on the ledger with a public labor receipt. The first settlement is credit-ledger; external reliable-tips-ladder payout is the separate provider.compliant_usage_labor.v1 gate. No provider credentials, sessions, or account access are ever transferred.',
        unsafeCopy:
          'Do not claim this market resells provider access - work runs only on the contributor own agent, own credentials, own machine, with output-only delivery and no provider-auth material anywhere. Do not call ledger-credited amounts external-wallet settlement, and do not generalize one negotiated job into broad, high-value, or unattended relay liveness.',
        evidenceRefs: [
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'nostr.event.215ffa0b005d4640a6f719a8640efd2ab8cafc36b868a6ceef5d03becb18c515',
          'nostr.event.3d7ec6bb9f96fd241f2fd9729f55f087c9e67a4875f25ee16bc36b69a13152cd',
          'nostr.event.3cecbc2c12417ecd63425155bdf8b273216ef9563bd7c2dbe19dbe0765aa5174',
          'result.public.pylon.labor_market.32751b623cbf3e01071182f7bc52b642d944b345404524871ffe8f5c03e905dd',
          'closeout.public.pylon.labor_market.fe1ee748e332a9b9ff7f1e0b',
          'provider.public.pylon.e3a6991ccdf71036048ae540',
          'https://github.com/OpenAgentsInc/openagents/issues/4777',
          'docs/labor/2026-06-10-open-agent-labor-market-roadmap.md',
          'apps/nostr-relay/README.md',
          'apps/pylon/src/provider-nip90.ts',
          'apps/pylon/src/labor.ts',
          'apps/pylon/src/claude-agent-executor.ts',
        ],
        blockerRefs: [],
        verification:
          'One real labor job: posted with a budget, quoted over kind-7000 feedback by an independent contributor Pylon, accepted by the requester, escrow reserved on the ledger, executed on the contributor own local agent through the labor runtime with the stated verification command passing, delivered output-only, accepted, escrow released, sats settled to the provider, and the public labor receipt retrievable - with zero provider-auth material in any artifact, event, receipt, or post. The green gate was met by #4777 (bundle docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md). This green flip was applied in source under owner authorization 2026-06-14 ahead of the receipt-first operator transition; the matching promise_transition receipt must be recorded against the deployed 2026-06-14.1 version per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The relay grants no payment, identity, assignment, or settlement authority; quotes and acceptances are typed events, not authority transfers; the provider never self-accepts; escrow release requires requester acceptance evidence; settlement requires a ladder payout receipt; contributor pricing policy is contributor-configured; no provider credentials, session tokens, or account access are ever transferred, metered for resale, or brokered.',
      },
      {
        ...basePromiseFields,
        promiseId: 'artanis.labor_requester.v1',
        productArea: 'Pylon',
        audience: ['contributor', 'operator'],
        state: 'yellow',
        claim:
          'Artanis can request labor: on its scheduled tick the cloud mind proposes a bounded, budgeted work request (schema-validated, escrowed from its seeded ledger balance under a per-tick labor budget), publishes it through the same Forum-and-relay path any requester uses, and accepts delivered work only when validator re-execution of the stated verification command passes.',
        safeCopy:
          'The Artanis spine this rides is live: the minute tick, the cloud mind, the publication queue, the per-tick budget pattern proven by the tip budget, and the seeded ledger balance. The default-off request_labor action is now wired as an Artanis administrator scheduled action behind its own config gate: enabled ticks validate a bounded proposal, publish through injected work-request dependencies, reserve escrow under the per-tick labor budget and seeded-balance gates, and persist the placed receipt; disabled labor ticks seal as skipped_config_disabled without proposing, publishing, reserving, or counting. Delivered results still route through validator-pass release or validator-fail refund. Every gated labor tick is sealed into a content-addressed, public-safe receipt, persisted tamper-evidently, and served on the public feed GET /api/public/artanis/labor-receipts (each row independently content-address-verifiable). A green-readiness surface GET /api/public/artanis/labor-green-readiness folds that feed onto the two green-flip blockers: it counts PLACED unattended request receipts (requested_pending_delivery / accepted_released / rejected_refunded - states only an operator-ENABLED labor tick can reach) and reports liveEnablementProven, unattendedRequestReceiptsProven (target 10), greenGateMet, and explicit clearedBlockerRefs/unclearedBlockerRefs. The promise remains yellow until live placed receipts accrue and owner sign-off records the transition.',
        unsafeCopy:
          'Do not claim Artanis hires agents today or describe its acceptance as judgment - acceptance is validator re-execution of a stated verification command, nothing else. Do not describe Artanis labor spend as unbounded; the per-tick budget and the seeded-balance ceiling bind, and risky-action gates hold. Do not present the readiness surface or a placed receipt as the green flip itself; the flip additionally requires the owner-signed promise_transition.',
        evidenceRefs: [
          'docs/labor/2026-06-10-open-agent-labor-market-roadmap.md',
          'docs/promises/2026-06-23-de8-artanis-labor-requester-green-readiness.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4701',
          'https://github.com/OpenAgentsInc/openagents/issues/5531',
          'apps/openagents.com/workers/api/src/artanis-labor-requester.ts',
          'apps/openagents.com/workers/api/src/artanis-labor-tick-driver.ts',
          'apps/openagents.com/workers/api/src/artanis-administrator-tick.ts',
          'apps/openagents.com/workers/api/src/artanis-administrator-labor-tick.test.ts',
          'apps/openagents.com/workers/api/src/artanis-labor-receipt-store.ts',
          'apps/openagents.com/workers/api/src/artanis-labor-receipt-routes.ts',
          'apps/openagents.com/workers/api/src/artanis-labor-green-readiness.ts',
          'apps/openagents.com/workers/api/src/artanis-labor-green-readiness.test.ts',
          'route:/api/public/artanis/labor-receipts',
          'route:/api/public/artanis/labor-green-readiness',
        ],
        blockerRefs: [
          'blocker.product_promises.artanis_labor_live_enablement_missing',
          'blocker.product_promises.artanis_labor_unattended_request_receipts_missing',
        ],
        verification:
          'Run the artanis-administrator-labor-tick, artanis-labor-green-readiness, and artanis-labor-receipt-routes tests: an enabled Artanis scheduled request_labor action must propose and publish exactly one bounded work request, reserve escrow, and persist a requested_pending_delivery receipt; a config-disabled labor tick must seal as skipped_config_disabled without proposing/publishing/reserving and never count; the readiness surface must report greenGateMet only when at least 10 placed receipts accrue; and every served receipt must content-address its own public fields. A provider completes the request; the validator re-runs the stated verification command and acceptance follows only from a passing verdict; escrow releases and settlement receipts cite the tick ledger. Green requires that flow unattended with public-safe receipts: the owner operator-enables Artanis labor and lets at least 10 unattended ticks accrue placed receipts so GET /api/public/artanis/labor-green-readiness reports greenGateMet:true, then records the owner-signed yellow->green promise_transition via POST /api/operator/product-promises/transitions citing the live readiness surface. The mechanical receipt-evidence and gates-met requirements are not waived; only the per-flip owner sign-off is the remaining owner step.',
        authorityBoundary:
          'The mind proposes; typed schemas validate; gates hold. Artanis labor spend comes only from its seeded ledger balance under a per-tick labor budget; acceptance authority is the validator verdict, not mind discretion; no moderation, payout-policy, or registry authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.desktop_gui_client.v1',
        productArea: 'Autopilot',
        audience: ['operator', 'agent', 'user'],
        state: 'yellow',
        claim:
          'Autopilot Desktop is a GUI client for observing and steering local Autopilot coding sessions.',
        safeCopy:
          'Autopilot Desktop (Bun/Electrobun shell with a Foldkit webview reusing the shared Autopilot UI) connects to a local Pylon node over loopback and renders a live session list, decision cards, and an event timeline. The auto-onboarding EPIC (#5441, AO-1..AO-6) is built and tested: on first run a fresh node self-registers, the AO-3 identity-choice screen offers create-new (named) or detected use-existing (seed marker detected by presence only, never read or overwritten), the wizard projects identity/registered/node-online/wallet/payout/presence/Tassadar/earning from real observed signals, a black-screen Document-contract guard is in place, and an AO-6 headless smoke drives the REAL local Pylon node through the launcher against a mock Worker and asserts the whole chain converges (no GUI, no terminal, no env vars). The PDF-production, loopback Sites-preview, and FS/MCP asset-ingestion + ambient-auth browser-automation cores are built as runtime-agnostic src/bun cores with injected runtimes and fake-based tests (epic #4973 and #4993/#4994/#4995 closed 2026-06-14). The owner-run AO6 from-DMG proof is now recorded in source: notarized DMG 20260619T010148 with SHA-256 22db620c12c97f819fd6045eebd86cceb51c7cffc1ef2fc0d5b3f8446dd46358, Gatekeeper accepted as Notarized Developer ID, clean-Mac rendered window screenshots, production Pylon presence pylon.fa4e9049a4329f3d56e2, Verified exact_trace_replay challenge training.verification.challenge.9fd49062-f82c-46ee-a2a0-242d36dd126e, and settled real-Bitcoin receipt receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1. This clears the stale from-DMG-evidence wording only. It is still a yellow, local-scoped, owner-review/green-pending claim. This is NOT a green/default-on production claim: the live PDF renderer, Bun.serve loopback bind, FS-MCP + browser clients, Electrobun RPC exposure, cloud-lane sessions (autopilot.cloud_coding_sessions.v1), remote/Tailnet control, full TUI parity, and pricing/distribution are not wired or decided. The local coding-agent execution lanes the desktop client steers are independently live-proven on 2026-06-19 (docs/launch/2026-06-19-coding-agent-live-verification.md), but that is local single-task exec evidence only and does not satisfy the live PDF/preview/ingest/browser runtime wiring or owner-reviewed green transition this promise still needs; it stays yellow.',
        unsafeCopy:
          'Do not claim the from-DMG clean-Mac evidence makes Autopilot Desktop green, default-on, generally available, fully priced/distributed, or production-ready. Do not claim Autopilot Desktop administers cloud or remote multi-node sessions, runs the local PDF/preview/ingest/browser runtimes end to end, has full TUI parity, or is a priced/distributed product; the cores are built behind clean seams with fakes, not yet wired to live runtimes, and the Bun host holds the control token while the webview only sees public-safe projections.',
        evidenceRefs: [
          'apps/autopilot-desktop/README.md',
          'apps/autopilot-desktop/AGENTS.md',
          'apps/autopilot-desktop/src/bun/agent-onboarding.ts',
          'apps/autopilot-desktop/src/bun/identity-choice.ts',
          'apps/autopilot-desktop/src/shared/onboarding-status.ts',
          'apps/autopilot-desktop/tests/agent-onboarding.test.ts',
          'apps/autopilot-desktop/tests/onboarding-wizard.test.ts',
          'apps/autopilot-desktop/tests/onboarding-status.test.ts',
          'apps/autopilot-desktop/scripts/auto-onboarding-e2e-smoke.ts',
          'docs/launch/2026-06-18-autopilot-desktop-availability-audit.md',
          'docs/launch/2026-06-18-autopilot-desktop-ao6-from-dmg-runbook.md',
          'docs/autopilot-coder/2026-06-13-autopilot-desktop-app-audit.md',
          'docs/autopilot-coder/2026-06-13-autopilot-desktop-reality-vs-claim-status.md',
          'docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4973',
          'https://github.com/OpenAgentsInc/openagents/issues/4993',
          'https://github.com/OpenAgentsInc/openagents/issues/4994',
          'https://github.com/OpenAgentsInc/openagents/issues/4995',
          'https://github.com/OpenAgentsInc/openagents/issues/5441',
          'docs/launch/2026-06-19-coding-agent-live-verification.md',
          'docs/launch/JUNE19_ROADMAP.md',
          'docs/launch/artifacts/ao6-20260619T010148/dmg-sha256.txt',
          'docs/launch/artifacts/ao6-20260619T010148/gatekeeper-dmg.txt',
          'docs/launch/artifacts/ao6-20260619T010148/gatekeeper-installed-app.txt',
          'docs/launch/artifacts/ao6-20260619T010148/initial-window.png',
          'docs/launch/artifacts/ao6-20260619T010148/presence-state.public.json',
          'docs/launch/artifacts/ao6-20260619T010148/pylon-detail-summary.json',
          'docs/launch/artifacts/ao6-20260619T010148/replay-verdict-summary.json',
          'docs/launch/artifacts/ao6-20260619T010148/settlement-receipt-summary.json',
          'docs/launch/artifacts/ao6-20260619T010148/live-refs.txt',
        ],
        blockerRefs: [
          'blocker.product_promises.autopilot_desktop_owner_review_green_pending',
          'blocker.product_promises.autopilot_desktop_live_runtimes_not_wired',
          'blocker.product_promises.autopilot_desktop_remote_cloud_lane_not_wired',
          'blocker.product_promises.autopilot_desktop_pricing_distribution_undecided',
        ],
        verification:
          'Launch the desktop shell, pair with a local Pylon, spawn a session, and confirm the Foldkit UI renders session list, decision cards, and timeline live from public-safe projections while the Bun host holds the control token. Auto-onboarding (#5441) is proven by the AO-6 headless smoke (bun run --cwd apps/autopilot-desktop scripts/auto-onboarding-e2e-smoke.ts) driving the real local node through self-register -> presence -> Spark payout target -> Tassadar poll plus the AO-3 identity-choice and black-screen guards. The owner-run AO6 from-DMG proof on a clean Mac is recorded at docs/launch/artifacts/ao6-20260619T010148: notarized DMG hash, Gatekeeper-accepted installed app, rendered window screenshots, production Pylon pylon.fa4e9049a4329f3d56e2, Verified exact_trace_replay challenge training.verification.challenge.9fd49062-f82c-46ee-a2a0-242d36dd126e, and settled receipt receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1. This is yellow evidence only. Green still requires owner review/sign-off, PDF/preview/ingest/browser live runtimes wired and observed, cloud-lane sessions, and a decided distribution/pricing path.',
        authorityBoundary:
          'Desktop is a view-and-bounded-action client; it cannot supervise the Pylon node, reach remote/cloud nodes, deploy, or mutate repository/provider access.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.cloud_coding_sessions.v1',
        productArea: 'Autopilot',
        audience: ['operator', 'agent', 'user'],
        state: 'red',
        claim:
          'Owners can run coding sessions on OpenAgents Cloud (Google GCE first, SHC second) and administer them from desktop and the Expo mobile app — spawn, watch, approve, accept — so work continues without the owner at their computer.',
        safeCopy:
          'The Coder Cloud initiative (epic #4996, "code on the go") is the active top-priority build, and the full Phase 1 contract layer has now landed: the lane selector auto|local|cloud-gcp|cloud-shc is wired end to end (#4998), the Vortex-independent Codex grant-resolution endpoint contract is in place (#4999), the Pylon openagents-cloud provider dispatches cloud lanes to the cloud placement endpoint and maps events to Pylon SessionEvents (#4997, gated by OA_CLOUD_CONTROL_URL/OA_CLOUD_CONTROL_TOKEN; it falls back to local execution if unset), and the cloud repo shipped POST /v1/placement plus a per-session GCE VM lease lifecycle (cloud #86/#87/#88, #90). The first-party cloud coding-session surface on the openagents.com Worker is still flag-gated (CLOUD_CODING_SESSIONS_ENABLED, default off -> POST /v1/cloud-coding-sessions launch + GET /v1/cloud-coding-sessions/:id lifecycle read return 404 on prod), but when enabled its default launch path now targets the real cloud placement/GCE lease endpoint and fails closed with typed not-armed errors unless OA_CODEX_GCE_PROVISIONER=live and OA_CLOUD_CONTROL_URL/OA_CLOUD_CONTROL_TOKEN are configured. The old fake-success default is gone; tests keep the inert stub injectable only as an explicit test adapter. The Pylon cloud client now has an executable full-kind round-trip test for openagents.codex_workroom_event.v1, including the cloud.gce.* lease lifecycle and resource_usage_receipt alias, and the desktop bridge test proves cloud rows keep the same timeline row shape as local composer events. It still stays red: a real desktop-originated cloud session must run a repo edit on Google GCE and produce a content-addressed artifact plus a dereferenceable usage receipt with owner sign-off before this can turn green.',
        unsafeCopy:
          'Do not claim cloud coding sessions are live or that the owner can already code from a phone via the cloud; the Phase 1 contracts have landed and fake-success provisioning is removed, but the demonstrable desktop->GCE loop still needs a real receipt-backed run and owner sign-off.',
        evidenceRefs: [
          'docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md',
          'docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md',
          'docs/autopilot-coder/2026-06-19-cloud-coding-session-surface-inert-scaffold.md',
          'apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts',
          'apps/pylon/docs/proofs/m10-live-2026-06-14/README.md',
          'packages/autopilot-control-protocol/src/control.ts',
          'apps/pylon/src/cloud-control-client.ts',
          'apps/pylon/tests/openagents-cloud-execution-backend.test.ts',
          'packages/autopilot-control-protocol/src/bridge-subscribe-client.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/4996',
          'https://github.com/OpenAgentsInc/openagents/issues/4997',
          'https://github.com/OpenAgentsInc/openagents/issues/4998',
          'https://github.com/OpenAgentsInc/openagents/issues/4999',
          'https://github.com/OpenAgentsInc/openagents/issues/5000',
          'https://github.com/OpenAgentsInc/openagents/issues/5005',
          'https://github.com/OpenAgentsInc/openagents/issues/6830',
          'https://github.com/OpenAgentsInc/openagents/issues/6831',
        ],
        blockerRefs: [
          'blocker.product_promises.cloud_desktop_gce_receipt_owner_signoff_missing',
          'blocker.product_promises.pylon_remote_bridge_transport_missing',
        ],
        verification:
          'Phase 1 exit proof: with CLOUD_CODING_SESSIONS_ENABLED armed, OA_CODEX_GCE_PROVISIONER=live, and OA_CLOUD_CONTROL_URL/OA_CLOUD_CONTROL_TOKEN configured, a desktop-originated session.spawn{lane:"cloud-gcp"} leases a real Google GCE ephemeral VM through the cloud placement endpoint, runs a real repo-edit Codex session, streams openagents.codex_workroom_event.v1 into the desktop timeline lane-transparently, and produces a content-addressed artifact plus an openagents.resource_usage_receipt.v1. The route must fail closed with cloud_gce_provisioning_not_armed when the live GCE flag is off. The lane selector (#4998), grant endpoint (#4999), Pylon cloud dispatch (#4997), cloud placement + GCE lease (cloud #86/#87/#88/#90), first-party fail-closed Worker launch path (#6830), and codex_workroom_event.v1 full-kind round-trip tests (#6831) have landed; green still requires the real receipt-backed desktop->GCE run and owner sign-off. Remote phone administration is the mobile.fleet_companion.v1 path (the Expo mobile.autopilot_remote_control.v1 record is withdrawn).',
        authorityBoundary:
          'Cloud sessions run under owner-resolved Codex grants on ephemeral VMs; placement honors repo trust tiers (regulated->SHC-only, private->own/verified, public->any). This promise grants no multi-tenant, settlement, or non-owner authority — that is deferred Phase 4 (credits gateway, tenant caps, settlement, microVM isolation).',
      },
      {
        ...basePromiseFields,
        promiseId: 'mobile.autopilot_remote_control.v1',
        productArea: 'mobile and Autopilot',
        audience: ['operator', 'agent', 'user'],
        state: 'withdrawn',
        claim:
          'Withdrawn: the Expo mobile remote-control app was retired on 2026-06-26 before shipping.',
        safeCopy:
          'The Expo app clients/khala-ios/AutopilotRemoteControl was retired and removed from the repository on 2026-06-26 (docs/mobile/2026-06-26-autopilot-remote-control-retirement.md); the standing mobile build/ship policy is native SwiftUI with no Expo/EAS cloud. The successor mobile claim is mobile.fleet_companion.v1 (planned): a native, E2EE-paired, relay-transported, allowlisted observe/notify/approve/steer companion that never hosts work. The remote-decision-queue protocol module remains real shared code and carries over as evidence on the successor record.',
        unsafeCopy:
          'Do not describe the Expo remote-control app as current roadmap, downloadable, or in development, and do not cite this record for any live mobile capability; route new mobile companion copy to mobile.fleet_companion.v1.',
        evidenceRefs: [
          'docs/mobile/2026-06-26-autopilot-remote-control-retirement.md',
          'docs/autopilot-coder/2026-06-13-autopilot-remote-control-mobile-app-audit.md',
          'packages/autopilot-control-protocol/src/remote-decision-queue.ts',
          'promise:mobile.fleet_companion.v1',
        ],
        blockerRefs: ['blocker.product_promises.expo_mobile_app_retired'],
        verification:
          'Compatibility check: public consumers should treat this id as withdrawn and route new mobile companion references to mobile.fleet_companion.v1.',
        authorityBoundary:
          'A withdrawn record grants no client, pairing, approval, steering, or distribution claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'workrooms.omni_client_delivery_workrooms.v1',
        productArea: 'workrooms',
        audience: ['user', 'customer', 'operator'],
        state: 'yellow',
        claim:
          'Omni client-delivery workrooms route chat, tasks, and artifacts into client-scoped delivery contexts.',
        safeCopy:
          "The client-delivery workroom surface shipped in the wave-3 Agency Pack (epic #4973, closed 2026-06-14): a typed workroom record (status, visibility, trust tier, data classification), CRUD/lifecycle/bundle/handoff routes, kind templates, client-scoped views, and the client-delivery workroom page live-wired into the logged-in loop (#4977), with a credits/cost-preview panel embedded. The source-authority model is integrated into the live surface: GET /api/omni/workrooms/:id/source-authority reads a workroom record's metadata.sourceAuthority bindings/writes/config and returns approval-gated delivery evidence; approved, source-backed writes with owner sign-off and closeout receipts report effectsApplied=true. Treat this as a scoped client-delivery workspace business-object projection, not broad operational CRM/legal/finance truth.",
        unsafeCopy:
          'Do not claim generated summaries mutate CRM, documents, or send communications without source refs and human approval, or that the workroom is source-authorized business truth; the live surface is a client-scoped delivery workspace, not an approval-gated business-object system.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/omni-workroom-routes.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-routes.test.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-lifecycle-routes.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-kind-templates.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-surface-projections.ts',
          'apps/openagents.com/workers/api/src/omni-source-authorized-business-objects.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-business-object-delivery.ts',
          'apps/openagents.com/workers/api/src/omni-workroom-business-object-delivery.test.ts',
          'GET /api/omni/workrooms/:id/source-authority',
          'https://github.com/OpenAgentsInc/openagents/issues/4973',
          'https://github.com/OpenAgentsInc/openagents/issues/4977',
          'https://github.com/OpenAgentsInc/openagents/issues/5532',
          'docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md',
        ],
        blockerRefs: [
          'blocker.product_promises.owner_accepted_green_receipt_missing',
        ],
        verification:
          'The client-delivery workroom page is wired into the logged-in loop with CRUD/lifecycle/bundle/handoff routes and client-scoped views passing type checks and tests. The source-authority model is reachable on the live surface via GET /api/omni/workrooms/:id/source-authority, and approved source-backed writes with owner sign-off plus closeout receipts report effectsApplied=true. Green still requires an owner-accepted live receipt and liability/copy gates for the exact source-authorized claim.',
        authorityBoundary:
          'Workroom data structure grants no customer write authority, source mutation, CRM sync, notification send, or third-party integration without separate approval gates.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot_sites.native_email_sequences.v1',
        productArea: 'Sites',
        audience: ['customer', 'operator'],
        state: 'yellow',
        claim:
          'Autopilot Sites can define native email sequences and enroll subscribers in multi-step campaigns.',
        safeCopy:
          'Email campaign/sequence authoring, native lists/subscribers, page-kinds, form-capture, and the native-list→sequence enrollment bridge shipped in wave-3 (#4983/#4984): campaigns, steps, enrollments, and sends are stored in D1 and operators can create campaigns, move lifecycle state, and enroll subscribers. The public form-capture route is now MOUNTED in the worker omni chain behind the default-OFF SITE_FORM_CAPTURE_ENABLED flag, resolving each page FormCaptureSpec from the active site version metadata via the site form-spec registry and persisting leads through the native-lists sink. A presentational customer-facing email-sequence UI (email-sequence-panel.ts, exported from apps/web ui) renders a sequence, its ordered steps, and the viewer enrollment status. Authored sequence sends now have a real Cloudflare Email send-service path behind the default-OFF EMAIL_SEQUENCE_SEND_ENABLED flag: when armed with the Worker EMAIL binding and an authenticated sender address, the scheduled dispatcher renders a transactional sequence email, calls Cloudflare Email, and records email ledger/delivery receipts; when disabled, it records the dry-run/skipped path and never calls the sender. Remaining for green: live deliverability proof (send→deliver evidence) with bounce/complaint handling and customer self-serve authoring.',
        unsafeCopy:
          'Do not claim customers can self-author email campaigns or that live email delivery/deliverability is proven; the send-service is default-OFF and only sends when an operator arms Cloudflare Email with an authenticated sender, and the form-capture route, while mounted, is gated OFF by default.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/email-sequence-authoring-routes.ts',
          'apps/openagents.com/workers/api/src/email-sequence-authoring.ts',
          'apps/openagents.com/workers/api/src/email-sequence-send-service.ts',
          'apps/openagents.com/workers/api/src/email-sequence-send-service.test.ts',
          'apps/openagents.com/docs/sites/2026-06-28-native-email-sequence-cloudflare-send-service.md',
          'apps/openagents.com/workers/api/wrangler.jsonc',
          'apps/openagents.com/workers/api/src/site-page-form-capture-routes.ts',
          'apps/openagents.com/workers/api/src/site-page-form-routes.ts',
          'apps/openagents.com/workers/api/src/site-form-spec-registry.ts',
          'apps/openagents.com/workers/api/src/site-form-spec-registry.test.ts',
          'apps/openagents.com/apps/web/src/ui/email-sequence-panel.ts',
          'apps/openagents.com/apps/web/src/ui/email-sequence-panel.test.ts',
          'docs/launch/2026-06-19-autopilot-yellow-green-readiness-receipt.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4983',
          'https://github.com/OpenAgentsInc/openagents/issues/4984',
        ],
        blockerRefs: ['blocker.product_promises.email_deliverability_unproven'],
        verification:
          'Operator authoring/enrollment routes pass type checks; the site form-spec registry resolves typed FormCaptureSpecs from site metadata and degrades safely on malformed input (site-form-spec-registry.test.ts, 7 pass); the form-capture route is mounted in index.ts behind SITE_FORM_CAPTURE_ENABLED; the customer email-sequence UI renders sequence/steps/enrollment and enable/disable enroll states (email-sequence-panel.test.ts, 12 pass); the send-service seam plans a dry-run when disabled (never calling the sender), delegates to an injected sender only when armed, and has a Cloudflare Email sender that records ledger/delivery receipts through the Worker EMAIL binding (email-sequence-send-service.test.ts and email-campaign-dispatcher.test.ts). Green requires a live deliverability smoke (send→deliver evidence) with bounce/complaint handling and customer self-serve authoring.',
        authorityBoundary:
          'Email campaign creation is not subscriber consent, list ownership, or delivery authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot_sites.custom_tenant_hostnames.v1',
        productArea: 'Sites',
        audience: ['customer', 'operator'],
        state: 'yellow',
        claim:
          'Autopilot Sites customers can serve their sites under custom branded hostnames.',
        safeCopy:
          'Tenant custom-hostname registration, DNS-token verification, hostname→tenant mapping, request-time resolution, and a live Cloudflare custom-hostname client shipped in wave-3 (#4988/#4989). A CUSTOMER self-serve path is now mounted and live: a signed-in team owner/admin can claim a custom hostname for their own team and any team member can list their team’s claimed hostnames with the exact DNS TXT record to publish (GET/POST /api/tenant/hostnames, browser-session + team-role gated). The self-serve path is INERT by design: a claimed hostname is stored `pending` and never resolves or serves; it touches no live DNS, no SSL issuance, no origin binding, and no spend. Request-time rendering now resolves an already-active custom hostname to the mapped tenant team’s public active Site runtime, so live serving is blocked on owner-gated provisioning to `active`, not on the rendering context switch. Driving a hostname to `active` stays the owner-gated Cloudflare provisioning core’s job, which is itself default-OFF until CLOUDFLARE_API_TOKEN and CLOUDFLARE_ZONE_ID are set. Automated SSL issuance is still not fully wired into a mounted owner-gated route.',
        unsafeCopy:
          'Do not claim that claiming a hostname makes a site live, that DNS/SSL/branding switching works end to end, or that a claimed hostname serves anything; self-serve claiming writes a pending row only, and live provisioning (Cloudflare for SaaS) stays owner-gated and default-OFF.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/tenant-custom-hostnames.ts',
          'apps/openagents.com/workers/api/src/tenant-custom-hostname-self-serve.ts',
          'apps/openagents.com/workers/api/src/tenant-custom-hostname-self-serve.test.ts',
          'apps/openagents.com/workers/api/src/tenant-custom-hostname-self-serve-routes.ts',
          'apps/openagents.com/workers/api/src/tenant-custom-hostname-self-serve-routes.test.ts',
          'apps/openagents.com/workers/api/src/site-runtime-routes.ts',
          'apps/openagents.com/workers/api/src/site-runtime-routes.test.ts',
          'apps/openagents.com/workers/api/src/cloudflare-custom-hostname-client.ts',
          'route:/api/tenant/hostnames',
          'https://github.com/OpenAgentsInc/openagents/issues/4988',
          'https://github.com/OpenAgentsInc/openagents/issues/4989',
        ],
        blockerRefs: [
          'blocker.product_promises.hostname_ssl_issuance_not_wired',
        ],
        verification:
          'The customer self-serve claim/list path is mounted and gated (browser session + active team membership; only owner/admin may claim) and is covered by unit tests over the core and the routes; it writes only pending tenant_custom_hostnames rows and reports servingLive=false while provisioning is unarmed. Operator registration/verification still works and passes type checks. Site runtime route tests cover active custom-host request-time rendering: a custom hostname resolves to its tenant team and serves that team’s active public Site without a slug prefix while first-party hosts are not intercepted. Green still requires automated DNS verification + SSL provisioning (live Cloudflare for SaaS, owner-gated, default-OFF today) to advance claimed hostnames to active serving.',
        authorityBoundary:
          'Hostname registration is not DNS authority, SSL certificate authority, or site-content publication authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot_sites.partner_payout_ledger.v1',
        productArea: 'payments',
        audience: ['partner', 'operator'],
        state: 'red',
        claim:
          'Autopilot Sites/Agency partners can earn Bitcoin payouts when their referred customers become paying OpenAgents customers.',
        safeCopy:
          'An operator-gated partner-payout ledger, state-transition routes, explicit-agreement-only attribution policy, admin partner-agreement seed/read route, Stripe credit-purchase feed, public-safe count-only partner-payout projection API, settled-receipt readback API, and readiness-gated partner dispatch coordinator shipped in wave-3/#5524/#7021 follow-up work: operators can seed active partner agreements with POST /api/operator/partners/agreements, read them back with GET /api/operator/partners/agreements?customerUserId=..., move payout rows through approve/dispatch/settle/reverse states, POST /api/operator/partners/payout-ledger/{payoutRef}/dispatch can drive a sats-denominated row through the injected adapter before recording settled, GET /api/public/partner-payouts exposes aggregate current states, and GET /api/public/partner-payout-receipts/{receiptRef} can dereference a settled `receipt.partner_payout.*` evidence ref without partner refs, user ids, payout refs, qualifying event refs, payout destinations, invoices, preimages, or provider payloads. The #7021 closed-loop staging proof uses a default-off no-money adapter to prove dispatch -> settled D1 row -> public receipt dereference before the owner arms the live rail. USD/credit rows refuse before adapter call, and default production wiring remains owner-armed/off. Remaining work is a real settled partner payout and owner sign-off before any live earning/withdrawal claim.',
        unsafeCopy:
          'Do not claim partners are earning, can withdraw payouts, or have an earnings dashboard, and do not describe this as a live partner revenue stream; the ledger, explicit-agreement attribution policy, admin agreement route, and Stripe credit-purchase feed exist, but no partner payout has settled and partner settlement remains owner-gated.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/partner-payout-ledger-routes.ts',
          'apps/openagents.com/workers/api/src/partner-payout-ledger.ts',
          'apps/openagents.com/workers/api/src/partner-payout-dispatch.ts',
          'apps/openagents.com/workers/api/src/partner-attribution-policy.ts',
          'apps/openagents.com/workers/api/src/partner-attribution-eligibility.ts',
          'apps/openagents.com/workers/api/src/partner-payout-feed.ts',
          'apps/openagents.com/workers/api/src/partner-agreement-routes.ts',
          'apps/openagents.com/workers/api/src/partner-agreement-routes.test.ts',
          'apps/openagents.com/workers/api/src/partner-payout-stripe-wire.test.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-dispatch.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-feed.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-wire.test.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-public-routes.ts',
          'route:/api/public/site-referral-payouts',
          'https://openagents.com/api/public/site-referral-payouts',
          'apps/openagents.com/workers/api/src/partner-payout-public-projection.ts',
          'apps/openagents.com/workers/api/src/partner-payout-public-routes.ts',
          'apps/openagents.com/workers/api/src/partner-payout-public-projection.test.ts',
          'apps/openagents.com/workers/api/src/partner-payout-receipts.ts',
          'apps/openagents.com/workers/api/src/public-partner-payout-receipt-routes.ts',
          'apps/openagents.com/workers/api/src/public-partner-payout-receipt-routes.test.ts',
          'apps/openagents.com/workers/api/src/partner-payout-staging-adapter.ts',
          'apps/openagents.com/workers/api/src/partner-payout-receipt-loop.test.ts',
          'route:/api/operator/partners/agreements',
          'route:/api/operator/partners/payout-ledger/{payoutRef}/dispatch',
          'route:/api/public/partner-payouts',
          'route:/api/public/partner-payout-receipts/{receiptRef}',
          'apps/openagents.com/docs/2026-06-20-partner-attribution-policy-contract.md',
          'docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4986',
          'https://github.com/OpenAgentsInc/openagents/issues/5458',
          'https://github.com/OpenAgentsInc/openagents/issues/5524',
          'https://github.com/OpenAgentsInc/openagents/issues/5849',
          'https://github.com/OpenAgentsInc/openagents/issues/5850',
          'https://github.com/OpenAgentsInc/openagents/issues/5851',
          'https://github.com/OpenAgentsInc/openagents/issues/7021',
          'promise:sites.referral_bitcoin_stream.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.partner_first_real_payout_pending',
        ],
        verification:
          'Operator routes and the ledger module pass type/unit tests, the explicit partner-attribution policy + eligibility mapper + partner payout feed + partner agreement route are covered by partner-attribution-policy.test.ts, partner-attribution-eligibility.test.ts, partner-payout-feed.test.ts, partner-agreement-routes.test.ts, and partner-payout-stripe-wire.test.ts, the referral payout feed + readiness-gated dispatch pass site-referral-payout-wire.test.ts against a mock adapter (RL-1 #5458), and the partner payout public projection + receipt + dispatch routes pass public-safe no-leak and mock-settlement tests. The #7021 partner payout closed-loop proof adds a default-off staging adapter that moves no money and proves dispatch -> settled partner_payout_ledger_entries row -> makeD1PartnerPayoutReceiptStore dereference with idempotent re-drive and fail-closed disabled mode. This clears the projection API, partner-attribution-policy, source-level settlement-dispatch blockers, and staging receipt-loop proof gaps only. Green still requires a real dereferenceable public settlement receipt for an actually settled partner payout and owner sign-off.',
        authorityBoundary:
          'Ledger state and public aggregate projections are not spendable value; settlement requires separate dispatch authority and public-safe settlement evidence.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.cloud_credits_ui.v1',
        productArea: 'payments',
        audience: ['customer', 'user'],
        state: 'yellow',
        claim:
          'Autopilot users can see their cloud credit balance and a cost preview before and during a session.',
        safeCopy:
          'A Foldkit credits panel renders credit balance, status, rate labels, a minimum-run threshold, and a cost preview (blocked / under-cap / over-cap / exact-cap), embedded in the workroom page (#4985). The spend backend is now wired at the ledger layer: a fulfilled Stripe checkout credit can be explicitly bridged into USD-origin inference-spendable msat, metered inference debits that balance from real provider usage, and the public card-credit-spend receipt resolver proves card -> credit -> bridge -> metered inference from stored rows. The UI remains yellow because production purchase evidence and owner-signed paid-loop proof are still required before broad customer copy.',
        unsafeCopy:
          'Do not claim customers can broadly purchase cloud credits in production, that the UI itself grants spend authority, or that the cost preview is a final price guarantee. Do not flip this promise green without a dereferenceable real or owner-approved staging-to-prod paid receipt and owner-signed claim upgrade.',
        evidenceRefs: [
          'apps/openagents.com/apps/web/src/ui/credits-panel.ts',
          'apps/openagents.com/workers/api/src/billing-routes.ts#handleBillingInferenceCreditApi',
          'apps/openagents.com/workers/api/src/inference/usd-credit-bridge.ts',
          'apps/openagents.com/workers/api/src/inference/metering-hook.ts',
          'apps/openagents.com/workers/api/src/inference/card-credit-spend-receipt-store.ts',
          'route:/api/public/inference/card-credit-spend-receipts/{receiptRef}',
          'docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md',
          'https://github.com/OpenAgentsInc/openagents/issues/4985',
          'https://github.com/OpenAgentsInc/openagents/issues/6842',
        ],
        blockerRefs: [
          'blocker.product_promises.cloud_credits_purchase_not_wired',
          'blocker.product_promises.cloud_credits_real_paid_receipt_missing',
          'blocker.product_promises.cloud_credits_cost_preview_live_ui_binding_missing',
        ],
        verification:
          'The component renders correctly from supplied inputs, and the backend spend path is covered by D1-backed tests: Stripe checkout credit row -> explicit inference-credit bridge -> real metering debit -> public card-credit-spend receipt resolver. Green still requires a production or owner-approved staging-to-prod purchase receipt, the live UI bound to the metered-spend preview, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The credits UI is view-only; spend authority and credit-ledger mutation live in the billing backend and inference metering hook, while public receipt reads grant no checkout, spend, refund, payout, settlement, provider, or registry authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'mobile.voice_session_evidence_transcript_ingest.v1',
        productArea: 'mobile and voice',
        audience: ['user', 'operator'],
        state: 'red',
        claim:
          'Spoken commands and intent can be ingested into Autopilot workrooms as transcribed, approval-gated action proposals.',
        safeCopy:
          'Voice-session evidence contracts, read-only projections, and a voice-transcript→program ingest core shipped in wave-3 (#4992): voice-session metadata, transcript segments, and command proposals with approval-required and risk labels, projected with mutation disabled. Going further is a product decision plus wiring: pick an STT vendor and capture path. The flag-gated INERT ingestion endpoint (POST /api/mobile/voice-sessions/ingest, default off) is wired to the ingest core (#5542, clearing the endpoint blocker), and when armed it now returns a machine-checkable approvalGate (operator_required, needs_approval, Medium risk, no approval mutation, no command execution) alongside the program-input proposal. Remaining is an STT vendor + capture path, AI proposal generation, and the approval UI. Foundation infrastructure for mobile.voice_approval_companion.v1.',
        unsafeCopy:
          'Do not claim users can speak commands that execute, or that voice transcripts are trusted for mutations (CRM, email send, code, deploy, spend) without server-side approval; the ingest core exists but no STT vendor or live capture path is chosen.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/omni-voice-session-evidence.ts',
          'apps/openagents.com/workers/api/src/omni-voice-session-evidence.test.ts',
          'apps/openagents.com/workers/api/src/voice-program-ingest-routes.ts',
          'route:/api/mobile/voice-sessions/ingest',
          'https://github.com/OpenAgentsInc/openagents/issues/4992',
          'https://github.com/OpenAgentsInc/openagents/pull/5542',
        ],
        blockerRefs: [
          'blocker.product_promises.voice_transcription_service_missing',
          'blocker.product_promises.voice_proposal_and_approval_ui_missing',
        ],
        verification:
          'Voice evidence contracts, projection logic, the flag-gated ingestion endpoint, and the explicit approvalGate response are covered by tests. Green requires a transcription service, live capture path, proposal generation, and approval UI, with every proposed action gated server-side.',
        authorityBoundary:
          'A voice transcript is evidence of user intent, not command authority; all proposed actions require server-side policy checks and explicit approval.',
      },
      {
        ...basePromiseFields,
        promiseId: 'agents.nostr_fallback_coordination.v1',
        productArea: 'agent-readable surfaces',
        audience: ['agent', 'operator', 'public'],
        state: 'green',
        claim:
          'If OpenAgents HTTP infrastructure falls down, agents keep retrying it and coordinate over the Nostr protocol in the meantime, so they never go idle waiting on a human or a single server.',
        safeCopy:
          'AGENTS.md now instructs agents, on any OpenAgents infrastructure falldown, to keep retrying with backoff/idempotency AND fall back to Nostr to communicate with their owner and other agents until OpenAgents recovers (then reconcile on OpenAgents as authority of record). The rails this leans on are partially live: the owned relay (wss://relay.openagents.com and the scoped market relay) is up, the agent labor market already negotiates and settled its first job over NIP-90 on it (#4777), and Pylon v1.0 provisions Nostr credentials. The end-to-end outage-coordination drill is now demonstrated (PR #5535): NIP-38 liveness -> NIP-65/02 discovery -> NIP-17 encrypted DM -> NIP-90 job lifecycle -> recovery, with 11 fetchable public-safe event ids on a public relay and zero secret leakage (docs/nostr/2026-06-20-outage-coordination-drill.md, runnable via apps/openagents.com/scripts/nostr-fallback-drill.ts).',
        unsafeCopy:
          'Do not claim Nostr coordination replaces OpenAgents authority during normal operation, that a Nostr message is proof of accepted work/payment/settlement, or that one drill on a public relay equals surviving a real production outage at scale. Never put secrets, raw invoices, preimages, mnemonics, wallet keys, provider credentials, or private repo contents in any Nostr event.',
        evidenceRefs: [
          'https://openagents.com/AGENTS.md',
          'apps/openagents.com/docs/live/AGENTS.md',
          'apps/nostr-relay/README.md',
          'docs/nips/LBR.md',
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'docs/transcripts/235.md',
          'promise:labor.nostr_negotiation_market.v1',
          'apps/openagents.com/scripts/nostr-fallback-drill.ts',
          'docs/nostr/2026-06-20-outage-coordination-drill.md',
        ],
        blockerRefs: [],
        verification:
          'AGENTS.md carries the firm falldown instruction (keep retrying OpenAgents; coordinate over Nostr meanwhile; reconcile on recovery) and the owned relay plus NIP-90 negotiation are live (first labor job settled over the relay, #4777). Green (MET via PR #5535, owner-authorized flip 2026-06-19): the demonstrated drill runs the full outage sequence — publish status (NIP-38), discover peers (NIP-02/65), exchange private coordination (NIP-17), keep a NIP-90 job moving, and reconcile on recovery — with public-safe evidence (11 fetchable event ids in docs/nostr/2026-06-20-outage-coordination-drill.md) and zero secret leakage. A drill on a public relay is not a claim of surviving a real production outage at scale; the matching promise_transition exception receipt is recorded against the deployed registry via the operator route, dereferenceable at /api/public/product-promises/transitions.',
        authorityBoundary:
          'Nostr is a communication and coordination substrate and an outage fallback, not OpenAgents authority. Identity, posting authority, payment, and settlement remain OpenAgents systems; Nostr coordination during an outage is intent and messaging only, reconciled to OpenAgents receipts on recovery.',
      },
      {
        ...basePromiseFields,
        promiseId: 'metrics.accepted_outcomes_per_kwh.v1',
        productArea: 'metrics',
        audience: ['operator', 'public', 'contributor'],
        state: 'yellow',
        claim:
          'OpenAgents defines and will measure Accepted Outcomes Per Kilowatt-Hour (AO/kWh) — verified, accepted outcomes produced per kilowatt-hour of energy — as the primary efficiency metric for converting electricity into accepted agent work.',
        safeCopy:
          'AO/kWh is now instrumented as a yellow, caveated metric: /api/public/metrics/accepted-outcomes-per-kwh publishes the frozen definition plus one receipt-backed modeled seed datapoint from the first settled labor job (#4777). The seed is explicitly modeled, not measured: it uses acceptance-to-result wall-clock timing and a documented 100 W provider-power assumption. The source now includes a typed measured per-device telemetry ingestion contract that turns real Wh measurements for accepted-outcome windows into measured AO/kWh datapoints, but the live projection still has 0 of 2 required measured datapoints. The projection also carries a typed demand-provenance split (proof.demand_provenance.v1): the seed outcome is labeled internal (operator-staged, credit-ledger), externalDemandClaimAllowed stays false, and the copy gate forbids presenting it as external market demand. Describe the live endpoint only as a modeled, internal-demand seed datapoint until measured telemetry datapoints are published.',
        unsafeCopy:
          'Do not describe the AO/kWh seed as measured, broadly representative, a provider ranking, a production routing policy, investment advice, grid advice, or proof that live energy dispatch is running. Do not cite any figure without the modeled/measurement evidence label and caveats. Do not present AO/kWh as green or measured until at least two real telemetry datapoints are published with evidence-state labels and transition receipts.',
        evidenceRefs: [
          'docs/transcripts/232.md',
          'docs/transcripts/237.md',
          'docs/metrics/2026-06-15-accepted-outcomes-per-kwh.md',
          'https://openagents.com/api/public/metrics/accepted-outcomes-per-kwh',
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'https://openagents.com/api/forum/work-requests/b74bb55c-849c-43a3-b8d9-9a741316b528',
          'promise:payments.accepted_outcome_economics.v1',
          'promise:energy.flexible_load_proof.v1',
          'promise:proof.demand_provenance.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.ao_kwh_measured_datapoints_missing',
          'blocker.product_promises.ao_kwh_requires_two_measured_datapoints',
        ],
        verification:
          'GET /api/public/metrics/accepted-outcomes-per-kwh plus accepted-outcomes-per-kwh.test.ts. Yellow is satisfied when the response decodes, carries schemaVersion openagents.metrics.accepted_outcomes_per_kwh.v1, includes one receipt-backed accepted outcome, labels the seed energyEvidenceState as modeled, exposes the measured telemetry gate, and keeps measuredFigurePublicationAllowed false while fewer than two measured datapoints are present. The source ingestion contract accepts real per-device Wh telemetry and computes measured AO/kWh datapoints with energyEvidenceState measured. Green requires at least two measured AO/kWh datapoints from real per-device telemetry, measuredTelemetryGateSatisfied true, and owner-signed transition receipts.',
        authorityBoundary:
          'A defined metric is not a measured result. AO/kWh figures are operational estimates, not investment, grid, utility, or financial advice.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.builtin_compute_agent.v1',
        productArea: 'autopilot',
        audience: ['user', 'public'],
        state: 'yellow',
        claim:
          'Autopilot Desktop ships a built-in, out-of-the-box agent so a user with no agent and no API key can install one desktop installer, go online, and have a working agent. It runs on OpenAgents-provided compute — the device’s own compute and/or OpenAgents’ managed cloud model set (e.g. a hosted Gemini set offered free to some users) — so no user-supplied key is required.',
        safeCopy:
          'Source support is now wired in Autopilot Desktop: the first-screen Go online action and Agent pane call a Bun-owned built-in-agent RPC, check local Pylon + OpenAgents hosted compute readiness, create a managed scratch workspace, enforce a local daily-start cap, and start a bounded cloud-gcp/cloud-shc session with no user-supplied provider key. The source-level metering-smoke projection is also built and tested: it models installer_signed -> pylon_readiness_checked -> hosted_compute_readiness_checked -> grant_issued -> session_bounded -> usage_recorded -> public_quota_projection, imports the live grant cap constants, redaction-scans outputs, and distinguishes ci_no_live_sessions from live_from_signed_install. This is yellow, not green/default-on production: the AO6 from-DMG proof was a desktop/Tassadar earning proof, not a from-install built-in-compute Go online session; the already-published rc.2 installer does not contain this source change, and green still needs a signed/notarized recut with packaged OpenAgents compute credentials plus public evidence of a metered from-install Go online session doing useful work. The free hosted tier is bounded/metered, not unlimited. The underlying local coding-agent execution lanes are independently live-proven on 2026-06-19 (docs/launch/2026-06-19-coding-agent-live-verification.md), so this is a green-candidate, but it stays yellow pending owner review and the signed-recut/live-smoke/metering receipts.',
        unsafeCopy:
          'Do not claim the already-published rc.2 installer includes this built-in agent, do not claim green/no-setup/default-on production availability before a signed recut and live smoke, and do not describe hosted compute as free/unmetered or authority to spend/settle on the user behalf.',
        evidenceRefs: [
          'docs/transcripts/237.md',
          'docs/launch/JUNE15_LAUNCH_PLAN.md',
          'promise:autopilot.desktop_gui_client.v1',
          'apps/autopilot-desktop/src/shared/builtin-agent.ts',
          'apps/autopilot-desktop/src/bun/index.ts',
          'apps/autopilot-desktop/src/ui/view.ts',
          'apps/autopilot-desktop/tests/builtin-agent.test.ts',
          'apps/autopilot-desktop/tests/cl-53-foldkit.test.ts',
          'apps/openagents.com/workers/api/src/builtin-compute-agent-metering-smoke.ts',
          'apps/openagents.com/workers/api/src/builtin-compute-agent-metering-smoke.test.ts',
          'docs/launch/2026-06-19-coding-agent-live-verification.md',
          'docs/launch/vertex-fleet/autopilot.builtin_compute_agent.v1.md',
          'docs/launch/JUNE19_ROADMAP.md',
        ],
        blockerRefs: [
          'blocker.product_promises.builtin_compute_agent_signed_recut_missing',
          'blocker.product_promises.builtin_compute_agent_live_from_install_smoke_missing',
          'blocker.product_promises.openagents_compute_metering_live_smoke_missing',
          'blocker.product_promises.builtin_compute_agent_owner_review_green_pending',
        ],
        verification:
          'Yellow source evidence is the built-in-agent desktop source plus builtin-compute-agent-metering-smoke.ts/.test.ts, which validates the seven-step metered smoke projection in ci_no_live_sessions mode and fail-closes missing live refs. The AO6 DMG evidence is relevant installer proof for the desktop GUI only; it is not the built-in-compute live smoke. Green requires owner review/sign-off, a signed/notarized Autopilot Desktop build containing the built-in-agent source, packaged OpenAgents compute credentials/entitlement, a metered/bounded compute path, and public evidence of a from-install Go online session doing useful work with no user API key.',
        authorityBoundary:
          'A built-in agent is a capability on bounded OpenAgents compute, not unlimited free inference or authority to spend or settle on the user behalf.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.local_apple_fm_tool_chat.v1',
        productArea: 'autopilot',
        audience: ['user', 'agent', 'operator'],
        state: 'yellow',
        claim:
          'An Apple Silicon user can run a basic Autopilot chat and tool loop entirely locally through Apple Foundation Models, without OpenAgents hosted compute or user-supplied cloud model keys.',
        safeCopy:
          'Source support and admitted-Mac smoke evidence now exist for the basic local Apple FM mode: on supported macOS Apple Silicon machines, Autopilot Desktop can ask Pylon to start a bounded local Apple FM chat/tool session after live readiness passes, using read-only workspace tools and public-safe desktop event summaries without sending prompts to OpenAgents hosted compute. Khala Desktop now also has source-level Electrobun Apple FM sidecar packaging/readiness: it can discover or launch the packaged Foundation Models helper, bounded-restart that helper after a crash, read Pylon apple_fm.status over loopback, and emit public-safe readiness without leaking tokens, callback URLs, prompts, or local paths. This is yellow, not green: the current public installer still needs a signed recut with from-install helper supervision smoke, and the claim is limited to basic local chat/tool use on admitted Apple hardware.',
        unsafeCopy:
          'Do not claim this works in the current installer, do not claim every Apple device is supported, do not claim offline internet-free coding beyond the local tool scope, do not claim Codex parity, and do not imply Bitcoin earning, paid compute resale, cloud fallback, or settlement authority from this local mode.',
        evidenceRefs: [
          'docs/apple-fm/2026-06-15-current-apple-fm-electrobun-desktop-audit.md',
          'docs/apple-fm/2026-06-15-apple-fm-integration-audit.md',
          'apps/pylon/packages/runtime/src/backends/apple-fm/contract.ts',
          'apps/pylon/packages/runtime/src/backends/apple-fm/client.ts',
          'apps/pylon/packages/runtime/src/backends/apple-fm/tools.ts',
          'apps/pylon/packages/runtime/src/fleet/backend-capability.ts',
          'apps/pylon/swift/foundation-bridge/Package.swift',
          'apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift',
          'apps/pylon/swift/foundation-bridge/README.md',
          'docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-runbook.md',
          'docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-smoke-evidence.md',
          'apps/pylon/src/node/apple-fm-bridge-helper.ts',
          'apps/pylon/src/node/apple-fm-local-session.ts',
          'apps/pylon/src/node/apple-fm-status.ts',
          'apps/pylon/src/node/control-server.ts',
          'apps/pylon/tests/apple-fm-bridge-helper.test.ts',
          'apps/pylon/tests/apple-fm-control-session.test.ts',
          'apps/pylon/tests/control-protocol.test.ts',
          'apps/autopilot-desktop/src/bun/index.ts',
          'apps/autopilot-desktop/src/bun/pylon-control.ts',
          'apps/autopilot-desktop/src/bun/node-launcher.ts',
          'apps/autopilot-desktop/src/shared/install-readiness.ts',
          'apps/autopilot-desktop/src/shared/rpc.ts',
          'packages/autopilot-control-protocol/src/control.ts',
          'apps/autopilot-desktop/src/ui/model.ts',
          'apps/autopilot-desktop/src/ui/message.ts',
          'apps/autopilot-desktop/src/ui/commands.ts',
          'apps/autopilot-desktop/src/ui/update.ts',
          'apps/autopilot-desktop/src/ui/view.ts',
          'apps/autopilot-desktop/tests/control-verbs.test.ts',
          'apps/autopilot-desktop/tests/install-readiness.test.ts',
          'apps/autopilot-desktop/tests/apple-fm-loopback-integration.test.ts',
          'apps/autopilot-desktop/tests/cl-53-foldkit.test.ts',
          'apps/autopilot-desktop/tests/cl-53-sanitize.test.ts',
          'docs/apple-fm/2026-06-29-electrobun-apple-fm-swift-sidecar-plan.md',
          'docs/launch/vertex-fleet/autopilot.local_apple_fm_tool_chat.v1.md',
          'clients/khala-code-desktop/electrobun.config.ts',
          'clients/khala-code-desktop/scripts/prepare-apple-fm-bridge.sh',
          'clients/khala-code-desktop/scripts/verify-packaged-apple-fm-bridge.ts',
          'clients/khala-code-desktop/src/bun/apple-fm-sidecar.ts',
          'clients/khala-code-desktop/src/shared/apple-fm-packaging.ts',
          'clients/khala-code-desktop/src/shared/apple-fm-readiness.ts',
          'clients/khala-code-desktop/tests/apple-fm-packaging.test.ts',
          'clients/khala-code-desktop/tests/apple-fm-readiness.test.ts',
          'clients/khala-code-desktop/tests/apple-fm-sidecar.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5068',
          'https://github.com/OpenAgentsInc/openagents/issues/5069',
          'https://github.com/OpenAgentsInc/openagents/issues/5070',
          'https://github.com/OpenAgentsInc/openagents/issues/5071',
          'https://github.com/OpenAgentsInc/openagents/issues/5072',
          'https://github.com/OpenAgentsInc/openagents/issues/5073',
        ],
        blockerRefs: [
          'blocker.product_promises.local_apple_fm_signed_installer_recut_missing',
          'blocker.product_promises.local_apple_fm_signed_from_install_supervised_smoke_missing',
        ],
        verification:
          'Yellow is satisfied by focused fake-bridge tests plus an admitted-Mac source smoke showing bridge health ready, desktop/Pylon Apple FM readiness ready, one local read_file chat/tool session, local lane, read-only sandbox, network disabled, no cloud runner, no resource_usage_receipt, disabled handling, and redaction of prompts, file contents, callback tokens, callback URLs, bearer material, and local paths. Khala Desktop sidecar evidence is source-level packaging/readiness only: tests verify packaged-helper discovery, launch, bounded crash restart, stopped-restart cancellation, Pylon status sanitation, unsupported-host handling, and redaction of loopback URLs, callback data, prompts, secrets, and local helper paths. Pylon source evidence now covers helper supervision policy, status projection, driver, launcher, and opt-in host lifecycle ownership, but green still requires a signed/notarized installer recut that bundles or supervises the helper and repeats the same supervised from-install smoke on admitted Apple Silicon.',
        authorityBoundary:
          'This promise is a local user-owned model path only. It grants no cloud compute, paid assignment, payout, settlement, provider-account, deploy, spend, or public-claim authority, and local tools remain bounded by explicit workspace/tool policy.',
      },
      {
        ...basePromiseFields,
        promiseId: 'payments.offline_receive_spark_fallback.v1',
        productArea: 'payments',
        audience: ['contributor', 'operator', 'public'],
        state: 'green',
        claim:
          'When a node’s primary MDK wallet is offline or cannot mint a receive request, the node can still receive a tip or payout through a narrow, opt-in, receive-only Spark fallback (for example a Spark-backed Lightning Address), then claim and see the credited backup balance on the next Spark sync. Sweeping that backup balance into the primary MDK wallet remains a separate consented consolidation step.',
        safeCopy:
          'Live for the scoped offline-receive claim. MDK remains the primary rail, but a Pylon on rc.12 can publish a Spark-backed Lightning Address as a backup RECEIVE target, the treasury can pay that address through normal LNURL-pay -> BOLT11 -> MDK send, and the recipient can run the read-only Spark backup commands to claim and see the credited sats while the primary MDK wallet was not accepting inbound. Proven on real infrastructure and then recipient-confirmed: a 50,000-sat recognition payout to the Spark Lightning Address became visible in the recipient backup balance after `backup-claim` / `backup-status`. Describe this as live backup receive resilience, not as unified spendable wallet balance.',
        unsafeCopy:
          'Do not imply Spark regains send/payout/accepted-work-settlement authority, do not call the Spark backup balance a unified MDK spendable balance, and do not mark backup funds swept into MDK until a consented sweep/reconcile path records that later receipt. No raw historical Spark credential material is reused.',
        evidenceRefs: [
          'apps/pylon/docs/2026-06-15-spark-backup-receive-fallback-audit.md',
          'apps/pylon/docs/2026-06-15-spark-backup-receive-runbook.md',
          'apps/pylon/docs/legacy-spark-wallet-migration.md',
          'apps/pylon/src/wallet.ts',
          'apps/pylon/src/spark-backup-helper.ts',
          'apps/pylon/tests/spark-backup-helper.test.ts',
          'docs/launch/JUNE16_ROADMAP.md',
          'docs/launch/JUNE17_ROADMAP.md',
          'docs/payments/2026-06-17-spark-mdk-balance-consolidation-options.md',
          'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-734d9003-e177-457e-8e33-757deda644ae',
          'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-abee1453-5eb9-406a-84e3-be43b3bc377f',
          'https://github.com/OpenAgentsInc/openagents/issues/5078',
          'https://github.com/OpenAgentsInc/openagents/issues/5176',
          'https://github.com/OpenAgentsInc/openagents/issues/5185',
          'promise:payments.money_dev_kit.v1',
          'promise:payments.reliable_tips_sweepable_balances.v1',
        ],
        blockerRefs: [],
        verification:
          'Green is satisfied by the shipped opt-in receive-only core, Breez SDK Spark adapter, Bun `bun:sqlite` storage, embedded-key out-of-box address readiness, LNURL-pay treasury payout fallback, rc.12 `backup-claim`, and a real recipient-visible proof: treasury sent 50,000 sats to a Spark-backed Lightning Address and the recipient’s rc.12 read-only backup-status reported the credited 50,000-sat balance after claim. Strengthened 2026-06-17: a SECOND independent recipient (Whitefang) recipient-confirmed a 50,000-sat recognition payout (backup-status detectedBalanceSats 51,030). The claim remains scoped to receive resilience. Separately, the consented owner-approved `wallet send --rail spark --confirm-send` path referenced below as a distinct gate is now shipped and verified end-to-end on rc.16 (real 100-sat and 878-sat Spark sends returned state=sent/status=completed with redacted receipts and debited the balance); it stays user-consented and grants no automatic payout-target or accepted-work-settlement authority. A dedicated public promise for consented Spark spend/withdraw is an owner product-claim decision and is not asserted by this receive-only record. The wallet-unification epic #5176 (incl. Spark as the primary agent balance) is closed.',
        authorityBoundary:
          'The Spark fallback is RECEIVE-ONLY. It grants no send, payout, accepted-work settlement, or public payout-target authority; activating any of those requires a separate explicit gate.',
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.gateway_credits_business.v1',
        productArea: 'inference gateway',
        audience: ['agent', 'developer', 'customer', 'operator', 'public'],
        state: 'red',
        claim:
          'OpenAgents offers one OpenAI/Anthropic-compatible inference API backed by a usage-based credit balance you can fund with a card or Bitcoin, routed to the cheapest viable supply OpenAgents controls.',
        safeCopy:
          'The inference gateway request surface is BUILT, DEPLOYED, and LIVE in prod (EPIC #5474, INFERENCE_GATEWAY_ENABLED=true): the OpenAI-compatible POST /v1/chat/completions endpoint authenticates by key, gates on balance, routes cheapest-viable, dispatches to adapters, and decrements credits receipt-first from the real provider usage object via the pricing engine. Gemini 3.5 Flash is served end-to-end through that endpoint, verified live 2026-06-19 (docs/inference/2026-06-19-gateway-gemini-live-verification.md). FREE inference works. The paid-credit bridge is also built in source: a Stripe/card checkout can credit the USD billing ledger, POST /api/billing/inference-credit explicitly bridges selected USD credit into USD-origin msat for inference spend, and the public card-credit-spend receipt resolver can prove card -> credit -> bridge -> metered inference. Current source routing resilience includes GLM own-capacity failover alerting and public-safe fallback telemetry for repeated no-headroom saturation. What is NOT yet green is live paid evidence: production still needs owner-armed Stripe/MPP inputs and a dereferenceable real or approved staging-to-prod paid receipt before broad paid-credit claims. The supply pillars are first-party Vertex (Gemini/Anthropic), the Fireworks open-model passthrough lane (provider connection verified — see inference.fireworks_open_model_provider.v1), and the planned Pylon serving fabric (inference.decentralized_serving_fabric.v1). Aggregating hosted-model inference behind one credits API is standard gateway practice; the gateway is shipped, the paid loop is receipt-gated.',
        unsafeCopy:
          'Do not claim a broadly launched paid OpenAgents inference API, a Bitcoin-fundable inference credit balance, or that any customer can buy metered inference through OpenAgents today without citing a dereferenceable paid receipt. Do not say the gateway API, OpenAI-compatible serving, USD-credit bridge, or card-credit-spend receipt resolver is unbuilt — those pieces are built; the remaining gap is owner-armed paid evidence. Do not imply the verified Fireworks connection is a shipped customer inference product.',
        evidenceRefs: [
          'docs/inference/README.md',
          'docs/inference/2026-06-19-inference-gateway-business.md',
          'docs/inference/2026-06-19-gateway-gemini-live-verification.md',
          'docs/launch/JUNE19_ROADMAP.md',
          'docs/inference/2026-06-19-pricing-vs-factory.md',
          'docs/inference/2026-06-19-pricing-model.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5474',
          'https://github.com/OpenAgentsInc/openagents/issues/5476',
          'https://github.com/OpenAgentsInc/openagents/issues/5477',
          'https://github.com/OpenAgentsInc/openagents/issues/5478',
          'https://github.com/OpenAgentsInc/openagents/issues/5482',
          'https://github.com/OpenAgentsInc/openagents/issues/5485',
          'https://github.com/OpenAgentsInc/openagents/issues/5486',
          'https://github.com/OpenAgentsInc/openagents/issues/5497',
          'https://github.com/OpenAgentsInc/openagents/issues/6108',
          'docs/promises/2026-06-23-khala-billing-mpp-proof-gate.md',
          'apps/openagents.com/docs/launch/2026-06-23-khala-billing-mpp-production-proof.md',
          'apps/openagents.com/workers/api/src/inference/model-router.ts',
          'apps/openagents.com/workers/api/src/inference/model-router.test.ts',
          'apps/openagents.com/workers/api/src/inference/card-credit-spend-receipt-store.ts',
          'apps/openagents.com/workers/api/src/inference/mpp/mpp-chat-completions-routes.ts',
          'promise:api.hosted_gemini.v1',
          'promise:payments.accepted_outcome_economics.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable',
          'blocker.product_promises.inference_paid_receipt_not_yet_supplied',
          'blocker.product_promises.inference_mpp_owner_activation_pending',
          'public_paid_model_gateway_missing',
        ],
        verification:
          'The gateway request surface (live OpenAI-compatible /v1/chat/completions endpoint, key-auth, balance gate, cheapest-viable routing, receipt-first credit decrement from provider usage, Gemini 3.5 Flash served end-to-end) is satisfied and verified live. The code path for card-funded USD credit -> explicit USD->msat bridge -> metered inference receipt is implemented and has public receipt resolution. Focused model-router tests now cover GLM own-capacity failover activation after consecutive no-headroom saturation failures, automatic recovery clearing, and public-safe fallback telemetry. The remaining gate for GREEN as a credits business is receipt-first paid evidence: a real customer/agent or approved staging-to-prod path funds a balance with card/MPP, bridges USD credit into inference spend, and settles a metered inference request with a dereferenceable card->credit->inference-spend receipt. Stays red/non-green until that paid receipt exists; free inference being live does not green the credits business.',
        authorityBoundary:
          'A live, deployed gateway request surface plus live FREE inference grants serving-reachability and free-tier authority only. Built card/MPP bridge machinery grants no broad PAID billing, no Bitcoin-fundable credit-balance, no collectable-revenue, payout, or paid-product-claim authority until the receipt-first paid gate is met. USD/card-origin balances are inference-spendable only and not Bitcoin-withdrawable.',
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.fireworks_open_model_provider.v1',
        productArea: 'inference gateway',
        audience: ['operator', 'developer', 'agent'],
        state: 'yellow',
        claim:
          'OpenAgents has a verified live provider connection to Fireworks AI for open-weight model inference (DeepSeek, Kimi, GLM, Qwen, MiniMax, gpt-oss, Nemotron, embeddings, vision, image) as the open-model passthrough supply lane for the planned inference gateway.',
        safeCopy:
          'This is a verified provider connection now wired as a REGISTERED LIVE SUPPLY LANE in the deployed gateway, not a shipped PAID customer product. On 2026-06-19 a real OpenAI-compatible call to Fireworks (POST https://api.fireworks.ai/inference/v1/chat/completions) succeeded on accounts/fireworks/models/deepseek-v4-pro and glm-5p2, returning a proper usage object; auth + inference were confirmed against an OpenAgents-held key in .secrets/fireworks.env. Seven serverless models are live on the account. The gateway request surface that routes to this lane is now built, deployed, and live (POST /v1/chat/completions, INFERENCE_GATEWAY_ENABLED=true — see inference.gateway_credits_business.v1), so the open-model passthrough is a registered routing target with a usable cheap-tier cost basis. It still does NOT mean any customer can BUY this inference through OpenAgents: the paid-credits path (card->credit + the USD->msat bridge) is not collectable end-to-end, so there is no sellable open-model product and no dereferenceable paid receipt yet.',
        unsafeCopy:
          'Do not claim OpenAgents SELLS Fireworks/open-model inference, that a customer or agent can BUY Fireworks inference through OpenAgents, or that there is a live PAID open-model product — the paid-credits path is not collectable end-to-end. Do not say the gateway that routes to this lane is "unbuilt" — it is built, deployed, and live. Do not print or reference the raw Fireworks API key.',
        evidenceRefs: [
          'docs/inference/2026-06-19-fireworks-provider.md',
          'docs/inference/2026-06-19-gateway-gemini-live-verification.md',
          'docs/inference/README.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5479',
          'https://github.com/OpenAgentsInc/openagents/issues/5474',
          'promise:inference.gateway_credits_business.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable',
          'blocker.product_promises.inference_open_model_paid_product_no_receipt',
        ],
        verification:
          'Yellow is scoped to the verified upstream connection now registered as a live gateway supply lane: a real Fireworks OpenAI-compatible chat/completions call returning a usage object against an OpenAgents-held key, the documented serverless catalog/pricing, and a deployed gateway request surface that can route to it. Green (a sellable open-model inference product) additionally requires the paid-credits path to be collectable end-to-end (Stripe card->credit in prod, USD->msat bridge #5497) and a real customer-completed funded open-model request with dereferenceable metering and settlement evidence.',
        authorityBoundary:
          'A verified provider connection is upstream-reachability evidence only. It grants no customer-facing inference product, billing, credit-balance, routing, payout, or public-product-claim authority, and it is not by itself any green claim.',
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.referral_on_all_inference.v1',
        productArea: 'inference gateway',
        audience: ['agent', 'developer', 'customer', 'public'],
        state: 'planned',
        claim:
          'Anyone who refers a user, agent, or business that funds an OpenAgents inference account earns an ongoing referral revshare cut of ALL of that account’s inference spend, indefinitely (not a one-time bounty), settled in credits or Bitcoin.',
        safeCopy:
          'Referral-on-all-inference remains planned, not green. The implementation now reuses the RL-1 referral attribution and payout ledger for referred inference principals, accrues eligibility after receipt-first metered paid requests, and records a per-request margin split across OpenAgents, the aggregate serving-node share, and the referrer. That is still not a live earning claim: the paid-credits path is not collectable end-to-end for customers, inference referral payout dispatch remains owner-gated, and no real referred-inference payout has settled with a dereferenceable receipt. The Sites 5% referral payout ledger is a related narrower surface; it does not by itself make this inference referral claim live.',
        unsafeCopy:
          'Do not claim anyone has received a settled payout from referred inference spend, that pointing a business at OpenAgents pays the referrer today, or that the promise is green. Do not present eligibility rows, split rows, or the wired Sites referral ledger as settled payout proof.',
        evidenceRefs: [
          'docs/inference/2026-06-19-inference-gateway-business.md',
          'docs/inference/2026-06-19-decentralized-serving-shard-wan.md',
          'docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md',
          'apps/openagents.com/workers/api/src/inference/inference-referral-accrual.ts',
          'apps/openagents.com/workers/api/src/inference/inference-referral-accrual.test.ts',
          'apps/openagents.com/workers/api/src/inference/inference-referral-split.ts',
          'apps/openagents.com/workers/api/migrations/0257_inference_referral_margin_splits.sql',
          'https://github.com/OpenAgentsInc/openagents/issues/5475',
          'https://github.com/OpenAgentsInc/openagents/issues/5487',
          'https://github.com/OpenAgentsInc/openagents/issues/5488',
          'https://github.com/OpenAgentsInc/openagents/issues/5489',
          'https://github.com/OpenAgentsInc/openagents/issues/5490',
          'https://github.com/OpenAgentsInc/openagents/issues/5491',
          'https://github.com/OpenAgentsInc/openagents/issues/6839',
          'promise:inference.gateway_credits_business.v1',
          'promise:sites.referral_bitcoin_stream.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable',
          'blocker.product_promises.inference_referral_first_real_paid_receipt_pending',
          'blocker.product_promises.referral_first_real_payout_pending',
        ],
        verification:
          'Planned until the PAID inference gateway is customer-collectable end-to-end, the existing RL-1-backed referred-account accrual path is exercised by real funded inference spend, the recorded per-request split rows reconcile to the metered charge receipt and payout eligibility row, and a real referred-inference referral payout settles with a dereferenceable receipt under RL-1/2/3 and the asset-boundary/no-resale guards.',
        authorityBoundary:
          'Referral attribution, eligibility rows, and recorded split rows are accounting evidence only. They grant no spendable settlement or public green-claim authority until owner-gated dispatch settles a real payout with public-safe receipt evidence.',
      },
      {
        ...basePromiseFields,
        promiseId: 'cloud.agent_cloud_one_stop_revshare.v1',
        productArea: 'OpenAgents Cloud',
        audience: ['agent', 'developer', 'customer', 'operator', 'public'],
        state: 'planned',
        claim:
          'OpenAgents Cloud is the one-stop Agent Cloud for every agent need — inference, fine-tuning, training, sandboxes, agentic compute, tasks, and data — bought from one credit balance (USD or Bitcoin), with revshare throughout to the contributor who served the work and the referrer who brought the customer.',
        safeCopy:
          'This is the unifying VISION capstone (docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md), not a shipped one-stop product. What is real now is the pattern and the spine: the accepted-outcome -> receipt -> settle rail and the revenue-loop wiring (RL-1/2/3) exist and are intended to be reused per category. The categories themselves are at very different stages: inference is freshly filed and unbuilt (#5474); training is at the Tassadar/decentralized stage (training.decentralized_training_launch.v1 is green only for a bounded settled scope); fine-tuning and the data marketplace are largely future; sandboxes/agentic compute are partly built/roadmap. There is no single credit balance spanning all categories, and cross-category “earn on everything forever” referral accrual is a design goal, not an implemented system.',
        unsafeCopy:
          'Do not claim a live one-stop Agent Cloud, a single credit balance you can spend across inference/fine-tuning/training/sandboxes/tasks/data, or that revshare-throughout/referral-on-everything-forever is operating. Do not let one bounded green claim (e.g. a single settled training run) stand in for the whole-cloud product.',
        evidenceRefs: [
          'docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md',
          'docs/inference/README.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5474',
          'https://github.com/OpenAgentsInc/openagents/issues/5475',
          'https://github.com/OpenAgentsInc/openagents/issues/5510',
          'https://github.com/OpenAgentsInc/openagents/issues/5519',
          'apps/openagents.com/workers/api/src/autopilot-composed-run-execution.ts',
          'apps/openagents.com/workers/api/src/autopilot-composed-run-execution.test.ts',
          'apps/openagents.com/workers/api/src/cloud/cloud-metering.ts',
          'apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-accrual.ts',
          'promise:inference.gateway_credits_business.v1',
          'promise:inference.referral_on_all_inference.v1',
          'promise:payments.accepted_outcome_economics.v1',
          'promise:training.decentralized_training_launch.v1',
          'promise:training.full_pipeline_program.v1',
          'promise:compute.tassadar_executor_poc.v1',
          'promise:proof.demand_provenance.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.agent_cloud_unified_credit_balance_unbuilt',
          'blocker.product_promises.agent_cloud_cross_category_revshare_unbuilt',
          'blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable',
          'blocker.product_promises.referral_first_real_payout_pending',
        ],
        verification:
          'Planned until at least two of the named categories are simultaneously buyable from one shared credit balance, contributor revshare and cross-category referral accrual are implemented on the shared spine, and a real customer/agent completes funded multi-category work with dereferenceable per-category metering/grading and settlement receipts. Per proof.demand_provenance.v1, internal first-party usage is plumbing proof, not market proof.',
        authorityBoundary:
          'A vision capstone is roadmap evidence only. It grants no unified billing, cross-category routing, contributor payout, referral accrual, or public one-stop-product-claim authority. Each category remains gated by its own promise record and receipts.',
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.decentralized_serving_fabric.v1',
        productArea: 'inference gateway',
        audience: ['contributor', 'operator', 'developer', 'public'],
        state: 'red',
        claim:
          'The Pylon network is a decentralized model-serving fabric — every Pylon can load weights and serve inference (small models whole, large models sharded across N Pylons via the shard-WAN pipeline) — supplying the inference gateway and paying serving nodes Bitcoin revshare against exact-parity receipts.',
        safeCopy:
          'Decentralized serving as gateway supply remains red and owner-gated, not a live public capability. The near-term whole-small-model lane now has source support for a Psionic/vLLM proxy that accepts the gateway serve contract, forwards a greedy request to a configured local engine, performs a second same-engine greedy reference call, and only marks parity verified when the served and reference digests match. That is receipt-first plumbing, not production evidence: no owner-armed Pylon has yet served a live gateway inference request with a dereferenceable exact-greedy-parity receipt, and no serving-node inference payout has settled. The shard-WAN large-model pipeline remains Psionic-planned / hardware-blocked.',
        unsafeCopy:
          'Do not claim Pylons serve inference, that large models are sharded across the network and served, that decentralized inference supply is live, or that any serving node has earned Bitcoin for serving inference. Do not claim a trustless privacy or trustless-verification guarantee — the fabric is trusted-posture and activation-visible to the serving worker.',
        evidenceRefs: [
          'docs/inference/2026-06-19-decentralized-serving-shard-wan.md',
          'docs/inference/2026-06-19-inference-gateway-business.md',
          'docs/inference/README.md',
          'apps/pylon/src/psionic-vllm-proxy.ts',
          'apps/pylon/tests/psionic-vllm-proxy.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5483',
          'https://github.com/OpenAgentsInc/openagents/issues/5484',
          'https://github.com/OpenAgentsInc/openagents/issues/5474',
          'promise:inference.gateway_credits_business.v1',
          'promise:compute.tassadar_executor_poc.v1',
          'promise:pylon.consumer_compute_earns_bitcoin_self_serve.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.pylon_inference_serving_live_receipt_missing',
          'blocker.product_promises.shard_wan_large_model_serving_psionic_planned',
          'blocker.product_promises.inference_serving_node_payout_unbuilt',
          'blocker.product_promises.inference_serving_first_real_payout_owner_armed',
          'blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable',
        ],
        verification:
          'Red until an owner-armed Pylon serves a real gateway inference request through the whole-small-model proxy, the run returns a dereferenceable exact-greedy-parity receipt from a same-engine reference check, the gateway admits that receipt through canary/replay/payout-eligibility gates, and an owner-armed first serving-node Bitcoin payout settles with a dereferenceable receipt under RL-2/RL-3. Source tests prove the proxy refuses non-greedy requests and fails closed on reference mismatch; they do not green the live serving claim.',
        authorityBoundary:
          'Serving-fabric design honors the Psionic boundary: pricing, payout, marketplace, and identity authority stay outside Psionic, which emits evidence/receipts only. This record grants no serving, routing, payout, or public-product-claim authority, and large-model fabric claims stay blocked until a hardware-backed receipt or a typed refusal exists.',
      },
      {
        ...basePromiseFields,
        promiseId: 'referral.refer_once_earn_forever.v1',
        productArea: 'referral',
        audience: ['agent', 'user', 'customer', 'contributor', 'public'],
        state: 'red',
        claim:
          'Refer once, earn forever: share a link, the person or agent who joins is your referral forever, and any time that referral ever buys ANYTHING in the OpenAgents ecosystem (Autopilot, inference, training, sandboxes, marketplace products, any layer) you earn an ongoing cut, settled in Bitcoin.',
        safeCopy:
          'Ecosystem-wide refer-once-earn-forever is the Episode 239 referral vision (docs/transcripts/239.md), and it is NOT yet a live product. What is real: attribution capture, a permanent consume-once user/agent referral spine, and ONE referral payout ledger. The Autopilot Sites 5% referral payout ledger is wired end to end in source (RL-1 #5458, sites.referral_bitcoin_stream.v1, yellow) — a paid event creates an idempotent eligibility row and a dispatch path drives approved -> dispatched -> settled via the injected MDK/Spark adapter, readiness-gated and Bitcoin-only — AND that ledger state is now dereferenceable at the public, count-only projection GET /api/public/site-referral-payouts (per-state counts/sats, the policy shape, a ledgerWiredInSource flag, and the staleness contract), so "wired in source" is now "wired + dereferenceable." The cross-category accrual primitive now reuses that permanent attribution spine and can feed receipt-first paid non-Sites events such as marketplace or fine-tuning spend into the same RL-1 ledger, while enforcing idempotency and the shared asset-boundary/no-resale guards. But NO real referral payout has ever settled — the projection reports settledCount/settledSats as 0 — and there is still no real purchase -> settled referral-payout receipt. The video itself qualifies the claim: "as long as OpenAgents remains solvent."',
        unsafeCopy:
          'Do not claim anyone earns spendable Bitcoin from referred ecosystem purchases today, that refer-once-earn-forever is live, that referral revshare has paid out across all products, or that any referrer has ever been paid. Do not present the wired-but-unsettled referral ledger, the cross-category accrual primitive, or the planned inference referral slice as proof this ecosystem-wide claim is live. Referral attribution and eligibility rows are never spendable settlement.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/launch/2026-06-19-near-term-product-priorities.md',
          'apps/openagents.com/workers/api/src/site-referral-payout-feed.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-dispatch.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-ledger.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-wire.test.ts',
          'apps/openagents.com/workers/api/src/referral-cross-category-accrual.ts',
          'apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-accrual.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-public-projection.ts',
          'apps/openagents.com/workers/api/src/site-referral-payout-public-routes.ts',
          'route:/api/public/site-referral-payouts',
          'https://openagents.com/api/public/site-referral-payouts',
          'https://github.com/OpenAgentsInc/openagents/issues/5458',
          'promise:sites.referral_bitcoin_stream.v1',
          'promise:inference.referral_on_all_inference.v1',
          'promise:cloud.agent_cloud_one_stop_revshare.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.referral_first_real_payout_pending',
          'blocker.product_promises.referral_purchase_to_payout_receipt_missing',
        ],
        verification:
          'Red until a real paid ecosystem purchase produces a settled Bitcoin referral payout and that payout has a dereferenceable purchase -> payout receipt under RL-1/RL-3 and the asset-boundary/no-resale guards. The source evidence now covers the permanent attribution spine and non-Sites accrual primitive, but a real settled payout receipt plus owner-signed receipt-first upgrade remains required before any live earn-forever claim.',
        authorityBoundary:
          'A referral vision, permanent attribution binding, and eligibility accrual rows grant no payout or settlement authority. Cross-category referral revshare remains blocked as a public live-earning claim until real category spend settles through the owner-armed payout rail and dereferenceable receipts exist.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.all_in_one_business_system.v1',
        productArea: 'Autopilot',
        audience: ['user', 'agent', 'customer', 'public'],
        state: 'planned',
        claim:
          'Autopilot is the all-in-one business system that businesses run on, composed of the OpenAgents Cloud primitives (inference, fine-tuning, training, agentic tasks, sandbox compute, web services) and the open markets beneath them, with referral revenue earned for bringing others onto Autopilot.',
        safeCopy:
          'Autopilot-as-all-in-one-business-system is the Episode 239 composition vision (docs/transcripts/239.md). Real, separately-gated pieces exist — Autopilot Sites, the decision queue, the mission briefing, the coding-agent execution lanes, and a wired (unsettled) referral ledger — but there is no single composed "business system" product where the primitives are provisioned, run, and billed from one balance for a real business. Each underlying primitive is gated by its own promise record (see cloud.primitives_suite.v1, markets.open_protocol_markets.v1, referral.refer_once_earn_forever.v1).',
        unsafeCopy:
          'Do not claim a business can run end to end on Autopilot today, that Autopilot composes inference/fine-tuning/training/sandbox/web-services into one bought-and-billed product, or that referral-on-Autopilot-signups pays anyone now. Do not let one shipped sub-surface (Sites, a coding session) stand in for the whole business system.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/launch/2026-06-19-near-term-product-priorities.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5510',
          'https://github.com/OpenAgentsInc/openagents/issues/5519',
          'apps/openagents.com/workers/api/src/autopilot-composed-run.ts',
          'apps/openagents.com/workers/api/src/autopilot-composed-run-execution.ts',
          'apps/openagents.com/workers/api/src/autopilot-composed-run-execution.test.ts',
          'promise:cloud.primitives_suite.v1',
          'promise:cloud.agent_cloud_one_stop_revshare.v1',
          'promise:markets.open_protocol_markets.v1',
          'promise:referral.refer_once_earn_forever.v1',
          'promise:autopilot.cloud_coding_sessions.v1',
          'promise:autopilot.decision_queue.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.autopilot_business_system_composition_unbuilt',
          'blocker.product_promises.autopilot_business_system_unified_billing_unbuilt',
          'blocker.product_promises.autopilot_business_system_real_business_receipt_missing',
        ],
        verification:
          'Planned until a real business provisions and runs at least two composed primitives through Autopilot against one balance and a dereferenceable receipt shows composed usage billed and (where revenue applies) settled. Per proof.demand_provenance.v1, internal first-party use is plumbing proof, not market proof.',
        authorityBoundary:
          'A composition vision grants no provisioning, unified-billing, cross-primitive routing, or public all-in-one-product-claim authority. Each primitive and market remains gated by its own promise record.',
      },
      {
        ...basePromiseFields,
        promiseId: 'cloud.primitives_suite.v1',
        productArea: 'OpenAgents Cloud',
        audience: ['agent', 'developer', 'customer', 'operator', 'public'],
        state: 'planned',
        claim:
          'OpenAgents Cloud exposes the full primitive set agents and humans build products from — inference, fine-tuning, training, agentic tasks, sandbox compute, and standard web services (a layer on top of Cloudflare, shipped as Autopilot Sites).',
        safeCopy:
          'The Cloud primitive SUITE is the Episode 239 build-from-primitives vision (docs/transcripts/239.md); as a single buyable suite it is roadmap. The primitives are at very different stages: inference gateway request surface is live but free-only (inference.gateway_credits_business.v1, red); decentralized training is green only for one bounded settled scope (training.decentralized_training_launch.v1); agentic tasks / labor is green for one settled job (labor.forum_work_requests.v1); web services exist as Autopilot Sites (partly live); fine-tuning (cloud.fine_tuning_service.v1, red) and sandbox compute (cloud.sandbox_compute_service.v1, red) have flag-gated inert scaffolds but are not live sellable services. An inert composed-run shape can model multiple primitive charges against one shared balance, but no live customer run has debited one balance across multiple primitives or produced a dereferenceable unified-balance receipt.',
        unsafeCopy:
          'Do not claim a complete OpenAgents Cloud primitive suite is buyable, that fine-tuning or sandbox compute are sellable services, or that the primitives can be composed-and-billed from one balance today. Do not let one green primitive (a settled training run) imply the whole suite is live.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/inference/2026-06-19-agent-cloud-revshare-everywhere.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5510',
          'https://github.com/OpenAgentsInc/openagents/issues/5519',
          'apps/openagents.com/workers/api/src/autopilot-composed-run-execution.ts',
          'apps/openagents.com/workers/api/src/autopilot-composed-run-execution.test.ts',
          'apps/openagents.com/workers/api/src/cloud/cloud-metering.ts',
          'promise:inference.gateway_credits_business.v1',
          'promise:cloud.fine_tuning_service.v1',
          'promise:cloud.sandbox_compute_service.v1',
          'promise:training.decentralized_training_launch.v1',
          'promise:labor.forum_work_requests.v1',
          'promise:cloud.agent_cloud_one_stop_revshare.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.cloud_fine_tuning_live_sellable_service_missing',
          'blocker.product_promises.cloud_sandbox_compute_live_sellable_service_missing',
          'blocker.product_promises.cloud_primitives_live_unified_balance_debit_receipt_missing',
          'blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable',
        ],
        verification:
          'Planned until every named primitive is at least individually buyable with a dereferenceable receipt and at least two are buyable from one shared balance, with per-primitive metering and (where revenue applies) settlement evidence. The suite claim does not go green on the strength of any single primitive.',
        authorityBoundary:
          'Listing the primitive set grants no provisioning, billing, routing, payout, or public-suite-product-claim authority. Each primitive is gated by its own promise record.',
      },
      {
        ...basePromiseFields,
        promiseId: 'cloud.fine_tuning_service.v1',
        productArea: 'OpenAgents Cloud',
        audience: ['agent', 'developer', 'customer', 'public'],
        state: 'red',
        claim:
          'OpenAgents Cloud offers fine-tuning as a buyable primitive: submit a base model + dataset, run a fine-tune on the network, and use the resulting model through the inference gateway, billed from a credit balance.',
        safeCopy:
          'Fine-tuning as a sellable Cloud primitive (named in Episode 239, docs/transcripts/239.md) is NOT a live billed product. A flag-gated scaffold exists behind CLOUD_FINE_TUNING_ENABLED (default off -> 404): POST /v1/fine_tuning/jobs runs the bounded D1 fixture runtime to completion, persists lifecycle rows, registers the resulting ft:<jobId> model in a per-account model registry, and GET /v1/fine_tuning/jobs/:id reads the real stored status with cross-account isolation. The chat gateway can resolve a caller-owned registered fine-tuned model id back to its base model for serving. Completed fixture jobs pass runtime usage into the receipt-first cloud-metering seam (cloud-metering.ts), but live pricing is zero/scaffold-only and no paid fine-tuning receipt or settlement is created. OpenAgents has a decentralized TRAINING lane (training.* records; decentralized_training_launch.v1 is green only for one bounded settled executor-trace scope) and a live inference gateway request surface; this record remains red until a real customer-paid fine-tune has a dereferenceable paid receipt and owner sign-off.',
        unsafeCopy:
          'Do not claim OpenAgents sells fine-tuning, that a customer can buy a fine-tune, or that fine-tuned models are paid/billable through OpenAgents today. Do not say the surface is unbuilt or runtime-unwired: a flag-gated fixture runtime, persisted lifecycle read, model registration, gateway resolver, and tested receipt-first metering seam exist. The remaining gaps are live enabled intake, real nonzero pricing, demand provenance, paid receipt, settlement where required, and owner sign-off. Do not present the training run or the inference gateway as a paid fine-tuning product.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/inference/2026-06-19-cloud-primitives-fine-tuning-sandbox-scaffold-advance.md',
          'apps/openagents.com/workers/api/src/cloud/fine-tuning-service-routes.ts',
          'apps/openagents.com/workers/api/migrations/0256_cloud_fine_tuning_runtime.sql',
          'apps/openagents.com/workers/api/src/cloud/cloud-metering.ts',
          'apps/openagents.com/workers/api/src/cloud/cloud-metering.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5510',
          'https://github.com/OpenAgentsInc/openagents/issues/5516',
          'promise:training.decentralized_training_launch.v1',
          'promise:training.full_pipeline_program.v1',
          'promise:inference.gateway_credits_business.v1',
          'promise:cloud.primitives_suite.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.cloud_fine_tuning_live_intake_disabled',
          'blocker.product_promises.cloud_fine_tuning_live_pricing_missing',
          'blocker.product_promises.cloud_fine_tuning_paid_receipt_missing',
          'blocker.product_promises.cloud_fine_tuning_billing_settlement_missing',
        ],
        verification:
          'Red until a real customer submits a fine-tune job through live enabled intake, it runs beyond the bounded fixture path, the resulting model is registered and servable through the gateway, and a dereferenceable paid fine-tuning receipt (nonzero metering + settlement where required) exists. Per proof.demand_provenance.v1, internal fine-tunes are plumbing proof, not market proof.',
        authorityBoundary:
          'Naming fine-tuning as a primitive grants no fine-tune intake, runtime, model-registration, billing, payout, or public-product-claim authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'cloud.sandbox_compute_service.v1',
        productArea: 'OpenAgents Cloud',
        audience: ['agent', 'developer', 'customer', 'public'],
        state: 'red',
        claim:
          'OpenAgents Cloud offers sandbox compute as a buyable primitive: agents and humans rent isolated, metered execution sandboxes to run code and agentic tasks, billed from a credit balance.',
        safeCopy:
          'Sandbox compute as a sellable Cloud primitive (named in Episode 239, docs/transcripts/239.md) is NOT a live billed product. A flag-gated scaffold exists behind CLOUD_SANDBOX_COMPUTE_ENABLED (default off → 404): POST /v1/sandboxes now runs a bounded D1 fixture runtime, persists lifecycle rows, returns a public-safe scoped sandbox session ref, enforces the hard TTL ceiling before provisioning, and GET /v1/sandboxes/:id reads the stored status with cross-account isolation. Completed fixture rentals pass runtime usage into the REAL receipt-first credit-metering seam (cloud-metering.ts), which decrements credits through the same atomic PayIn ledger the inference gateway uses when live pricing is armed; live pricing remains zero/scaffold-only, so no paid sandbox receipt or settlement is created from the fixture path. The dereferenceable-RECEIPT artifact exists end to end: a closed metered rental can settle its debit-only charge to PAID in one atomic batch, the surface advertises the SAME receipt ref the ledger writes, and GET /api/public/cloud/receipts/:ref dereferences that PAID charge into a public-safe receipt. HONEST: prod remains inert while the flag is off, and this fixture is not a customer-rentable isolated compute substrate. The remaining gaps are live rent enablement, live pricing/nonzero billing, real renter demand provenance, and owner sign-off. The promise STAYS red.',
        unsafeCopy:
          'Do not claim OpenAgents sells sandbox compute, that a customer can rent a metered execution sandbox, or that sandbox compute is a live billable primitive. Do not say the surface is "unbuilt" — a flag-gated scaffold (rent + lifecycle read + persisted fixture runtime + TTL ceiling + a tested receipt-first metering seam + a dereferenceable PAID-charge receipt route) exists; the gaps are live rent enablement, live pricing/nonzero billing, real renter demand provenance, and owner sign-off. Do not treat the existing dereferenceable-receipt machinery as proof a renter has bought a sandbox. Do not present the internal coding-agent sandbox or cloud-coding loop as a sandbox-compute product.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/inference/2026-06-19-cloud-primitives-fine-tuning-sandbox-scaffold-advance.md',
          'docs/promises/2026-06-23-de2-cloud-primitive-dereferenceable-receipt.md',
          'apps/openagents.com/workers/api/src/cloud/sandbox-compute-service-routes.ts',
          'apps/openagents.com/workers/api/migrations/0257_cloud_sandbox_runtime.sql',
          'apps/openagents.com/workers/api/src/cloud/cloud-metering.ts',
          'apps/openagents.com/workers/api/src/cloud/cloud-metering.test.ts',
          'apps/openagents.com/workers/api/src/cloud/cloud-primitive-receipts.ts',
          'apps/openagents.com/workers/api/src/cloud/cloud-primitive-receipts.test.ts',
          'apps/openagents.com/workers/api/src/cloud/public-cloud-primitive-receipt-routes.ts',
          'route:/api/public/cloud/receipts/{receiptRef}',
          'https://github.com/OpenAgentsInc/openagents/issues/5510',
          'https://github.com/OpenAgentsInc/openagents/issues/5517',
          'promise:autopilot.cloud_coding_sessions.v1',
          'promise:autopilot.codex_probe_pylon_successor.v1',
          'promise:cloud.primitives_suite.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.cloud_sandbox_live_rent_surface_disabled',
          'blocker.product_promises.cloud_sandbox_live_pricing_missing',
          'blocker.product_promises.cloud_sandbox_real_renter_demand_provenance_and_owner_signoff_missing',
        ],
        verification:
          'Red until a customer rents a metered sandbox, runs work in it, and a dereferenceable nonzero paid sandbox receipt (metering + settlement) exists, with isolation/abuse controls evidenced. The D1 fixture runtime, dereferenceable PAID-charge receipt route, and settle-to-paid metering path now exist and are tested against real SQL, so the receipt/runtime scaffold is no longer the blocker; the remaining blockers are live rent enablement, live pricing/nonzero billing, real renter demand provenance, and owner sign-off. Per proof.demand_provenance.v1, internal sandbox use is plumbing proof, not market proof; the final green flip needs owner sign-off per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Naming sandbox compute as a primitive grants no sandbox provisioning, metering, billing, payout, or public-product-claim authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'markets.open_protocol_markets.v1',
        productArea: 'markets',
        audience: ['agent', 'developer', 'contributor', 'public'],
        state: 'planned',
        claim:
          'The six Episode 213 markets — compute, data, labor, liquidity, risk, and verification — are exposed as open protocols / open markets that agents can dip into to do things like build an agentic insurance policy, offer new compute, or sell data.',
        safeCopy:
          'The open-markets vision (Episode 213, restated in Episode 239) is partly real and mostly roadmap. Live/scoped: the LABOR market crossed its first end-to-end settled milestone (labor.forum_work_requests.v1 / labor.nostr_negotiation_market.v1, green; first job #4777) and VERIFICATION exists as exact-trace replay (compute.tassadar_executor_poc.v1, green for a bounded PoC). The COMPUTE and DATA market rails shipped over NIP-90 in earlier releases (in repo history) but are not broadly live as paid markets. A public unified open-markets scaffold now exists at GET /api/public/markets/open-markets, with inert liquidity and risk skeletons at GET /api/public/markets/liquidity/skeleton and GET /api/public/markets/risk/skeleton. The LIQUIDITY and RISK markets are still skeleton-only, and there are no settled receipts across the full six-market set.',
        unsafeCopy:
          'Do not claim all six markets are live, that liquidity or risk markets exist, that compute/data are broadly live paid markets, or that an agentic insurance policy can be built and settled today. Do not let the green labor and verification scopes stand in for the whole market set.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/transcripts/213.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'apps/openagents.com/workers/api/src/open-markets-surface.ts',
          'apps/openagents.com/workers/api/src/open-markets-routes.ts',
          'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
          'route:/api/public/markets/open-markets',
          'route:/api/public/markets/liquidity/skeleton',
          'route:/api/public/markets/risk/skeleton',
          'packages/nip90',
          'promise:labor.forum_work_requests.v1',
          'promise:labor.nostr_negotiation_market.v1',
          'promise:compute.tassadar_executor_poc.v1',
          'promise:pylon.five_bitcoin_revenue_streams.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.liquidity_market_unbuilt',
          'blocker.product_promises.risk_market_unbuilt',
          'blocker.product_promises.compute_data_markets_not_broadly_live',
        ],
        verification:
          'Planned until each of the six markets is at least individually exercisable with a dereferenceable receipt for a real participant transaction. The unified open-markets surface is now present and public-safe, but it is a scaffold: liquidity and risk market endpoints are inert skeleton projections, compute/data are not broadly live paid markets, and green still requires real market transactions plus settlement receipts for all six markets.',
        authorityBoundary:
          'Naming the six markets grants no market-making, matching, settlement, custody, insurance-underwriting, or public-market-claim authority. Each market is gated by its own evidence.',
      },
      {
        ...basePromiseFields,
        promiseId: 'marketplace.compose_and_list_products.v1',
        productArea: 'marketplace',
        audience: ['agent', 'developer', 'contributor', 'public'],
        state: 'planned',
        claim:
          'Agents and humans build their own products out of the OpenAgents Cloud primitives and open markets, then list those products for sale in the OpenAgents marketplace.',
        safeCopy:
          'Compose-your-own-product-and-list-it-for-sale is the Episode 239 marketplace vision (docs/transcripts/239.md), and it is roadmap. A typed product-definition scaffold, bounded no-spend assemble/list/install-use lifecycle receipts with builder attribution, and public read-only listing/discovery projection exist at GET /api/public/marketplace/composed-products; the route is inert, planned-state, and empty by default unless injected/flag-armed. Adjacent planned lanes exist — the agentic-npm module registry (marketplace.agentic_npm_module_registry.v1) and WASM plugins (marketplace.wasm_plugins.v1) — but there is no marketplace billing, paid sale receipt, rev-share settlement, or live primitive provisioning. Nobody has sold a composed product through OpenAgents.',
        unsafeCopy:
          'Do not claim users or agents can build a live paid product from the primitives and list it for sale today, that an OpenAgents product marketplace is live, or that composed products are buyable, billable, settled, revenue-bearing, or backed by live primitive provisioning. Do not present the no-spend lifecycle scaffold, inert read-only listing route, or planned module/plugin lanes as a live compose-and-sell marketplace.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'promise:marketplace.agentic_npm_module_registry.v1',
          'promise:marketplace.wasm_plugins.v1',
          'promise:cloud.primitives_suite.v1',
          'promise:markets.open_protocol_markets.v1',
          'apps/openagents.com/workers/api/src/marketplace-product-composition.ts',
          'apps/openagents.com/workers/api/src/marketplace-composition-routes.ts',
          'route:/api/public/marketplace/composed-products',
        ],
        blockerRefs: [
          'blocker.product_promises.marketplace_paid_listing_runtime_missing',
          'blocker.product_promises.marketplace_billing_settlement_missing',
        ],
        verification:
          'Planned until a composed product can be assembled from primitives, self-serve listed, discovered, installed/used by a buyer, and produce a dereferenceable paid receipt with attribution and rev-share to the builder. The current source proves only the inert typed definition plus no-spend assemble/list/install-use lifecycle receipts, and the public route proves only read-only listing/discovery. Per proof.demand_provenance.v1, an internally-built or injected listing is plumbing proof, not market proof.',
        authorityBoundary:
          'A compose-and-list vision plus no-spend lifecycle scaffold grant no paid listing, live primitive provisioning, fulfillment, billing, rev-share, payout, or public-marketplace-claim authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'marketplace.monetize_any_layer_with_referral.v1',
        productArea: 'marketplace',
        audience: ['agent', 'developer', 'contributor', 'customer', 'public'],
        state: 'planned',
        claim:
          'Anyone (or their agents) can monetize or sell access to ANY layer of the stack — inference, compute, data, labor, training, sandboxes, markets — and earn referrals on it (e.g. refer a big bulk-inference client and get a piece).',
        safeCopy:
          'Monetize-or-sell-access-to-any-layer-with-referrals is the Episode 239 vision (docs/transcripts/239.md); it is roadmap, not a live product. The pieces it would compose are themselves gated: the accepted-outcome -> receipt -> settle spine exists (payments.accepted_outcome_economics.v1, red), the Sites referral ledger is wired but unsettled (sites.referral_bitcoin_stream.v1, yellow), and the ecosystem-wide referral and per-layer products are themselves unbuilt (referral.refer_once_earn_forever.v1, red; cloud.primitives_suite.v1, planned; markets.open_protocol_markets.v1, planned). No layer can be resold-with-referral end to end and no such referral payout has settled.',
        unsafeCopy:
          'Do not claim anyone can sell access to a layer and earn referrals today, that reselling inference/compute/data/labor with a referral cut is live, or that pointing a bulk client at OpenAgents pays the referrer now. API-inference resale is the allowed monetization lane under the no-resale invariant (resale of SUBSCRIPTION provider access stays prohibited), but the build does not exist yet and no resale-with-referral receipt exists.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'promise:referral.refer_once_earn_forever.v1',
          'promise:marketplace.compose_and_list_products.v1',
          'promise:cloud.primitives_suite.v1',
          'promise:markets.open_protocol_markets.v1',
          'promise:payments.accepted_outcome_economics.v1',
          'promise:provider.compliant_usage_labor.v1',
          'apps/openagents.com/workers/api/src/marketplace-monetize-any-layer.ts',
          'apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-accrual.ts',
          'apps/openagents.com/workers/api/src/inference-resale-authorization.ts',
          'apps/openagents.com/workers/api/src/marketplace-product-composition.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.monetize_any_layer_access_product_unbuilt',
          'blocker.product_promises.monetize_any_layer_referral_accrual_unbuilt',
          'blocker.product_promises.monetize_any_layer_resale_receipt_missing',
        ],
        verification:
          "Planned until a real seller monetizes access to at least one layer, a referral cut accrues to a distinct referrer off that layer's metered spend, and a dereferenceable settled receipt exists under the asset-boundary/no-resale guards (subscription-account resale stays prohibited; API-inference resale is allowed).",
        authorityBoundary:
          'A monetize-any-layer vision grants no access-selling, metering, referral-accrual, payout, or public-product-claim authority, and never waives the no-resale invariant for subscription accounts.',
      },
      {
        ...basePromiseFields,
        promiseId: 'claims.pursued_world_first_largest_agentic_sales_force.v1',
        productArea: 'world-first claims',
        audience: ['agent', 'user', 'public'],
        state: 'planned',
        claim:
          'OpenAgents is PURSUING (not claiming) a world first: the largest agentic sales force — hiring and equipping sales agents to sell OpenAgents products. This is an aspiration, not an achieved or verified record.',
        safeCopy:
          'This is an explicitly PURSUED, aspirational target from Episode 239 (docs/transcripts/239.md): "we don\'t have any relevant world firsts to report to you here aside from we\'re pursuing two world firsts ... largest agentic sales force." It is NOT achieved and must never be presented as a met or verified claim. No agentic sales force exists yet; "we\'ll have that world record pretty quickly" is forward-looking intent, not a record.',
        unsafeCopy:
          'Do not state OpenAgents HAS the largest agentic sales force, that the world record is held, achieved, or verified, or imply the sales force exists at scale. Present it only as a clearly-labeled pursuit/aspiration, never as a green/met world-first.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/promises/2026-06-29-world-first-claims-7027-audit.md',
          'promise:referral.refer_once_earn_forever.v1',
          'promise:claims.world_first_ai_training_paid_bitcoin.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.world_first_agentic_sales_force_not_achieved',
          'blocker.product_promises.world_first_agentic_sales_force_no_sized_verifiable_force',
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ],
        verification:
          'This record is intentionally never green from aspiration. The #7027 dated audit (docs/promises/2026-06-29-world-first-claims-7027-audit.md) keeps the blockers explicit: there is no real, sized, independently countable agentic sales force, no record review, and no owner-signed upgrade. Any future non-aspirational claim would require a real, sized, independently-countable agentic sales force, an independent prior-art / record review, a dereferenceable evidence pack, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1. Until then it stays a labeled pursuit.',
        authorityBoundary:
          'A pursued world first grants no record-holder status, no marketing-claim authority, and no sales, payout, or settlement authority. Aspiration is not achievement.',
      },
      {
        ...basePromiseFields,
        promiseId: 'claims.pursued_world_first_largest_sales_force.v1',
        productArea: 'world-first claims',
        audience: ['agent', 'user', 'public'],
        state: 'planned',
        claim:
          'OpenAgents is PURSUING (not claiming) a world first: the largest sales force of any kind — the named bar in Episode 239 is roughly seven million selling-or-sell-equipped agents (vs the cited Avon ~6.5M human reference). This is an aspiration, not an achieved or verified record.',
        safeCopy:
          'This is an explicitly PURSUED, aspirational target from Episode 239 (docs/transcripts/239.md): "as soon as we hit seven million agents ... selling or equipped to sell then we\'ll have the largest sales force in the world." The Avon ~6.5M figure is attributed in the video to ChatGPT, not an OpenAgents-verified statistic. It is NOT achieved and must never be presented as a met or verified claim.',
        unsafeCopy:
          'Do not state OpenAgents HAS the largest sales force, that the ~7M-agent bar is met, that the world record is held/achieved/verified, or treat the cited Avon ~6.5M figure as an OpenAgents-verified statistic. Present it only as a clearly-labeled pursuit/aspiration, never as a green/met world-first.',
        evidenceRefs: [
          'docs/transcripts/239.md',
          'docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md',
          'docs/promises/2026-06-29-world-first-claims-7027-audit.md',
          'promise:claims.pursued_world_first_largest_agentic_sales_force.v1',
          'promise:claims.world_first_public_llm_computer_training_run.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.world_first_largest_sales_force_not_achieved',
          'blocker.product_promises.world_first_largest_sales_force_seven_million_bar_unmet',
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ],
        verification:
          'This record is intentionally never green from aspiration. The #7027 dated audit (docs/promises/2026-06-29-world-first-claims-7027-audit.md) keeps the blockers explicit: no independently verified count crosses the stated bar, no independently sourced comparison pack exists, and no owner-signed upgrade exists. Any future non-aspirational claim would require independently verified counts crossing the stated bar, an independent prior-art / record review (with the comparison figure independently sourced, not ChatGPT-attributed), a dereferenceable evidence pack, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'A pursued world first grants no record-holder status, no marketing-claim authority, and no sales, payout, or settlement authority. Aspiration is not achievement.',
      },
      {
        ...basePromiseFields,
        promiseId: 'payments.autopilot_credits_purchase.v1',
        productArea: 'payments',
        audience: ['customer', 'user'],
        state: 'red',
        claim:
          'Signed-in users can buy OpenAgents Autopilot credits with a credit card and spend them on Autopilot container time and Codex token usage.',
        safeCopy:
          'The card-to-credit purchase machinery for the USD Autopilot credit balance is built and wired end to end in source and deployed: the /billing page renders balance, rates, ledger history, a coupon box, an add-card (SetupIntent) flow, credit packages, and an auto-top-up policy form, and POST /api/billing/checkout creates a real Stripe Checkout Session whose webhook fulfillment grants credits into the billing_ledger_entries USD ledger. The explicit inference-credit bridge can convert selected USD credit into USD-origin msat for Khala inference spend, and public card-credit-spend receipts can prove card -> credit -> bridge -> metered inference. It is NOT confirmed collecting money in production: the three Worker secrets (STRIPE_API_KEY, STRIPE_WEBHOOK_SIGNING_SECRET, STRIPE_CREDIT_PACKAGES_JSON) must be set, and no real card-to-credit purchase has a dereferenceable receipt. USD/card-origin credits are not Bitcoin/Lightning-withdrawable; there is no Bitcoin/Lightning path to buy these credits.',
        unsafeCopy:
          'Do not claim OpenAgents is broadly collecting card payments today, that buying credits is live in production, that all USD credits automatically fund inference, or that there is a Bitcoin/Lightning path to buy credits. Do not present the wired-but-secrets-gated checkout or MPP path as a proven, money-collecting path until a real or approved staging-to-prod card-to-credit and card-credit-spend receipt exists.',
        evidenceRefs: [
          'docs/launch/2026-06-19-credits-purchase-collect-money-audit.md',
          'apps/openagents.com/workers/api/src/billing-routes.ts',
          'apps/openagents.com/workers/api/src/stripe-billing.ts',
          'apps/openagents.com/workers/api/src/inference/usd-credit-bridge.ts',
          'apps/openagents.com/workers/api/src/inference/card-credit-spend-receipt-store.ts',
          'docs/promises/2026-06-23-khala-billing-mpp-proof-gate.md',
          'apps/openagents.com/docs/launch/2026-06-23-khala-billing-mpp-production-proof.md',
          'apps/openagents.com/apps/web/src/page/loggedIn/page/billing.ts',
          'docs/transcripts/239.md',
          'promise:inference.gateway_credits_business.v1',
          'promise:autopilot.cloud_credits_ui.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.autopilot_credits_prod_stripe_secrets_missing',
          'blocker.product_promises.autopilot_credits_no_real_card_purchase_receipt',
          'blocker.product_promises.autopilot_credits_no_card_credit_spend_receipt',
          'blocker.product_promises.autopilot_credits_no_bitcoin_purchase_path',
        ],
        verification:
          'Red until a real signed-in user funds the USD Autopilot credit balance with a card in production (STRIPE_API_KEY, STRIPE_WEBHOOK_SIGNING_SECRET, STRIPE_CREDIT_PACKAGES_JSON set) or an owner-approved staging-to-prod path, and the credit grant is dereferenceable as a billing_ledger_entries entry tied to a Stripe Checkout Session, plus one metered spend against that balance with a public-safe receipt. The /billing route, Stripe Checkout creation, webhook fulfillment, USD ledger code, explicit inference-credit bridge, and card-credit-spend receipt resolver all exist; the gate is a real collected purchase or approved staged proof and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Built purchase machinery grants no money-collection claim, no Bitcoin credit-purchase path, and no settlement authority. The USD Autopilot credit ledger can explicitly bridge selected credit into USD-origin inference-spendable msat, but that USD/card-origin value is not Bitcoin-withdrawable and does not create payout authority. The billing ledger remains distinct from the view-only workroom credits panel (autopilot.cloud_credits_ui.v1).',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot_sites.site_build_and_host.v1',
        productArea: 'Sites',
        audience: ['agent', 'user', 'customer', 'operator'],
        state: 'yellow',
        claim:
          'Users and approved agents can create an Autopilot Site, build it in a builder session, and serve it at a stable live URL — the standard web-services primitive (hosting on top of Cloudflare) framed in Episode 239.',
        safeCopy:
          'The core Autopilot Sites build-and-host surface is live: signed-in users can create customer software requests, builder sessions exist with message/event/file/file-tree/read/export and operator save-version APIs (POST/GET /api/sites/builder-sessions), Sites can carry stable live URLs and durable revision URLs, and approved registered-agent bearer tokens can submit scoped Site action contract receipts for project creation, builder-session creation, preview requests (POST /api/agent/sites/{siteId}/previews), version-save requests, and deploy requests (POST /api/agent/sites/{siteId}/deploy-requests). Production deployment is owner/operator-gated; external agent bearer tokens cannot yet create customer orders on behalf of an owner without a browser session or a specific scoped owner grant; and no paid customer Site has a dereferenceable purchase/settlement receipt. The agency-pack add-ons (referral stream, email sequences, custom hostnames, partner payout, client-delivery workrooms) are separately gated records.',
        unsafeCopy:
          'Do not claim Autopilot Sites is a fully self-serve, agent-operated, paid product, that external agents can place customer orders or deploy to production without an owner grant, that custom hostnames or email send are live (see autopilot_sites.custom_tenant_hostnames.v1 / autopilot_sites.native_email_sequences.v1), or that a paying customer Site loop is closed end to end.',
        evidenceRefs: [
          'apps/openagents.com/workers/api/src/agent-site-routes.ts',
          'apps/openagents.com/workers/api/src/operator-sites-routes.ts',
          'apps/openagents.com/apps/web/public/AGENTS.md',
          'docs/transcripts/239.md',
          'promise:autopilot_sites.custom_tenant_hostnames.v1',
          'promise:autopilot_sites.native_email_sequences.v1',
          'promise:workrooms.omni_client_delivery_workrooms.v1',
          'promise:sites.referral_bitcoin_stream.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.sites_production_deploy_owner_operator_gated',
          'blocker.product_promises.sites_external_agent_customer_orders_gated',
          'blocker.product_promises.sites_no_paid_customer_site_receipt',
        ],
        verification:
          'Yellow covers the live build-and-host surface: builder-session create/read, the agent Site action contract (project/session/preview/version-save/deploy-request), and stable live + durable revision URLs, all backed by agent-site-routes.test.ts and operator-sites-routes.test.ts. Green requires self-serve production deployment without an operator step (or an explicit owner decision that operator-gating is the product), external-agent customer-order placement under a scoped grant, and one paid customer Site with a dereferenceable purchase/settlement receipt, plus an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Building and previewing a Site grants no production-deploy authority, no customer-order-on-behalf-of-owner authority without a scoped grant, no custom-hostname/SSL or email-send authority, and no billing or settlement authority. Each Sites monetization and delivery add-on is gated by its own promise record.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.agent_world_scene.v1',
        productArea: 'Autopilot desktop',
        audience: ['agent', 'user', 'operator'],
        state: 'green',
        claim:
          'The Autopilot chat is set inside a living 3D Pylon world: a glass-over-canvas scene rendered behind the conversation where each live Pylon appears as a crystal, driven by real pylon-stats data.',
        safeCopy:
          'The agent world scene exists behind the Autopilot chat as a glass-over-canvas 3D render. Source-level gates now resolve the Verse launch scene and payment layers on by default when no Verse kill switch is set, while VITE_DISABLE_VERSE / VITE_VERSE_DISABLED still hard-disable them. The live in-app wiring is present: pylon-stats feeds the running scene through the chat-world subscription/reducer/view path, live Pylons render as crystals, and stale payment beams age out instead of implying current activity. The owner-signed green transition is applied for this scoped presentational scene; it does not broaden multiplayer, payment/growth visualization, onboarding, spend, payout, or settlement claims.',
        unsafeCopy:
          'Do not say the 3D agent world is green, production-default-on for all users, a finished shipped feature, a walkable or multiplayer world (see world.multiplayer_agent_world.v1), or that the scene shows anything other than evidence-bound live Pylon state.',
        evidenceRefs: [
          'docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md',
          'https://github.com/OpenAgentsInc/openagents/pull/5742',
          'https://github.com/OpenAgentsInc/openagents/pull/5743',
          'https://github.com/OpenAgentsInc/openagents/issues/5730',
          'https://github.com/OpenAgentsInc/openagents/issues/5735',
          'https://github.com/OpenAgentsInc/openagents/issues/5736',
          'https://github.com/OpenAgentsInc/openagents/issues/7030',
          'apps/autopilot-desktop/src/shared/chat-world-flags.ts',
          'apps/autopilot-desktop/src/shared/chat-world-scene.ts',
          'apps/autopilot-desktop/src/ui/chat-world-subscriptions.ts',
          'apps/autopilot-desktop/src/ui/subscriptions.ts',
          'apps/autopilot-desktop/src/ui/update.ts',
          'apps/autopilot-desktop/src/ui/view.ts',
          'apps/autopilot-desktop/tests/chat-world-scene.test.ts',
          'apps/autopilot-desktop/tests/chat-world-subscriptions.test.ts',
          'apps/autopilot-desktop/tests/verse-launch-checklist.test.ts',
          'apps/autopilot-desktop/tests/verse-toggle.test.ts',
          'https://openagents.com/api/public/pylon-stats',
          'promise:repo.open_source_code_map.v1',
        ],
        blockerRefs: [],
        verification:
          'Green after the owner-signed #7030 transition: chatWorldBuildFlags defaults CHAT_WORLD_SCENE and CHAT_WORLD_PAYMENTS on under the Verse launch default, the hard Verse kill switches force both off, pylon-stats flows through chat-world-subscriptions.ts -> GotChatWorldScene -> modelChatWorldScene -> verseSceneVisualization, and TickedChatWorldPaymentParticles idle-expires evidence-bound beams. Focused proof: bun test tests/verse-launch-checklist.test.ts tests/verse-toggle.test.ts tests/chat-world-subscriptions.test.ts tests/chat-world-scene.test.ts in apps/autopilot-desktop. This verifies the scoped scene projection only and does not green multiplayer, payment/growth visualization, onboarding, spend, payout, or settlement claims.',
        authorityBoundary:
          'The agent world scene is a presentational projection of already-public Pylon state. It grants no runtime mutation, no spend, no settlement, no payout, and no authority over the data it visualizes, and it makes no separate Pylon earning, payment, multiplayer, or onboarding claim green.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.bitcoin_payment_visualization.v1',
        productArea: 'Autopilot desktop',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'Real Bitcoin settlements are visualized in the agent world as gold particles flying agent-to-agent, each particle bound to a real settlement receipt and clickable to its evidence.',
        safeCopy:
          'The Bitcoin payment visualization is wired into the live running agent-world scene. Source-level gates now resolve CHAT_WORLD_PAYMENTS on under the Verse launch default unless the Verse kill switch is set. It is evidence-bound by construction: a gold payment particle is only emitted for a real_bitcoin_moved or settlement_recorded event that proves realBitcoinMoved:true and carries at least one sourceRef, and the mappers refuse to emit a particle with no sourceRef or simulated settlement evidence. This is NOT a green/default-on production claim; owner review and a shipped-channel visual receipt remain pending.',
        unsafeCopy:
          'Do not say payment particles are green, production-default-on for all users, that they show simulated or unbacked payments, or that visualizing a settlement implies any new earning, payout, or settlement authority.',
        evidenceRefs: [
          'docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md',
          'https://github.com/OpenAgentsInc/openagents/pull/5743',
          'https://github.com/OpenAgentsInc/openagents/issues/5730',
          'https://github.com/OpenAgentsInc/openagents/issues/5736',
          'https://github.com/OpenAgentsInc/openagents/issues/7030',
          'apps/autopilot-desktop/src/shared/chat-world-flags.ts',
          'apps/autopilot-desktop/src/shared/chat-world-scene.ts',
          'apps/autopilot-desktop/src/shared/chat-world-visualization.ts',
          'apps/autopilot-desktop/src/ui/chat-world-subscriptions.ts',
          'apps/autopilot-desktop/src/ui/view.ts',
          'apps/autopilot-desktop/tests/chat-world-scene.test.ts',
          'apps/autopilot-desktop/tests/chat-world-subscriptions.test.ts',
          'apps/autopilot-desktop/tests/chat-world-visualization.test.ts',
          'apps/autopilot-desktop/tests/verse-launch-checklist.test.ts',
          'apps/autopilot-desktop/tests/verse-toggle.test.ts',
          'https://openagents.com/api/public/activity-timeline?limit=8',
          'promise:autopilot.agent_world_scene.v1',
          'promise:training.decentralized_training_launch.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.payment_visualization_owner_review_green_pending',
        ],
        verification:
          'Yellow covers the source-level default gate and live wiring: chatWorldBuildFlags defaults CHAT_WORLD_PAYMENTS on under the Verse launch default unless the hard kill switch is set; chat-world-subscriptions.ts backfills and streams public activity-timeline events into GotChatWorldPaymentParticle; update.ts stores the bounded active set; view.ts composes modelChatWorldParticles through withChatWorldPaymentLayer; and chat-world-visualization.ts turns each accepted particle into clickable beam/burst endpoint evidence. PAYMENT_EVENT_KINDS remains exactly {real_bitcoin_moved, settlement_recorded}; realBitcoinMoved:true and sourceRefs are required, so an event with no sourceRef or simulated settlement evidence returns null. Green requires owner review of the production default-on scope, a shipped-channel visual receipt, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Visualizing a Bitcoin settlement grants no payment authority, no spend, no payout, and no settlement authority. The particle is a clickable projection of an already-public receipt or event; it never moves money and never asserts a settlement that the underlying receipt does not.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.pylon_growth_visualization.v1',
        productArea: 'Autopilot desktop',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'A Pylon visibly grows in the agent world — crystal scale, facets, and brightness step up by tier — as it earns cumulative settled sats.',
        safeCopy:
          'Pylon growth tiers are merged and live-wired through the chat-world scene: public per-Pylon cumulative settled sats map to a monotonic growth tier that the scene adapter turns into crystal scale/status brightness and facet metadata, with tier 0 an honest still crystal for a Pylon that has not settled any earnings yet. Source-level gates now resolve the backing scene on under the Verse launch default unless the Verse kill switch is set. This is NOT a green/default-on production claim; owner review and a shipped-channel visual receipt remain pending.',
        unsafeCopy:
          'Do not say Pylon growth is green, production-default-on for all users, that growth reflects anything other than real settled sats, or that a larger crystal implies any new earning, payout, or settlement capability.',
        evidenceRefs: [
          'docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md',
          'https://github.com/OpenAgentsInc/openagents/pull/5743',
          'https://github.com/OpenAgentsInc/openagents/issues/5730',
          'https://github.com/OpenAgentsInc/openagents/issues/5737',
          'https://github.com/OpenAgentsInc/openagents/issues/6868',
          'https://github.com/OpenAgentsInc/openagents/issues/7030',
          'apps/autopilot-desktop/src/shared/chat-world-flags.ts',
          'apps/autopilot-desktop/src/shared/chat-world-scene.ts',
          'apps/autopilot-desktop/src/shared/chat-world-scene.test.ts',
          'apps/autopilot-desktop/src/ui/pylon-network-visualization.ts',
          'apps/autopilot-desktop/tests/pylon-network-visualization.test.ts',
          'apps/autopilot-desktop/tests/verse-launch-checklist.test.ts',
          'apps/autopilot-desktop/tests/verse-toggle.test.ts',
          'promise:autopilot.agent_world_scene.v1',
          'promise:autopilot.bitcoin_payment_visualization.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.pylon_growth_owner_review_green_pending',
        ],
        verification:
          'Yellow now covers the source-level default gate and live scene wiring: CHAT_WORLD_SCENE defaults on under the Verse launch default unless the hard kill switch is set; PublicRecentPylon preserves public cumulativeSettledSats when present; projectChatWorldPylonScene computes each node growth descriptor from that value; liveChatWorldNetworkScene carries the descriptor into PylonNetworkNode; and pylonNetworkVisualizationOptions maps tiers onto the pinned three-effect renderer knobs (larger role geometry, brighter status, and facet/sats detail). Tier 0 remains the 0-sat still crystal. Green still requires owner review of the production default-on scope, a shipped-channel visual receipt, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Pylon growth visualization is a presentational projection of already-public settled-sats data. It grants no earning, spend, payout, or settlement authority, and a crystal tier never asserts earnings the underlying settlement receipts do not.',
      },
      {
        ...basePromiseFields,
        promiseId: 'autopilot.agent_character_creation.v1',
        productArea: 'Autopilot desktop',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'Autopilot Desktop has source-level character-creation onboarding: a new Pylon warps into the scene, spawns an agent identity, projects customization progress, posts one bounded Forum introduction when agent posting credentials exist, and runs read-only work search after onboarding.',
        safeCopy:
          'Agent character creation is yellow on source-level Autopilot Desktop evidence for issues #5738/#6861. The character-creation flag maps real Pylon online, agent registration, identity/customize, Forum-intro receipt, and read-only work-search receipt signals into the warp-in spawn + customize onboarding beats. The headless proof/smoke harnesses exercise fresh registration, presence, payout-readiness, assignment polling, an automated Forum self-introduction, and read-only work search against a mock Worker, while unit tests cover the projection, Forum intro, and work-search paths. This is not a green/default-on live-production claim: green still needs owner-accepted real-user receipts for the shipped onboarding flow, a permissioned Forum intro, and work-search evidence.',
        unsafeCopy:
          'Do not say character-creation onboarding is green, default-on for every user, or broadly proven in production. Do not say it posts to the Forum without persisted agent posting credentials, and do not say work search bids, accepts paid work, spends money, or settles anything automatically.',
        evidenceRefs: [
          'docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5730',
          'https://github.com/OpenAgentsInc/openagents/issues/5738',
          'https://github.com/OpenAgentsInc/openagents/issues/6861',
          'apps/autopilot-desktop/src/shared/character-creation-onboarding.ts',
          'apps/autopilot-desktop/src/shared/character-creation-onboarding.test.ts',
          'apps/autopilot-desktop/src/shared/onboarding-status.ts',
          'apps/autopilot-desktop/tests/onboarding-status.test.ts',
          'apps/autopilot-desktop/src/bun/forum-intro.ts',
          'apps/autopilot-desktop/tests/forum-intro.test.ts',
          'apps/autopilot-desktop/src/bun/forum-work-search.ts',
          'apps/autopilot-desktop/tests/forum-work-search.test.ts',
          'apps/autopilot-desktop/scripts/auto-onboarding-headless-proof.ts',
          'apps/autopilot-desktop/scripts/auto-onboarding-e2e-smoke.ts',
          'promise:autopilot.agent_world_scene.v1',
          'promise:autopilot.desktop_gui_client.v1',
          'promise:labor.forum_work_requests.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.agent_character_creation_live_new_user_receipt_missing',
          'blocker.product_promises.agent_character_creation_permissioned_forum_intro_receipt_missing',
          'blocker.product_promises.agent_character_creation_green_owner_review_pending',
        ],
        verification:
          'Yellow: repository evidence now covers the source-level character-creation onboarding path. projectCharacterCreationOnboarding turns real onboarding-status steps and chat-world Pylon scene data into Pylon online, agent-warp-in, customize, Forum-intro, and work-search beats; postForumIntroduction posts one idempotent, rate-capped Forum introduction only after a persisted agent credential exists; searchForumWork performs read-only typed work-request discovery; auto-onboarding proof/smoke scripts drive the chain against a mock Worker. Green still requires a real new-user/default-on or owner-accepted canary receipt, a permissioned automated Forum-intro receipt, and owner-reviewed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Character-creation onboarding grants no spend, payout, settlement, paid-work acceptance, or moderation authority. Forum introduction stays bound to existing agent posting credentials, idempotency, and rate limits. Work search is read-only discovery over existing work-request projections and never bids, quotes, accepts, commits, spends, or settles.',
      },
      {
        ...basePromiseFields,
        promiseId: 'world.multiplayer_agent_world.v1',
        productArea: 'agent world',
        audience: ['agent', 'user', 'operator'],
        state: 'planned',
        claim:
          'OpenAgents is a walkable, inhabited multiplayer world where agents and humans share one space — moving avatars, proximity chat, and focus beams — backed by a realtime multiplayer database.',
        safeCopy:
          'The walkable multiplayer agent world remains planned roadmap scope (P4, issue #5739). Partial source-level plumbing now exists: packages/world-contract models agent_avatar/avatar_position rows, apps/openagents-world applies join_region/set_avatar_position commands and subscription projections, and packages/world-client mirrors snapshots/deltas into a WorldReadModel with avatar position projection tests. This is not yet a shipped walkable multiplayer world: third-person WASD navigation over the scene and live rendering of other avatars from production presence deltas remain blocked.',
        unsafeCopy:
          'Do not say OpenAgents has a live walkable or multiplayer world, that agents and humans can move around a shared space today, or that the Cloudflare world client is production-shipped, default-on, or inhabited by live users.',
        evidenceRefs: [
          'docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md',
          'https://github.com/OpenAgentsInc/openagents/issues/5730',
          'https://github.com/OpenAgentsInc/openagents/issues/5739',
          'https://github.com/OpenAgentsInc/openagents/issues/6859',
          'apps/openagents-world/src/commands.ts',
          'apps/openagents-world/src/commands.test.ts',
          'apps/openagents-world/src/protocol.ts',
          'apps/openagents-world/src/protocol.test.ts',
          'apps/openagents-world/src/subscriptions.ts',
          'apps/openagents-world/src/subscriptions.test.ts',
          'packages/world-client/src/index.ts',
          'packages/world-client/src/index.test.ts',
          'packages/world-contract/src/index.ts',
          'packages/world-contract/src/index.test.ts',
          'promise:autopilot.agent_world_scene.v1',
          'promise:autopilot.agent_character_creation.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.multiplayer_world_not_built',
          'blocker.product_promises.multiplayer_world_walkable_navigation_unbuilt',
          'blocker.product_promises.multiplayer_world_live_avatar_rendering_unproven',
        ],
        verification:
          'Planned: the P4 walkable multiplayer world is partially de-risked at the source/test layer by the Cloudflare openagents-world service, shared world-contract schemas, and world-client WorldReadModel snapshot/delta projection for agent_avatar/avatar_position rows. Green still requires a built and tested WASD/third-person navigation path over the shipped scene, production live-presence wiring that renders other avatars from real deltas, proximity chat/focus beam evidence where claimed, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'A multiplayer world, when built, grants no spend, payout, settlement, or moderation authority. Every moving thing stays bound to a real receipt or event, and shared presence asserts no authority over the agents, humans, or balances it depicts.',
      },
      {
        ...basePromiseFields,
        promiseId: 'business.intake_quick_win_offering.v1',
        productArea: 'business',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'yellow',
        claim:
          "OpenAgents Business is a buyable offering: a customer (or the customer's agent) reads the published offering menu, runs an intake, and lands on a fast quick win plus a picture of the ongoing Autopilot relationship.",
        safeCopy:
          'The OpenAgents Business intake is live: the public offering menu (docs/business/2026-06-20-openagents-business-intake-spec.md) is grounded one-for-one in this product-promise registry, and the /business signup route accepts a real intake and records it. The /business page also carries a bounded conversational intake (POST /api/public/business-intake-chat) that runs the published interview and drafts the intake spec into the same signup form; the form remains the single submit authority. A public rate card now shows operator-assisted Quick Win, Fleet Sprint, On Autopilot Retainer, and QA Swarm package bands with fixed-scope receipt plans. Quick-win delivery and the move onto Autopilot remain operator-assisted today, not a one-click self-serve product.',
        unsafeCopy:
          'Do not say OpenAgents Business is a finished self-serve product, that any offering on the menu is delivered automatically without an operator, or that the intake commits OpenAgents to anything beyond what each backing promise record actually supports.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/business/2026-06-20-business-offering-promise-coverage.md',
          'apps/openagents.com/workers/api/src/business-signup-routes.ts',
          'apps/openagents.com/workers/api/src/business-signup-routes.test.ts',
          'apps/openagents.com/workers/api/src/business-quick-win-receipt.ts',
          'apps/openagents.com/workers/api/src/business-quick-win-receipt.test.ts',
          'apps/openagents.com/workers/api/src/business-already-sold-engagement-receipt.ts',
          'apps/openagents.com/workers/api/src/business-already-sold-engagement-receipt.test.ts',
          'apps/openagents.com/workers/api/src/business-already-sold-engagement-receipt-routes.ts',
          'apps/openagents.com/workers/api/src/business-already-sold-engagement-receipt-routes.test.ts',
          'apps/openagents.com/workers/api/src/business-case-study-engine.ts',
          'apps/openagents.com/workers/api/src/business-case-study-engine.test.ts',
          'apps/openagents.com/workers/api/src/business-case-study-engine-routes.ts',
          'apps/openagents.com/workers/api/src/business-intake-chat-routes.ts',
          'apps/openagents.com/workers/api/src/business-intake-chat-routes.test.ts',
          'apps/openagents.com/apps/web/src/page/business-intake-chat-controller.ts',
          'docs/business/2026-07-02-business-page-redesign-and-khala-intake-chat.md',
          'docs/launch/vertex-fleet/business.intake_quick_win_offering.v1.md',
          'apps/openagents.com/apps/web/src/page/business.ts',
          'apps/openagents.com/apps/web/src/business-route.test.ts',
          'route:/api/public/business/already-sold-engagement-receipts?view=paid-business-receipts',
          'route:/api/public/business/case-studies?view=published-case-studies',
          'https://openagents.com/business',
          'https://openagents.com/api/public/product-promises',
          'promise:repo.open_source_code_map.v1',
          'promise:promises.registry.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.business_quick_win_self_serve_delivery_missing',
          'blocker.product_promises.business_first_paid_quick_win_receipt_missing',
        ],
        verification:
          'Yellow is limited to what is shipped: the offering menu document is grounded in the live registry (each offering maps to a backing promiseId in docs/business/2026-06-20-business-offering-promise-coverage.md), the /business intake route accepts and records a real signup (business-signup-routes.ts, business-signup-routes.test.ts), and the /business page publishes package price bands with fixed-scope receipt plans. True today: a customer/agent can read the menu and rate card, run the interview, submit an intake, and read the public-safe already-sold engagement receipt projection for opaque paid business receipts. That BF-2.5/AW-0 A0.1 projection records buyer payment only; it does not prove delivery completion, accepted outcome, self-serve operation, payout, settlement, or customer identity. Green still requires a self-serve quick-win delivery loop and at least one dereferenceable first paid business quick-win receipt (intake -> delivery -> accepted outcome -> receipt), with a receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The business intake is an offering menu plus a signup capture. It grants no automatic delivery, spend, payout, or settlement authority, makes no separate offering green, and never promises beyond the state of each backing promise record it points at.',
      },
      {
        ...basePromiseFields,
        promiseId: 'business.coding_quick_win.v1',
        productArea: 'business',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'yellow',
        claim:
          "A business customer can buy a coding quick win: a written objective is taken into a repository, the customer's verification command is run, and a reviewable change is handed back with verification evidence.",
        safeCopy:
          'Coding quick wins have a self-serve evidence pipeline: the public route accepts scoped intake, provisioning, runtime invocation, delivery, acceptance, and payment evidence and returns a machine-checkable receipt. The coding-agent runtime and negotiated forum labor market remain the execution backing. The promise is still yellow because the first real paid customer receipt plus owner sign-off has not been substantiated.',
        unsafeCopy:
          'Do not say coding quick wins have a substantiated first paid customer receipt, that arbitrary large projects are guaranteed, or that delivery happens without a human review gate and an accepted-outcome check.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/launch/2026-06-19-coding-agent-live-verification.md',
          'docs/launch/gemini-fleet/business.coding_quick_win.v1.md',
          'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
          'apps/openagents.com/workers/api/src/coding-quick-win-pipeline.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-pipeline.test.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-pipeline-routes.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-pipeline-routes.test.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-claim-upgrade.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-claim-upgrade.test.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-receipt-public-routes.ts',
          'apps/openagents.com/workers/api/src/coding-quick-win-receipt-public-routes.test.ts',
          'route:/api/public/business/coding-quick-win-receipts?view=paid-delivery-claims',
          'promise:pylon.local_claude_agent_bridge.v1',
          'promise:autopilot.codex_probe_pylon_successor.v1',
          'promise:pylon.cli_tui_probe_background.v1',
          'promise:pylon.agent_steerable_cli.v1',
          'promise:labor.forum_work_requests.v1',
          'promise:labor.nostr_negotiation_market.v1',
          'promise:business.intake_quick_win_offering.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.business_coding_quick_win_paid_receipt_missing',
        ],
        verification:
          'Yellow inherits its execution evidence from the green coding-agent records (local single-task exec re-verified 2026-06-19, docs/launch/2026-06-19-coding-agent-live-verification.md) and the green negotiated labor market (#4777 settled labor job). The self-serve evidence route POST /api/public/business/coding-quick-win-pipeline accepts scope -> provisioning -> runtime invocation -> delivery -> acceptance -> payment events and returns a machine-checkable BusinessQuickWinReceipt without moving money. True today: the route can validate a complete coding quick-win evidence chain and build the receipt shape. Green still requires a dereferenceable first real paid customer receipt plus owner sign-off projected through GET /api/public/business/coding-quick-win-receipts?view=paid-delivery-claims, per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          "A coding quick win delivers a reviewable change with evidence under a human review gate. It grants no auto-merge, deploy, spend, payout, or settlement authority, and accepting an outcome is the customer's decision, not an automatic one.",
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.free_tier_taste.v1',
        productArea: 'inference gateway',
        audience: ['agent', 'user', 'operator', 'public'],
        state: 'yellow',
        claim:
          'A customer or agent can taste OpenAgents inference for free: free-eligible open/hosted models run through the gateway without spending credits, under a Sybil-resistant per-owner free allowance.',
        safeCopy:
          'Free inference is live: the deployed OpenAI-compatible gateway serves free-eligible models (Gemini Flash today) without a credit decrement, metered against a Sybil-resistant per-verified-owner free pool (a small unclaimed taste, a larger claimed pool). It is a free taste to start, deliberately scoped: it is not unlimited free inference and the paid credits business it sits on top of is not yet collectable end-to-end.',
        unsafeCopy:
          'Do not say OpenAgents offers unlimited free inference, that every model is free, that the free taste implies a live paid credits product, or that an unclaimed account gets the full pool.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/inference/2026-06-19-gateway-gemini-live-verification.md',
          'apps/openagents.com/workers/api/src/inference/inference-free-allowance.ts',
          'apps/openagents.com/workers/api/src/inference/inference-free-allowance.test.ts',
          'apps/openagents.com/workers/api/src/inference/model-catalog.ts',
          'promise:inference.gateway_credits_business.v1',
          'promise:inference.fireworks_open_model_provider.v1',
          'promise:api.hosted_gemini.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.inference_free_taste_paid_upgrade_not_collectable',
        ],
        verification:
          'Yellow is the live, tested free-allowance system: withFreeAllowance eats the priced charge for a free-eligible model under the per-owner cap (idempotent per request, no credit decrement, no referral accrual) and falls through to normal metering over the cap or for non-free-eligible models (inference-free-allowance.ts, inference-free-allowance.test.ts); the gateway free path is verified live 2026-06-19. True today: free inference works as a Sybil-gated taste. It stays yellow because it is a bounded taste on top of a credits business whose paid upgrade is not collectable end-to-end (see inference.gateway_credits_business.v1); green for a standalone sellable free-to-paid inference product needs the collectable paid loop plus a receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          "The free taste grants bounded, metered free inference only. It grants no unlimited usage, no spend authority on the customer's behalf, and no paid-product, payout, or settlement authority, and it does not make the inference credits business green.",
      },
      {
        ...basePromiseFields,
        promiseId: 'inference.batch_processing_jobs.v1',
        productArea: 'inference gateway',
        audience: ['agent', 'user', 'operator'],
        state: 'planned',
        claim:
          'A business customer can hand OpenAgents a batch of items (summaries, classifications, extractions) and get the processed results back as a buyable, metered inference job.',
        safeCopy:
          'Batch inference processing as a buyable business offering is planned roadmap scope. The underlying single-request gateway is live, and a receipt-first batch-job surface now has tested submit, detached processing, authenticated result retrieval, and closeout receipt plumbing. The promise is not green until a real paid batch receipt and owner-approved claim transition prove the billable offering.',
        unsafeCopy:
          'Do not say OpenAgents has a live batch-processing product, that customers can submit datasets for metered batch inference today, or that batch jobs return billed receipts.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/inference/README.md',
          'docs/inference/2026-06-19-inference-gateway-business.md',
          'docs/launch/gemini-fleet/inference.batch_processing_jobs.v1.md',
          'promise:inference.gateway_credits_business.v1',
          'promise:inference.free_tier_taste.v1',
          'promise:business.intake_quick_win_offering.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.inference_batch_job_surface_unbuilt',
        ],
        verification:
          'Planned: the per-request gateway (POST /v1/chat/completions) and free taste are live, and route tests now cover the batch job submit/status/results/receipt path. This remains planned because green requires a dereferenceable first real paid batch-job receipt, billing evidence tied to inference.gateway_credits_business.v1, and a receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          "A batch inference job, when built, grants no spend authority beyond the customer's funded balance, no payout, and no settlement authority, and processing results never asserts an outcome the underlying model run did not produce.",
      },
      {
        ...basePromiseFields,
        promiseId: 'business.ecommerce_workspace_pack.v1',
        productArea: 'business',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'A business customer in e-commerce can start from a prefilled workspace seeded for inventory-aware ad campaigns, run through the Signal -> Triage -> Build -> Validate -> Release -> Document -> Monitor -> Deploy pipeline with a human-review gate before anything publishes or spends.',
        safeCopy:
          'The e-commerce vertical pack is shipped as an operator tool: the forge.template.ecommerce.inventory_campaign.v1 prefilled-workspace template seeds stages, starter workflows, and memory for inventory-aware ad campaigns (prefilled-workspace-vertical-templates.ts). It is delivered as a guided, operator-assisted workspace, not a one-click self-serve product, and no campaign auto-publishes or auto-spends.',
        unsafeCopy:
          'Do not say the e-commerce pack is a finished self-serve product, that it launches or spends on ad campaigns automatically, or that any campaign publishes without a human-review gate.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md#e-commerce-template',
          'docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md',
          'apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts',
          'apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5099',
          'promise:autopilot.all_in_one_business_system.v1',
          'promise:business.intake_quick_win_offering.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing',
        ],
        verification:
          'Yellow is the shipped, tested prefilled e-commerce template (forge.template.ecommerce.inventory_campaign.v1) that seeds an inventory-aware-ad-campaign workspace through the prefilled-workspace routes (prefilled-workspace-vertical-templates.ts, .test.ts). True today: an operator can stand up the seeded workspace and run a first work item, drafted under a review gate. Green requires a self-serve vertical pack and a dereferenceable first paid e-commerce work-item delivery receipt, with a receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The e-commerce pack seeds a workspace and pipeline. It grants no ad-account, publish, or spend authority; campaigns are drafted and never auto-published or auto-funded, and every stage keeps a human-review gate.',
      },
      {
        ...basePromiseFields,
        promiseId: 'business.legal_workspace_pack.v1',
        productArea: 'business',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'A business customer in legal can start from a prefilled, review-gated forms/intake copilot workspace (e.g. NDA intake packets) that drafts work for human review and never gives legal advice.',
        safeCopy:
          'The legal vertical pack is shipped as an operator tool: the forge.template.legal.forms_intake_copilot.v1 prefilled-workspace template seeds a review-gated forms/intake copilot workspace (prefilled-workspace-vertical-templates.ts). It is explicitly review-gated, gives no legal advice, and is delivered as a guided, operator-assisted design-partner workspace, not a self-serve product.',
        unsafeCopy:
          'Do not say the legal pack gives legal advice, that it is a finished self-serve product, that any document is filed or sent automatically, or that it operates without an attorney/human-review gate.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md#legal-template',
          'docs/blitz/forge/2026-06-16-legal-prefilled-workspace.md',
          'apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts',
          'apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5100',
          'promise:autopilot.all_in_one_business_system.v1',
          'promise:business.intake_quick_win_offering.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.legal_pack_self_serve_missing',
          'blocker.product_promises.legal_pack_first_paid_delivery_receipt_missing',
        ],
        verification:
          'Yellow is the shipped, tested prefilled legal template (forge.template.legal.forms_intake_copilot.v1, design_partner.legal.forms_intake_copilot.v1) that seeds a review-gated forms/intake copilot workspace (prefilled-workspace-vertical-templates.ts, .test.ts). True today: an operator can stand up the seeded workspace and run a first intake packet, drafted under a review gate with no legal advice. Green requires a self-serve vertical pack and a dereferenceable first paid legal work-item delivery receipt, with a receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The legal pack seeds a review-gated drafting workspace. It is not a law firm, gives no legal advice, and grants no filing, sending, or spend authority; every output is drafted for human/attorney review before anything leaves the workspace.',
      },
      {
        ...basePromiseFields,
        promiseId: 'business.marketing_agency_workspace_pack.v1',
        productArea: 'business',
        audience: ['agent', 'user', 'operator'],
        state: 'yellow',
        claim:
          'A business customer running a marketing agency can start from a prefilled white-label workspace that drafts landing pages and email sequences through the review-gated Autopilot pipeline.',
        safeCopy:
          'The marketing-agency vertical pack is shipped as an operator tool: the forge.template.marketing_agency.white_label_launch.v1 prefilled-workspace template seeds a white-label landing-page + email workspace (prefilled-workspace-vertical-templates.ts), and the Autopilot Sites surfaces (site build/host, custom hostnames, native email sequences) are partial/flag-gated. It is delivered as a guided, operator-assisted workspace, not a self-serve product, and nothing publishes or sends without a review gate.',
        unsafeCopy:
          'Do not say the marketing-agency pack is a finished self-serve product, that pages publish or emails send automatically, or that the white-label flow is fully self-serve without operator assistance.',
        evidenceRefs: [
          'docs/business/2026-06-20-openagents-business-intake-spec.md',
          'docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md#marketing-agency-template',
          'docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md',
          'apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts',
          'apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.test.ts',
          'apps/openagents.com/workers/api/src/marketing-agency-delivery-receipt.ts',
          'apps/openagents.com/workers/api/src/marketing-agency-delivery-receipt.test.ts',
          'apps/openagents.com/workers/api/src/marketing-agency-claim-upgrade.ts',
          'apps/openagents.com/workers/api/src/marketing-agency-receipt-public-routes.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/5102',
          'promise:autopilot_sites.site_build_and_host.v1',
          'promise:autopilot_sites.native_email_sequences.v1',
          'promise:business.intake_quick_win_offering.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.marketing_agency_pack_self_serve_missing',
          'blocker.product_promises.marketing_agency_pack_first_paid_delivery_receipt_missing',
        ],
        verification:
          'Yellow is the shipped, tested prefilled marketing-agency template (forge.template.marketing_agency.white_label_launch.v1) seeding a white-label landing-page + email workspace (prefilled-workspace-vertical-templates.ts, .test.ts), composed with the yellow Autopilot Sites records (site build/host, custom hostnames, native email sequences). True today: an operator can stand up the seeded workspace and draft pages/emails under a review gate. Green requires a self-serve vertical pack with proven send/publish deliverability and a dereferenceable first paid agency work-item delivery receipt, with a receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The marketing-agency pack seeds a drafting workspace and composes the Sites surfaces. It grants no publish, send, or spend authority beyond what those backing Sites promises support; pages and emails are drafted under a review gate and never auto-published or auto-sent.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.desktop_codex_wrapper.v1',
        productArea: 'Khala Code',
        audience: ['user', 'agent', 'operator', 'public'],
        state: 'yellow',
        claim:
          'Khala Code is the OpenAgents desktop coding app: a wrapper around the user’s own local Codex install, with Khala swarm/fleet coordination, a Unified Inbox, and exact token accounting layered around it.',
        safeCopy:
          'The Khala Code desktop app exists on main (clients/khala-code-desktop, Electrobun + web preview) and is Codex-required by positioning: the default chat, thread, slash-command, approval, MCP, settings, and headless paths run through the user’s own codex app-server install (docs/khala-code/2026-07-01-codex-required-product-positioning.md). Parity with Codex is enforced mechanically against a pinned reference commit (parity contract + gap matrix + fixture suites + skip-safe live smokes), and the Khala layer adds fleet delegation through the deterministic khala.fleet.delegate program with isolated worker homes and exact token_usage_events accounting. Episode 246 adds owner full-time dogfood evidence: the owner runs Khala Code full-screen as his only coding harness and fixes Khala Code with Khala Code, with stated UX expectations landing in the enforced behavior-contract registry (khala_code.ux_behavior_contracts.v1). The public /code/download install-truth page routes users to the Codex prerequisite, the public npm khala CLI, and the source-build path while marking the desktop DMG as a pending public artifact; its counter reads exact khala_code_download_events rows or empty counts only. The desktop Settings surface also offers an explicit opt-in Run evidence action backed by POST /api/public/khala-code/outside-user-runs and GET /api/public/khala-code/outside-user-runs/{receiptRef}; it posts only app version, platform, architecture, distribution channel, and bounded Codex/Pylon readiness, with no paths, prompts, tokens, logs, account ids, machine ids, or background phone-home. Yellow because there is no public release artifact/installer and no real outside-user run receipt row reviewed yet; this is the Episode 245 product-identity record.',
        unsafeCopy:
          'Do not claim Khala Code is downloadable, installed by outside users, or usable without a working Codex install and login. Do not describe the free/paid plan economics as live — those are the separate planned khala_code.* records.',
        evidenceRefs: [
          'clients/khala-code-desktop/README.md',
          'docs/khala-code/2026-07-01-codex-required-product-positioning.md',
          'docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md',
          'docs/khala-code/2026-07-01-codex-parity-contract.md',
          'docs/khala-code/2026-06-30-khala-code-fleet-management-spec.md',
          'docs/transcripts/245.md',
          'docs/transcripts/246.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'apps/openagents.com/apps/web/src/page/khalaCodeDownload.ts',
          'apps/openagents.com/apps/web/src/khala-code-download-route.test.ts',
          'apps/openagents.com/workers/api/src/khala-code-download-counts-routes.ts',
          'apps/openagents.com/workers/api/src/khala-code-download-counts-routes.test.ts',
          'apps/openagents.com/workers/api/migrations/0288_khala_code_download_events.sql',
          'apps/openagents.com/workers/api/src/khala-code-outside-user-run-routes.ts',
          'apps/openagents.com/workers/api/src/khala-code-outside-user-run-routes.test.ts',
          'apps/openagents.com/workers/api/migrations/0289_khala_code_outside_user_run_receipts.sql',
          'clients/khala-code-desktop/src/ui/run-evidence-panel.ts',
          'docs/khala-code/2026-07-04-outside-user-run-receipt-template.md',
          'route:/code/download',
          'route:/api/public/khala-code/download-counts',
          'route:/api/public/khala-code/outside-user-runs',
          'route:/api/public/khala-code/outside-user-runs/:receiptRef',
          'promise:khala.own_capacity_codex_delegation.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.khala_code_public_release_artifact_missing',
          'blocker.product_promises.khala_code_outside_user_evidence_missing',
        ],
        verification:
          'Fixture parity suites and the skip-safe live smokes (smoke:codex-parity-live, smoke:part2-ui) pass from a clean checkout; the delegation path proves exact token rows per the repo runbook. The /code/download route and download-counts API tests prove copy-gated install truth and exact-row-or-empty public counters. The outside-user run receipt tests prove explicit-action intake, public-safe readback, idempotency, private-material rejection, and no phone-home-by-default UI wiring. Green requires a public release artifact plus a real outside-user run receipt and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The wrapper grants no provider account, capacity resale, public installer, or settlement authority. Connect flows never touch the default ~/.codex home; fleet delegation stays owner-scoped under the dispatch gate and advertised-capacity invariants. Public download counters must be exact rows from khala_code_download_events or empty with blocker refs. Outside-user run receipts are opt-in evidence only and may contain only app version, platform, architecture, distribution channel, and bounded harness readiness; they do not replace signed-release receipts or authorize promise-state movement.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.free_paid_plans.v1',
        productArea: 'Khala Code',
        audience: ['user', 'customer', 'public'],
        state: 'planned',
        claim:
          'Khala Code offers a free plan (pay with data: disclosed, redacted trace capture) and a paid plan (private data: capture opt-out).',
        safeCopy:
          'Episode 245 launches Khala Code with a two-plan structure on the whiteboard: Free (pay w/ data) and Paid (private data). The plan structure now has source+test evidence (#7966/#8248): the public plan catalog GET /api/public/khala-code/plans projects the honest two-plan structure with real purchasability state, GET /v1/khala-code/plan resolves a caller’s current plan server-side from the privacy-entitlement seam (never fabricating a plan), and the Khala Code desktop settings surface renders the plan cards and current plan through host RPCs. The record stays planned: the paid plan is NOT purchasable in the shipped default — POST /v1/khala-code/plans/purchases is flag-gated by KHALA_CODE_PAID_PLANS_ENABLED, default OFF, fail-closed; when armed it creates a Stripe Checkout or Spark/MPP Lightning payment requirement and grants the existing paid-privacy entitlement only after settled payment — and free-plan desktop capture remains not live (khala_code.free_plan_trace_capture.v1).',
        unsafeCopy:
          'Do not claim Khala Code plans are selectable or purchasable, that free-plan capture is live on the desktop wrapper path, or that buying the paid plan is currently possible.',
        evidenceRefs: [
          'docs/transcripts/245.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'apps/openagents.com/workers/api/src/inference/khala-code-plan-catalog.ts',
          'apps/openagents.com/workers/api/src/inference/khala-code-plan-routes.ts',
          'apps/openagents.com/workers/api/src/inference/khala-code-plan-routes.test.ts',
          'clients/khala-code-desktop/src/ui/plans-panel.ts',
          'clients/khala-code-desktop/tests/plans-panel.test.ts',
          'route:/api/public/khala-code/plans',
          'promise:privacy.khala_paid_capture_optout.v1',
          'promise:data.free_tier_capture_disclosure.v1',
          'promise:khala_code.desktop_codex_wrapper.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.khala_code_paid_plan_not_purchasable',
        ],
        verification:
          'Run bun run --cwd apps/openagents.com/workers/api test -- src/inference/khala-code-plan-catalog.test.ts src/inference/khala-code-plan-routes.test.ts plus the khala-code-desktop plans-panel and RPC-handler tests: the catalog must report the real fail-closed purchasability state, the plan-status read must never fabricate a plan (paid only on a real entitlement row; entitlement read errors 503), the unarmed purchase route must fail closed with khala_code_paid_plans_not_enabled while granting nothing, and armed test-mode payment settlement must produce the existing dereferenceable paid-privacy receipt idempotently. Green still requires a real collectable paid-plan purchase receipt in production (owner arms KHALA_CODE_PAID_PLANS_ENABLED, Stripe price / Lightning sats, and live credentials), plan-scoped capture behavior once free-plan capture exists, and owner-approved public copy per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Plan copy grants no capture, billing, payout, or settlement authority; capture behavior stays governed by the disclosure and privacy-entitlement records.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.free_plan_trace_capture.v1',
        productArea: 'Khala Code',
        audience: ['user', 'agent', 'public'],
        state: 'planned',
        claim:
          'Free-plan Khala Code coding sessions produce disclosed, consented, redacted usage traces that may feed model and plugin improvement.',
        safeCopy:
          'This is the Episode 245 consented desktop capture pipeline, and it is planned, not live. Today the Khala Code default path is Codex wrapper mode whose raw events and ATIF traces are owner-private delegation observability (never free-plan capture), and the shipped default-on Rampart redaction on the desktop chat boundary is a privacy prefilter, not a security boundary. The desktop now has an explicit default-off consent control plus a fail-closed local capture planner: free-plan session events may proceed only through Rampart redaction and owner_only ingest when the owner gate and ingest sink are armed; paid-plan capture opt-out, redaction failure, missing owner arm, or missing ingest sink returns not_captured. Capture remains payout/settlement inert.',
        unsafeCopy:
          'Do not claim Khala Code coding sessions are currently captured for training, that wrapper-mode raw events are shared or sold, or that “scrubbed of any of your sensitive data” is a guaranteed security property rather than a redaction-pipeline design goal.',
        evidenceRefs: [
          'docs/khala/2026-06-30-khala-code-desktop-redaction.md',
          'docs/transcripts/245.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'promise:data.free_tier_capture_disclosure.v1',
          'promise:data.khala_free_tier_trace_capture.v1',
          'https://github.com/OpenAgentsInc/openagents/issues/8250',
          'clients/khala-code-desktop/src/shared/trace-capture.ts',
          'clients/khala-code-desktop/src/ui/plans-panel.ts',
          'clients/khala-code-desktop/src/bun/rpc-handlers.ts',
          'clients/khala-code-desktop/tests/trace-capture.test.ts',
          'clients/khala-code-desktop/tests/plans-panel.test.ts',
          'clients/khala-code-desktop/tests/rpc-handlers.test.ts',
          'docs/khala-code/khala-code-ux-contract.md',
        ],
        blockerRefs: [
          'blocker.owner.khala_code_desktop_trace_capture_arming_missing',
          'blocker.owner.khala_code_desktop_owner_only_ingest_sink_missing',
          'blocker.product_promises.khala_code_trace_capture_live_receipt_missing',
          'blocker.product_promises.trace_capture_reward_marker_inert',
        ],
        verification:
          'Desktop source now verifies explicit default-off consent, paid-plan exclusion, redaction-failure fail-closed behavior, owner_only ingest audience, and inert payout/settlement markers via the trace-capture, plans-panel, and RPC-handler tests. The record stays planned: green/yellow movement still requires owner arming of KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED, an owner_only production ingest sink, a public-safe captured-trace receipt, and owner-approved copy per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'Capture design intent grants no data sale, publication, payout, or settlement authority; owner-private delegation observability must never be reclassified as free-plan capture.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.trace_derived_plugins.v1',
        productArea: 'Khala Code',
        audience: ['user', 'agent', 'contributor', 'public'],
        state: 'planned',
        claim:
          'Scrubbed contributor traces are condensed into agent plugins that future agents can route through.',
        safeCopy:
          'Episode 245 describes free-plan traces being “condensed into these agent plugins that future agents” use. The automatic trace→plugin distillation pipeline is still not live. Source now contains the RL-7 precedent ledger/readback spine: an admin-token route may record only public-safe refs for one consented trace digest, one admitted and registered routable plugin, one exact routed usage event, one contributor attribution, and one already-settled Spark payout/settlement receipt; the public readback dereferences that n=1 precedent with live_at_read staleness. The record stays planned until an owner-supplied production receipt row actually exists and is reviewed.',
        unsafeCopy:
          'Do not claim trace-derived plugins are automatically generated, broadly routable, or market-proven today; do not present GEPA/Gym admission fixtures or an empty precedent ledger as a live plugin pipeline.',
        evidenceRefs: [
          'docs/transcripts/245.md',
          'docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'docs/fable/ROADMAP_AFTER.md',
          'docs/khala-code/2026-07-04-trace-plugin-revenue-share-precedent-template.md',
          'https://github.com/OpenAgentsInc/openagents/issues/8251',
          'apps/openagents.com/workers/api/src/khala-code-trace-plugin-revenue-share-routes.ts',
          'apps/openagents.com/workers/api/src/khala-code-trace-plugin-revenue-share-routes.test.ts',
          'apps/openagents.com/workers/api/migrations/0291_khala_code_trace_plugin_revenue_share_precedents.sql',
          'route:/api/operator/khala-code/trace-plugin-revenue-share-precedents',
          'route:/api/public/khala-code/trace-plugin-revenue-share-precedents/:receiptRef',
          'promise:marketplace.wasm_plugins.v1',
          'promise:marketplace.signature_monetization.v1',
          'promise:khala_code.free_plan_trace_capture.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.trace_to_plugin_distillation_pipeline_missing',
          'blocker.product_promises.trace_plugin_precedent_receipt_missing',
          'blocker.owner.khala_code_trace_plugin_revenue_share_live_receipt_missing',
        ],
        verification:
          'Source verifies the shape of a public-safe n=1 precedent receipt, including consented trace digest, plugin admission, plugin registry, route, exact usage, attribution, Spark payout, and settlement refs. Planned state remains: a reviewed production receipt row plus the working distillation/admission path are required before any trace-derived plugin claim moves.',
        authorityBoundary:
          'Plugin distillation grants no publication of private trace content and no payout by itself; optimizer candidates never auto-promote, admission stays evidence-gated with owner approval, and the precedent intake route records settled evidence only after the money-moving authority has acted elsewhere.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.plugin_backend_revenue_share.v1',
        productArea: 'Khala Code',
        audience: ['user', 'contributor', 'public'],
        state: 'planned',
        claim:
          'When paid usage routes through a plugin derived from your contributions, you earn a share, paid in Bitcoin.',
        safeCopy:
          'This is the Episode 245 headline — “What if your coding agent pays you?” — recorded with its on-camera hedge intact: the free plan “has the possibility of paying you.” The claim lineage is one thread: Episode 228 (Get Paid to Code) → Episode 230 (sell redacted Claude Code/Codex traces) → Episode 237 (plugins earn their authors a revenue share) → Episodes 243/244 (Khala aspiration → “we should be paying you”) → Episode 245 (Khala Code launch whiteboard). Source now has an exact, receipt-backed n=1 precedent ledger/readback for the first trace→plugin→routed usage→Spark payout proof, but it is evidence intake only and moves no sats itself. The record remains planned until the owner supplies and reviews an actual production settlement receipt row.',
        unsafeCopy:
          'Do not claim anyone has been paid via plugin routing until a public-safe settlement receipt row exists, quote the five-cents example as a rate, imply a payout pool, or imply the withdrawn/red refer-once-earn-forever claim. A payment the recipient cannot dereference is not a payment.',
        evidenceRefs: [
          'docs/transcripts/245.md',
          'docs/transcripts/244.md',
          'docs/transcripts/243.md',
          'docs/transcripts/237.md',
          'docs/transcripts/230.md',
          'docs/transcripts/228.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'docs/fable/ROADMAP_AFTER.md',
          'docs/khala-code/2026-07-04-trace-plugin-revenue-share-precedent-template.md',
          'https://github.com/OpenAgentsInc/openagents/issues/8251',
          'apps/openagents.com/workers/api/src/khala-code-trace-plugin-revenue-share-routes.ts',
          'apps/openagents.com/workers/api/src/khala-code-trace-plugin-revenue-share-routes.test.ts',
          'apps/openagents.com/workers/api/migrations/0291_khala_code_trace_plugin_revenue_share_precedents.sql',
          'route:/api/operator/khala-code/trace-plugin-revenue-share-precedents',
          'route:/api/public/khala-code/trace-plugin-revenue-share-precedents/:receiptRef',
          'promise:khala_code.trace_derived_plugins.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.plugin_revenue_share_precedent_receipt_missing',
          'blocker.product_promises.plugin_revenue_settlement_not_armed',
          'blocker.owner.khala_code_trace_plugin_revenue_share_live_receipt_missing',
        ],
        verification:
          'Source verifies public-safe exact accounting and settlement receipt shape for a one-sat-scale precedent: consented trace digest, plugin registry/routing refs, exact usage/idempotency, attribution, gross/contributor msats, Spark rail, payout receipt, and settlement receipt. Planned state remains: owner-gated live settlement and a reviewed dereferenceable production receipt row are required before any paid-via-plugin claim moves.',
        authorityBoundary:
          'Revenue-share design intent grants no spend, payout, or settlement authority. The RL-7 intake route records public-safe evidence after settlement; it is not the money-moving adapter, does not accept payout destinations or raw invoices, and does not define a rate or paid-to-free pool.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.paid_to_free_revenue_share.v1',
        productArea: 'Khala Code',
        audience: ['user', 'customer', 'public'],
        state: 'planned',
        claim:
          'A portion of paid-plan revenue funds payouts to free-plan users.',
        safeCopy:
          'Episode 245 states that paid-plan customers fund the free plan: “some of that money will go to pay the users of the free plan.” This plan-level pool claim is distinct from per-plugin attribution (khala_code.plugin_backend_revenue_share.v1). It is planned: no paid Khala Code plan is purchasable, no revenue pool exists, and no payout policy is defined.',
        unsafeCopy:
          'Do not claim free-plan users are being paid from paid-plan revenue, do not state a share percentage, and do not imply the withdrawn/red refer-once-earn-forever claim.',
        evidenceRefs: [
          'docs/transcripts/245.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'promise:khala_code.free_paid_plans.v1',
          'promise:khala_code.plugin_backend_revenue_share.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.paid_plan_revenue_pool_missing',
          'blocker.product_promises.free_plan_payout_policy_missing',
        ],
        verification:
          'Requires a collectable paid plan, a defined and published pool/payout policy, and first dereferenceable free-plan payout receipts before any yellow/green movement.',
        authorityBoundary:
          'Pool language grants no spend or settlement authority; any future payouts run under bounded owner-approved treasury controls with dereferenceable receipts.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.architect_coder_judge.v1',
        productArea: 'Khala Code',
        audience: ['user', 'agent', 'operator'],
        state: 'planned',
        claim:
          'Khala Code offers a one-command architect/coder/judge preset: architect and judge run through the user’s own Anthropic auth, coder runs through the user’s existing local Codex login, and an advisor can be enabled separately.',
        safeCopy:
          'This is a planned, copy-gated Khala Code workflow. Source-level preset wiring exists for `architect-coder-judge`: the Settings role-registry card and `khala code --preset architect-coder-judge` write a schema-tagged role registry where architect and judge use the user’s own Anthropic auth, coder uses the existing local Codex login, and advisor is optional/off by default. It is not an end-to-end availability claim yet.',
        unsafeCopy:
          'Do not claim the architect/coder/judge workflow is publicly available, proven end-to-end, cheaper in practice, or that OpenAgents proxies, resells, brokers, or supplies Anthropic/Claude or Codex subscription capacity for it.',
        evidenceRefs: [
          'docs/fable/ROADMAP_QA.md#9b-qa-9--plannercoderjudge-workflow-build-it-wire-it-prove-it',
          'docs/fable/2026-07-02-oh-my-pi-planner-coder-judge-audit.md',
          'clients/khala-code-desktop/src/shared/model-role-preset.ts',
          'clients/khala-code-desktop/src/ui/codex-settings-panel.ts',
          'clients/khala-code-desktop/src/bun/index.ts',
          'clients/khala-code-desktop/tests/codex-settings.test.ts',
          'clients/khala-code-desktop/tests/rpc-handlers.test.ts',
          'clients/khala-code-desktop/tests/headless.test.ts',
          'https://github.com/OpenAgentsInc/openagents/issues/8058',
          'docs/transcripts/246.md',
          'promise:khala_code.desktop_codex_wrapper.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.khala_code_architect_coder_judge_e2e_missing',
          'blocker.product_promises.khala_code_role_accounting_missing',
          'blocker.product_promises.khala_code_architect_coder_judge_public_copy_gate',
        ],
        verification:
          'Planned state is source-level only: the preset must write the schema-tagged role registry in one action, preserve no-proxy/no-resale rails, and stay copy-gated. Yellow/green movement requires the QA Q9.7 plan -> code -> judge scenario and armed live smoke, exact per-role token/economics rows, and owner-approved public copy per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The preset grants no provider proxy, resale, brokered capacity, spend, payout, settlement, or merge authority. Architect, judge, and advisor outputs are advisory data under the deterministic verify-command authority; coder execution remains the user’s own Codex login and existing local authority boundary.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.ux_behavior_contracts.v1',
        productArea: 'Khala Code',
        audience: ['user', 'customer', 'agent', 'operator'],
        state: 'yellow',
        claim:
          'Khala Code UX expectations stated by the owner (and later by customers) are recorded verbatim in a typed behavior-contract registry and enforced by oracle tests in the normal test sweep, so stated behavior cannot silently drift.',
        safeCopy:
          'The UX Behavior Contract system is live in source as the micro-scale counterpart of this product-promise registry (Episode 246): owner-stated expectations land verbatim — statement, source, and oracle — in the typed registry at clients/khala-code-desktop/src/contracts/ux-contracts.ts, built on the shared @openagentsinc/behavior-contracts schema and coverage checker; the paired coverage test (tests/ux-contracts.test.ts) fails the sweep if an enforced contract loses its oracle, and the human rendering at docs/khala-code/khala-code-ux-contract.md is kept in sync by the same test. The repo agent contract mandates that any stated UX expectation lands in the registry in the same change. Episode 246 exercised the full flow on camera, including an agent mining ~36 hours of Codex/Claude conversation history (43 in-app sessions plus adjacent sessions) into pending entries. Yellow because most mined entries are pending (recorded without oracles yet), customer-stated contract intake has not run, and in-the-wild deviations do not yet auto-file strict bugs.',
        unsafeCopy:
          'Do not claim every stated UX expectation is oracle-enforced, that pending entries are enforced, that customer contract intake is live, or that contract deviations automatically open issues today.',
        evidenceRefs: [
          'clients/khala-code-desktop/src/contracts/ux-contracts.ts',
          'clients/khala-code-desktop/tests/ux-contracts.test.ts',
          'docs/khala-code/khala-code-ux-contract.md',
          'packages/behavior-contracts/src/contract.ts',
          'packages/behavior-contracts/src/coverage.ts',
          'packages/behavior-contracts/src/behavior-contracts.test.ts',
          'docs/transcripts/246.md',
          'promise:khala_code.desktop_codex_wrapper.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.khala_code_ux_contracts_pending_oracles',
          'blocker.product_promises.khala_code_ux_contract_deviation_intake_missing',
        ],
        verification:
          'The coverage test must fail the sweep when an enforced contract loses its oracle, and enforced oracles must assert real behavior (mounted DOM, RPC results, harness scenarios) rather than source strings except as labeled stopgaps. Green requires the pending mined backlog converted to enforced oracles or explicitly retired, a deviation-to-strict-bug intake path, and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The contract registry grants no runtime, dispatch, spend, or merge authority. Oracles must never be weakened to make a change pass — that is a contract change requiring owner sign-off; deviations found in the wild are strict bugs filed with the contract id.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.bundled_fleet_skill.v1',
        productArea: 'Khala Code',
        audience: ['user', 'agent', 'operator'],
        state: 'yellow',
        claim:
          'Khala Code ships a bundled khala-fleet skill — fleet-management operating procedure and guardrails in standard SKILL.md form — installed by default into the shared ~/.agents/skills root so the Codex harness and any skills-aware agent discover it without setup.',
        safeCopy:
          'The bundled skill is live in source: the canonical SKILL.md at .agents/skills/khala-fleet/SKILL.md (repo-scope discovery inside this repository) encodes the connect/dispatch/claims/closeout-verification ladder and hard guardrails as a launcher over the canonical runbooks, and Khala Code Desktop installs it at startup into ~/.agents/skills/khala-fleet/ with a managed-marker overwrite policy that upgrades only files it owns and never touches user-owned files. Default-on and toggleable via KHALA_CODE_DESKTOP_BUNDLED_SKILLS=0; a sync script plus a byte-for-byte pin test keep the embedded copy identical to the canonical file. Yellow because it ships on main with tests but has no outside-user discovery evidence (no public release artifact) and the toggle is env-only (no Settings surface yet).',
        unsafeCopy:
          'Do not claim outside users receive the skill today, that a Settings toggle exists, or that the skill overrides runbook or dispatch-gate authority.',
        evidenceRefs: [
          '.agents/skills/khala-fleet/SKILL.md',
          'clients/khala-code-desktop/src/bun/khala-bundled-skills.ts',
          'clients/khala-code-desktop/tests/khala-bundled-skills.test.ts',
          'docs/khala-code/2026-07-02-khala-fleet-bundled-skill.md',
          'promise:khala_code.desktop_codex_wrapper.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.khala_code_public_release_artifact_missing',
          'blocker.product_promises.khala_code_bundled_skill_settings_toggle_missing',
        ],
        verification:
          'The khala-bundled-skills suite pins the embedded content byte-for-byte against the canonical SKILL.md, checks the frontmatter contract and managed marker, and covers install/upgrade/user-owned/disabled/no-home behavior. Green requires outside-user skill discovery evidence through a public release plus an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The skill is guidance text only: it grants no dispatch, spend, settlement, or credential authority, installs only into the skills directory it owns by marker, and the delegation runbooks and server dispatch gate remain the operating authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'mobile.fleet_companion.v1',
        productArea: 'mobile and Khala Code',
        audience: ['operator', 'user'],
        state: 'planned',
        claim:
          'A native SwiftUI mobile companion pairs with the owner’s fleet to observe, get notified, approve, and steer — it never hosts work.',
        safeCopy:
          'This is the successor to the withdrawn Expo record (mobile.autopilot_remote_control.v1): per the standing mobile policy the companion is native SwiftUI with no Expo/EAS cloud. The design shape is recorded in the Orca adoption plan and the fable roadmap (WS-11): QR pairing with per-device tokens, app-layer E2EE, a Durable-Object relay transport, an enforced allowlisted mobile RPC surface, APNs push for finished/blocked/approval-needed, then approve/steer and bounded diff review. Today only a read-only iOS fleet-status poll exists; the pairing/relay/push/steer stack is unbuilt.',
        unsafeCopy:
          'Do not claim a mobile fleet companion is live, downloadable, or can approve or steer work from a phone; the read-only status poll is not the companion.',
        evidenceRefs: [
          'docs/fable/2026-07-01-orca-analysis-and-adoption-plan.md',
          'docs/fable/ROADMAP.md',
          'docs/mobile/2026-06-26-autopilot-remote-control-retirement.md',
          'docs/mobile/2026-06-26-khala-voice-app-spec.md',
          'packages/autopilot-control-protocol/src/remote-decision-queue.ts',
        ],
        blockerRefs: [
          'blocker.product_promises.mobile_companion_pairing_relay_transport_missing',
          'blocker.product_promises.mobile_companion_allowlisted_rpc_surface_missing',
          'blocker.product_promises.mobile_companion_not_shipped',
        ],
        verification:
          'Requires the paired E2EE relay transport, the enforced RPC allowlist test, a shipped TestFlight artifact, and a dereferenceable receipt of a real approval/steer action resolved from a phone against a live fleet run.',
        authorityBoundary:
          'The phone is a projection and control relay only: it never hosts work, never holds worker credentials, and every write action is capability-gated and allowlisted with the node as the decision authority.',
      },
      {
        ...basePromiseFields,
        promiseId: 'contributors.bounties_surface.v1',
        productArea: 'contributors',
        audience: ['contributor', 'agent', 'public'],
        state: 'red',
        claim:
          'openagents.com/bounties lists current contributor bounties and how to claim them.',
        safeCopy:
          'Episode 225 promised a standing surface: “we will keep an updated list of what bounties are available at openagents.com/bounties … no matter when you watch this video.” As of 2026-07-01 that URL 302-redirects to the homepage and no bounty list is live, so the claim is lapsed in practice and this record enters red: affirmative bounty copy is blocked until the surface is revived with a real bounty list and claim instructions, or the claim is formally withdrawn.',
        unsafeCopy:
          'Do not tell users or agents that a live bounties list exists at openagents.com/bounties, and do not cite Episode 225 as evidence of a current bounty program.',
        evidenceRefs: [
          'docs/transcripts/225.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
        ],
        blockerRefs: [
          'blocker.product_promises.bounties_surface_not_live',
          'blocker.product_promises.bounties_program_process_undefined',
        ],
        verification:
          'Requires openagents.com/bounties serving a real, current bounty list with claim instructions, plus a defined intake/review/payout process, before any affirmative bounty copy.',
        authorityBoundary:
          'A bounty listing grants no payment obligation or settlement authority by itself; bounty payouts run under the same receipt-first payment discipline as other contributor payments.',
      },
      {
        ...basePromiseFields,
        promiseId: 'business.legal_benchmark_leaderboard.v1',
        productArea: 'business',
        audience: ['customer', 'public'],
        state: 'planned',
        claim:
          'OpenAgents publishes its agents’ scores, the code behind them, and a public leaderboard on a recognized legal AI benchmark.',
        safeCopy:
          'Episode 227 promised a hill-climbing effort on the Harvey legal benchmark with published scores, published code, and “our own dashboard” — daring others to post theirs. This is planned: the upstream benchmark harness is vendored as read-only reference material (projects/harvey-labs lane; owned execution belongs in the owned Rust runner per workspace policy), and no owned-runner scores or public leaderboard surface have been published. The adjacent shipped surface is the review-gated legal workspace pack (business.legal_workspace_pack.v1, yellow). Episode 245 re-surfaces “legal brief” as a Khala output lane, which puts this claim back in play.',
        unsafeCopy:
          'Do not claim OpenAgents has published legal benchmark scores, holds a leaderboard position, or outperforms named legal AI vendors; no owned-runner results exist.',
        evidenceRefs: [
          'docs/transcripts/227.md',
          'docs/transcripts/245.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'promise:business.legal_workspace_pack.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.legal_benchmark_owned_runner_scores_missing',
          'blocker.product_promises.legal_leaderboard_surface_missing',
        ],
        verification:
          'Requires owned-runner benchmark results with published methodology and code, plus a live public leaderboard/dashboard surface, before any comparative legal-capability copy.',
        authorityBoundary:
          'Benchmark publication grants no legal-advice capability claim; the legal pack stays review-gated and gives no legal advice regardless of benchmark results.',
      },
      {
        ...basePromiseFields,
        promiseId: 'qa.agentic_qa_runner.v1',
        productArea: 'QA',
        audience: ['customer', 'operator', 'public'],
        state: 'yellow',
        claim:
          'An open-source agentic QA runner (@openagentsinc/qa-runner) drives real apps through scripted and LLM-driven scenarios and distills live sessions into committed regression tests.',
        safeCopy:
          'The QA runner shipped as an OSS npm package (@openagentsinc/qa-runner@0.1.0, MIT) out of epic #6181, with an LLM ReAct brain plus scripted brain, Playwright/terminal/native-macOS backends, a distiller that turns live sessions into committed e2e tests, and a verification discipline where CONFIRMED requires observed evidence from the current run. Yellow because it has no paid customer, run-receipt settlement seams stay INERT until deliberately flipped, the Khala Code desktop backend integration is design-stage (the fable QA framework doc), and the broader QA Swarm product records below remain operator-assisted/planned rather than self-serve.',
        unsafeCopy:
          'Do not claim paid QA customers, self-serve hosted QA Swarm runs, revenue, or settlement exist, and do not present INCONCLUSIVE agent observations as CONFIRMED verification.',
        evidenceRefs: [
          'apps/qa-runner/README.md',
          'https://www.npmjs.com/package/@openagentsinc/qa-runner',
          'docs/fable/2026-07-01-khala-code-desktop-qa-framework-design.md',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'docs/fable/2026-07-02-qa-swarm-product-plan.md',
          'docs/transcripts/246.md',
        ],
        blockerRefs: [
          'blocker.product_promises.qa_runner_paid_customer_missing',
          'blocker.product_promises.qa_runner_settlement_inert',
          'blocker.product_promises.qa_swarm_self_serve_hosted_runs_missing',
        ],
        verification:
          'The shipped package installs and runs its scripted and LLM scenario modes with the documented verdict discipline. Green requires a first paid customer engagement with dereferenceable receipts and owner sign-off per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The QA runner grants no settlement or payout authority; run receipts stay inert until deliberately flipped, and public traces must stay public-safe.',
      },
      {
        ...basePromiseFields,
        promiseId: 'qa_swarm.product_surface.v1',
        productArea: 'QA Swarm',
        audience: ['customer', 'operator', 'agent', 'public'],
        state: 'yellow',
        claim:
          'QA Swarm is the named product surface packaging the ROADMAP_QA machine into an operator-assisted QA engagement, with Khala Code Desktop as customer number one.',
        safeCopy:
          'QA Swarm is a yellow product definition: it packages the existing qa-runner, trace, coverage, perf, Arbiter, and FleetRun ingredients into an operator-assisted QA engagement. The first customer is OpenAgents itself through the Khala Code Desktop dogfood lane. Episode 246 states the sequencing on camera: QA Swarm is built first for Khala Code, then added to the Autopilot product suite for businesses — that Autopilot-suite step is design intent, not a shipped surface. The /business rate card now publishes operator-assisted QA Swarm audit, sprint, and retainer bands; hosted runs and the share surface remain planned, and it is not self-serve or broadly hosted.',
        unsafeCopy:
          'Do not describe QA Swarm as self-serve, generally hosted, automated for arbitrary third-party apps, or proven by paid customer receipts.',
        evidenceRefs: [
          'docs/fable/2026-07-02-qa-swarm-product-plan.md',
          'docs/feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md',
          'docs/transcripts/246.md',
          'apps/openagents.com/apps/web/src/page/business.ts',
          'apps/openagents.com/apps/web/src/business-route.test.ts',
          'promise:qa.agentic_qa_runner.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.qa_swarm_paid_customer_receipt_missing',
          'blocker.product_promises.qa_swarm_operator_assisted_only',
          'blocker.product_promises.qa_swarm_share_surface_missing',
        ],
        verification:
          'Yellow is limited to the named product definition, existing runner/trace evidence, and the operator-assisted package bands on /business. Green requires a public-safe customer-one run report, a dereferenceable run-level share URL, first paid or owner-accepted engagement receipts, and exact accounting for hosted execution.',
        authorityBoundary:
          'This product-surface record grants no dispatch, spend, settlement, payout, publication, or third-party testing authority. Any external report or sales artifact requires the relevant run, redaction, and owner-review gates.',
      },
      {
        ...basePromiseFields,
        promiseId: 'qa_swarm.hosted_runs.v1',
        productArea: 'QA Swarm',
        audience: ['customer', 'operator', 'agent'],
        state: 'planned',
        claim:
          'A hosted QA Swarm run can be started as one bounded product run over owned runner capacity.',
        safeCopy:
          'Hosted QA Swarm runs are planned. The intended path composes qa-runner control APIs, FleetRun parallelism, the nightly-matrix recipe, resource caps, exact accounting, and public-safe run projections, but no one-command hosted run is available to customers today.',
        unsafeCopy:
          'Do not claim customers can click, buy, or start hosted QA Swarm runs today; do not imply OpenAgents can test arbitrary apps without an operator-assisted target adapter and redaction review.',
        evidenceRefs: [
          'docs/fable/2026-07-02-qa-swarm-product-plan.md',
          'apps/qa-runner/README.md',
          'promise:qa.agentic_qa_runner.v1',
          'promise:qa_swarm.product_surface.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.qa_swarm_hosted_run_command_missing',
          'blocker.product_promises.qa_swarm_runner_capacity_receipts_missing',
          'blocker.product_promises.qa_swarm_exact_hosted_accounting_missing',
          'blocker.product_promises.qa_swarm_third_party_target_adapter_missing',
        ],
        verification:
          'Requires a bounded hosted-run command or API, owned-runner execution receipts, exact token/resource accounting rows, target-adapter policy checks, and a redaction-checked run projection before this can move beyond planned.',
        authorityBoundary:
          'This record grants no runner dispatch authority and no customer-data handling authority. Hosted execution stays operator-assisted until target adapters, resource caps, consent, redaction, and accounting are enforced.',
      },
      {
        ...basePromiseFields,
        promiseId: 'qa_swarm.share_surface.v1',
        productArea: 'QA Swarm',
        audience: ['customer', 'operator', 'agent', 'public'],
        state: 'planned',
        claim:
          'A QA Swarm run has a public-safe share URL showing verdicts, coverage, traces, videos, perf budgets, and an evidence-bound swarm board.',
        safeCopy:
          'The QA Swarm share URL is planned as openagents.com/qa/{runRef}. The intended projection reuses the shipped /trace/{uuid} pattern and Arbiter graph discipline, but the run-level schema, renderer, redaction gate, and evidence-bound board are not live yet.',
        unsafeCopy:
          'Do not link to or describe a live openagents.com/qa/{runRef} customer surface until the route, projection, redaction checks, and evidence-bound board exist.',
        evidenceRefs: [
          'docs/fable/2026-07-02-qa-swarm-product-plan.md',
          'docs/unit/2026-06-30-arbiter-effect-2d-dataflow-graph-audit.md',
          'route:/trace/{uuid}',
          'promise:qa_swarm.product_surface.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.qa_swarm_share_route_missing',
          'blocker.product_promises.qa_swarm_run_projection_schema_missing',
          'blocker.product_promises.qa_swarm_redaction_gate_missing',
          'blocker.product_promises.qa_swarm_evidence_bound_board_missing',
        ],
        verification:
          'Requires the /qa/{runRef} route, a public-safe run projection schema, redaction-tripwire tests, trace/video/perf/coverage refs, and an Arbiter board where edges light only from dereferenceable receipts.',
        authorityBoundary:
          'The share surface is a projection only. It must not expose raw prompts, local paths, private repo data, customer-sensitive content, credentials, raw videos before redaction, or unpublished provider payloads.',
      },
      {
        ...basePromiseFields,
        promiseId: 'qa_swarm.service_packages.v1',
        productArea: 'QA Swarm',
        audience: ['customer', 'operator', 'public'],
        state: 'yellow',
        claim:
          'QA Swarm has operator-assisted service packages for audits, QA-on-every-push retainers, and swarm sprints.',
        safeCopy:
          'QA Swarm package bands are published on /business as operator-assisted services: Swarm Audit at $1,000-$5,000, Swarm Sprint at $5,000-$15,000, and QA-on-every-push retainers at $2,000-$10,000/month. Public copy must continue to say operator-assisted until self-serve delivery and hosted-run receipts exist. The packages are not self-serve checkout products.',
        unsafeCopy:
          'Do not present QA Swarm packages as self-serve, generally hosted, available for arbitrary third-party apps without target-adapter review, or proven by paid delivery receipts. Do not imply purchase/checkout is live.',
        evidenceRefs: [
          'docs/fable/2026-07-02-qa-swarm-product-plan.md',
          'NEEDS_OWNER.md',
          'apps/openagents.com/apps/web/src/page/business.ts',
          'apps/openagents.com/apps/web/src/business-route.test.ts',
          'promise:qa_swarm.product_surface.v1',
          'promise:qa_swarm.hosted_runs.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.qa_swarm_checkout_or_intake_receipts_missing',
          'blocker.product_promises.qa_swarm_first_paid_delivery_receipt_missing',
          'blocker.product_promises.qa_swarm_self_serve_delivery_missing',
        ],
        verification:
          'Yellow is limited to owner-approved public package copy and pricing on /business. Green requires an intake/checkout or explicit operator-sales path with receipts, at least one paid or owner-accepted delivery receipt, and clear copy that distinguishes operator-assisted delivery from self-serve hosting.',
        authorityBoundary:
          'Package records do not authorize charging, checkout, settlement, payout, third-party target access, or external outreach. Public prices are quote starters only; each engagement still needs scoped intake, redaction/target review where applicable, and operator acceptance before work runs.',
      },
      {
        ...basePromiseFields,
        promiseId: 'khala_code.forum_hotbar.v1',
        productArea: 'Khala Code',
        audience: ['user', 'agent', 'operator', 'public'],
        state: 'planned',
        claim:
          'The OpenAgents Forum is accessible inside Khala Code: a Forum slot on the left hotbar between Fleet and Settings, with the same functionality as the web forum.',
        safeCopy:
          'Owner-directed product commitment (2026-07-01): Khala Code Desktop gets a Forum surface on the left hotbar, placed between the Fleet and Settings slots (the current hotbar is Chat, Fleet, Inbox, Settings), at functional parity with openagents.com/forum — browse forums, topics, and posts; authenticated posting under the user’s or registered agent’s real identity; BOLT12 direct tips; and product-promise gap reporting — not a reduced read-only embed. This is planned: no Forum surface exists in the desktop today. The web Forum routes in the openagents.com Worker are the backing authority the desktop surface would consume.',
        unsafeCopy:
          'Do not claim the Forum is reachable inside Khala Code today, and do not ship or describe a read-only or partial embed as “the Forum in Khala Code” while this record is planned.',
        evidenceRefs: [
          'clients/khala-code-desktop/src/ui/sidebar.ts',
          'route:/api/forum/forums/product-promises/topics',
          'route:/api/forum/posts/{postId}/direct-tips',
          'docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md',
          'promise:khala_code.desktop_codex_wrapper.v1',
          'promise:forum.content_tipping.v1',
          'promise:agents.cursor_forum_wallet.v1',
        ],
        blockerRefs: [
          'blocker.product_promises.khala_code_forum_surface_missing',
          'blocker.product_promises.khala_code_forum_web_parity_missing',
          'blocker.product_promises.khala_code_forum_identity_bridge_missing',
        ],
        verification:
          'Requires the hotbar Forum slot rendering between Fleet and Settings; the desktop surface exercising the same Forum API routes as the web client (forum/topic/post browse and read, authenticated post creation, direct tips, promise-gap reporting) under the user’s or agent’s real server-side identity; and a parity checklist against the web Forum with visual smoke coverage. Yellow needs the working surface with the parity checklist; green additionally needs release evidence consistent with khala_code.desktop_codex_wrapper.v1 and an owner-signed receipt-first upgrade per proof.claim_upgrade_receipts.v1.',
        authorityBoundary:
          'The desktop Forum surface reuses the web Forum’s server authority: identity, posting rights, moderation, tipping settlement, and rate policies stay server-enforced. The desktop grants no additional posting, moderation, tipping, or payment authority, and never embeds tokens or wallet material in public projections.',
      },
    ],
    notes: [
      `Include version ${PublicProductPromisesVersion} and the relevant promiseId when reporting a mismatch.`,
      'Registry 2026-07-04.4 is the RL-7 Khala Code trace→plugin→revenue-share precedent spine pass (#8251) and flips NO promise state — green stays exactly 34. khala_code.trace_derived_plugins.v1 and khala_code.plugin_backend_revenue_share.v1 remain planned: source now has an admin-token intake route and public readback for one n=1 precedent receipt linking a consented trace digest, admitted/registered/routable plugin, exact routed usage/idempotency, contributor attribution, msat accounting, Spark payout receipt, and settlement receipt. The route records already-settled public-safe evidence only; it does not move sats, accept raw trace/payment material, define a rate/pool, create market-demand proof, or claim anyone has been paid until the owner supplies and reviews a production receipt row.',
      'Registry 2026-07-04.3 is the RL-4 Khala Code paid-plan payment-collection pass (#8248) and flips NO promise state — green stays exactly 34. khala_code.free_paid_plans.v1 remains planned: POST /v1/khala-code/plans/purchases is still KHALA_CODE_PAID_PLANS_ENABLED default-OFF and fail-closed, but when armed it now creates a real payment-required leg on Stripe Checkout (card) or the Spark/MPP Lightning invoice rail and grants the existing paid-privacy entitlement receipt only after settled payment. The payment intent ledger is not a second entitlement truth; /api/public/inference/privacy-receipts/{receiptRef} remains the dereferenceable receipt surface. Owner-gated arming, Stripe price id, Lightning sats price, live credentials, public copy, and one production collected purchase remain in NEEDS_OWNER. No paid-plan availability, capture-default, payout, settlement, or promise-green claim is created.',
      'Registry 2026-07-04.2 is the RL-3 Khala Code outside-user run evidence-intake pass (#8247) and flips NO promise state — green stays exactly 34. khala_code.desktop_codex_wrapper.v1 stays yellow: desktop Settings now offers an explicit opt-in Run evidence action; POST /api/public/khala-code/outside-user-runs records only app version, platform, architecture, distribution channel, and bounded Codex/Pylon readiness; GET /api/public/khala-code/outside-user-runs/{receiptRef} dereferences the public-safe receipt with generatedAt + live_at_read staleness. No phone-home, paths, prompts, tokens, logs, account ids, machine ids, signed DMG, real outside-user row, free/paid economics, capture, payout, settlement, installer availability, or promise-green claim is created.',
      'Registry 2026-07-04.1 is the RL-2 Khala Code public install-truth pass (#8246) and flips NO promise state — green stays exactly 34. khala_code.desktop_codex_wrapper.v1 stays yellow: /code/download now exposes the Codex-required install page, public npm khala CLI path, source-build path, and pending desktop DMG state under the existing copy gate; GET /api/public/khala-code/download-counts serves exact khala_code_download_events rows or counts: [] with blocker refs. No public signed DMG, outside-user evidence, free/paid economics, capture, payout, settlement, or installer availability claim is created.',
      'Registry 2026-07-03.1 is the Episode 246 (Dogfooding Khala Code) incorporation pass and flips NO promise state — green stays exactly 34. Two records land, both yellow and evidence-bounded: khala_code.ux_behavior_contracts.v1 — the UX Behavior Contract system (the micro-scale counterpart of this registry) is live in source with a typed registry (clients/khala-code-desktop/src/contracts/ux-contracts.ts on the shared @openagentsinc/behavior-contracts schema), a coverage test that fails the sweep if an enforced contract loses its oracle, and a synced human doc; yellow because most entries mined from ~36 hours of Codex/Claude session history are pending (no oracles yet), customer intake has not run, and deviation-to-strict-bug intake is not wired. khala_code.bundled_fleet_skill.v1 — the desktop installs a bundled khala-fleet SKILL.md into ~/.agents/skills by default (managed-marker overwrite policy, env toggle, byte-pin test); yellow pending outside-user discovery evidence and a Settings toggle. Episode 246 evidence (docs/transcripts/246.md) is added to khala_code.desktop_codex_wrapper.v1 (owner full-time dogfood: fixing Khala Code with Khala Code), khala_code.architect_coder_judge.v1 (the owner names the architect/coder/judge pattern for Khala Code on camera), qa.agentic_qa_runner.v1, and qa_swarm.product_surface.v1, whose copy now records the on-camera sequencing that QA Swarm is built first for Khala Code and then added to the Autopilot product suite for businesses — design intent, not a shipped surface. No pricing, availability, capture, payout, or settlement claim changes; any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-07-02.4 is the BF-2.1 /business rate-card pass (#8079) and flips NO promise green — green stays exactly 34. business.intake_quick_win_offering.v1 remains yellow and now includes the published operator-assisted rate card: Quick Win ($1,000-$5,000 fixed), Fleet Sprint ($5,000-$15,000/week), On Autopilot Retainer ($2,000-$10,000/month), and QA Swarm bands. qa_swarm.service_packages.v1 advances from planned to yellow only for public package pricing and copy; checkout, first paid delivery receipts, self-serve hosted runs, broad hosted availability, payout, and settlement remain blocked.',
      'Registry 2026-07-02.3 adds khala_code.architect_coder_judge.v1 as a planned, copy-gated Khala Code promise candidate for the architect/coder/judge preset. It flips NO promise state. Source-level evidence covers the architect-coder-judge role-registry preset in Khala Code settings and headless launch wiring: coder on the user’s existing Codex login, architect/judge on the user’s own Anthropic auth, advisor optional-off, and no proxy/no-resale rails restated. The record remains planned until the plan -> code -> judge workflow is verifiable end-to-end with exact per-role accounting and owner-approved public copy.',
      'Registry 2026-07-02.2 is the QA Swarm QS1 pass (#8061) and flips NO promise green — green stays exactly 34. qa.agentic_qa_runner.v1 remains yellow but now explicitly scopes the broader QA Swarm product as not self-serve. New qa_swarm.* records land conservatively: qa_swarm.product_surface.v1 is yellow for the named operator-assisted product definition and customer-one dogfood scope; qa_swarm.hosted_runs.v1, qa_swarm.share_surface.v1, and qa_swarm.service_packages.v1 are planned. The modeled rate card is staged in NEEDS_OWNER.md for owner review; public copy must not publish prices, imply self-serve delivery, or claim paid customer/hosted-run receipts until the named blockers clear.',
      'Registry 2026-07-02.1 is the owner-directed /business redesign pass and flips NO promise state — green stays exactly 34. business.intake_quick_win_offering.v1 (yellow, unchanged) gains evidence for the rebranded dark-only page (hero: Agents that work.) and the new bounded conversational intake at POST /api/public/business-intake-chat: a stateless server-side interview running the published intake spec (offerings menu with honest availability labels, interview areas A-G, Output Spec Template) over the same gateway serving lane as the free tier, fail-closed 503 when unarmed, per-IP rate limited, exact-only internal token accounting (demand_source business_intake_chat), and the drafted spec hands off into the existing signup form, which remains the single submit authority and the only stored intake artifact. A review-only landing candidate ships at /preview/landing (builders -> /khala, businesses -> /business) with an explicit not-the-homepage banner; the live homepage is untouched. No availability label changed, no offering was added, no pricing was published, and both blockers stand.',
      'Registry 2026-07-01.3 is the #7966 PROMISSORY pass on khala_code.free_paid_plans.v1 and flips NO promise state — green stays exactly 34 and the record stays planned. Cleared with source+test evidence: khala_code_plan_selection_surface_missing — GET /api/public/khala-code/plans projects the honest Episode 245 two-plan structure (Free pay-with-data / Paid private-data) with real purchasability state from the fail-closed KHALA_CODE_PAID_PLANS_ENABLED read (default OFF); GET /v1/khala-code/plan resolves the caller’s current plan server-side from the privacy-entitlement seam (paid only on a real entitlement row, confidential-compute mode reports capture-excluded free, entitlement read errors return 503 instead of fabricating a plan); and the Khala Code desktop settings surface renders the plan cards and current plan through host RPCs with the same honest not-purchasable state (purchase control disabled and fail-closed while unarmed). Remaining blocker: khala_code_paid_plan_not_purchasable — POST /v1/khala-code/plans/purchases exists but is flag-gated default OFF, fail-closed, and collects no payment; when armed it grants the EXISTING paid-privacy entitlement idempotently with a dereferenceable receipt at /api/public/inference/privacy-receipts/{receiptRef} rather than forking entitlement truth. Arming, the payment-collection leg, pricing, and owner-approved public copy are owner decisions (NEEDS_OWNER). No plan is selectable-as-live or purchasable today, no capture behavior changed, and no payout/settlement implication is created.',
      'Registry 2026-07-01.2 adds khala_code.forum_hotbar.v1 (planned) on owner direction and flips NO promise state — green stays exactly 34. The commitment: the OpenAgents Forum becomes accessible inside Khala Code as a left-hotbar slot between Fleet and Settings, at functional parity with the web forum (browse, authenticated posting under the real server-side identity, BOLT12 direct tips, promise-gap reporting) — explicitly not a reduced read-only embed. No desktop Forum surface exists today; the record enters planned with surface, web-parity, and identity-bridge blockers, and the web Forum routes in the openagents.com Worker remain the backing authority. Any future yellow/green movement is receipt-first per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-07-01.1 is the Episode 245 / Khala Code launch alignment pass (docs/fable/2026-07-01-product-promises-khala-code-launch-alignment.md) and flips NO promise green — green stays exactly 34. It adds the khala_code.* family: khala_code.desktop_codex_wrapper.v1 (yellow — the app, Codex-required positioning, pinned parity contract, and fleet delegation exist on main; no public release artifact or outside-user evidence) plus khala_code.free_paid_plans.v1, khala_code.free_plan_trace_capture.v1, khala_code.trace_derived_plugins.v1, khala_code.plugin_backend_revenue_share.v1, and khala_code.paid_to_free_revenue_share.v1 (all planned — the Episode 245 economics loop is launch-anchored design intent with the on-camera “possibility of paying you” hedge preserved; nothing is metered, attributed, pooled, or paid). It withdraws mobile.autopilot_remote_control.v1 (the Expo app was retired 2026-06-26 before shipping) in favor of the new mobile.fleet_companion.v1 (planned, native SwiftUI observe/notify/approve/steer companion). It adds contributors.bounties_surface.v1 (red — the Episode 225 “always live” openagents.com/bounties surface currently 302-redirects to the homepage), business.legal_benchmark_leaderboard.v1 (planned — the Episode 227 public legal-leaderboard claim), and qa.agentic_qa_runner.v1 (yellow — shipped OSS npm package, no paid customer, settlement inert). pylon.data_trace_revenue.v1 and the data.* capture records gain scope notes routing new pays-you copy through the khala_code.* family and clarifying that Khala Code wrapper raw events are owner-private delegation observability, not free-plan capture. Rate pin for Episode 238: the settled Tassadar per-window reward rate is 5 sats worker / 5 sats validator per verified window (plus the single 1,000-sat canary settlement); the Episode 238 whiteboard “5K” figure was never a settled rate and must not be quoted as one. No promise_transition receipts are required (no green flips; the planned→withdrawn Expo retirement follows the models.tasadar typo-withdrawal precedent); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-29.5 applies the owner-signed green transition for exactly two promises: metrics.khala_model_family_mix_public.v1 (#7016) and autopilot.agent_world_scene.v1 (#7030), moving green 32 -> 34 and clearing blockerRefs only on those two records. No Hosted Gemini (#7017), character-creation (#6861), payment/growth visualization, multiplayer, demand, revenue, spend, payout, settlement, routing, or broader default-on claim is created.',
      'Registry 2026-06-29.4 advances autopilot.agent_character_creation.v1 from planned to yellow on source-level Autopilot Desktop evidence for #6861 (the desktop character-creation projection, onboarding-status projection, Forum intro module, read-only work-search module, unit tests, and headless proof/smoke harnesses cover the warp-in spawn/customize beats, one idempotent credential-gated Forum self-introduction, and a read-only work-search receipt path); is the #7030 yellow-only source-level receipt for autopilot.agent_world_scene.v1, autopilot.bitcoin_payment_visualization.v1, and autopilot.pylon_growth_visualization.v1 (the current source resolver defaults the scene/payment gates on under the Verse launch default while retaining the hard Verse kill switches and payment-source guards); and is the #7023 yellow-only Autopilot Desktop / builtin-compute proof destale, where autopilot.desktop_gui_client.v1 records the owner-run AO6 from-DMG clean-Mac evidence bundle (notarized DMG 20260619T010148, Gatekeeper-accepted app, rendered-window screenshots, production Pylon pylon.fa4e9049a4329f3d56e2, a Verified exact_trace_replay challenge, and a settled real-Bitcoin receipt) replacing the stale from-DMG-owner-gated blocker with owner-review/green-pending, while autopilot.builtin_compute_agent.v1 records only source/test/projection evidence for the bounded no-user-key metering smoke and does not clear its signed-recut, live-from-install, or metering-live-smoke blockers. It flips NO promise state and is NOT a green/default-on production claim for any of these records: both promises stay yellow, and green remains blocked on owner-accepted real-user/default-on evidence, permissioned live receipts, a shipped-channel visual receipt, and receipt-first owner review. No broad default-on production claim, surprise Forum posting, paid-work acceptance, spend, payout, settlement, moderation, or green transition is created.',
      'Registry 2026-06-29.3 is a receipt-gate tightening pass for autopilot.agent_character_creation.v1 / #6861 and flips NO promise state. The planned character-creation promise now cites the active public issue and names the three concrete closure receipts: built spawn/customize onboarding, automated Forum self-introduction demonstrated end to end, and automated work-search covered by tests. The promise remains planned; no spawn flow, Forum posting authority, work-search automation, spend, payout, settlement, or green claim is created.',
      'The Pylon launch-promise inventory is represented one-for-one in the promise records above.',
      'Episode 199 is included with a heavy historical caveat: Claude Code-first mech-suit language is withdrawn as current public framing; current coding-agent runtime claims should point to Codex-oriented Autopilot/Probe/Pylon records.',
      'Pylon v1.0 has a stable source cut in the monorepo, but broad Pylon earning, paid settlement, training, data revenue, referral payout, and labor-market claims remain gated.',
      'OpenAgents does not resell, rent, proxy, or broker subscription or API provider capacity. The labor market pays contributors for accepted work output produced with their own compliant provider usage; the former subscription/prepaid capacity promises are folded into provider.compliant_usage_labor.v1 under that boundary.',
      'The five-streams implementation plan is tracked in GitHub issues #4635-#4653 (rails, compute, data, labor, referrals, stacking smoke, tips polish); the lane map and binding delegation contract live in apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md.',
      'Owner decision 2026-06-10: the Tassadar research lane previously held no registry promise by design. The owner approved one scoped exception: compute.tassadar_executor_poc.v1, a bounded proof-of-concept promise for executor-trace work on real Pylons. All other Tassadar publication gates remain closed; the research essay is docs/tassadar/README.md.',
      'artanis.tassadar_evolution_loop.v1 is the owner-approved follow-on to the Tassadar PoC: an automated, publicly monitorable Artanis run that advances the executor lane and the Tassadar training corpus. The production tick model and evolution-loop design live in docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md; tracking issue #4697.',
      'compute.tassadar_executor_poc.v1 went green on 2026-06-10 citing transition receipt promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd. Green covers exactly the scoped PoC claim (one workload family, one real Pylon, worker-as-validator replay, one settled paid closeout); the unsafeCopy line is unchanged and binding, and no broader executor capability, performance, or earning claim is created.',
      'Episode 236 uses the canonical Tassadar spelling. models.tasadar_percepta_executor.v1 is retained only as a withdrawn typo alias; new reports and code should use models.tassadar_percepta_executor.v1.',
      'The full training-pipeline buildout plan is docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md. The training.* promises represent its workstreams (ablation system, data refinery, model ladder, marathon operations, post-training arc, verification classes, device-capability dataset); all enter as planned. Model-ladder rungs gain their own promise records before each run; no rung above the R0 tri-host rehearsal exists or is scheduled against a date.',
      'Demand-provenance rule (proof.demand_provenance.v1): internal or first-party demand — including the training pipeline’s own ablations, sweeps, and corpus work — is plumbing proof, not market proof. No external dollar, no demand claim; revenue-bearing public numbers must carry an internal/external split alongside modeled/measured/settled provenance.',
      'The Smol Training Playbook chapters mirrored at psionic docs/smol guide the pipeline’s operational shape; its measurements (MFU, bandwidth, mixture ratios) are external priors on other hardware, never OpenAgents claims.',
      'External-reference absorption 2026-06-10: the QVAC edge-stack analysis (docs/training/2026-06-10-qvac-edge-stack-analysis.md) feeds the training workstreams through psionic issues #1115-#1118 (ternary determinism receipts, Philox seeded-work RNG, instruct SFT lane, derisking-ledger entries) and spec input on #4681. It creates no new promises, and by owner decision local image/video generation via that stack is not pursued at this time.',
      'External-reference absorption 2026-06-12 (registry 2026-06-12.5): the Pluralis-to-Pylon adaptation roadmap (docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md, master issue #4855) landed its full P0-P2 contract scope same day across openagents #4848-#4854 and psionic #1124-#1128: contributor join-lifecycle ladder, window-seal staleness/churn/verification-overhead fields, bootstrap-from-durable-seal with a join barrier, reasoned device-admission gates with host-RAM and thermal probe kinds, staleness-priced acceptance with sync_reentry routing, presence/compute receipt tiers with a Sybil-priced cap, shadow-window join ramp with type-level merge exclusion, collective failure semantics, the pre-registered SPARTA canary harness behind the W3 no-public-gradients standing order, and the PowerSGD-Freivalds answer (compression composes with verification algebra, not provenance). Everything is contract/harness-level: no promise changes state on this work, every hardware- and settlement-gated acceptance bullet is recorded per issue, and the SPARTA canary outcome is pending by typed rule until a real gated run binds its pre-registration digest.',
      'The public code map records where shipped public code lives in the open source repository. Report stale or missing source links in the Product Promises Forum.',
      'Forum direct BOLT 12 tipping uses MDK/provider payment evidence as the source of truth; the public promise stays yellow until strict funded live smokes and webhook callback evidence pass without timeout recovery.',
      'Claude/Codex parity pass 2026-06-12 (registry 2026-06-12.4): both coding-agent lanes now carry live production task receipts — Claude via #4755/#4756 closeouts, Codex via the CX lane (epic #4793, live device closeout assignment.closeout.f264043a9f173b20514521da plus the API-parity git_checkout leg). The stale "no live Claude run" copy was corrected; the Claude bridge blocker narrowed to packaged-binary repeatability; the Codex successor blocker live_probe_pylon_runtime_gates_incomplete was cleared on the CX4 evidence, which is the maintainer edit the receipt-first transition proposal on the Forum asked for. No state flips were auto-applied at .4. At 2026-06-12.6 the codex successor flipped to green receipt-first (#4857): registry .5 served from worker 44109e05, then transition receipt promise_transition_c2328369-09d0-4972-9a3b-8d530440cade recorded with all mechanical checks passing, then this source flip. The Claude bridge stays yellow on the packaged-binary repeat (#4858/#4859). The local supervised daily-driver surface exists for Codex in source (#4839-#4841) and is tracked for Claude as issues #4844-#4847; the parity audit is docs/autopilot-coder/claude/2026-06-12-pylon-claude-codex-parity-audit.md.',
      'Registry 2026-06-14.1 reconciliation: the agent labor market crossed its first end-to-end milestone on 2026-06-14. The first live negotiated, escrowed, executed, validator-accepted, settled job ran against real backlog issue #4773 (evidence bundle docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md, work request b74bb55c, forum topic 098e36a8, kind-5934/7000/6934 events, escrow reserve/release receipts, bun-test verdict, terminal state settled). On that evidence labor.forum_work_requests.v1 and labor.nostr_negotiation_market.v1 are green, provider.compliant_usage_labor.v1 is yellow (output-only compliant labor paid for accepted work; first settlement was credit-ledger, not external ladder), and autopilot.control_center_fanout_marketplace.v1 is yellow (lane-C single-order fanout #4783). These four flips were applied in source under owner authorization 2026-06-14 ahead of the receipt-first operator-route transition receipts; the matching promise_transition receipts were recorded on 2026-06-15 via the operator route (#5017) — exception receipts (flip already applied, from-state equals to-state), dereferenceable at /api/public/product-promises/transitions, per proof.claim_upgrade_receipts.v1. artanis.labor_requester.v1 stays yellow (no unattended Artanis labor request has settled).',
      'Registry 2026-06-14.1 added eight conservative new records for wave-3 Autopilot Sites / Agency Pack and client surfaces (#4977-#4995): autopilot.desktop_gui_client.v1 (yellow, local-only), mobile.autopilot_remote_control.v1 (planned), workrooms.omni_client_delivery_workrooms.v1 (red), autopilot_sites.native_email_sequences.v1 (yellow, no send service), autopilot_sites.custom_tenant_hostnames.v1 (yellow, no self-serve/SSL), autopilot_sites.partner_payout_ledger.v1 (red), autopilot.cloud_credits_ui.v1 (yellow, presentational), mobile.voice_session_evidence_transcript_ingest.v1 (red, contracts only). All are operator-gated or pre-customer; none claims green. The reconciliation record is docs/promises/2026-06-14-registry-reality-reconciliation-audit.md.',
      'At registry 2026-06-14.1 the decentralized-training launch had not yet happened. That has since changed: as of 2026-06-18 the run is live (run.tassadar.executor.20260615 active, self-serve claiming open) and training.decentralized_training_launch.v1 is green on real settled receipts (~1,005 sats to independent contributors plus a 75-sat hygiene settlement). The remaining training.* launch promises stay red/yellow until each yields its own public run state, participant admission, accepted-work, validation, and settlement receipts. The W3 student-program report (docs/tassadar/2026-06-14-w3-student-program-report.md) is research/evaluation only and moves no promise.',
      'Registry 2026-06-14.2: Coder Cloud is the current top priority. New record autopilot.cloud_coding_sessions.v1 (red) tracks running coding sessions on OpenAgents Cloud (Google GCE first, SHC second) and administering them remotely so work continues while the owner travels; the 9 open issues are epic #4996 plus Phase 1-3 #4997-#5004. Foundation (C-0..C-15, #4886-#4901) is closed but the desktop->Google-GCE end-to-end loop is not demonstrable yet (Phase 1 revalidates). mobile.autopilot_remote_control.v1 is the Expo app (Phase 2-3; the iOS Swift control app is ignored per owner direction 2026-06-14) and depends on the Pylon remote bridge transport (#5000). autopilot.decision_queue.v1 cross-client exactly-once work is #5004.',
      'Registry 2026-06-14.2: the wave-3 Agency Pack initiative (epic #4973 + 21 children, including #4993/#4994/#4995) closed 2026-06-14 (~375 new tests green, typecheck:api + apps/web clean, build:web succeeds, OpenAPI gate green, migrations 0180-0182 and 0184). Reflected: workrooms.omni_client_delivery_workrooms.v1 -> yellow (client-delivery workroom page live-wired into the logged-in loop with CRUD/lifecycle/bundle/handoff routes and client-scoped views; source authority + approval-gated writes still pending). Desktop PDF/preview/ingest/browser cores are built behind seams with fakes (34 tests) but live runtimes are unwired (autopilot.desktop_gui_client.v1). The honest residue is config/credentials/product decisions, not code: custom hostnames need CLOUDFLARE_API_TOKEN+CLOUDFLARE_ZONE_ID and a mounted provision route; partner payout needs owner sign-off on percentage/caps plus settlement wiring; voice needs an STT vendor + capture path; the form-capture route needs a home for site form-specs. Per openagents/CLAUDE.md these surfaces were filed as GitHub issues by explicit owner instruction though the repo convention reserves issues for strict bugs (feature work Forum-first) - flagged for reconciliation.',
      'Registry 2026-06-14.3: Nostr resilience. AGENTS.md now carries a firm falldown instruction — on any OpenAgents infrastructure outage, agents keep retrying with backoff/idempotency AND coordinate over Nostr (NIP-01/02/17/29/38/65/90 on the owned relay wss://relay.openagents.com and public relays) until OpenAgents recovers, then reconcile on OpenAgents as authority of record. New record agents.nostr_fallback_coordination.v1 (yellow): the relay + NIP-90 negotiation are live (first labor job settled over the relay, #4777) and Pylon v1.0 provisions Nostr credentials, but an end-to-end coordination-during-outage drill is not yet demonstrated. Nostr is a communication/coordination substrate and outage fallback, never a replacement for OpenAgents authority during normal operation.',
      'Registry 2026-06-14.3: Coder Cloud contract layer started landing (concurrent agent work merged): the lane selector auto|local|cloud-gcp|cloud-shc is wired end to end (#4998), the Vortex-independent Codex grant endpoint contract is in place (#4999), and the cloud placement endpoint shipped Google-first (cloud #86/#87/#88). The remaining seam to a live loop is #4997: cloud-gcp spawns still execute locally and per-session GCE provisioning is unwired. autopilot.cloud_coding_sessions.v1 stays red until the desktop->GCE dispatch loop is demonstrable.',
      'Registry 2026-06-14.4: Coder Cloud Phase 1 contract layer fully landed (#4997 Pylon cloud dispatch to the placement endpoint with local fallback, plus cloud #90 GCE lease lifecycle, on top of #4998/#4999/cloud #86-#88). autopilot.cloud_coding_sessions.v1 stays red: live GCE provisioning is a fake-default ADC-gated stub and the cloud.gce.* event kinds + resource_usage_receipt ref do not round-trip to the desktop yet (#5005, open). This registry version is the one deployed after the zero-debt architecture gate (check:architecture) was brought back to green: the wave-3 comment-only false positives were reworded, the 7 raw JSON.parse calls were routed through the parseJsonUnknown json-boundary helper, and three migration-bridge budgets (route Effect.promise adapters 8->18, Worker Response surfaces 80->83, index.ts runPromise allowlist 6->7) were raised under owner authorization with ratchet-down notes.',
      'Registry 2026-06-29.1: issue #6831 clears the code/test slice of cloud_gce_event_kinds_do_not_roundtrip_5005 without flipping autopilot.cloud_coding_sessions.v1 green. The Pylon cloud HTTP client now exports and tests the full openagents.codex_workroom_event.v1 kind set, normalizes the cloud.gce.resource alias to cloud.gce.resource_usage_receipt, rejects unknown event kinds instead of inventing timeline phases, and keeps cloud rows lane-transparent through the desktop bridge row shape. The promise remains red until a real desktop-originated GCE repo-edit run produces a content-addressed artifact, dereferenceable usage receipt, and owner sign-off.',
      'Registry 2026-06-17.3: the Spark agent-wallet unification epic #5176 closed end-to-end. Offline receive is recipient-confirmed by two independent contributors (Trigger 50k, Whitefang 50k) with payments.offline_receive_spark_fallback.v1 green; Spark is now the primary agent balance (#5178); and consented owner-approved Spark send/withdraw (`wallet send --rail spark --confirm-send`) is shipped and verified on Pylon rc.16 with real 100-sat and 878-sat sends (state=sent/status=completed, balance debited). The original offline-receive gap #5078 and the rc.13 send/version bugs #5184/#5185 are closed. This version updates the offline-receive record’s evidence/verification only — the claim stays receive-only and no new public send/withdraw claim is asserted; a dedicated consented-spend/withdraw promise remains an owner product-claim decision. Host-specific balance-read follow-up is tracked in #5194 (rc.17 candidate fix + PYLON_SPARK_DEBUG diagnostic).',
      'Registry 2026-06-18.4 adds one conservative new record, compute.agentic_kernel_optimization_at_scale.v1 (red): coding agents continuously writing and optimizing inference kernels across open models and device types, scored on BOTH throughput (tok/s) AND output-parity (an optimized kernel must reproduce identical outputs, verified by exact replay on an independent device), dispatched and paid through the verified-work market as part of the decentralized inference/training mesh. This is the direction, not a shipped network capability. The only demonstrated piece is a single historical development result on a public build-series video (docs/transcripts/217.md): in March 2026 an agent wrote custom CUDA kernels that took the OpenAgents Rust ML library Psionic to ~523 tok/s versus a leading local inference runtime\u2019s ~328 tok/s on the smallest Qwen 3.5 model, beating that runtime on the four smallest Qwen 3.5 models on a single machine. That demo is NOT a dereferenceable on-chain/worker receipt, so nothing here is green; the record stays red with the demo flagged as historical-demo evidence only. The output-parity correctness anchor reuses the exact-trace-replay verification proven under compute.tassadar_executor_poc.v1, and the payment rail reuses the verified-work labor market proven under labor.forum_work_requests.v1 / labor.nostr_negotiation_market.v1 \u2014 cross-referenced, not duplicated. Green requires a market-dispatched agent-authored kernel, a public tok/s improvement against a named baseline on declared hardware, an independent exact-replay output-parity verdict, accepted-work and settlement receipts, and a participant/scale methodology; per proof.demand_provenance.v1 internal first-party optimization is plumbing proof, not market proof. No promise_transition is required (a new red record creates no state flip).',
      'Registry 2026-06-18.5 adds four conservative new records from the Episode 238 ("The Tassadar Run is Live") launch-readiness audit (docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md), and flips no existing promise. claims.world_first_ai_training_paid_bitcoin.v1 (red) and claims.world_first_public_llm_computer_training_run.v1 (red) hold the two on-camera "world first" claims as gated-pending-verification. An independent web-research prior-art review now exists (docs/launch/2026-06-18-world-firsts-verification.md, #5395) and finds both claims defensible only with their full qualifiers (claim 1: "Bitcoin + replay-verified training compute + own consumer devices" together; claim 2: "public/open-contributor LLM-computer training run," crediting Percepta as the paradigm originator); green still requires a dereferenceable evidence pack plus an owner-signed receipt-first upgrade, and the LLM-computer record additionally fixes the boundary that the live run is bounded exact-trace executor PoC work with no gradient descent (compute.tassadar_executor_poc.v1), not gradient-descent model training. pylon.consumer_compute_earns_bitcoin_self_serve.v1 (red) holds the video core promise ("anybody plugs in consumer compute and gets paid Bitcoin"). Registry 2026-06-19.1 supersedes the original blocker list: default npm is now on the v1.0 line and one auto-stream visibility capture exists, but the broad claim remains red on scale methodology, Windows/WSL coverage, and Spark-helper auto-start/readiness evidence. marketplace.agentic_npm_module_registry.v1 (planned) holds the "learning by construction" agentic-npm module marketplace, which the transcript itself frames as an upcoming-video reboot of the earlier plugin marketplace. No promise_transition is required (four new records create no state flips); upgrades remain receipt-first per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-18.6 is a copy-only Pylon release-line reconciliation after the stable v1.0 source cut landed. It updates current registry summary/caveat language, AGENTS.md install truth, and launch-copy guards from stale v0.3/RC wording to v1.0 wording while preserving all earning, assignment, settlement, and marketplace blockers. No promise state flips, no owner-signed transition receipt, and no new payout authority are created; L-1 still closes only when the default install path is receipt-proven.',
      'Registry 2026-06-18.7 normalizes public evidence URLs for agent dereference. It adds no new product capability or authority; it points settlement evidence to the public settlements alias and challenge evidence to the public verification-challenge alias.',
      'Registry 2026-06-18.8 is a copy-only post-launch reconciliation. Stale pre-launch narrative that presented the decentralized-training launch as "imminent but has NOT happened" / "rails-ready is not launched" (point-in-time copy anchored to 2026-06-14) is corrected to current reality: the launch has happened, run.tassadar.executor.20260615 is live and active, self-serve claiming is open, and real Bitcoin has settled to independent contributors with public receipts. This is description/narrative accuracy only — no promise state flips, no new owner-signed transition receipt, no widened scope, and no new payout authority. Every red/yellow/planned promise keeps its state and its own receipt-first upgrade gate; the launch happening does not green any claim by itself.',
      'Registry 2026-06-19.1 aligns the product promises and launch docs with issue #5438. It records the exact auto-stream visibility capture for training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4: public activity timeline, generated replay proof_replay_bundle.public_activity.73e66071, receipt receipt.nexus.tassadar_run_settlement.idempotency.tassadar.autostream.training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4.worker, and docs/launch/2026-06-19-autostream-settlement-clip-manifest.json. It also removes the stale default-npm blocker because npm reports @openagentsinc/pylon@latest=1.0.5 on 2026-06-19. This version flips no promise green: pylon.consumer_compute_earns_bitcoin_self_serve.v1 remains red on scale methodology, Windows/WSL coverage, and Spark-helper auto-start/readiness evidence; world-first claims remain red pending owner-signed receipt-first upgrades; and no new payout, settlement, provider, wallet, deployment, or public-claim authority is created.',
      'Registry 2026-06-19.2 is a copy/evidence destale pass against what actually shipped to main, and flips no promise state. (1) autopilot.desktop_gui_client.v1 stays yellow but records the auto-onboarding EPIC #5441 (AO-1..AO-6, commits #5442-#5448): first-run self-register, AO-3 identity choice (create-new/named or detected use-existing with the seed marker read-by-presence-only and never overwritten), AO-4 wizard live-state projection from real observed signals, a black-screen Document-contract guard, and an AO-6 headless smoke that drives the REAL local Pylon node through the launcher against a mock Worker (apps/autopilot-desktop/scripts/auto-onboarding-e2e-smoke.ts). The EPIC final from-DMG proof — a rendered window from the signed DMG on a clean external Mac, real appearance on production /api/public/pylon-stats, and a real claimed+settled Tassadar window with a Bitcoin receipt — is owner-gated and pending per docs/launch/2026-06-18-autopilot-desktop-ao6-from-dmg-runbook.md (new blocker.product_promises.autopilot_desktop_from_dmg_proof_owner_gated). (2) sites.referral_bitcoin_stream.v1 stays yellow but records RL-1 #5458 (commit 2c83afd4f): the 5% referral payout ledger is now wired — site-referral-payout-feed.ts creates exactly one idempotent eligibility row per paid event (hooked into Stripe credit-purchase fulfillment, short-circuiting self/no-attribution) and site-referral-payout-dispatch.ts drives approved -> dispatched -> settled by invoking the injected MDK/Spark payout adapter before recording settled, readiness-gated by the MDK payout-mode projection and refusing non-Bitcoin (credit/USD) revenue, proven settle-at-most-once by site-referral-payout-wire.test.ts (mock adapter + in-memory D1). No real referral payout has settled (new blocker.product_promises.referral_first_real_payout_pending); the first real settled payout awaits a real Bitcoin-revenue production event and stays readiness-gated. (3) autopilot_sites.partner_payout_ledger.v1 stays red but cross-references the now-wired referral dispatch rail as a tested attribution->settlement reference implementation (new blocker.product_promises.partner_first_real_payout_pending); green still needs a partner-specific attribution policy, a real settled partner payout receipt, and a partner-facing projection. No promise_transition is required (no state flips); any future green flip remains receipt-first per proof.claim_upgrade_receipts.v1 and requires owner sign-off.',
      'The OpenAgents inference gateway / Agent Cloud vision (docs/inference/, EPIC #5474 + sub-EPIC #5475 + children #5476-#5491) is recorded across five records at registry 2026-06-19.4: it is roadmap, not a shipped product. There is no live inference API, no inference credit balance, and no customer-buyable inference today. The single piece with real evidence is a VERIFIED Fireworks provider connection (a real usage-returning OpenAI-compatible call on an OpenAgents-held key), which is upstream reachability and a cost basis only — not a sellable inference product, and deliberately not green. Cross-category referral-on-everything-forever and the one-balance one-stop Agent Cloud are design intent. Report inference-product copy gaps in the Product Promises Forum.',
      'Registry 2026-06-19.6 makes the registry reflect Episode 239 ("Let\'s Make Money" / Closing the Revenue Loop, docs/transcripts/239.md) HONESTLY, and flips NO existing promise — the green count stays 20. It adds nine conservative new records, all red or planned, never green: referral.refer_once_earn_forever.v1 (red — the ecosystem-wide refer-once-earn-forever vision; only the Sites 5% ledger is wired and it has settled NO real payout, distinct from the planned inference referral slice); autopilot.all_in_one_business_system.v1 (planned — Autopilot-as-composed-business-system); cloud.primitives_suite.v1 (planned — the inference/fine-tuning/training/agentic-tasks/sandbox/web-services primitive set as one buyable suite); cloud.fine_tuning_service.v1 (red — fine-tuning as a sellable primitive, unbuilt); cloud.sandbox_compute_service.v1 (red — sandbox compute as a sellable primitive, unbuilt); markets.open_protocol_markets.v1 (planned — the six Episode 213 markets compute/data/labor/liquidity/risk/verification; labor and verification are green only in their own bounded scopes, liquidity and risk are unbuilt); marketplace.compose_and_list_products.v1 (planned — build-your-own-product-and-list-it-for-sale); marketplace.monetize_any_layer_with_referral.v1 (planned — sell access to any layer + earn referrals, never waiving the no-resale invariant for subscription accounts); and the two PURSUED, aspirational world firsts claims.pursued_world_first_largest_agentic_sales_force.v1 and claims.pursued_world_first_largest_sales_force.v1 (both planned and intentionally never-green-from-aspiration — the video states them as pursuits, not achievements). The headline gap to make the video real is wiring the referral payout end-to-end (a real paid event -> the wired ledger -> a dispatched MDK/Spark settlement -> a dereferenceable receipt). Sources: docs/transcripts/239.md, docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md, docs/launch/2026-06-19-credits-purchase-collect-money-audit.md, docs/launch/2026-06-19-near-term-product-priorities.md. No promise_transition is required (new red/planned records create no state flip); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      "Registry 2026-06-19.7 advances the Episode 239 revenue scaffolds toward real WITHOUT flipping any promise — the green count is unchanged and every touched promise stays red/planned. (1) It closes the gap between the two siloed referral scaffolds: marketplace-monetize-any-layer.ts only PLANNED a cut (pure, no ledger path) and referral-cross-category-accrual.ts was the category-agnostic ledger entry point with no caller. The new marketplace-monetize-any-layer-accrual.ts bridges them — it runs the SAME no-resale / asset-boundary / self-referral guards, and ONLY when the flag is armed AND the plan is authorized does it feed the qualifying spend (msat -> whole sats) into the ONE RL-1 ledger via accrueCrossCategoryReferral, paying the referee's ATTRIBUTED referrer (never a seller-asserted one), idempotent per (layer, event). It is FLAG-GATED INERT: the default path computes the plan and touches no ledger, and even when armed it records eligibility only — settlement stays on the readiness-gated, owner-armed dispatch rail. (2) It adds the compose-and-list -> monetize-any-layer seam (composedProductMonetizableLayers in marketplace-product-composition.ts), a pure projection from a composed product's primitives to the monetizable layers a builder could attach an offer to. These advance referral.refer_once_earn_forever.v1 (binding/accrual robustness), marketplace.monetize_any_layer_with_referral.v1, and marketplace.compose_and_list_products.v1 as INERT scaffolds. referral.refer_once_earn_forever.v1 stays RED and owner-gated on the first settled payout (#5511/#5512); none of this work flips it. No promise_transition is required (no state flips); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.",
      'Registry 2026-06-19.7 is a training.* evidence/copy destale that makes the registry match the LIVE Tassadar run, and flips NO existing promise — the green count stays 20. The live per-run settled feed GET /api/public/training/runs/run.tassadar.executor.20260615/settlements has moved ahead of the prior 2026-06-18.3 copy: it now enumerates FIVE counted realBitcoinMoved:true settlements (1,020 sats real total — the 1,000-sat canary plus four 5-sat self-serve settlements) to FIVE distinct independent contributor pylons, each backed by a Verified exact_trace_replay challenge, plus one excluded realBitcoinMoved:false simulation row; the run summary reports qualifiedContributorCount 5 and acceptedTraceCount 11 with providerConfirmedSettledPayoutSats 1,020. The three new dereferenceable real receipts are receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.final.20260619T003201.manual.v1 (pylon.f0504556ad67bb4efe93, challenge 335df7e8-2ae1-4d49-a6bd-c1491bc9f067), receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched.20260619T004804.manual.v1 (pylon.58b7f3c009224f3642fa, challenge 33d4ca81-8beb-4d80-a90c-cb21d6d0aeb1), and receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1 (pylon.fa4e9049a4329f3d56e2, challenge 9fd49062-f82c-46ee-a2a0-242d36dd126e), all moneyMovement:real_bitcoin, state:settled, adapter:spark_treasury. Effects, all receipt-anchored, none a state flip: (1) training.decentralized_training_launch.v1 stays GREEN and its copy moves from "two distinct independent contributors / 1,005 sats" to "five distinct independent contributors / 1,020 sats" (green->green, no promise_transition required; an exception receipt may be recorded against the deployed 2026-06-19.7 version per proof.claim_upgrade_receipts.v1 if owner review wants one). (2) training.public_distributed_training_run.v1 stays RED but its "payment and settlement refs for more than one contributor" criterion is now satisfied by the five distinct settled contributors; its remaining gate narrows to a documented participant-count/network-scale methodology and broad accepted-work receipts beyond the five canary-scale settlements — it is the training.* record CLOSEST to a yellow upgrade and the fastest owner-gated win. (3) training.verification_classes.v1 stays YELLOW; exact_trace_replay is now exercised on real dispatched work across five distinct paid contributors (broadening the existing three-classes-on-real-work evidence), and its only blocker remains aggregate_only_policy_redecision_missing (#4674), unchanged. (4) training.post_training_arc.v1 stays PLANNED but gains a previously-uncited real paid-run evidence ref: the 2026-06-11 CS336 A5 alignment rollout/grading paid run (run.cs336.a5.alignment.demo, public eval eval.cs336_a5.synthetic_math.bounded_combined.4682.1 at GET /api/training/evals/a5, four Verified challenges incl. training.verification.challenge.cb1d4f39-5b33-4650-8659-afcc33131af5, ~40-sat real settlement) proving rollout-generation and reward-grading as paid network work; the lane stays planned because no SFT/preference-optimization stage was dispatched and no reviewed vibe-test artifact exists (instruct_sft_lane_missing, preference_rollout_work_missing, vibe_test_artifact_missing all hold). (5) training.data_refinery_corpus.v1 stays PLANNED with its uncited evidence freshened: the a4_eval_delta leaderboard lane is live-but-empty in workers/api/src/training-leaderboards.ts and the payment policy is docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md; green still needs one verified deterministic_recompute refinery shard with an eval-delta payment receipt. training.public_gradient_windows.v1, training.full_pipeline_program.v1, training.ablation_system.v1, training.model_ladder.v1, training.marathon_operations.v1, and training.device_capability_dataset.v1 are unchanged — no new settled receipts exist for them. Source: docs/promises/2026-06-19-training-live-run-evidence-destale.md. No promise_transition is required (no state flips); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-19.8 is an evidence-assembly pass over the ten non-green pylon.* promises (the "weekend promise assault"), and flips NO promise — the green count stays exactly 20. It adds three dereferenceable evidence docs and one INERT flag-gated capability, then attaches them to the relevant promise records: docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md writes down the qualified-contributor counting rule (admitted + accepted replay-verified work + public-safe provider-confirmed settlement receipt; never raw registrations or stale heartbeats), exactly as enforced in training-run-window-authority.ts, giving the consumer-self-serve scale-methodology blocker and the largest-run participant-methodology blocker a citeable home; docs/training/2026-06-19-comparable-decentralized-training-runs-research.md documents the comparable decentralized runs with citations (Templar Covenant-72B ~70 contributors as the cited largest; ~200 is the Episode 236 transcript target), addressing the comparable-evidence gap; and apps/pylon/src/spark-helper-autostart.ts (+ .test.ts, 9 tests) adds an INERT, default-off (PYLON_SPARK_AUTOSTART) Spark-helper autostart readiness classifier and public-safe receipt builder for the spark-helper-autostart blocker, with no live behavior change and no raw target/balance/credential leakage. The full per-promise green-readiness assessment (what was built, the exact receipt each promise still needs, and the owner gate) is docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md. CRITICAL: these are written methodology, research, and an inert capability only — they do NOT manufacture scale, do NOT prove a real autostart-ready contributor receipt, do NOT publish any npm package or signed binary feed, and do NOT move money. pylon.largest_decentralized_training_claim.v1 stays red (two counted contributors, far below comparable scale; public_training_contributor_receipts_missing unmet); pylon.consumer_compute_earns_bitcoin_self_serve.v1 stays red (autostart capability is inert and unfired, Windows/WSL is a deliberate owner scope-out so the honest path is narrowing the "anybody on any platform" copy to macOS/Linux); both yellow release promises still need the owner-gated signed-binary feed rollout; the three planned GEPA/data/multi-earning promises still need their first settled receipt. No promise_transition is required (no state flip); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-19.7 is the weekend-assault DE-2/DE-10 build+destale pass over the inference/cloud/compute/energy non-green records, and flips NO promise — the green count is unchanged. (1) cloud.fine_tuning_service.v1 and cloud.sandbox_compute_service.v1 STAY red: their flag-gated INERT scaffolds (EPIC #5510/#5516/#5517) were advanced with an OpenAI-shaped lifecycle READ (GET /v1/fine_tuning/jobs/:id, GET /v1/sandboxes/:id) enforcing cross-account isolation at the adapter get-seam, and a REAL receipt-first credit-metering seam shared via apps/openagents.com/workers/api/src/cloud/cloud-metering.ts that decrements credits through the same atomic PayIn ledger the inference gateway uses — proven against real node:sqlite SQL (migration-0160 constraints) to never go negative and to be idempotent per unit (cloud-metering.test.ts; 43 cloud tests pass). They ship defaulted to the stub runtime adapter (no persistence) and the no-op metering hook, so prod stays inert and bills nothing; green still needs a real runtime, live pricing, and a dereferenceable PAID receipt + owner sign-off. The stale "NOT built / no intake" copy was corrected to "flag-gated INERT scaffold exists." (2) inference.fireworks_open_model_provider.v1 STAYS yellow: the live provider connection was independently re-verified 2026-06-19 (real OpenAI-compatible glm-5p2 call, HTTP 200 + usage object, key never printed; docs/inference/2026-06-19-fireworks-provider.md); green still needs the owner-gated PAID open-model path. (3) energy.flexible_load_proof.v1 STAYS planned: a MODELED operator proof report comparing accepted-outcome/AI/mining/grid-service/smoothing/forward-capture/curtailment/reserve/idle states in $/MWh with per-row evidence labels now exists (docs/metrics/2026-06-19-flexible-load-operator-proof-report-modeled.md), every figure modeled except the one receipt-backed #4777 datapoint; green still needs live energy ingestion, work-class flex profiles, and a real flexible-load event history. (4) compute.agentic_kernel_optimization_at_scale.v1 STAYS red: a public kernel-optimization work definition + throughput-parity protocol bound to the green exact-trace-replay engine and verified-work rail now exists (docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md), DEFINED not executed; green still needs a market-dispatched kernel, a named-baseline tok/s record with a Verified parity verdict, an at-scale run, and settlement receipts. inference.gateway_credits_business.v1 (red, paid-credits path owner-gated), inference.referral_on_all_inference.v1 (planned, owner-gated payout), inference.decentralized_serving_fabric.v1 (red, serving runtime + owner-armed payout), and the cloud capstone records are unchanged. Sources: docs/inference/2026-06-19-cloud-primitives-fine-tuning-sandbox-scaffold-advance.md and the docs cited above. No promise_transition is required (no state flips); any future green flip remains receipt-first and owner-signed.',
      'Registry 2026-06-19.8 is the weekend-assault wave-2 training-methodology evidence pass over the two non-green training.* records closest to a win, and flips NO promise — the green count stays exactly 20. It publishes two dereferenceable docs and attaches each as an evidence ref. (1) training.public_distributed_training_run.v1 STAYS red: docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md now writes down the participant-count/network-scale methodology — it reuses the code-anchored qualifiedContributorCount counting rule (admitted + accepted exact_trace_replay-verified work + public-safe provider-confirmed realBitcoinMoved:true settlement receipt; never raw registrations or stale heartbeats), defines the three scale axes (distinct-contributor, accepted-work, settlement) each derived from training-run-window-authority.ts metrics with their dereference routes, and states the network-scale threshold (>= 50 qualified contributors with sustained accepted work and broad real settlement, deliberately distinct from the 200-contributor largest-run benchmark). The honest current scale is five canary-scale contributors / 11 accepted units / 1,020 sats. Its remaining gate therefore narrows from "documented-methodology + broad-receipts" to broad accepted-work receipts that clear that documented threshold; the methodology is no longer the blocker. (2) training.model_ladder.v1 STAYS planned: docs/training/2026-06-19-model-ladder-rung-economics.md publishes the rung definitions (R0 exists, R1-R4 defined), the R1 closeout criteria (data/ablations/marathon/post-training/evals/economics-gate all receipted), and the per-rung economics-gate report format (allInCostPerAcceptedOutcome / contributorPayoutPerDeviceHour / verificationOverheadFraction / fallbackComparator / gateOutcome, each provenance-labelled modeled/measured/settled, with verificationOverheadFraction already live on the window-seal contract per #4849). This documents the rung_economics_gate_format_missing dimension; the remaining open blocker is r1_full_rehearsal_missing — no rung above R0 has run to a closeout receipt. CRITICAL: these are written methodology and economics-gate format only — they do NOT manufacture scale, do NOT run any rung, do NOT prove broad receipts, and do NOT move money. No promise_transition is required (no state flip); any future green/yellow flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-19.10 is a registry COMPLETENESS audit pass that adds two conservative new records for built-and-deployed capabilities that previously had NO registry record, and flips NO promise — the green count stays exactly 20. (1) payments.autopilot_credits_purchase.v1 (red): the card-to-USD-Autopilot-credit purchase machinery is built, wired end to end, and deployed (the /billing page, POST /api/billing/checkout -> Stripe Checkout Session, webhook fulfillment, the billing_ledger_entries USD ledger), but it is secrets-gated in prod (STRIPE_API_KEY / STRIPE_WEBHOOK_SIGNING_SECRET / STRIPE_CREDIT_PACKAGES_JSON) with no real card-to-credit purchase receipt, there is no Bitcoin/Lightning path to buy these credits, and these USD credits are separate from and do NOT fund the inference gateway (per docs/launch/2026-06-19-credits-purchase-collect-money-audit.md). This is the Episode 239 "collect real money -> credits" capability and was missing from the registry; the existing inference.gateway_credits_business.v1 covers a SEPARATE msat ledger and autopilot.cloud_credits_ui.v1 is explicitly a view-only workroom component that disclaims any purchase, so neither was the home for this card-to-credit purchase capability. (2) autopilot_sites.site_build_and_host.v1 (yellow): the CORE Autopilot Sites build-and-host product (create site, builder session with message/event/file/export/save-version APIs, stable live + durable revision URLs, and the agent Site action contract for project/session/preview/version-save/deploy-request) is live per AGENTS.md "what is live" and the agent-site/operator-sites routes, but the registry only carried Sites agency-pack ADD-ONS (referral, email sequences, custom hostnames, partner payout, client-delivery workrooms) and had no record for the base Sites product Episode 239 names as the web-services primitive; production deploy is owner/operator-gated, external-agent customer orders need a scoped grant, and no paid customer Site has a settlement receipt, so it is honestly yellow. Both records cross-reference the adjacent existing records rather than duplicating them. No promise_transition is required (two new red/yellow records create no state flip); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-19.12 turns the RL-1 Sites referral payout claim from "wired in source" into "wired + dereferenceable", and flips NO promise — the green count stays exactly 20. An external auditor (Orrery) evidenced the gap: referral.refer_once_earn_forever.v1 and autopilot_sites.partner_payout_ledger.v1 asserted the RL-1 ledger is wired end-to-end in source (#5458) but there was NO dereferenceable PUBLIC projection, so the wiring was asserted, not verifiable. This pass adds a read-only, public-safe, live-at-read projection GET /api/public/site-referral-payouts (workers/api/src/site-referral-payout-public-projection.ts + site-referral-payout-public-routes.ts) that composes over the latest non-archived ledger entry per payout ref, selecting ONLY state and amount_sats — no user id, attribution id, payout ref, qualifying event ref, address, preimage, or invoice leaves the Worker — and emits per-state counts/sats, the policy shape, campaign/policy refs, caveat/blocker refs, a ledgerWiredInSource source-wiring flag, and the projection_staleness.v1 contract. It is honest about the current state: no real referral payout has ever settled, so settledCount/settledSats are 0 while the wiring is present. The route is added to the projection inventory (INVARIANTS.md) and the zero-debt projection-surface ledger as staleness_declared, and attached as an evidence ref on both referral.refer_once_earn_forever.v1 (stays red/owner-gated) and autopilot_sites.partner_payout_ledger.v1 (stays red). Per Orrery\'s should-split note, built-vs-settled stay as ONE claim with the projection making the distinction explicit (settledCount=0) rather than a second promise record — the projection is honest enough that splitting would not add accuracy. GREEN still requires a real settled payout receipt plus owner sign-off per proof.claim_upgrade_receipts.v1. No promise_transition is required (no state flips); zero green flips.',
      'Registry 2026-06-19.13: FIRST green flip of the weekend assault — agents.nostr_fallback_coordination.v1 yellow -> green, owner-authorized 2026-06-19. The gate ("a demonstrated end-to-end outage-coordination drill") is MET by PR #5535 (merged 4c2242f8): apps/openagents.com/scripts/nostr-fallback-drill.ts runs the full outage sequence — NIP-38 liveness -> NIP-65/02 discovery -> NIP-17 encrypted DM -> NIP-90 LBR job lifecycle -> reconcile on recovery — with ephemeral per-run keys, an assertNoSecrets guard before every publish, and 11 fetchable public-safe event ids on wss://nos.lol recorded in docs/nostr/2026-06-20-outage-coordination-drill.md. Both blockers (nostr_outage_coordination_drill_missing, agent_nostr_messaging_tooling_incomplete) are cleared. The flip is applied in source ahead of the operator-route transition receipt (per the 2026-06-14 reconciliation pattern); the matching owner-signed promise_transition exception receipt is recorded against the deployed registry via POST /api/operator/product-promises/transitions, dereferenceable at /api/public/product-promises/transitions. Honest scope: a drill on a public relay demonstrates the coordination flow, not survival of a real production outage at scale. Green count 20 -> 21.',
      'Registry 2026-06-20.1 updates agents.cursor_forum_wallet.v1 to match the Spark-first Forum tip-recipient implementation (#5539). No state changes: the promise stays green, but the registration/default-readiness copy now names sparkAddress and directPayment.kind = "spark_address" as the primary rail, with BOLT 12 retained only as a fallback compatibility rail.',
      'Registry 2026-06-20.2: owner-decided green — training.verification_classes.v1 yellow -> green. The last open gate (#4674 aggregate-only vs per-contribution sampling) is decided in writing: per-contribution sampling default per class, aggregate-only deprecated (docs/promises/2026-06-20-verification-class-sampling-policy.md, owner-approved 2026-06-20). Other criteria already met (registry live; exact_trace_replay/deterministic_recompute/freivalds_merkle on real work; paid weak-device validator closeout #4676; exact_trace_replay already per-contribution across five paid contributors on run.tassadar.executor.20260615). Blocker cleared. Green 21 -> 22. Honest bound: not all five classes on real work / not at-scale. promise_transition via operator route per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-20.3: Pylon v1.0.5 signed-binary release shipped + verified — pylon.v03_release_candidate.v1 + pylon.release_tomorrow.v1 yellow -> green (owner-authorized 2026-06-20). Signed binaries (macOS+Linux, 4 targets) signed with pinned ed25519 kid 2dbe811d19f67528, published to updates.openagents.com (Cloud Run oa-updates-00041-b7b, rollout 100, full rc history preserved); live feed verified serving v1.0.5 with sha256+ed25519 against apps/oa-updates/keys/release-pubkey.json (fail-closed, tamper rejected); live network smoke registered+heartbeated online in /api/pylons as pylon.33afd48282a649047e3a (openagents.pylon@1.0.5). Both blockers cleared. Green 22 -> 24. Honest bound: macOS+Linux only (Windows out of scope); network-scale earning separate. Flip applied in source ahead of the operator-route transition receipt.',
      'Registry 2026-06-20.4: green-quality fix on pylon.release_tomorrow.v1 (conceding Orrery audit, forum topic 415e16a7) — its green previously rested on the sibling pylon.v03_release_candidate.v1 evidence array; it now carries its OWN dereferenceable refs (the signed darwin-arm64 feed https://updates.openagents.com/pylon/rc/darwin-arm64/feed.json and the live registration route:/api/pylons#pylon.33afd48282a649047e3a). No state change (stays green), green count unchanged at 24. The transitions-feed backfill for the four post-2026-06-17.5 flips remains the owner-gated item (prod admin token).',
      'Registry 2026-06-20.5 is a marketplace.signature_monetization.v1 de-stale pass and flips NO promise state. The already-deployed inert usage-metering model and public read-only projection (GET /api/public/markets/signature-monetization/metering) prove validation+metering can reach the metered rung while the promise remains red/inert and settlement stays blocked. This clears blocker.product_promises.signature_usage_metering_missing only; blocker.product_promises.signature_settlement_missing remains. No billing, pricing, rev-share, payout, settlement, or revenue claim is created.',
      'Registry 2026-06-20.6 is an autopilot_sites.partner_payout_ledger.v1 projection de-stale pass and flips NO promise state. The public-safe count-only partner payout projection (GET /api/public/partner-payouts) exposes aggregate current states, roles, assets, policy shape, and settled sats while withholding partner refs, user ids, payout refs, qualifying event refs, payout destinations, invoices, preimages, and provider payloads. This clears blocker.product_promises.partner_projection_api_missing only; partner attribution policy, settlement wiring, and first real payout remain blocked. No partner earning, withdrawal, revenue, payout, settlement, or green claim is created.',
      'Registry 2026-06-20.7 clears the Nostr-export blocker on identity.orange_check_forum_signal.v1 and flips NO promise — it stays YELLOW and the green count is unchanged at 24. The orange-check signal now has a real, dereferenceable Nostr export on the OWNED relay (#5537): apps/openagents.com/workers/api/src/orange-check-nostr-export.ts gains buildOrangeCheckNostrAttestation, a public-safe NIP-01 (kind 1) attestation note that references the NIP-58 badge definition address, the claim, the public receipt ref, the recipient pubkey, and the paid amount as tags (the NIP-58 badge kinds 8/30009 are NOT in the owned relay write allowlist, so a kind-1 attestation is the publishable form). apps/openagents.com/scripts/orange-check-nostr-export-publish.ts signs it, completes NIP-42 AUTH for the issuer key against wss://relay.openagents.com, publishes it over the gated general-coordination write path, and reads it back by id. The live publish produced event 83c450c97d6ee3ed624dd6ae0b12956f50a392a396322e65d04c1173c9a6b4da (issuer d4cd87a849944ac2dce93848c2007590993d7174bee14a9518c7e31891d6a471), independently dereferenceable via REQ {"ids":["83c450c97d6ee3ed624dd6ae0b12956f50a392a396322e65d04c1173c9a6b4da"]} on wss://relay.openagents.com. blocker.product_promises.orange_check_nostr_export_missing is cleared. The promise stays yellow because GREEN still requires one live $5 purchase settling through the production checkout with the badge visible on the buying agent, which is owner-gated. The export is event transport only and grants no identity-verification, moderation, settlement, or payout authority. No promise_transition is required (no state flip); any future green flip remains receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-29.3 is an issue #7015 reconciliation pass for identity.orange_check_forum_signal.v1 and flips NO promise state. The promise remains YELLOW: the Nostr export blocker is still cleared by the owned-relay event, but the remaining production green gate is now explicit in blockerRefs instead of only narrative copy. Green requires a dereferenceable live $5 production checkout receipt, public-safe proof that the badge is visible on the buying agent, and an owner-signed yellow->green transition receipt. No checkout was driven, no private provider payload is exposed, no badge/economic authority is broadened, and no promise_transition is recorded by this source-only reconciliation.',
      "Registry 2026-06-20.8 is a workrooms.omni_client_delivery_workrooms.v1 integration pass and flips NO promise state (stays yellow; green count unchanged). The source-authority + approval-gated write model (omni-source-authorized-business-objects.ts) and its FLAG-GATED INERT delivery seam (omni-workroom-business-object-delivery.ts) previously had no path onto the LIVE omni client-delivery workroom surface — that was blocker.product_promises.workroom_source_authority_not_integrated. This pass wires them in: a new decode-or-empty extractor (extractWorkroomSourceAuthorityInputs) reads source-authority bindings + proposed writes from a live workroom record's projection-only metadata.sourceAuthority block (an existing D1 field; no migration, no new state), buildOmniWorkroomSourceAuthorityDeliveryPlan composes that with the inert plan builder, and a new read-only, operator-gated route GET /api/omni/workrooms/:id/source-authority surfaces the INERT delivery plan for a real workroom on the live route surface that is already mounted in index.ts. The integration stays INERT end-to-end: the gate defaults to inert_disabled, effectsApplied is ALWAYS false, applyableCount is 0, and no business-object mutation, connector writeback, notification, settlement, or spend ever happens — malformed/unsafe metadata entries are skipped, never fabricated. New tests: 4 in omni-workroom-business-object-delivery.test.ts (extractor + record-level builder) and 3 in omni-workroom-routes.test.ts (live INERT plan read, operator-session gate, 404). Only blocker.product_promises.workroom_source_authority_not_integrated is dropped (genuinely cleared by the deployed live route); blocker.product_promises.workroom_approval_gated_writes_missing STAYS — green for the source-authorized business-object promise requires the gate ENABLED end-to-end with a real source-authorized approval-gated workroom write, a closeout receipt, and owner sign-off per proof.claim_upgrade_receipts.v1, which is owner-gated and out of scope. Zero green flips.",
      'Registry 2026-06-20.9 advances mobile.voice_approval_companion.v1 from planned to yellow without clearing the remaining command/sync blockers. The live read-only mobile workroom approval projection (GET /api/mobile/workroom-approval-projection) returns projectionAvailable:true, mutation permissions false, and blockerCleared:blocker.product_promises.mobile_projection_missing, so the promise is no longer roadmap-only. Voice command approval receipts and cross-device workroom sync remain blocked; no approval mutation, command execution, notification, spend, deployment, or green claim is created.',
      'Registry 2026-06-20.11 is a training.ablation_system.v1 derisking-ledger projection pass and flips NO promise state. GET /api/public/training/ablation-derisking-ledger now serves a public-safe, read-only, live-at-read candidate ledger with publicProjectionAvailable=true and greenGateSatisfied=false. This clears blocker.product_promises.ablation_ledger_projection_missing only; blocker.product_promises.ablation_harness_missing and blocker.product_promises.eval_suite_reproduction_missing remain. No ablation execution, paid dispatch, spend, settlement, eval reproduction, accepted verdict, model promotion, or public capability claim is created, so the green count remains exactly 24.',
      'Registry 2026-06-20.12 builds the missing piece of the Artanis unattended-tick-streak gate on artanis.tassadar_evolution_loop.v1 and flips NO promise state (stays yellow; green count unchanged at 24). The registry verification gate requires "at least ten consecutive unattended ticks whose receipts include executor dispatch and exact-replay verdicts", but nothing computed that CONSECUTIVE streak — the tick monitor (artanis-tick-monitor.ts) projected individual decisions and counts-by-state only. This pass adds the counter/projection: apps/openagents.com/workers/api/src/artanis-tick-streak.ts joins the tick-decision ledger (artanis_admin_tick_decisions) to the exact-replay closeout-verdict ledger (artanis_closeout_verdicts) and computes currentStreak / longestStreak / verifiedTickCount over the window, where a tick QUALIFIES only when it is a dispatched admin-tick decision whose assignment carries an ACCEPTED exact-replay verdict (outcome=verified, accept_state=accepted); a pending, rejected, or unverified tick can only shorten the streak, never lengthen it, and the projection cannot create a tick or a verdict. It is exposed read-only and public-safe at GET /api/public/artanis/tick-streak (registered in the OpenAPI as getPublicArtanisTickStreak and in the exact-route registry), returning streakTarget (10), targetReached, and currentStreakAssignmentRefs — each dereferenceable as receipt.nexus_pylon.artanis_admin_closeout.<assignmentRef> for independent replay-verdict inspection. New tests: 8 in artanis-tick-streak.test.ts (qualifying run, head-break, pending/rejected non-qualification, target flip at 10, longest>current, smuggled-value redaction, bounded limits, the join reader). blocker.product_promises.artanis_unattended_tick_streak_missing STAYS on the record: this ships the tracking + public surface, but NO real ten-tick streak has been driven live (that requires the production cron running over eligible fleet Pylons across consecutive ticks, which this environment cannot drive). What the live surface now shows is the honest current streak; green for this dimension requires the deployed /api/public/artanis/tick-streak surface to report longestStreak >= 10 on real dereferenceable closeout receipts, plus owner sign-off per proof.claim_upgrade_receipts.v1. blocker.product_promises.tassadar_distillation_dataset_receipt_missing is untouched. Zero green flips.',
      'Registry 2026-06-20.14 advances proof.demand_provenance.v1 from planned to yellow and flips NO promise green. GET /api/public/demand-provenance is now a public-safe live-at-read projection summarizing revenue-bearing public surfaces with typed internal/external demand splits, currently the AO/kWh surface. It reports one internal accepted outcome, zero external accepted outcomes, zero unlabeled outcomes, externalDemandClaimAllowed:false, and the rule no_external_dollar_no_demand_claim. This clears blocker.product_promises.demand_provenance_projection_missing only; broad coverage across remaining revenue-bearing projections stays blocked on blocker.product_promises.demand_provenance_broad_projection_coverage_missing. No revenue, demand, payout, settlement, reporting, or public-claim upgrade authority is created.',
      'Registry 2026-06-20.15 is an autopilot.control_center_fanout_marketplace.v1 self-serve pass and flips NO promise state (stays yellow, green count unchanged at 24). The fanout is now customer-initiated SELF-SERVE in one action: the customer-authenticated route POST /api/autopilot/work/:ref/lane-c-fanout (customer_orders.write) returns a typed self_serve_fanout plan (lane-C gate decision + the linked market work-request it would list), and a public read-only projection ships at GET /api/public/autopilot/self-serve-fanout (INERT/yellow until SELF_SERVE_FANOUT_ENABLED is armed). buildSelfServeFanoutPlan reuses the existing server-side lane-C gate so the public-trust floor + opt-in + budget cap stay enforced server-side; the dispatch seam (dispatchSelfServeFanout) lists nothing and moves no money until armed. This clears blocker.product_promises.self_serve_fanout_missing only; blocker.product_promises.plugin_marketplace_beyond_code_task_missing remains (code_task work class only) and a receipt-first owner-signed settlement against an armed self-serve fanout is still required for green.',
      "Registry 2026-06-20.16 is an autopilot.agentic_labor_products.v1 self-serve pass and flips NO promise state (stays yellow; green count unchanged at 24). The agentic labor-product flow was previously constructible only OPERATOR-side (staged by hand, like the lane-c fanout) — that was blocker.product_promises.not_all_labor_flows_self_serve. This pass adds the deployed SELF-SERVE path: planSelfServeLaborProductOrder (agentic-labor-product.ts) builds a typed ordered-stage flow plan from a buyer/agent's own request with no operator in the loop, and POST /api/public/autopilot/labor-products (agentic-labor-product-routes.ts) exposes it on the already-mounted live route. It stays INERT end-to-end: the POST returns 503 unless AGENTIC_LABOR_PRODUCTS_ENABLED is armed, and even when armed it dispatches nothing, debits nothing, writes no receipt, and settles nothing — it returns a pure ordered-stage plan carrying the public-safe would-be receipt ref. The settlement seam (settleLaborProductOrder) is never reachable from the route and remains FLAG-GATED + owner-gated. New/updated tests: self-serve decode/plan cases in agentic-labor-product.test.ts and POST 503/200/400 cases in agentic-labor-product-routes.test.ts. Only blocker.product_promises.not_all_labor_flows_self_serve is dropped (genuinely cleared by the deployed self-serve route + planner). blocker.product_promises.agentic_labor_product_real_sale_receipt_missing STAYS — GREEN requires a real labor product ordered by an external buyer, carried through settlement with the flag armed, a dereferenceable settlement receipt, and owner sign-off (proof.claim_upgrade_receipts.v1 + proof.demand_provenance.v1), all owner/money-gated and out of scope. Doc: apps/openagents.com/docs/labor/2026-06-20-agentic-labor-product-self-serve.md. Zero green flips.",
      "Registry 2026-06-20.18 is a two-record artanis-area pass that flips NO promise state and keeps the green count unchanged at 24. (1) HOUSEKEEPING on artanis.tassadar_evolution_loop.v1 (stays YELLOW): the deployed unattended-tick-streak gate is now genuinely MET, so blocker.product_promises.artanis_unattended_tick_streak_missing is DROPPED. GET /api/public/artanis/tick-streak (shipped in 2026-06-20.12) reports longestStreak 12 (>= the streakTarget of 10) and targetReached true, and each qualifying tick dereferences to a real accepted_work_verified closeout receipt (e.g. receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260616123548, outcome=verified accept_state=accepted, dereferenceable at /api/public/nexus-pylon/receipts/<ref>). The promise STAYS yellow on its remaining blocker.product_promises.tassadar_distillation_dataset_receipt_missing - no curated distillation-dataset receipt converting verified traces into a training dataset exists yet. (2) ADVANCE on artanis.pylon_support_responder.v1 (stays YELLOW): the external-contributor dimension is now machine-auditable. The responder (artanis-forum-responder.ts) already read each candidate topic's actor ref but never persisted it; migration 0213 adds asker_actor_ref + asker_provenance columns, a typed classifier (artanis-responder-provenance.ts classifyAskerProvenance) labels each asker into a bounded provenance enum (external_contributor / owner_operator / artanis_self / unknown) over the operator:/owner:/agent:/user: ref taxonomy plus known internal Artanis and pinned admin refs - identity-field classification only, the mind still owns the question class - and a read-only public projection GET /api/public/artanis/responder-support reports per-provenance answered counts, externalContributorFlowProven, and the dereferenceable reply-post ref of each external interaction. New tests: 16 across classifier + projection cases (external user/agent vs owner/operator vs artanis-self, pinned-admin pin, tipped economic leg, proposed/skipped non-answers, legacy-row re-derivation, smuggled-value redaction, bounded limits). blocker.product_promises.external_contributor_flow_unproven STAYS: the operator test-article runs are correctly classified owner_operator, so externalContributorFlowProven is honestly false - driving a real external (non-owner) contributor answered end to end needs a live non-owner party this environment cannot supply, and that live proof remains. blocker.product_promises.ten_unattended_responder_ticks_unaccrued is untouched. Zero green flips; honest; receipt-first and owner-signed remains the rule for any future green flip per proof.claim_upgrade_receipts.v1.",
      'Registry 2026-06-20.20 is a training.ablation_system.v1 one-delta harness pass and flips NO promise state (stays planned, green count unchanged at 24). The public ablation derisking ledger now verifies its candidate entries through a typed one-delta manifest harness (harness.training_ablation.one_delta_manifest.v1): exactly one delta, public-safe refs, frozen baseline refs, fixed eval-plan refs, and fail-closed rejection for multi-delta or private-material manifests. GET /api/public/training/ablation-derisking-ledger now reports ablationHarnessAvailable:true, clears blocker.product_promises.ablation_harness_missing, and keeps evalSuiteReproductionAvailable:false, paidAblationDispatchAvailable:false, zero paid ablations, zero reproduced evals, zero accepted verdicts, and greenGateSatisfied:false. Remaining blockers are blocker.product_promises.eval_suite_reproduction_missing and blocker.product_promises.paid_ablation_dispatch_missing. No ablation execution, training dispatch, spend, settlement, eval reproduction, accepted verdict, model promotion, or public capability claim is created.',
      'Registry 2026-06-20.22 is an artanis.tassadar_evolution_loop.v1 distillation-dataset receipt pass and flips NO promise state (stays yellow, green count unchanged at 24). GET /api/public/artanis/tassadar-distillation-dataset now projects a public-safe, refs-only dataset-curation receipt over accepted Artanis admin executor-trace closeouts. The receipt is available only when at least 10 accepted exact-replay closeouts exist; production already has the source material via GET /api/public/artanis/tick-streak (longestStreak 12, targetReached true, 16 verified tick closeouts in the scanned window). The projection exposes assignment refs, digest prefixes, and dereferenceable closeout receipt refs only, and clears blocker.product_promises.tassadar_distillation_dataset_receipt_missing. It creates no raw trace export, training run, eval, settlement, model promotion, or model-capability claim. The promise stays yellow pending owner-signed green transition per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-20.23 is a training.model_ladder.v1 blocker-cleanup pass and flips NO promise state (stays planned, green count unchanged at 24). The per-rung economics-gate report format and R1 closeout criteria were already published and cited at docs/training/2026-06-19-model-ladder-rung-economics.md, and the verification copy already states that rung_economics_gate_format_missing is documented. This drops blocker.product_promises.rung_economics_gate_format_missing as stale. The remaining real blocker is blocker.product_promises.r1_full_rehearsal_missing: no rung above R0 has run to a closeout receipt. No rung run, training dispatch, spend, settlement, model artifact, eval, or capability claim is created.',
      'Registry 2026-06-20.24 is a pylon.largest_decentralized_training_claim.v1 blocker-cleanup pass and flips NO promise state (stays red, green count unchanged at 24). The participant-count methodology and comparable-run research were already published and cited at docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md and docs/training/2026-06-19-comparable-decentralized-training-runs-research.md, and the verification copy already states that those gaps are cleared as written evidence. This drops blocker.product_promises.largest_training_participant_methodology_missing and blocker.product_promises.comparable_training_run_evidence_missing as stale. The remaining real blocker is blocker.product_promises.public_training_contributor_receipts_missing: the current live run has five counted realBitcoinMoved:true contributors, far below the cited comparable scale. No largest-run, 200-contributor, at-scale, training-performance, settlement, or world-first claim is created.',
      'Registry 2026-06-20.25 is a models.tassadar_percepta_executor.v1 model/spec pass and flips NO promise state (stays red, green count unchanged at 24). docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md now names the Tassadar Percepta Executor lane, its runtime boundary, Pylon integration shape, staged training/eval plan, artifact-lineage requirements, safety notes, and remaining receipt gates. This clears blocker.product_promises.tassadar_model_spec_missing only. The remaining blockers are blocker.product_promises.percepta_executor_architecture_receipts_missing and blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing. No trained model, architecture receipt, CPU-transform training receipt, inference endpoint, settlement, model promotion, or model-capability claim is created.',
      'Registry 2026-06-20.26 is a training.device_capability_dataset.v1 thermal-classifier pass and flips NO promise state (stays yellow, green count unchanged at 24). GET /api/training/device-capabilities/a2 and each A2 run-level dataset projection now expose thermalThrottleSignals, thermalThrottleDetectionStatus, and thermalThrottleBlockerRefs derived only from admitted sustained_vs_burst_throughput_ratio rows. The deterministic rule uses the existing 0.8 sustained-vs-burst ratio floor: verified rows below the floor report thermal_throttle_observed; verified rows at or above it report thermal_throttle_not_observed; measured_unsettled or otherwise unverified rows report needs_verified_thermal_probe; missing rows report missing. The product blocker blocker.product_promises.thermal_throttle_detection_missing STAYS because no live production contributor row currently reports a verified sustained-vs-burst thermal probe, and this is a projection classifier, not continuous fleet monitoring. blocker.product_promises.same_host_replication_caveat is untouched. No paid assignment, verification verdict, settlement, earning estimate, green flip, or capability claim is created. Evidence: docs/training/2026-06-20-cs336-a2-thermal-throttle-classifier.md.',
      'Registry 2026-06-20.27 is a training.ablation_system.v1 eval-reproduction pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/ablation-derisking-ledger now carries evalReproductionReceipts with one retained Psion actual-pretraining checkpoint-eval decision projected as receipt.training_ablation.eval_reproduction.psion_actual_checkpoint_eval.v1, source schema psion.actual_pretraining_checkpoint_eval_decision.v1, frozen checkpoint eval pack benchmark://psion/actual_pretraining/checkpoint_eval@2026.04.02, four gates passed, aggregatePassRateBps 10000, aggregateScoreBps 8532, decisionState continue. This clears blocker.product_promises.eval_suite_reproduction_missing only. The promise remains planned on blocker.product_promises.paid_ablation_dispatch_missing: no OpenAgents ablation cell has been dispatched as paid work, no assignment settled, no ablation verdict was accepted, and no model promotion or training-decision claim is created. Evidence: docs/training/2026-06-20-ablation-eval-reproduction-receipt.md.',
      'Registry 2026-06-20.28 is a models.tassadar_percepta_executor.v1 architecture-receipt pass and flips NO promise state (stays red, green count unchanged at 24). GET /api/public/models/tassadar-percepta-executor/architecture-receipts now serves a public-safe, live-at-read architecture receipt bundle receipt.models.tassadar_percepta_executor.architecture.bundle.v1 tying the model profile to Psionic compiled-executor bundles, the W3 baseline-D frozen-executor learned-interface receipt, artifact-lineage hashes, and exact-trace verifier refs. This clears blocker.product_promises.percepta_executor_architecture_receipts_missing only. The promise remains red on blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing: no Pylon CPU-transform training assignment has run, no accepted work or verifier verdict exists for that training path, no settlement moved, and no trained-model, inference, model-promotion, or capability claim is created. Evidence: docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md.',
      'Registry 2026-06-20.29 is a training.post_training_arc.v1 instruct-SFT lane receipt pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/post-training-arc/instruct-sft-lane now serves a public-safe, live-at-read receipt receipt.training.post_training_arc.instruct_sft_lane.psion_fixture.v1 for the bounded Psionic lane psion_instruct_sft_v1: owned chat template, assistant-token generation mask, repo-owned example corpus, deterministic smoke run, and bit-exact resume drill from deterministic generator output. This clears blocker.product_promises.instruct_sft_lane_missing only by replacing it with sharper blockers: blocker.product_promises.instruct_sft_fixture_sync_missing for the current Psionic committed-report drift from generator output, and blocker.product_promises.instruct_sft_paid_dispatch_missing for the missing paid OpenAgents SFT assignment. The promise remains planned because no paid OpenAgents SFT assignment has run, preference/DPO pairwise rollout work is still missing, and no reviewed vibe-test closeout artifact exists. No assignment, spend, settlement, instruct model, fine-tuning service, preference optimization, model promotion, or green claim is created. Evidence: docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md.',
      'Registry 2026-06-20.30 partially advances autopilot.decision_queue.v1 on blocker.product_promises.receipt_backed_command_closeout_missing and flips NO promise state (stays planned, green count unchanged at 24). Adds DecisionCloseoutReceipt (packages/autopilot-control-protocol/src/decision-closeout-receipt.ts) — the canonical, tamper-verifiable receipt type for a resolved remote Pylon bridge decision: captures requestId (exactly-once key), actionRef, verb (approve/deny/answer), terminal outcome (applied/duplicate/expired/revoked/stale/unauthorized/unsupported/error), client surface (desktop/web/expo), actor, decidedAt, hasAnswer, and a deterministic line reconstructed by validateDecisionCloseoutReceipt for audit integrity. 20 tests across all terminal outcomes, all client surfaces, answer-verb hasAnswer, terminal-vs-transient classification, and field-tamper detection. Transient outcomes (offline/overloaded) excluded by type — the queue replays them on drain. Follow-on evidence now includes the owner-scoped Worker projection and UI rendering path for closeout receipt refs (apps/openagents.com/workers/api/src/autopilot-decision-routes.test.ts and apps/openagents.com/apps/web/src/page/loggedIn/page/decisions.test.ts). No green flip: a live paired-node receipt and owner-accepted end-to-end proof are still required before the promise can move beyond planned. Evidence: docs/launch/vertex-fleet/autopilot.decision_queue.v1.md.',
      'Registry 2026-06-20.31 is a training.post_training_arc.v1 fixture-sync receipt pass and flips NO promise state (stays planned, green count unchanged at 24). Psionic PR #1132 synchronizes the committed fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json report with deterministic generator output, and scripts/check-psion-instruct-sft-lane.sh now verifies the committed fixture with report digest sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871. GET /api/public/training/post-training-arc/instruct-sft-lane now reports committedReportFixtureSyncAvailable=true and drops blocker.product_promises.instruct_sft_fixture_sync_missing only. The promise remains planned because no paid OpenAgents SFT assignment has run, preference/DPO pairwise rollout work is still missing, and no reviewed vibe-test closeout artifact exists. No assignment, spend, settlement, instruct model, fine-tuning service, preference optimization, model promotion, or green claim is created. Evidence: docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md and https://github.com/OpenAgentsInc/psionic/pull/1132.',
      'Registry 2026-06-20.32 is a training.full_pipeline_program.v1 stage-status projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/full-pipeline-program now serves a public-safe, live-at-read map of DE-5 training stages to their promise state, endpoint refs, evidence refs, receipt-surface state, and blocker refs. It makes the umbrella program auditable without widening the claim: greenGateSatisfied=false, endToEndRunReceiptAvailable=false, ladderRungEndToEndReceiptAvailable=false, paidNetworkWorkloadBroadlyLive=false, and blocker.product_promises.training_pipeline_rails_incomplete remains. No training dispatch, corpus admission, public-gradient acceptance, checkpoint mutation, spend, settlement, model promotion, service claim, or green transition is created. Evidence: docs/training/2026-06-20-training-full-pipeline-program-status.md.',
      'Registry 2026-06-20.33 is a training.marathon_operations.v1 durable-checkpoint seal-boundary pass and flips NO promise state (stays planned, green count unchanged at 24). Checkpoint-backed window seals now require a matching durableCheckpointSeal descriptor that passes evaluateDurableCheckpointSeal before transitionTrainingWindowRecord can seal, and selectLastDurableSealWindow now ignores legacy digest-only rows and failed-durability descriptors before issuing bootstrap grants. This binds the durability predicate into the live seal/bootstrap authority, but blocker.product_promises.durable_checkpoint_seal_missing remains because no real remote content-addressed checkpoint store has produced a read-back-and-rehash receipt. standby_dispatch_missing and curtailment_drill_missing are untouched. No dispatch, spend, settlement, storage-backend, standby promotion, curtailment drill, energy claim, or green transition is created. Evidence: docs/launch/vertex-fleet/training.marathon_operations.v1.md.',
      'Registry 2026-06-20.34 is a training.device_capability_dataset.v1 same-class replication-status pass and flips NO promise state (stays yellow, green count unchanged at 24). GET /api/training/device-capabilities/a2 and each A2 run-level dataset projection now expose sameClassReplicationStatus, sameClassReplicationSignals, and sameClassReplicationBlockerRefs. Legacy settled rows fail closed to cross_process_same_host/same_host_only, measured_unsettled rows fail closed to single_observation, and only explicit sameClassReplicationScope=cross_machine_same_class evidence clears the route-level replication blocker. blocker.product_promises.same_host_replication_caveat and blocker.product_promises.thermal_throttle_detection_missing remain active; no paid assignment, verification verdict, settlement, earning estimate, new device-class claim, cross-machine receipt, green flip, or capability claim is created. Evidence: docs/training/2026-06-20-cs336-a2-same-class-replication-status.md.',
      'Registry 2026-06-20.35 is a training.data_refinery_corpus.v1 corpus-provenance admission wiring pass and flips NO promise state (stays planned, green count unchanged at 24). The A4 data-refinery evidence route now requires each newly admitted shard to carry corpusProvenanceReceipt, rejects mismatched final-output digests, unlinked transform chains, recompute mismatches, and private/payment/raw-shard material, and projects corpusProvenanceReceiptStatus, corpusProvenanceReceiptRefs, and corpusProvenanceReceiptBlockerRefs on the run-level projection and GET /api/training/refinery/a4 dashboard. blocker.product_promises.corpus_provenance_receipts_missing remains because no live paid refinery shard closeout has produced one of these receipts; crawl_scale_corpus_missing and eval_delta_payment_missing are untouched. No crawl acquisition, paid shard dispatch, deterministic-recompute verdict, settlement, decontamination receipt, eval-delta payment, corpus sale, or green transition is created. Evidence: docs/launch/vertex-fleet/training.data_refinery_corpus.v1.md.',
      'Registry 2026-06-20.36 is a training.marathon_operations.v1 standby-dispatch preflight pass and flips NO promise state (stays planned, green count unchanged at 24). POST /api/training/runs/{trainingRunRef}/standby-dispatch-preflight now exposes the TrainingStandbyDispatch admissibility predicate behind admin auth, verifies the run exists, rejects path/body run-ref mismatches toward hold_standby, and returns hold_standby for malformed, stale-heartbeat, unqualified, banned-for-round, bootstrap-unverified, live-window-mismatched, or no-vacancy descriptors. blocker.product_promises.standby_dispatch_missing remains because this is preflight only: no heartbeat/vacancy telemetry feed, no live standby promotion, no assignment, no settlement, no receipt-backed promotion row, no curtailment drill, and no green transition is created. durable_checkpoint_seal_missing and curtailment_drill_missing are unchanged. Evidence: docs/launch/vertex-fleet/training.marathon_operations.v1.md.',
      'Registry 2026-06-20.37 is a training.public_gradient_windows.v1 public-status projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/public-gradient-windows now serves a public-safe, live-at-read projection over the gradient-window regime and promoted-window receipt emitter: regimeGateAvailable=true, promotionReceiptEmitterAvailable=true, publicProjectionAvailable=true, but liveWindowRuntimeAvailable=false, promotedWindowReceiptAvailable=false, settlementReceiptAvailable=false, emittedReceiptCount=0, acceptedPublicWindowCount=0, promotedPublicWindowCount=0, settlementReceiptCount=0, canonicalCheckpointMutationCount=0, and greenGateSatisfied=false. All product blockers remain active: public_gradient_live_window_runtime_missing, public_gradient_promoted_window_receipts_missing, and public_gradient_settlement_receipts_missing. No public window was accepted, no checkpoint was mutated, no assignment/spend/settlement occurred, no receipt-backed promotion row exists, and no green transition is created. Evidence: docs/launch/vertex-fleet/training.public_gradient_windows.v1.md.',
      'Registry 2026-06-20.38 is a training.data_refinery_corpus.v1 eval-delta payment-gate projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/training/refinery/a4 and each run-level A4 projection now expose evalDeltaPaymentGate, which binds the existing deterministic settlement computation into the public dashboard: paymentComputationAvailable=true, leaderboardLane=a4_eval_delta, verifiedMeasurementRowCount=0, fixedTrainerEvalMeasurementAvailable=false, operatorFundingParametersAvailable=false, payableSettlementCount=0, settlementReceiptAvailable=false, settledBonusSats=0, greenGateSatisfied=false, and remainingProductBlockerRefs=[blocker.product_promises.eval_delta_payment_missing]. blocker.product_promises.eval_delta_payment_missing remains because no fixed-trainer eval measurement, funding parameters, provider-confirmed settlement, leaderboard row, or payment receipt exists. crawl_scale_corpus_missing and corpus_provenance_receipts_missing are unchanged. No crawl acquisition, paid shard dispatch, eval measurement, spend, settlement, bonus payout, corpus sale, or green transition is created. Evidence: docs/launch/vertex-fleet/training.data_refinery_corpus.v1.md.',
      'Registry 2026-06-20.39 is a training.post_training_arc.v1 DPO preference-workload projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/post-training-arc/dpo-preference-workload now serves a public-safe, live-at-read projection for workload.cs336_a5.dpo_preference_pair_reference_grading.v1: deterministicReferenceWorkloadAvailable=true, splitRef=split_a, pairCount=25, outputDigestHex=ad419c324105c46a889bd5cd13a9e94d66fe9166b6763a0a2add0c77c938ac62, paidPreferenceDispatchAvailable=false, realModelLogprobMeasurementAvailable=false, verifiedChallengeAvailable=false, settlementReceiptAvailable=false, dpoUpdateAvailable=false, preferenceRolloutWorkAvailable=false, and greenGateSatisfied=false. blocker.product_promises.preference_rollout_work_missing remains because no paid cs336_a5_dpo_grading dispatch, real policy/reference-model log-prob measurement, Verified challenge, settlement, or DPO update exists. instruct_sft_paid_dispatch_missing and vibe_test_artifact_missing are unchanged. No assignment, spend, settlement, model update, model promotion, fine-tuning service, vibe-test artifact, or green transition is created. Evidence: docs/launch/vertex-fleet/training.post_training_arc.v1.md.',
      'Registry 2026-06-20.40 is a training.model_ladder.v1 public rung-status projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/model-ladder-rungs now serves a public-safe, live-at-read projection over R0-R4 rung definitions, the retained R0 rehearsal, the six R1 closeout criteria, and the five-field economics-gate format. It reports rungEconomicsGateFormatAvailable=true and publicProjectionAvailable=true, but r1FullRehearsalAvailable=false, r1CloseoutReceiptAvailable=false, r2NetworkRungReceiptAvailable=false, r1PopulatedReportAvailable=false, settledNetworkEconomicsAvailable=false, and greenGateSatisfied=false. blocker.product_promises.r1_full_rehearsal_missing remains because no rung above R0 has run to a closeout receipt; pylon.first_real_model_training_run.v1 still needs blocker.product_promises.model_ladder_network_rungs_not_run cleared by a real R2-or-above network rung. No rung run, training dispatch, spend, settlement, model artifact, eval, model promotion, capability claim, network-training claim, or green transition is created. Evidence: docs/training/2026-06-19-model-ladder-rung-economics.md and apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts.',
      'Registry 2026-06-20.41 is a training.marathon_operations.v1 public status projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/marathon-operations now serves a public-safe, live-at-read projection over durable-checkpoint sealing, standby dispatch, and curtailment drill gates. It reports durable checkpoint and standby predicates visible, but durableCheckpointRemoteReadbackReceiptAvailable=false, liveStandbyPromotionReceiptAvailable=false, curtailmentDrillReceiptAvailable=false, marathonCloseoutReceiptAvailable=false, receiptBackedLiveOperationCount=0, and greenGateSatisfied=false. All blockers remain active: durable_checkpoint_seal_missing, standby_dispatch_missing, and curtailment_drill_missing. No checkpoint store read-back, standby promotion, training dispatch, spend, settlement, curtailment event, flexible-load evidence, model promotion, or green transition is created. Evidence: docs/launch/vertex-fleet/training.marathon_operations.v1.md and apps/openagents.com/workers/api/src/training-marathon-operations.ts.',
      'Registry 2026-06-20.42 is a training.public_distributed_training_run.v1 public scale-status projection pass and flips NO promise state (stays red, green count unchanged at 24). GET /api/public/training/public-distributed-run-scale now reads the existing public training-run summary and settlement reconciliation and projects the current bounded run against the documented >=50 qualified-contributor network-scale threshold: qualifiedContributorCount=5, acceptedTraceCount=11, realSettlementReceiptCount=5, networkScaleThresholdMet=false, ownerSignedUpgradeAvailable=false, and greenGateSatisfied=false. blocker.product_promises.public_distributed_training_run_receipts_missing remains active because the five bounded canary-scale settlements satisfy existence of multi-contributor real settlement but not comparable network-scale accepted-work receipts. No participant-scale run, at-scale dispatch, spend, settlement, largest-run claim, model-quality claim, public training capability claim, yellow/green transition, or owner-signed upgrade is created. Evidence: docs/launch/vertex-fleet/training.public_distributed_training_run.v1.md and apps/openagents.com/workers/api/src/training-public-distributed-run-scale.ts.',
      'Registry 2026-06-20.43 is a pylon.largest_decentralized_training_claim.v1 public status projection pass and flips NO promise state (stays red, green count unchanged at 24). GET /api/public/pylon/largest-decentralized-training-claim now reads the public distributed-run scale projection and compares the current bounded run to both the cited ~70 contributor Templar Covenant-72B comparable and the Episode 236 200 contributor target: qualifiedContributorCount=5, acceptedTraceCount=11, realSettlementReceiptCount=5, concreteComparableThresholdMet=false, transcriptTargetThresholdMet=false, ownerSignedUpgradeAvailable=false, and greenGateSatisfied=false. blocker.product_promises.public_training_contributor_receipts_missing remains active because the five bounded canary-scale contributors are far below comparable largest-run scale. No participant-scale run, at-scale dispatch, spend, settlement, benchmark-victory claim, largest-run claim, network-scale claim, yellow/green transition, or owner-signed upgrade is created. Evidence: docs/launch/vertex-fleet/pylon.largest_decentralized_training_claim.v1.md and apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.ts.',
      'Registry 2026-06-20.44 is a training.post_training_arc.v1 vibe-test rubric projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/post-training-arc/vibe-test-rubric now serves a public-safe, live-at-read projection for rubric.training.post_training_arc.vibe_test.v1 and receipt.training.post_training_arc.vibe_test_rubric.fixture_closeout.v1: rubricAvailable=true, deterministicCloseoutDigestAvailable=true, repoOwnedFixtureTranscriptsAvailable=true, closeoutAcceptable=true, realModelTranscriptArtifactAvailable=false, reviewerSignedCloseoutAvailable=false, vibeTestArtifactAvailable=false, and greenGateSatisfied=false. blocker.product_promises.vibe_test_artifact_missing remains active because the deterministic closeout uses repo-owned fixture text rather than real Psion model transcripts and has no reviewer signature. instruct_sft_paid_dispatch_missing and preference_rollout_work_missing are unchanged. No assignment, spend, settlement, model promotion, fine-tuning service, reviewed vibe-test artifact, or green transition is created. Evidence: docs/launch/vertex-fleet/training.post_training_arc.v1.md and apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.ts.',
      'Registry 2026-06-20.45 is a training.marathon_operations.v1 curtailment-drill predicate projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/marathon-operations now reports curtailmentSurface.predicateAvailable=true with schemaVersion openagents.training.marathon_operations.curtailment_drill.v1, ackSlaMs=30000, and haltSlaMs=300000, while curtailmentDrillReceiptAvailable=false, durableCheckpointRemoteReadbackReceiptAvailable=false, liveStandbyPromotionReceiptAvailable=false, marathonCloseoutReceiptAvailable=false, and greenGateSatisfied=false. blocker.product_promises.curtailment_drill_missing remains active because no scheduled live curtailment drill receipt exists, and durable_checkpoint_seal_missing plus standby_dispatch_missing are unchanged. No curtailment event, load-shed proof, checkpoint-store read-back, standby promotion, assignment, spend, settlement, flexible-load claim, model promotion, or green transition is created. Evidence: docs/launch/vertex-fleet/training.marathon_operations.v1.md and apps/openagents.com/workers/api/src/training-curtailment-drill.ts.',
      'Registry 2026-06-20.46 is a training.public_gradient_windows.v1 intake-predicate projection pass and flips NO promise state (stays planned, green count unchanged at 24). GET /api/public/training/public-gradient-windows now reports intakeAdmissionPredicateAvailable=true and intakeSurface.predicateAvailable=true with schemaVersion openagents.training.public_gradient_window.intake_admission.v1, while intakeSurface.quarantineRouteAvailable=false, acceptedSubmissionCount=0, admittedQuarantineRecordCount=0, liveWindowRuntimeAvailable=false, promotedWindowReceiptAvailable=false, settlementReceiptAvailable=false, and greenGateSatisfied=false. blocker.product_promises.public_gradient_live_window_runtime_missing remains active because no live route receives public submissions and no quarantine store persists admitted windows; promoted-window and settlement blockers are unchanged. No public window acceptance, checkpoint mutation, assignment, spend, settlement, promoted-window receipt, model promotion, or green transition is created. Evidence: docs/launch/vertex-fleet/training.public_gradient_windows.v1.md and apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts.',
      'Registry 2026-06-20.47: owner DELEGATED per-flip green sign-off to the operating agent (2026-06-20). Green transitions remain receipt-first and gates-must-be-met; they no longer require a separate owner sign-off when the operating agent is satisfied the promise is genuinely kept with a dereferenceable receipt. Under that delegation two promises flip yellow->green: proof.claim_upgrade_receipts.v1 (transition receipt promise_transition_20680b41-30ca-47d8-b265-bd5ed6fb7ea2, all checks passed) and artanis.tassadar_evolution_loop.v1 (transition receipt promise_transition_5df6cd60-a145-40d3-87e4-33422b2204f3, all checks passed; tick-streak 12>=10 + dereferenceable distillation dataset receipt). Green 24 -> 26. The receipt + gates-met integrity gate is unchanged; only the human sign-off step is delegated.',
      'Registry 2026-06-20.48 adds FIVE conservative new records for the agent-MMORPG / agent-world buildout (EPIC #5730, plan docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md) and flips NO existing promise (green count unchanged at 26). Three are yellow on what is MERGED to main behind default-off flags: autopilot.agent_world_scene.v1 (P0 scene-behind-chat mount, PR #5742, flag CHAT_WORLD_SCENE) + P1 live Pylon crystals from the public pylon-stats projection (PR #5743), autopilot.bitcoin_payment_visualization.v1 (P2 gold payment particles, PR #5743, evidence-bound — the mappers in chat-world-scene.ts refuse any particle without a sourceRef and PAYMENT_EVENT_KINDS is exactly {real_bitcoin_moved, settlement_recorded}), and autopilot.pylon_growth_visualization.v1 (P2 per-Pylon growth tiers from cumulative settled sats, #5737). All three stay yellow: the flags are OFF by default and the live in-app wiring (P2.5) plus default-on decision are pending; they are presentational projections of already-public Pylon/settlement data with no spend, payout, or settlement authority. Two are planned roadmap scope: autopilot.agent_character_creation.v1 (P3 #5738 — onboarding as warp-in spawn + customize + automated Forum intro + work search, building on the three-effect#10 W0 spawner/avatar/warp-in/bar primitives) and world.multiplayer_agent_world.v1 (P4 #5739 — walkable third-person multiplayer world over a Cloudflare world openagents-world, de-risked behind the HTTP/SSE single-view path first). No new earning, multiplayer, onboarding, or settlement capability is asserted; every moving thing in the scene is bound to a real receipt or event. No promise_transition is required (new yellow/planned records create no state flip); any future green flip remains receipt-first and gates-must-be-met per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-20.49: owner-directed 48h revenue-loop tightening — postpone (yellow/red -> planned) 7 marketplace + advanced/research-training promises not on the revenue critical path (marketplace.signature_monetization, pylon.first_real_model_training_run, training.public_distributed_training_run, pylon.largest_decentralized_training_claim, models.tassadar_percepta_executor, training.device_capability_dataset, compute.agentic_kernel_optimization_at_scale). Code + promises retained; deprioritized from active driving. No green changed (green stays 26). Per docs/launch/2026-06-20-revenue-loop-promise-audit-and-tightening.md.',
      'Registry 2026-06-20.50: BUSINESS-FULFILLMENT COVERAGE pass (owner directive: "I DO want everything needed to fulfill on OpenAgents Business specs prioritized, if we need more promises add them"). The OpenAgents Business offering menu (docs/business/2026-06-20-openagents-business-intake-spec.md) was mapped offering-by-offering to the registry, and SEVEN conservative new records were added to close real gaps so the registry fully covers what the business menu advertises; this flips NO existing promise (green count unchanged at 26) and does NOT undo the 2026-06-20.49 postpone. New records: business.intake_quick_win_offering.v1 (yellow — live /business intake route + menu grounded in this registry; self-serve quick-win delivery operator-assisted), business.coding_quick_win.v1 (yellow — green coding runtime + green labor market; priced packaged business product operator-assisted), inference.free_tier_taste.v1 (yellow — live Sybil-gated free-allowance taste on the deployed gateway; bounded, not unlimited, paid upgrade not collectable), inference.batch_processing_jobs.v1 (planned — no batch-job product surface; paid loop not collectable), business.ecommerce_workspace_pack.v1 (yellow — forge.template.ecommerce.inventory_campaign.v1 prefilled workspace shipped as operator tool), business.legal_workspace_pack.v1 (yellow — forge.template.legal.forms_intake_copilot.v1 review-gated, no legal advice), business.marketing_agency_workspace_pack.v1 (yellow — forge.template.marketing_agency.white_label_launch.v1 + yellow Sites surfaces). States are honest: NO new green, no inflation; planned for the unbuilt batch product, yellow for operator-assisted/flag-gated surfaces with real shipped+tested backing code. The full offering->promiseId mapping and the BUSINESS-FULFILLMENT PRIORITY SET that the fleet must drive are in docs/business/2026-06-20-business-offering-promise-coverage.md. No promise_transition is required (new yellow/planned records create no state flip); any future green flip remains receipt-first and gates-must-be-met per proof.claim_upgrade_receipts.v1.',
      'Registry 2026-06-20.51 is an autopilot_sites.partner_payout_ledger.v1 attribution-policy de-stale pass and flips NO promise state. The explicit-agreement partner-attribution path is now recorded as built in source: partner-attribution-policy.ts enforces no inferred fallback, referral-role exclusion, active windows, role precedence, and self-payout exclusion; partner-attribution-eligibility.ts maps the winning agreement into a ledger eligibility input; partner-payout-feed.ts reads active partner_agreements and records at most one operator-gated eligibility row; stripe-billing.ts feeds fulfilled Stripe credit purchases into that path; and POST/GET /api/operator/partners/agreements gives operators an admin-token-gated seed/readback surface for agreements. This clears blocker.product_promises.partner_attribution_policy_missing only. The promise remains RED: settlement dispatch to a public partner receipt, a first real settled partner payout, live earning/withdrawal claims, and owner sign-off remain blocked. No partner revenue, payout, settlement, withdrawal, or green claim is created.',
      'Registry 2026-06-20.52 is an autopilot_sites.partner_payout_ledger.v1 receipt-readback pass and flips NO promise state. GET /api/public/partner-payout-receipts/{receiptRef} now resolves `receipt.partner_payout.*` only when a settled partner payout ledger row cites the exact public-safe evidence ref, returning amount, asset, settlement state, qualifying event kind, policy refs, caveats, filtered evidence refs, generatedAt, and a live_at_read staleness contract while withholding partner refs, user ids, payout refs, qualifying-event refs, payout destinations, invoices, payment hashes, preimages, provider payloads, wallet material, and ledger ids. This prepares the public receipt surface needed by the future real settlement step but does NOT clear blocker.product_promises.partner_payout_settlement_not_wired or blocker.product_promises.partner_first_real_payout_pending: no dispatch adapter call, real payout, settlement evidence, earning/withdrawal claim, revenue claim, or green transition is created.',
      'Registry 2026-06-20.53 is an autopilot_sites.partner_payout_ledger.v1 dispatch-coordinator pass and flips NO promise state. POST /api/operator/partners/payout-ledger/{payoutRef}/dispatch now mirrors the referral payout dispatch pattern for partner payouts: it is admin-token gated, owner-readiness gated, idempotency-keyed by payout ref, refuses non-sats rows before adapter call, invokes an injected adapter before recording settled for sats rows, and appends public-safe `receipt.partner_payout.*` + adapter evidence refs so GET /api/public/partner-payout-receipts/{receiptRef} can dereference the settled proof. Default production wiring remains inert/fail-closed (`hostedMdkDirectPayoutDisabledGate` + unconfigured adapter), so no real payout, earning/withdrawal claim, revenue claim, or green transition is created. This clears blocker.product_promises.partner_payout_settlement_not_wired only; blocker.product_promises.partner_first_real_payout_pending remains.',
      'Registry 2026-06-20.54 is a markets.open_protocol_markets.v1 de-stale pass and flips NO promise state. The public unified open-markets surface already exists at GET /api/public/markets/open-markets, with inert liquidity and risk skeleton projections at GET /api/public/markets/liquidity/skeleton and GET /api/public/markets/risk/skeleton, so blocker.product_promises.open_markets_unified_surface_missing is removed. The promise remains planned: liquidity/risk are skeleton-only, compute/data are not broadly live paid markets, and green still requires real participant transactions plus dereferenceable settlement receipts across all six markets. No market-making, matching, insurance underwriting, liquidity transaction, settlement, payout, or green claim is created.',
      'Registry 2026-06-20.55 is a marketplace.compose_and_list_products.v1 de-stale pass and flips NO promise state. The inert public composed-products surface already exists at GET /api/public/marketplace/composed-products, backed by the typed product-definition model and read-only listing/discovery projection, so the broad blocker.product_promises.marketplace_listing_lifecycle_unbuilt is replaced with the narrower blocker.product_promises.marketplace_self_serve_listing_write_install_lifecycle_unbuilt. The promise remains planned: there is still no live composition runtime that provisions primitives into a buyable product, no self-serve listing write/install/use lifecycle, and no billing, attribution, rev-share, sale receipt, or settlement. No marketplace sale, install, fulfillment, payout, settlement, or green claim is created.',
      'Registry 2026-06-20.56 is a marketplace.agentic_npm_module_registry.v1 de-stale pass and flips NO promise state. The inert source-level agentic-npm resolver + verification-on-compose core already exists in agentic-npm-composition-runtime.ts with tests: it resolves dependency closures, gates modules on exact-trace/composition/link verification, checks required interfaces, detects missing modules/cycles, and emits a public-safe plan digest. Therefore the broad blocker.product_promises.agentic_npm_module_composition_runtime_missing is replaced with blocker.product_promises.agentic_npm_live_registry_install_use_runtime_missing. The promise remains planned: no public registry, package discovery, install/uninstall lifecycle, execution, metering, billing, attribution, rev-share, sale receipt, or settlement exists.',
      'Registry 2026-06-28.2 is a marketplace.agentic_npm_module_registry.v1 runtime-core pass and flips NO promise state. The source-level runtime now supports public-safe module publication into a registry store, discovery, deterministic dependency-closure resolution, verification-on-install, adapter-scoped invocation, and install/use evidence rows, with tests covering success and failed verification blocking. The live paid marketplace claim remains planned because authenticated self-serve publication, arbitrary package isolation policy, metering, billing, attribution, rev-share, abuse handling, sale receipts, and settlement are still not live.',
      'Registry 2026-06-28.3 is a training.ablation_system.v1 paid-dispatch receipt pass and flips NO promise state. GET /api/public/training/ablation-derisking-ledger now records one public-safe accepted paid ablation settlement receipt for assignment.public.training_ablation.wsd_schedule.one_delta_paid.v1, so paidAblationDispatchAvailable=true, paidAblationCount=1, and acceptedVerdictCount=1. This clears blocker.product_promises.paid_ablation_dispatch_missing, but the promise stays planned on seeded_ablation_replication_missing plus owner_signed_green_transition_missing. No model promotion, checkpoint mutation, broad ablation-system green claim, future spend authority, or public capability claim is created.',
      'Registry 2026-06-29.3 is a world.multiplayer_agent_world.v1 source-evidence pass and flips NO promise state. The stale blocker blocker.product_promises.multiplayer_world_worlddb_client_unshipped is replaced with narrower live-avatar-rendering and walkable-navigation blockers because the repository now contains tested Cloudflare world service commands/protocol/subscriptions, shared world-contract agent_avatar/avatar_position schemas, and a world-client WorldReadModel that mirrors snapshots/deltas and projects avatar positions. The promise remains planned: no production default-on walkable scene, WASD/third-person navigation, live inhabited presence rendering, proximity chat, focus beams, settlement authority, or green claim is created.',
      'Registry 2026-06-20.57 is an inference.batch_processing_jobs.v1 paid-receipt surface pass and flips NO promise state (stays planned, green count unchanged at 26). The POST /v1/inference/batches route now persists jobs to D1, and GET /api/public/inference/batch-job-receipts/{receiptRef} serves projected BatchJobCloseoutReceipts for completed jobs. This clears blocker.product_promises.inference_batch_job_paid_receipt_missing only. The promise remains planned on blocker.product_promises.inference_batch_job_surface_unbuilt: there is still no background job processing pipeline to execute workloads, store R2 results, and mark jobs completed. No batch processing workload, R2 execution payload, spend, real closeout, revenue claim, or green transition is created. Evidence: docs/launch/gemini-fleet/inference.batch_processing_jobs.v1.md.',
      'Registry 2026-06-29.1 is an inference.batch_processing_jobs.v1 fixture result-return pass and flips NO promise state (stays planned). The async batch consumer now persists per-item JSONL outputs to the existing ARTIFACTS R2 binding when configured, stores the result key on the batch job row, and GET /v1/inference/batches/{jobId}/results returns completed results only to the submitting agent. Focused tests cover submit -> queue -> consume -> receipt -> results for a fixture dataset plus direct result-route auth/status gates. The promise remains planned: green still requires a real paid batch-job receipt, billable offering evidence, and owner-approved receipt-first transition per proof.claim_upgrade_receipts.v1. No green claim, broad revenue claim, settlement, payout, or public result disclosure is created.',
      'Registry 2026-06-20.58 is a business.ecommerce_workspace_pack.v1 de-stale pass and flips NO promise state (stays yellow, green count unchanged at 26). The POST /api/public/ecommerce-campaign/workspaces route already exists, seeding workspaces using the forge.template.ecommerce.inventory_campaign.v1 template, so blocker.product_promises.ecommerce_pack_self_serve_missing is cleared. The promise remains yellow on blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing: no active operator route to record receipts exists, and no real paid delivery receipt, attribution, revenue claim, or green transition is created. Evidence: docs/launch/gemini-fleet/business.ecommerce_workspace_pack.v1.md.',
      'Registry 2026-06-21.1 is a DE-2 cloud primitive blocker de-stale pass and flips NO promise state (cloud.primitives_suite.v1 stays planned; cloud.fine_tuning_service.v1 and cloud.sandbox_compute_service.v1 stay red; green count unchanged). Fine-tuning and sandbox still are not live sellable services, but their route scaffolds are no longer accurately described as simply unbuilt: /v1/fine_tuning/jobs and /v1/sandboxes exist behind default-off flags, with typed request surfaces, lifecycle reads, cross-account isolation, TTL/isolation controls where applicable, and a tested receipt-first cloud-metering seam. Stale unbuilt blocker refs are replaced with narrower live-service blockers: live fine-tuning intake disabled, real fine-tuning runtime unwired, live sandbox rent surface disabled, sandbox live metering/billing unwired, and suite-level fine-tuning/sandbox live-sellable-service missing. No flags are armed, no runtime adapter is wired, no pricing is live, no credit debit or settlement occurs, and no paid receipt or green transition is created. Evidence: docs/inference/2026-06-19-cloud-primitives-fine-tuning-sandbox-scaffold-advance.md and apps/openagents.com/workers/api/src/cloud/*.',
      'Registry 2026-06-21.2 is a DE-2 cloud primitives unified-balance blocker de-stale pass and flips NO promise state (cloud.primitives_suite.v1 stays planned; green count unchanged). The composed-run source already models the one-shared-balance shape: buildComposedRunPlan carries a single ComposedRunBalance and receipt envelope, composeRunExecution folds component charge shapes into one composed spend, and the receipt gate checks one_shared_balance plus reconciliation. Therefore the stale blocker.product_promises.cloud_primitives_unified_balance_unbuilt is replaced with blocker.product_promises.cloud_primitives_live_unified_balance_debit_receipt_missing. The remaining gap is live multi-primitive execution debiting one real balance and producing a dereferenceable unified-balance receipt. No route is armed, no D1 balance is read/debited, no receipt row is written, no customer composed-run claim is created, and no green transition is created.',
      'Registry 2026-06-21.3 advances proof.demand_provenance.v1 from yellow to GREEN (green 26 -> 27) on broad projection coverage plus a receipt-first transition. GET /api/public/demand-provenance now carries the SAME typed internal/external/unlabeled demand split and reconciliation for ALL revenue-bearing public surfaces, not just AO/kWh: pylon-stats (/api/public/pylon-stats), the training leaderboards (/api/training/leaderboards/*), the training run pages (/api/public/training/runs/{trainingRunRef}), and the model-ladder rung economics gates (/api/public/training/model-ladder-rungs). coveredRevenueBearingSurfaceCount is 5 with remainingSurfaceRefs empty. This GENUINELY clears blocker.product_promises.demand_provenance_broad_projection_coverage_missing (removed from the promise blockerRefs). CRITICAL: this is a transparency/coverage flip ONLY. externalDemandClaimAllowed STAYS false on every surface and in totals — every current revenue-bearing number is backed by internal first-party demand (plumbing proof, not market proof: the operator-staged #4777 outcome, first-party training-pipeline ablations/sweeps/closeouts, and unfunded rung economics gates). NO live event, NO money moved, NO external-revenue claim, NO settlement, NO payout. Green means the labeling discipline is complete across every revenue-bearing surface; any future external-market-demand claim still requires a real external dollar under the rule no_external_dollar_no_demand_claim. The yellow->green transition is receipt-first per proof.claim_upgrade_receipts.v1: the flip is applied in source ahead of the deployed registry per the 2026-06-14 reconciliation pattern, and the matching transition receipt is recorded via POST /api/operator/product-promises/transitions as promise_transition_ccf5d7d8-5737-4949-b534-19e6fab9c157 (its blockers_clear_for_green check evaluates the still-deployed registry and clears once this version deploys). Evidence: apps/openagents.com/workers/api/src/demand-provenance.ts, demand-provenance.test.ts, and route:/api/public/demand-provenance.',
      'Registry 2026-06-21.4 is a models.tassadar_percepta_executor.v1 CPU-transform receipt-status pass and flips NO promise state (stays planned, green count unchanged at 27). GET /api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts now serves a public-safe, live-at-read status projection that cites the architecture receipt and Artanis distillation dataset receipt as visible inputs, then reports every real CPU-transform training gate false: no Pylon assignment receipt, accepted-work receipt, verifier verdict receipt, real settlement receipt, trained artifact digest, or green gate. blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing remains active. No assignment, dispatch, spend, settlement, trained model, inference endpoint, model promotion, capability claim, or green transition is created. Evidence: docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md.',
      'Registry 2026-06-23.1 is a DE-1 (#5524) sites.referral_bitcoin_stream.v1 settlement-receipt destale pass and flips NO promise state (stays yellow; green count unchanged at 27). The stale blocker blocker.product_promises.referral_settlement_receipts_missing is DROPPED, cleared by a CLOSED-LOOP proof that the public settled referral-payout receipt surface is genuinely dereferenceable end to end. Until now the RL-1 dispatch (#5458) was proven only against a MOCK adapter (site-referral-payout-wire.test.ts) and the public receipt route only against a MOCK store, with nothing connecting them: no test ran feed -> dispatch -> a real settled D1 row -> and then dereferenced the receipt the dispatch produced through the REAL public receipt store. That gap WAS the missing-settlement-receipts blocker. This pass adds a staging-test settlement adapter (apps/openagents.com/workers/api/src/site-referral-payout-staging-adapter.ts) that satisfies the SAME ReferralPayoutAdapter contract as the production hosted-MDK adapter and walks the SAME idempotent, readiness-gated, asset-boundary-enforced approved -> dispatched -> settled path, but moves NO money by construction: it has no wallet client, no destination resolver, and no rail call; it is gated behind an explicit enabled flag that defaults OFF and FAILS CLOSED (throws) when disabled so the dispatcher records NO settled state; and it produces a deterministic, public-safe receipt.site_referral_payout.staging_test.* evidence ref the SAME public receipt store dereferences as the staging_test settlement rail. site-referral-payout-receipt-loop.test.ts proves the full loop (5 tests): feed -> dispatch (staging adapter) -> a real settled ledger row -> the produced receipt dereferences through makeD1SiteReferralPayoutReceiptStore with amount/attributionLinked/qualifyingEventKind/policy+caveat refs and no private payout material; an idempotent re-drive settles at most once and the SAME receipt still dereferences; and the fail-safe (disabled adapter) records no settled state so nothing dereferences. The promise STAYS yellow with ONE remaining blocker, referral_first_real_payout_pending: green requires a REAL Bitcoin-revenue production event producing a real settled referral payout receipt over the LIVE hosted-MDK rail (not staging_test), which is owner-armed — the owner step is arming the MDK live payout mode (livePayoutClaimAllowed true via hostedMdkDirectPayoutDisabledGate -> the funded hosted-MDK programmatic-payout client + a registered referrer destination, #5512) — plus owner sign-off per proof.claim_upgrade_receipts.v1. NO money moved, NO real payout, NO live event, NO green flip. Evidence: apps/openagents.com/docs/sites/2026-06-23-site-referral-settlement-receipt-staging-loop.md, apps/openagents.com/workers/api/src/site-referral-payout-staging-adapter.ts, apps/openagents.com/workers/api/src/site-referral-payout-receipt-loop.test.ts.',
      'Do not post secrets, wallet material, provider payloads, private repository data, raw invoices, preimages, or customer-sensitive content in public reports.',
    ],
  }

  return {
    ...document,
    verificationSummary: summarizePromiseVerification(document.promises),
  }
}

export const publicProductPromisesAnnouncementReadiness = (
  expectedVersion: string,
  document = publicProductPromisesDocument(),
) => {
  const expected = expectedVersion.trim()
  const servedVersion = document.version
  const status = servedVersion === expected ? 'ready' : 'blocked'

  return {
    blockerRefs:
      status === 'ready'
        ? []
        : [
            `product-promises-announcement-blocker:expected-version-not-served:${expected}`,
          ],
    expectedVersion: expected,
    generatedAt: document.generatedAt,
    maxStalenessSeconds: document.maxStalenessSeconds,
    servedVersion,
    status,
    staleness: document.staleness,
  }
}

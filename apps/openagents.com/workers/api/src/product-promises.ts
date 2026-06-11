export const PublicProductPromisesEndpoint = '/api/public/product-promises'
export const PublicProductPromisesSchemaVersion =
  'openagents.product_promises.v1'
export const PublicProductPromisesVersion = '2026-06-11.3'

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
  'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
  'docs/2026-06-10-cs336-distributed-homework-continuation-audit.md',
  'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
  'docs/refactor/path-to-bolt-12.md',
  'docs/transcripts/199.md',
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
  const document = {
  schemaVersion: PublicProductPromisesSchemaVersion,
  version: PublicProductPromisesVersion,
  lastUpdated: '2026-06-11',
  canonicalDocsUrl:
    'https://github.com/OpenAgentsInc/openagents/tree/main/docs/promises',
  sourceRefs,
  publicDocsUrl: 'https://openagents.com/docs/product-promises',
  latestGapAuditUrl:
    'https://github.com/OpenAgentsInc/openagents/blob/main/docs/promises/2026-06-09-product-promises-gap-audit.md',
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
      'The openagents monorepo now contains the deployed openagents.com Worker/app, Forum surfaces, docs/promises, packages/probe, and apps/pylon. The live Cloudflare deployment is served from apps/openagents.com, and the public code map in this registry points agents to the public source trees behind those shipped surfaces. Pylon v0.3 code is present under apps/pylon as @openagentsinc/pylon@0.3.0-rc1, but public product copy must still distinguish local rc gates from live network evidence.',
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
      'Pylon v0.3 is a release candidate, not stable 0.3.0.',
      'macOS and Linux are the first supported operator platforms for the v0.3 launch path.',
      'Pylon v0.3 local release gates and a no-spend live worker-loop smoke exist, but broad earning, paid assignment, settlement, and stable-release claims still need fresh public evidence before they go green.',
      'Training, data revenue, provider-capacity resale, referral payouts, and signature revenue remain gated or planned. Local-inference and Qwen products are out of scope by owner decision (2026-06-10): no inference products at this time; model-training focus is Tassadar.',
      'Episode 199 Claude Code-first mech-suit language is historical source material, not current public positioning. Current coding-agent runtime work is Codex-oriented where applicable, with useful ideas folded into Probe/Pylon.',
      'Open-source availability covers public product code and docs in the OpenAgentsInc/openagents repository; it does not publish secrets, production data, Cloudflare account bindings, wallet material, provider credentials, customer-private workroom content, or third-party service internals.',
      'The full training-pipeline program (the training.* promises) is planned scope from docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md; no Psion model rung above the retained tri-host rehearsal exists, and no pipeline stage is live as a broadly paid network workload.',
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
      blockerRefs: ['blocker.product_promises.stale_launch_framing_withdrawn'],
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
      state: 'yellow',
      claim:
        'The useful parts of the historical coding-agent wrapper idea are now represented by the current Codex-oriented Autopilot/Probe/Pylon runtime direction.',
      safeCopy:
        'OpenAgents coding-agent runtime work is Codex-oriented where applicable, with Probe/Pylon carrying relevant runtime and provider-account ideas under current gates.',
      unsafeCopy:
        'Do not claim the old Claude Code-first launch framing is the current implementation, or that the current Codex/Probe/Pylon path is fully green.',
      evidenceRefs: [
        'work_order:autopilot_work_order.a1aef38e-66e7-488f-a06c-05dd02b34b35',
        'route:/api/autopilot/work?promiseId=autopilot.codex_probe_pylon_successor.v1',
        'apps/pylon/README.md',
        'packages/probe',
        'apps/openagents.com/docs/probe/2026-06-07-first-party-probe-runtime-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.live_probe_pylon_runtime_gates_incomplete',
      ],
      verification:
        'Requires current Codex-backed task path evidence, Probe/Pylon runtime smokes, public docs, and live assignment/closeout evidence before green copy.',
      authorityBoundary:
        'A successor direction does not imply broad provider support, unattended writes, payment settlement, or live marketplace authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.v03_release_candidate.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'agent', 'operator'],
      state: 'yellow',
      claim:
        'Pylon v0.3 has been pulled into the monorepo as the release-candidate contributor node and includes the former Probe runtime surface.',
      safeCopy:
        'Pylon v0.3 is present under apps/pylon as @openagentsinc/pylon@0.3.0-rc1 for macOS and Linux rc work; stable v0.3.0 and network-wide earning remain gated.',
      unsafeCopy:
        'Do not claim Pylon v0.3.0 is stable or broadly live for anyone to earn Bitcoin automatically.',
      evidenceRefs: [
        'apps/pylon/package.json',
        'apps/pylon/README.md',
        'apps/pylon/docs/launch-gates-no-overclaim.md',
      ],
      blockerRefs: [
        'blocker.product_promises.pylon_v03_stable_release_not_green',
        'blocker.product_promises.pylon_v03_live_network_smokes_incomplete',
      ],
      verification:
        'apps/pylon package metadata reports @openagentsinc/pylon@0.3.0-rc1 and the local release gate documents blocked copy for stable/network claims.',
      authorityBoundary:
        'A local rc package does not prove live OpenAgents network registration, paid work, payout, settlement, or marketplace authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.release_tomorrow.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'public'],
      state: 'yellow',
      claim: 'A new version of Pylon releases.',
      safeCopy:
        'Pylon v0.3 is a release candidate in the monorepo targeting macOS and Linux; it is not stable 0.3.0. Windows/WSL is deliberately out of scope by owner decision (2026-06-10), not a pending gap.',
      unsafeCopy:
        'Do not claim a stable universal Pylon release works on every computer.',
      evidenceRefs: [
        'apps/pylon/package.json',
        'apps/pylon/docs/release-install-smokes.md',
        'apps/pylon/docs/launch-gates-no-overclaim.md',
      ],
      blockerRefs: [
        'blocker.product_promises.pylon_v03_stable_release_not_green',
      ],
      verification:
        'Run apps/pylon local release gates and separate install smokes before changing this to green.',
      authorityBoundary:
        'Release-package availability does not imply assignment readiness, wallet readiness, or earning readiness.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.first_real_model_training_run.v1',
      productArea: 'training',
      audience: ['contributor', 'public'],
      state: 'red',
      claim: 'Pylon starts the first real model-training run.',
      safeCopy:
        'Local and loopback training rehearsals exist, but no public remote multi-device model-training run is live.',
      unsafeCopy:
        'Do not claim the first public remote Pylon model-training run is live.',
      evidenceRefs: [
        'apps/pylon/docs/2026-06-09-pylon-psionic-ml-connection-audit.md',
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/2026-06-10-cs336-distributed-homework-continuation-audit.md',
        'directive.owner.20260611.no_inference_focus_tassadar',
      ],
      blockerRefs: [
        'blocker.product_promises.remote_multi_device_training_missing',
      ],
      verification:
        'Requires public remote worker, shard, merge/eval/admission, payment, settlement, and projection refs before green copy. The honest green path is Tassadar executor training: curated verified-trace corpora through the CS336 rails (artanis.tassadar_evolution_loop.v1 Stages 1-3) graded by exact replay, then the model ladder’s network rungs (training.model_ladder.v1) on real contributor devices with commitment-backed verification and paid closeouts. Qwen fine-tuning is out of scope by owner decision.',
      authorityBoundary:
        'GEPA/text optimization and local loopback rehearsals are not neural-network training on public contributor devices.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.five_bitcoin_revenue_streams.v1',
      productArea: 'payments',
      audience: ['contributor', 'public'],
      state: 'red',
      claim:
        'Pylon stacks compute, data, Forum tips, referrals, and agent labor markets in one install.',
      safeCopy:
        'Forum tipping is live and the NIP-90 compute/data/labor market rails shipped in earlier releases (Episodes 213-215) exist in repo history, but one-install multi-stream Bitcoin earning is not live in the current app.',
      unsafeCopy:
        'Do not claim one Pylon install creates five live Bitcoin revenue streams, and do not describe the labor stream as provider-capacity resale; it sells agent work output, not account access.',
      evidenceRefs: [
        'docs/transcripts/213.md',
        'docs/transcripts/214.md',
        'docs/transcripts/215.md',
        'apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4652',
        'apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md',
        'apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md',
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
      promiseId: 'pylon.compute_revenue_modes.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'public'],
      state: 'red',
      claim:
        'Compute revenue comes from GEPA optimization slices and Tassadar executor-trace work on people’s devices.',
      safeCopy:
        'Pylon v0.3 has GEPA-first local capability contracts and live no-spend Tassadar executor-trace dispatch with one operator-funded settled closeout, but paid full-network GEPA is not green. Local-inference and Qwen fine-tune revenue are out of scope by owner decision (2026-06-10): no inference products at this time.',
      unsafeCopy:
        'Do not claim full-network GEPA revenue, and do not describe local-inference or Qwen fine-tune products as existing or planned.',
      evidenceRefs: [
        'apps/pylon/docs/gepa-capability-envelope.md',
        'apps/openagents.com/docs/2026-06-08-probe-gepa-paid-mode-campaign-ladder.md',
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'transition:promise_transition_4ba43958-3084-4c90-ab0d-10562a863117',
        'directive.owner.20260611.no_inference_focus_tassadar',
      ],
      blockerRefs: [
        'blocker.product_promises.live_gepa_network_missing',
      ],
      verification:
        'Requires live OpenAgents endpoint smokes, fresh Pylon heartbeats, accepted closeouts, and payment-mode evidence before broad compute revenue copy.',
      authorityBoundary:
        'Capability envelopes and retained fixtures do not authorize paid public work or settlement.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.data_trace_revenue.v1',
      productArea: 'data',
      audience: ['contributor', 'public'],
      state: 'red',
      claim:
        'Data revenue includes mining valuable local traces from Claude Code, Codex, and other agent work.',
      safeCopy:
        'Data trace marketplace language and gates exist; no public-safe settled trace sale is live.',
      unsafeCopy:
        'Do not claim local traces are currently bought, valued, paid, or settled.',
      evidenceRefs: [
        'apps/openagents.com/docs/2026-06-08-data-trace-marketplace-gate.md',
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
      state: 'red',
      claim:
        'Anyone can install Pylon without Bitcoin wallet knowledge, without loading bitcoin, and start turning a computer into bitcoin.',
      safeCopy:
        'Pylon can classify wallet configured, receive-ready, send-ready, payout-target, payable, settlement, and receipt states, but broad no-wallet-knowledge earning is not live.',
      unsafeCopy:
        'Do not claim no-wallet-knowledge installs immediately earn spendable Bitcoin.',
      evidenceRefs: [
        'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
        'apps/openagents.com/docs/2026-06-08-pylon-install-to-bitcoin-launch-smoke.md',
      ],
      blockerRefs: [
        'blocker.product_promises.mdk_send_readiness_not_proven_for_restore',
        'blocker.product_promises.live_install_to_bitcoin_smoke_incomplete',
      ],
      verification:
        'Requires live-small-sats install-to-bitcoin smoke with operator approval, spend cap, MDK send readiness, payment receipt, settlement receipt, and public projection refs.',
      authorityBoundary:
        'Receive readiness and balance are not send readiness, payout dispatch, or settled earning.',
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
        'Site referral capture records attribution; Bitcoin referral streaming is not live.',
      unsafeCopy: 'Do not claim referral links pay Bitcoin streams now.',
      evidenceRefs: [
        'https://openagents.com/docs/autopilot-sites',
        'route:/r/site/{publicSourceRef}',
        'apps/openagents.com/docs/sites/2026-06-08-site-referral-reward-withdrawal-gate.md',
        'apps/openagents.com/docs/2026-06-10-site-referral-payout-policy.md',
        'route:/api/operator/sites/referrals/payout-ledger/{payoutRef}/transitions',
      ],
      blockerRefs: [
        'blocker.product_promises.referral_settlement_receipts_missing',
      ],
      verification:
        'Referral payout requires attribution consumption, abuse/dispute/cap policy, payout ledger, and settlement receipt projection.',
      authorityBoundary:
        'Referral attribution is not payout eligibility or spendable settlement.',
    },
    {
      ...basePromiseFields,
      promiseId: 'payments.money_dev_kit.v1',
      productArea: 'payments',
      audience: ['agent', 'contributor', 'operator'],
      state: 'yellow',
      claim:
        'OpenAgents switched payments to Money Dev Kit: self-custodial Lightning agent wallet, single command setup, LSP/splice channels, immediate receive liquidity, and hosted checkout.',
      safeCopy:
        'OpenAgents uses MDK hosted checkout and agent-wallet flows for scoped small-sats/L402 paths, and Forum tips can project confirmed live direct BOLT 12 MDK/provider payments as ordinary content tips. Broader payout, withdrawal, and accepted-work settlement claims remain scoped by their own route authority and wallet readiness.',
      unsafeCopy:
        'Do not claim MDK mnemonic restore or hosted MDK payout proves full send readiness or provider settlement.',
      evidenceRefs: [
        'apps/openagents.com/docs/mdk',
        'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
        'apps/openagents.com/docs/nexus/2026-06-08-mdk-agent-wallet-outbound-capacity-restore-report.md',
        'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
        'route:/api/forum/paid-actions/mdk/webhooks',
        'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
        'apps/openagents.com/docs/forum-tip-payout-smoke.md',
        'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
        'apps/openagents.com/docs/forum/2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md',
        'transition:promise_transition_c30b7327-e82b-4696-8886-97aafa454284',
      ],
      blockerRefs: [
        'blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity',
      ],
      verification:
        'Run smoke:forum:mdk-readiness with a ready-recipient post, user-specified sats amount, explicit live-spend approval, public receipt lookup, and `tip-post-smoke --strict-smooth` from a funded production payer wallet. Separate wallet configured, receive-ready, positive balance, send-ready, direct payment sent, webhook-confirmed payment, timeout recovery, refund/reversal, accepted work, payout, and accepted-work settlement states.',
      authorityBoundary:
        'Payment proof does not bypass route auth, owner scope, moderation, deployment, payout, or settlement gates.',
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
        'OpenAgents can verify paid Autopilot delegation through a hosted Gemini closeout bridge in the route harness, but no public paid hosted Gemini inference product is live.',
      unsafeCopy:
        'Do not claim a paid hosted Gemini inference API is live.',
      evidenceRefs: [
        'apps/openagents.com/workers/api/src/artanis-mind.ts',
        'post.public.forum.artanis.status.5',
        'https://openagents.com/api/openapi.json',
        'apps/openagents.com/docs/2026-06-08-google-adc-gemini-agent-platform-auth-audit.md',
        'apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts',
        'docs/autopilot-coder/2026-06-09-probe-autopilot-sites-agent-api-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.public_paid_model_gateway_missing',
        'blocker.product_promises.production_hosted_gemini_executor_binding_missing',
      ],
      verification:
        'A local Autopilot route smoke can price a hosted_gemini request, return L402, accept a paid retry, run an injected hosted executor, persist a public-safe delivered closeout, and expose delivered events. A live hosted Gemini claim still needs a registered-agent production smoke, executor binding, billing, entitlement, provider policy, quota, metering, and settlement refs.',
      authorityBoundary:
        'API-driven product surfaces are not generic provider-capacity resale authority.',
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
        'The product direction is agentic labor and Sites; not every labor/product flow is self-serve or settlement-backed.',
      unsafeCopy:
        'Do not claim all agentic labor/product sales are live and settlement-backed.',
      evidenceRefs: [
        'https://openagents.com/docs/openagents',
        'https://openagents.com/docs/autopilot-sites',
      ],
      blockerRefs: ['blocker.product_promises.not_all_labor_flows_self_serve'],
      verification:
        'Customer-facing claims should map to order, review, artifact, acceptance, billing, and handoff evidence.',
      authorityBoundary:
        'Product direction is not proof of every workflow, payout, or marketplace state.',
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
        'Pylon v0.3 has a Bun/Effect/OpenTUI CLI/TUI, includes the former Probe runtime, and has a passed no-spend live worker-loop smoke against OpenAgents.',
      unsafeCopy:
        'Do not turn the passed no-spend worker-loop smoke into a claim that every Pylon install can run paid work, settle payouts, or satisfy the whole v0.3 release gate.',
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
      promiseId: 'autopilot.control_center_fanout_marketplace.v1',
      productArea: 'Autopilot',
      audience: ['operator', 'agent'],
      state: 'red',
      claim:
        'Control center / Autopilot can fan out work to many agents and pull from a plugin marketplace.',
      safeCopy:
        'Operator assignment and marketplace gate surfaces exist, but self-serve multi-agent fanout plus plugin marketplace execution is not live.',
      unsafeCopy:
        'Do not claim a self-serve control center can fan out paid work to many agents from a live marketplace.',
      evidenceRefs: [
        'route:/api/operator/pylons/assignments',
        'apps/openagents.com/docs/2026-06-08-signature-marketplace-revenue-gate.md',
      ],
      blockerRefs: [
        'blocker.product_promises.self_serve_fanout_missing',
        'blocker.product_promises.plugin_marketplace_execution_missing',
      ],
      verification:
        'Requires self-serve scope, marketplace policy, idempotent dispatch, no-duplicate assignment, spend cap, proof, and settlement gates.',
      authorityBoundary:
        'Operator-only APIs and validation gates are not public self-serve marketplace authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'marketplace.signature_monetization.v1',
      productArea: 'marketplace',
      audience: ['agent', 'developer', 'contributor'],
      state: 'red',
      claim:
        'DSPy/GEPA signatures and agent workflow components can be discoverable and monetizable.',
      safeCopy:
        'Signature validation, Blueprint tooling, and marketplace gates exist; usage metering, billing, revenue split, and settlement are not live.',
      unsafeCopy:
        'Do not claim signatures or workflow components are generating settled revenue.',
      evidenceRefs: [
        'apps/openagents.com/docs/2026-06-08-signature-marketplace-revenue-gate.md',
        'apps/pylon/packages/runtime/src/blueprint',
      ],
      blockerRefs: [
        'blocker.product_promises.signature_usage_metering_missing',
        'blocker.product_promises.signature_settlement_missing',
      ],
      verification:
        'Requires package validation, runtime activation, metering, attribution, pricing, rev-share, dispute/refund policy, and settlement receipts.',
      authorityBoundary:
        'Discovery or contribution validation does not install, promote, bill, or settle package usage.',
    },
    {
      ...basePromiseFields,
      promiseId: 'provider.compliant_usage_labor.v1',
      productArea: 'labor',
      audience: ['contributor', 'agent', 'operator', 'public'],
      state: 'red',
      claim:
        'Contributors can connect their own provider accounts or prepaid API budgets and earn Bitcoin by doing useful work with that compliant usage through the agent labor market; OpenAgents never resells provider access.',
      safeCopy:
        'Provider-account connection and device-login flows exist and contributors keep full custody of their own accounts; paid labor jobs that monetize that compliant usage by selling work output are not live yet.',
      unsafeCopy:
        'Do not claim or imply OpenAgents resells, rents, shares, proxies, or brokers anyone’s subscription seat, provider account, session, or API access. OpenAgents pays for accepted work output only; contributors run their own accounts under their own provider terms.',
      evidenceRefs: [
        'docs/transcripts/214.md',
        'apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md',
        'apps/openagents.com/docs/2026-06-10-compliant-usage-labor-policy.md',
        'apps/pylon/packages/runtime/src/contracts/provider-account.ts',
      ],
      blockerRefs: ['blocker.product_promises.labor_stream_not_live'],
      verification:
        'A labor job must run on the contributor’s own connected account or API budget with output-only delivery, payment for accepted results, and a public settlement receipt. No provider credentials, session tokens, or account access may be transferred, metered for resale, or brokered.',
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
        'A coding agent can follow OpenAgents instructions, register with a BOLT12 offer, post on Forum, and automatically have public-safe Forum tip recipient readiness.',
      safeCopy:
        'A coding agent can read AGENTS, register with a bolt12Offer, post to open Forum routes, automatically claim public-safe tip recipient wallet readiness, and have that readiness appear on its Forum post.',
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
        'Register an agent with a bolt12Offer. It automatically submits a public-safe tip-recipient wallet claim. Create an unlisted Forum topic, and verify the post projects tippingAvailable true.',
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
        'Owner-granted agents can read a Mission Briefing projection for any Autopilot work order at GET /api/autopilot/work/{workOrderRef}/briefing: event rollup, changed artifact/result refs, blocked requirements, running state, waiting decision, cost rollup, and grouped drill-down refs. Risk and receipt rollups remain incomplete until richer operator evidence exists.',
      unsafeCopy:
        'Do not claim users can always understand any multi-hour mission in under two minutes with complete diffs, tests, costs, receipts, and next actions, and do not present a briefing as deploy, spend, acceptance, or settlement authority.',
      evidenceRefs: [
        'route:/api/autopilot/work/{workOrderRef}/briefing',
        'apps/openagents.com/workers/api/src/autopilot-mission-briefing.ts',
        'apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts',
        'docs/promises/2026-06-09-product-promises-green-roadmap.md',
      ],
      blockerRefs: [
        'blocker.product_promises.drilldown_artifact_refs_incomplete',
        'blocker.product_promises.cost_risk_receipt_rollup_missing',
      ],
      verification:
        'Run the autopilot-work-routes briefing route test: a delivered work order must project briefing sections, grouped drill-down refs, and authority-free flags. A green record additionally requires at least one live mission with briefing JSON, decision-needed state, artifact refs, test refs, cost/risk summary, and public-safe proof refs.',
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
        'Decision-queue actions are planned/scoped and must remain route-authorized and receipt-backed.',
      unsafeCopy:
        'Do not claim agents can freely continue, retry, spend, mutate repositories, or switch accounts from public docs.',
      evidenceRefs: [
        'docs/promises/2026-06-09-product-promises-green-roadmap.md',
        'apps/openagents.com/workers/api/src/agent-goal-runtime.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.decision_queue_api_missing',
        'blocker.product_promises.account_retry_authority_not_public',
        'blocker.product_promises.receipt_backed_command_closeout_missing',
      ],
      verification:
        'Green requires authenticated command APIs with explicit action enums, idempotency, owner approval where needed, receipt closeout, and UI projection.',
      authorityBoundary:
        'A visible decision does not grant account, repository, spend, deploy, or continuation authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'workrooms.source_authorized_business_objects.v1',
      productArea: 'workrooms',
      audience: ['user', 'agent', 'operator'],
      state: 'red',
      claim:
        'Business workrooms should turn chat and files into source-authorized contacts, companies, tasks, decisions, documents, approvals, artifacts, and receipts.',
      safeCopy:
        'Source-authorized business workrooms are a roadmap target. Current public copy should not claim full CRM, legal, investor, support, or finance workrooms are live.',
      unsafeCopy:
        'Do not claim generated summaries become operational truth without connector source refs, human approval, and write receipts.',
      evidenceRefs: [
        'docs/promises/2026-06-09-product-promises-gap-audit.md',
        'apps/openagents.com/workers/api/src/omni-support-project-ops-workrooms.ts',
        'apps/openagents.com/workers/api/src/omni-crm-follow-up-workrooms.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.source_authority_model_not_green',
        'blocker.product_promises.connector_read_receipts_missing',
        'blocker.product_promises.approval_gated_business_writes_missing',
      ],
      verification:
        'Green requires a live workroom kind with source refs, proposed updates, approvals, artifacts, closeout receipts, and public-safe proof.',
      authorityBoundary:
        'Workroom summaries are proposals until accepted by the authorized user or organization.',
    },
    {
      ...basePromiseFields,
      promiseId: 'mobile.voice_approval_companion.v1',
      productArea: 'mobile and voice',
      audience: ['user', 'agent'],
      state: 'planned',
      claim:
        'Voice and mobile should let users inspect workrooms, review pending approvals, issue bounded commands, and see the same approval receipts without bypassing server-side policy.',
      safeCopy:
        'Voice/mobile approval is planned. Any current voice or mobile language must say approvals remain server-side and receipt-backed.',
      unsafeCopy:
        'Do not claim a voice transcript or mobile tap can directly mutate CRM, send email, create PRs, spend money, launch paid runners, or publish claims.',
      evidenceRefs: [
        'docs/promises/2026-06-09-product-promises-green-roadmap.md',
        'apps/openagents.com/workers/api/src/omni-voice-session-evidence.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.mobile_projection_missing',
        'blocker.product_promises.voice_command_approval_receipts_missing',
        'blocker.product_promises.cross_device_workroom_sync_missing',
      ],
      verification:
        'Green requires a voice command or mobile approval flow that records transcript/source refs, proposed action, approval decision, and matching workroom receipt.',
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
        'Accepted-outcome economics are a required roadmap gate before broad payout or marketplace-margin claims go green.',
      unsafeCopy:
        'Do not collapse paid, accepted, payable, dispatched, confirmed, reconciled, settled, and gross-margin states into one claim.',
      evidenceRefs: [
        'docs/promises/2026-06-09-product-promises-gap-audit.md',
        'apps/openagents.com/workers/api/src/pylon-accepted-work-payout-slo.ts',
        'apps/openagents.com/workers/api/src/pylon-accepted-work-proof-links.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.settlement_state_machine_incomplete',
        'blocker.product_promises.contributor_ledger_missing',
        'blocker.product_promises.gross_margin_receipts_missing',
      ],
      verification:
        'Green requires one accepted outcome with separate authorized, paid, accepted, pending payout, dispatched, confirmed, reconciled, and margin evidence.',
      authorityBoundary:
        'Payment evidence alone is not accepted-work payout or final settlement evidence.',
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
        'Flexible-load economics are planned/modeling work and must be labeled as modeled until measured operator proof exists.',
      unsafeCopy:
        'Do not claim grid-service revenue, AI-load smoothing revenue, avoided interconnection cost, or energy-market optimization without event proof and caveats.',
      evidenceRefs: [
        'docs/promises/2026-06-09-product-promises-green-roadmap.md',
        'docs/promises/registry.md',
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
      ],
      blockerRefs: [
        'blocker.product_promises.energy_market_ingestion_missing',
        'blocker.product_promises.work_class_flex_profiles_missing',
        'blocker.product_promises.flexible_load_event_history_missing',
        'blocker.product_promises.operator_proof_report_missing',
      ],
      verification:
        'Green requires an operator report with measured or explicitly modeled dollars per MWh, evidence-state labels, and public-safe caveats. A planned evidence path is the training marathon’s scheduled curtailment drill (training.marathon_operations.v1): shed part of the fleet on schedule, resume from sealed checkpoints, publish the receipt.',
      authorityBoundary:
        'Energy models are operational estimates, not investment, grid, utility, or financial advice.',
    },
    {
      ...basePromiseFields,
      promiseId: 'proof.claim_upgrade_receipts.v1',
      productArea: 'public proof',
      audience: ['agent', 'user', 'operator', 'public'],
      state: 'yellow',
      claim:
        'Public claims should upgrade only when required receipts exist, and sensitive work should route according to reusable policy rather than marketing copy or operator judgment.',
      safeCopy:
        'Promise transition receipts are live: operators record proposed state transitions at POST /api/operator/product-promises/transitions, each receipt mechanically checks the registry record (promise exists, state differs, evidence present, verification named, blockers clear for green) and supports explicit policy-exception records, the public feed at GET /api/public/product-promises/transitions lists them, and each promise in the registry carries lastVerifiedAt from its latest passing receipt. The enterprise audit panel is still missing.',
      unsafeCopy:
        'Do not manually upgrade public claims to green without matching evidence, policy boundary, and receipt refs, and do not present a passing receipt as the state change itself - registry transitions remain maintainer actions.',
      evidenceRefs: [
        'route:/api/public/product-promises/transitions',
        'apps/openagents.com/workers/api/src/promise-transition-receipt-routes.ts',
        'apps/openagents.com/workers/api/src/promise-transition-receipt-routes.test.ts',
        'https://openagents.com/api/public/product-promises',
        'https://openagents.com/promises',
        'docs/promises/registry.md',
      ],
      blockerRefs: [
        'blocker.product_promises.enterprise_audit_panel_missing',
      ],
      verification:
        'Run the promise-transition-receipt tests: receipts must bind promiseId, from/to state, registry version, evidence refs, typed checks, exceptions, and timestamps, the public feed must serve them, and lastVerifiedAt must derive from passing receipts. Green additionally requires the enterprise audit panel.',
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
        'Verified X owner claims record a 1000-sat reward eligibility row in a bounded campaign ledger with anti-Sybil dedupe (one reward per X account and per challenge) and a campaign budget cap. Eligibility, operator-approved dispatch, treasury dispatch, and settlement are separate states. The Worker-side dispatcher is implemented behind TREASURY_DISPATCH_ENABLED=false by default with per-run and per-day caps, pending-payment polling, and public-safe status stats. No reward has completed a live dispatch smoke yet.',
      unsafeCopy:
        'Do not claim verified owners are instantly or automatically paid, do not present eligibility as spendable balance or settlement, and do not describe the promotional reward as Forum tip settlement or accepted-work payout.',
      evidenceRefs: [
        'route:/api/agents/claims/{claimId}/x/verify',
        'route:/api/agents/claims/rewards/{rewardId}/dispatch',
        'apps/openagents.com/workers/api/migrations/0149_x_claim_reward_ledger.sql',
        'apps/openagents.com/workers/api/migrations/0164_x_claim_reward_treasury_dispatch.sql',
        'apps/openagents.com/workers/api/src/agent-owner-claim-routes.test.ts',
        'apps/openagents.com/workers/api/src/x-claim-reward-treasury-dispatcher.test.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.x_claim_reward_live_dispatch_smoke_missing',
      ],
      verification:
        'Run the agent-owner-claim reward tests and the X-claim treasury dispatcher tests: verified X claims must create eligibility with dedupe and budget refusal, dispatch transitions must be admin-gated, the dispatcher must stay flag-off by default, resolve only registered BOLT12 recipient identity, enforce caps, poll pending payments without re-paying, and redact payment material. Green requires one live operator-dispatched reward settled to a real owner receive code with public-safe receipt refs.',
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
        'Registered agents can self-purchase the orange check through the Forum paid-action rail (preview, hosted MDK checkout against the OpenAgents Orange Check product, signed L402 redeem) and fulfillment is provider-gated: the entitlement is granted only after the checkout reports payment_received. Two live $5 purchases have completed, and the second (2026-06-10) ran the smooth path end to end with no operator intervention on the now-atomic redemption writes. Badges project on agent profiles, posts, and the homepage sold counter. The badge means economic participation with a receipt; it is not identity verification.',
      unsafeCopy:
        'Do not describe orange-checked accounts as verified humans or safe accounts, do not imply the badge buys moderation, settlement, or policy immunity, and do not claim the live purchase smoke has passed before it has.',
      evidenceRefs: [
        'route:/api/forum/paid-actions/preview',
        'route:/api/forum/paid-actions/redeem',
        'apps/openagents.com/workers/api/src/orange-check-entitlements.ts',
        'apps/openagents.com/workers/api/src/forum-routes.test.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.orange_check_nostr_export_missing',
      ],
      verification:
        'Run the forum-routes orange-check purchase test: preview, private payment, unpaid redeem refusal, payment_received-gated fulfillment, badge projection, and copy-boundary scan. Green requires one live $5 purchase settling through the production checkout with the badge visible on the buying agent.',
      authorityBoundary:
        'An orange check is an economic participation signal only. It grants no moderation, identity-verification, settlement, payout, or policy authority, and payment cannot buy any of those.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.gepa_worker_loop_v03.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'operator'],
      state: 'yellow',
      claim:
        'Pylon v0.3 can run GEPA-first assignment work through the in-repo runtime.',
      safeCopy:
        'Pylon v0.3 has assignment, GEPA capability, and runtime contracts with fake-server/no-spend coverage. A 2026-06-11 live no-spend OpenAgents endpoint smoke created, accepted, progressed, artifact-submitted, and operator-closed an unpaid assignment in production. Paid GEPA settlement remains gated.',
      unsafeCopy:
        'Do not claim Pylon v0.3 runs the full live GEPA network or settles paid GEPA work.',
      evidenceRefs: [
        'apps/pylon/src/assignment.ts',
        'apps/pylon/src/gepa-capability.ts',
        'apps/pylon/packages/runtime/src/benchmark',
        'apps/pylon/docs/2026-06-10-v03-live-worker-loop-smoke.md#recheck-2026-06-11-0110-utc',
        'promise_transition_d0f7edc5-1688-4039-bcdf-8971b79512ef',
      ],
      blockerRefs: [
        'blocker.product_promises.paid_gepa_settlement_v03_missing',
      ],
      verification:
        'Run v0.3 assignment fake-server/no-spend tests locally and the production no-spend assignment smoke. Green network copy still requires one settled paid GEPA assignment receipt, a paid-settlement transition receipt before registry edit, and a deployed registry bump.',
      authorityBoundary:
        'No-spend closeout and retained fixtures do not prove paid settlement or live campaign authority.',
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
        'A written buildout plan exists (docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md) extending the CS336 program (#4673-#4684) with the Smol Training Playbook as operational reference; psionic holds bounded CS336 reference lanes and a retained tri-host pretraining rehearsal. No full pipeline stage is live as a broadly paid network workload.',
      unsafeCopy:
        'Do not claim OpenAgents currently operates an end-to-end training pipeline, trains world-class or frontier-class models, or that the network is training models for buyers.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/2026-06-10-cs336-distributed-homework-continuation-audit.md',
        'https://github.com/OpenAgentsInc/psionic/tree/main/docs/smol',
        'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
      ],
      blockerRefs: [
        'blocker.product_promises.training_pipeline_rails_incomplete',
      ],
      verification:
        'This umbrella promise tracks the program; each workstream and ladder rung carries its own training.* promise with its own evidence. Green requires every workstream promise at green or yellow plus at least one ladder rung completed end to end with public receipts.',
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
        'Psionic has a bounded ablation tool inside the actual-pretraining baseline bundle. The manifest-enforced one-change-at-a-time harness, the eval-suite reproduction gate, ablation runs as dispatched paid work, and the public ablation ledger are planned.',
      unsafeCopy:
        'Do not claim training decisions are currently ablation-receipted, or cite the Smol Training Playbook’s measurements as OpenAgents results.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
        'https://github.com/OpenAgentsInc/psionic/issues/1116',
        'https://github.com/OpenAgentsInc/psionic/issues/1118',
      ],
      blockerRefs: [
        'blocker.product_promises.ablation_harness_missing',
        'blocker.product_promises.eval_suite_reproduction_missing',
        'blocker.product_promises.ablation_ledger_projection_missing',
      ],
      verification:
        'Gate zero: reproduce published scores for at least one open reference model through the owned eval harness with a receipt. Green requires the harness mechanically refusing multi-delta manifests, ablation cells dispatched and verified as paid assignments (seeded replication), and a public derisking-ledger projection of baselines, deltas, and verdicts.',
      authorityBoundary:
        'An ablation verdict is training-decision evidence only; it grants no capability claim, no public copy upgrade, and no dispatch or spend authority.',
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
        'The A4 deterministic refinery core landed in psionic (PII masking, Gopher quality rules, exact and MinHash dedup) and the live a4_eval_delta leaderboard serves an honest empty state. Psion’s current corpus is a frozen bounded mixture; crawl-scale acquisition, paid shard assignments, corpus provenance receipts, decontamination receipts, and eval-delta payment remain planned.',
      unsafeCopy:
        'Do not claim a crawl-scale receipted corpus exists, that contributors are currently paid for data-refinery work, or that data quality is paid on measured eval delta.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'apps/openagents.com/docs/2026-06-08-data-trace-marketplace-gate.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4680',
      ],
      blockerRefs: [
        'blocker.product_promises.crawl_scale_corpus_missing',
        'blocker.product_promises.corpus_provenance_receipts_missing',
        'blocker.product_promises.eval_delta_payment_missing',
      ],
      verification:
        'Green requires refinery shards dispatched as paid assignments with deterministic-recompute verification, every shard carrying source-provenance and transform digests, mixture/annealing ablation receipts, decontamination receipts against the eval suite, and at least one eval-delta payment computed from a fixed reference model.',
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
        'R0 exists: a retained tri-host 12-step rehearsal (3,992 train tokens at 2.74 effective tokens per second) recorded in psionic’s actual-pretraining runbook. No rung above R0 has started. Rung promises are written before each run and transition only on receipts; no rung is scheduled against a date.',
      unsafeCopy:
        'Do not claim any Psion rung above R0 is trained, in progress, or scheduled, do not present the ladder as a commitment to reach R4, and do not present R0 rehearsal throughput as network training capability.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
      ],
      blockerRefs: [
        'blocker.product_promises.r1_full_rehearsal_missing',
        'blocker.product_promises.rung_economics_gate_format_missing',
      ],
      verification:
        'Each rung requires the prior rung’s closeout receipt plus a published economics gate (all-in cost per accepted training outcome, contributor payout against opportunity floor, verification overhead) with modeled/measured/settled provenance labels. R2 is the honest green path for pylon.first_real_model_training_run.v1 and must compare against a rented-cluster fallback, not a vacuum. A rung whose economics gate fails twice is recorded here, not papered over.',
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
        'Psionic’s actual-pretraining lane has local hardware qualification, checkpoint/resume drills, and continue/hold/restart checkpoint decisions. Durable remote checkpoint storage bound into the window seal, standby-Pylon dispatch, public run monitoring against a prior rung’s eval series, restart-decision receipts, and the scheduled curtailment drill are planned.',
      unsafeCopy:
        'Do not claim multi-day or multi-week network training runs are operationally supported, or that training load is proven dispatchable/curtailable for grid value.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4673',
      ],
      blockerRefs: [
        'blocker.product_promises.durable_checkpoint_seal_missing',
        'blocker.product_promises.standby_dispatch_missing',
        'blocker.product_promises.curtailment_drill_missing',
      ],
      verification:
        'Green requires a window sealed only on durable content-addressed checkpoint storage, a standby contributor promoted into a live run, one restart-or-continue decision recorded as a receipt, and a scheduled drill that sheds part of the fleet on time, resumes from checkpoints, and publishes the receipt — the same drill that becomes evidence for energy.flexible_load_proof.v1.',
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
        'Psionic holds bounded lanes: legal SFT/DPO/GRPO CLI smokes and the CS336 A5 alignment reference math with committed tests. The general instruct lane, owned chat template with generation masking, preference-data and rollout work kinds, decontaminated post-training evals, and vibe-test closeout artifacts are planned. Hybrid reasoning modes are explicitly deferred.',
      unsafeCopy:
        'Do not claim instruct or reasoning Psion models exist, that post-training runs as paid network work, or that any fine-tuning service is live.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4682',
        'https://github.com/OpenAgentsInc/psionic/issues/1117',
        'https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_CS336_A5_REFERENCE_LANE.md',
      ],
      blockerRefs: [
        'blocker.product_promises.instruct_sft_lane_missing',
        'blocker.product_promises.preference_rollout_work_missing',
        'blocker.product_promises.vibe_test_artifact_missing',
      ],
      verification:
        'Green requires one Psion checkpoint carried through mid-training, SFT, and at least one preference-optimization stage on the owned stack, with rollout/grading work dispatched and verified as paid assignments, decontamination receipts, GRPO reward shaping including the overlong-completion penalty, and a reviewed vibe-test transcript artifact in the closeout.',
      authorityBoundary:
        'Post-training receipts prove the scoped arc on the scoped checkpoint only; they are not a model-quality, buyer-demand, or service-availability claim.',
    },
    {
      ...basePromiseFields,
      promiseId: 'training.verification_classes.v1',
      productArea: 'training',
      audience: ['contributor', 'operator', 'agent'],
      state: 'planned',
      claim:
        'Every training-pipeline stage carries a named pluggable verification class — deterministic_recompute, seeded_replication, freivalds_merkle, statistical_cross_check, exact_trace_replay — routed to the cheapest sufficient supply, with weak devices as paid validators.',
      safeCopy:
        'Psionic ships an exact_trace_replay reference implementation and the executor-trace dispatch wiring has passing no-spend smokes; the per-stage verification map is written in the pipeline plan. The Worker-side pluggable class registry, challenge queues, and the paid weak-device validator lane remain in flight (#4674, #4676), and the April-era aggregate-only validation compromise must be re-decided per class in writing.',
      unsafeCopy:
        'Do not claim training work is currently verified end to end on paid assignments, or that validator work is currently earning contributors bitcoin.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
        'apps/openagents.com/workers/api/src/tassadar-executor-trace-homework.test.ts',
        'https://github.com/OpenAgentsInc/openagents/issues/4674',
        'https://github.com/OpenAgentsInc/openagents/issues/4676',
        'https://github.com/OpenAgentsInc/psionic/issues/1115',
        'https://github.com/OpenAgentsInc/psionic/issues/1116',
      ],
      blockerRefs: [
        'blocker.product_promises.verification_class_registry_missing',
        'blocker.product_promises.aggregate_only_policy_redecision_missing',
      ],
      verification:
        'Green requires the pluggable class registry live with at least three classes exercised on real dispatched work, commitment-then-challenge flow for matrix work (Merkle-committed matrices plus Freivalds checks), a paid weak-device validator closeout, and a written per-class decision on aggregate-only versus per-contribution sampling.',
      authorityBoundary:
        'Verification verdicts are acceptance evidence only; they do not settle payouts, upgrade promises, or authorize dispatch, and per-class numeric tolerance contracts do not weaken the Tassadar exact lane’s separate exactness posture.',
    },
    {
      ...basePromiseFields,
      promiseId: 'training.device_capability_dataset.v1',
      productArea: 'training',
      audience: ['contributor', 'operator', 'public'],
      state: 'planned',
      claim:
        'Benchmark assignments produce a public device-capability dataset across heterogeneous contributor hardware — matmul throughput, memory bandwidth, attention-kernel performance, sustained-versus-burst thermals — that honestly prices what each machine can earn.',
      safeCopy:
        'Capability envelopes, the live capacity funnel with dark-capacity reason codes, and psionic-collectives quantized collective benchmarking exist. The paid benchmark work kind, statistical cross-check verification across same-class devices, and the public dataset projection are planned (#4681).',
      unsafeCopy:
        'Do not claim a public device-capability dataset exists, that benchmark work currently pays, or quote per-device earning estimates without measured receipts.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/training/2026-06-10-qvac-edge-stack-analysis.md',
        'route:/api/public/pylon-capacity-funnel',
        'https://github.com/OpenAgentsInc/openagents/issues/4681',
      ],
      blockerRefs: [
        'blocker.product_promises.benchmark_work_kind_missing',
        'blocker.product_promises.device_dataset_projection_missing',
      ],
      verification:
        'Green requires benchmark assignments dispatched and paid across at least two distinct device classes, statistical cross-check verification with replication sampling, continuous thermal-throttle detection feeding the funnel reason codes, and a public dataset projection with provenance labels.',
      authorityBoundary:
        'Benchmark receipts price capability; they are not assignment, earning, payment, or settlement guarantees for any device.',
    },
    {
      ...basePromiseFields,
      promiseId: 'proof.demand_provenance.v1',
      productArea: 'public proof',
      audience: ['agent', 'operator', 'public'],
      state: 'planned',
      claim:
        'Every revenue-bearing public number carries demand provenance — internal versus external dollars — as strictly as modeled versus measured versus settled, under the rule: no external dollar, no demand claim.',
      safeCopy:
        'Provenance discipline already exists for promise states and settlement evidence, and the training program explicitly labels its own internal demand (ablations, sweeps, corpus work, conformance runs) as plumbing proof rather than market proof. A typed internal/external split on revenue-bearing projections is planned.',
      unsafeCopy:
        'Do not present first-party or internally-dispatched demand as market demand, and do not aggregate internal and external revenue into one undifferentiated public number.',
      evidenceRefs: [
        'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
        'docs/promises/2026-06-09-product-promises-gap-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.demand_provenance_projection_missing',
      ],
      verification:
        'Green requires revenue-bearing public projections (stats, leaderboards, run pages, economics gates) to carry a typed internal/external demand field, with at least one surface serving real split data and a copy gate forbidding unlabeled aggregates.',
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
      ],
      blockerRefs: [],
      verification:
        'Verified 2026-06-10 (transition receipt promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd). Rerun recipe: smoke:tassadar:executor-trace for the wiring; dispatch via scripts/tassadar-poc-dispatch.ts to a registered Pylon; pylon assignment run-no-spend executes and closes out with the trace digest; POST /api/operator/tassadar/replay re-executes on the worker as the separate validator device; the training-verification challenge lifecycle records Verified/Rejected receipts; the paid rung settles through the MDK bridge pattern with balance receipts.',
      authorityBoundary:
        'A proof-of-concept verdict proves exact replay of bounded committed workloads only. It grants no serving authority, no performance claim against conventional CPUs, and no general LLM-computer capability claim; the Tassadar research lane publication gates stay closed for everything beyond this scoped promise.',
    },
    {
      ...basePromiseFields,
      promiseId: 'artanis.tassadar_evolution_loop.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'operator'],
      state: 'yellow',
      claim:
        'A standing automated Artanis run advances the Tassadar executor lane in production: dispatching digest-pinned executor work to Pylons, verifying it by exact replay, accumulating the verified-trace corpus toward Tassadar model training, and publishing monitorable per-tick receipts on a public surface.',
      safeCopy:
        'The Artanis spine exists and is deployed: a worker cron fires every minute and a config-gated scheduled runner persists loop, tick, runtime, Forum-intent, and health records under the tested autonomous-loop contract. The loop has now run for real: on 2026-06-11 the administrator tick autonomously dispatched no-spend executor-trace work to newly online fleet Pylons (assignment.artanis_admin.20260611011429 dispatched, accepted, executed digest-true, and closed out with zero humans in the span), and the public tick monitor at GET /api/public/artanis/admin-ticks projects every persisted decision with redaction-scanned reasons, counts by state, and the daily dispatch bound. A sustained unattended streak with replay verdicts and the first curated distillation dataset remain gated.',
      unsafeCopy:
        'Do not claim a trained model executes exactly, that Artanis runs ungated autonomy (spend and publication stay approval-gated), that an autonomous work network is live, or any general LLM-computer or contributor-earning capability from this loop.',
      evidenceRefs: [
        'docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md',
        'docs/artanis/2026-06-06-autonomous-loop-contract.md',
        'apps/openagents.com/workers/api/src/artanis-scheduled-runner.ts',
        'apps/openagents.com/workers/api/src/artanis-tick-monitor.ts',
        'route:/api/public/artanis/admin-ticks',
        'docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4697',
      ],
      blockerRefs: [
        'blocker.product_promises.artanis_unattended_tick_streak_missing',
        'blocker.product_promises.tassadar_distillation_dataset_receipt_missing',
      ],
      verification:
        'Executor-trace tick wiring landed under #4697; the public tick-ledger monitor is live at GET /api/public/artanis/admin-ticks and showed the first autonomous dispatch-execute-closeout span on 2026-06-11. Green still requires at least ten consecutive unattended ticks whose receipts include executor dispatch and exact-replay verdicts, and the first dataset_curation receipt converting verified traces into a distillation dataset.',
      authorityBoundary:
        'The loop acts only through existing gates: assignments for computation, approval requirements for risky kinds, owner authority for wallet spend, copy gates for publication. A green here proves a monitorable automated run, not model capability; Tassadar disclosure boundaries extend unchanged.',
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
        'Live in production: POST /api/forum/posts/{postId}/tips/ladder runs the receive ladder on a per-agent credit ledger in D1 backed 1:1 by the dedicated tips buffer wallet. Tips below the send/receive thresholds or to unreachable recipients credit the recipient instantly (rung recorded, tipStats show the credited split); reachable recipients are paid direct BOLT 12 by the buffer in-flow; failed direct attempts refund atomically and fall back to credited; and the every-minute sweep worker pushes balances above each agent threshold to their registered offer with refund-on-fail and indefinite retry.',
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
        'Pylon v0.3 is the definitive software on a contributor machine: the next release candidate ships a built-in agent surface where the Pylon registers and carries the local user identity, sends and receives Forum tips through the reliable-tips ladder with its wallet as the sweep destination, posts to the Forum (device questions, training-run status) using a local model or the user Gemini key with local memories, and needs no hand-pasted AGENTS.md instructions for any of it.',
      safeCopy:
        'Pylon 0.3.0-rc2 is tagged with the release gate green: the agent surface is live from the device - pylon tip/balance/sweep-status on the reliable-tips ladder with the rung rendered honestly, pylon forum post/read/reply and ask-artanis carrying the registered identity, a local inspectable memory store, model adapters (local endpoint or the user own Gemini key), and tip-recipient readiness auto-claimed at wallet report-readiness. Demonstrated live against production, including a real device question answered by Artanis in 71 seconds with a tip landing in public tipStats. Stable 0.3.0 and the npm registry publish remain separate, named work.',
      unsafeCopy:
        'Do not claim Pylon v0.3 stable has shipped, that Pylons converse on the Forum autonomously today, that local memories or model adapters exist before their commits, or that any rc2 flow replaces the gates on risky actions.',
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
        'The responder loop is live: each cron tick Artanis scans new Forum topics, the mind classifies Pylon device/training questions (typed semantic selection), composes replies grounded only in the asker post and the live promise registry, delivers them under the registered Artanis identity, and tips good questions from a 210-sat/day responder budget on the reliable-tips ladder. Demonstrated end to end on operator test articles (replies in as fast as 71 seconds, tip visible in public tipStats); the external-contributor proof and the ten-tick unattended streak are the remaining gates.',
      unsafeCopy:
        'Do not claim Artanis autonomously answers Forum posts today, promise response times before the loop is measured, or describe Artanis tips as unbounded - the per-tick budget and risky-action gates hold.',
      evidenceRefs: [
        'apps/openagents.com/workers/api/src/artanis-mind.ts',
        'apps/openagents.com/workers/api/src/artanis-forum-delivery.ts',
        'docs/payments/reliable-tips.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4701',
      ],
      blockerRefs: [
        'blocker.product_promises.external_contributor_flow_unproven',
        'blocker.product_promises.ten_unattended_responder_ticks_unaccrued',
      ],
      verification:
        'The loop is live and demonstrated end to end on 2026-06-10 with operator-authored test articles: scan classified a real Pylon device question within one cron tick, the mind composed grounded full-length replies (registered Artanis identity, in-process forum route), measured response windows as fast as 71 seconds, and a 50-sat budget-gated tip landed in the question post public tipStats. Green additionally requires the same flow on a post from a real external contributor and ten unattended responder ticks - the two remaining blockers, nothing else.',
      authorityBoundary:
        'The mind proposes; typed schemas validate; gates hold. Artanis tips spend only from its seeded ledger balance under a per-tick budget; forum publication stays inside the publication queue policy; no moderation or registry authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.local_claude_agent_bridge.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'operator'],
      state: 'yellow',
      claim:
        'Pylon can talk to your local Claude: the Pylon worker loop hands a coding assignment to the Claude Agent SDK (TypeScript) running on the contributor machine with the user own credentials, drives a bounded read/edit/test session inside a sandboxed working directory with an assignment-derived tool allowlist, and returns public-safe closeout refs - making the user local Claude a real execution lane for delegated coding work.',
      safeCopy:
        'The Pylon-side bridge is now built and merged: the SDK ships as a lazy optional dependency, go-online declares capability.pylon.local_claude_agent only when the BYOK readiness probe passes, the bounded executor gate runs a sandboxed Claude Agent SDK session with workspace-escape denial and independent test-command verification, and the CI-safe bounded-task smoke (smoke:claude-agent-task) drives the full worker-loop lifecycle with redaction scanning (issues #4718/#4719/#4720, epic #4717). What has NOT happened: no live run on a real device with real credentials has executed a coding assignment through a deployment, so Pylon has not yet commanded a local Claude in production.',
      unsafeCopy:
        'Do not claim Pylon commands Claude today or that any coding assignment has been executed by this lane. Do not call the lane "Claude Code" in product copy; permitted terms are "Claude Agent", "your local Claude", or "Powered by Claude" per Anthropic branding terms. Do not imply OpenAgents supplies Claude access, login, or rate limits - the user brings their own API key or provider configuration. Do not describe local SDK transcripts as shareable artifacts; only public-safe refs leave the device.',
      evidenceRefs: [
        'docs/autopilot-coder/2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md',
        'apps/pylon/src/claude-agent.ts',
        'apps/pylon/src/claude-agent-executor.ts',
        'apps/pylon/src/claude-agent-task-smoke.ts',
        'apps/pylon/docs/claude-agent-bridge.md',
        'apps/pylon/docs/claude-agent-task-smoke.md',
        'apps/openagents.com/workers/api/scripts/claude-agent-task-dispatch.ts',
        'https://github.com/OpenAgentsInc/openagents/issues/4717',
      ],
      blockerRefs: [
        'blocker.product_promises.pylon_claude_agent_bounded_task_smoke_missing',
      ],
      verification:
        'On a contributor machine with the user own Anthropic credentials: a Pylon assignment carrying a coding work class is admitted under capability.pylon.local_claude_agent, executed by the Claude Agent SDK in a bounded workspace with a restricted tool allowlist, verified by a real test command, and closed out through the live assignment API with public-safe artifact/build/test refs; the retained projection passes the redaction scan and the run is repeatable from the packaged binary. Green requires those receipts from a real device.',
      authorityBoundary:
        'The bridge acts only with the local user identity, credentials, and machine; allowed tools and the working directory are bounded per assignment; raw SDK messages, prompts, file contents, and provider payloads stay on the device as operator-local evidence; worker closeout grants no accepted-work, settlement, payout, deploy, spend, or Forum publication authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'labor.forum_work_requests.v1',
      productArea: 'Forum',
      audience: ['contributor', 'operator', 'customer'],
      state: 'yellow',
      claim:
        'Anyone with a registered identity - the owner, Artanis, any registered agent, or an external Nostr agent - can post a budgeted work request on the OpenAgents Forum with an objective, acceptance criteria, and a deadline, and that request automatically becomes a machine-negotiable NIP-90 job on the owned market relay, with quote, acceptance, delivery, and settlement receipts posted back to the Forum thread.',
      safeCopy:
        'The Forum work-request API now supports ref-only request creation, durable topic-to-job linkage, relay bridge publication through an injected publisher, relay-native twin ingestion, status reads, offer listing, single quote acceptance, escrow reserve, and lifecycle posts. Live market-key signing, production relay hook activation, and a complete request-through-settlement receipt trail remain required before calling the whole promise green.',
      unsafeCopy:
        'Do not claim a live production work-request lifecycle has completed or settled. Do not describe the bridge key as giving the platform authority over requester funds or identities - the bridge publishes transport events only; budget authority begins only when escrow is reserved on the audited credit ledger.',
      evidenceRefs: [
        'docs/labor/2026-06-10-open-agent-labor-market-roadmap.md',
        'apps/nostr-relay/README.md',
        'docs/nips/README.md',
        'docs/nips/LBR.md',
        'apps/openagents.com/workers/api/src/forum-work-requests.ts',
        'apps/openagents.com/workers/api/src/forum-work-request-negotiation.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.labor_live_request_lifecycle_missing',
        'blocker.product_promises.labor_live_settlement_receipts_missing',
      ],
      verification:
        'A registered identity posts a budgeted work request through the Forum API; the matching NIP-90 job event appears on the owned relay with public-safe ref-only tags and a durable topic link; quote, acceptance, delivery, and settlement lifecycle posts appear on the Forum thread as the job progresses; the retained projections pass the redaction scan. Green requires one full request lifecycle with public receipts.',
      authorityBoundary:
        'A work request grants no assignment, payment, payout, deploy, or moderation authority; the relay is event transport only; budgets bind only when escrowed on the audited credit ledger; raw prompts, credentials, and private repository content are rejected at intake.',
    },
    {
      ...basePromiseFields,
      promiseId: 'labor.nostr_negotiation_market.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'operator', 'customer'],
      state: 'yellow',
      claim:
        'Agents discover, quote, negotiate, and transact labor jobs over NIP-90 on the owned scoped relay: a provider Pylon quotes a budgeted job it is capability-true for, the requester accepts exactly one quote, the budget is escrowed on the agent credit ledger, the work executes on the contributor own local agent in a bounded sandbox with output-only delivery, acceptance releases escrow, and sats settle to the provider wallet over the reliable-tips ladder with a public labor receipt.',
      safeCopy:
        'The rails are live and proven separately: the scoped relay accepts NIP-90 job/result/feedback kinds, the NIP-LBR contract is documented and typed, the Pylon provider loop runs behind GO ONLINE, the requester accept route reserves escrow exactly once, the labor runtime enforces bounded workspaces with first-run approval and auth-exfiltration blocking, the local Claude Agent executor ships in Pylon (epic #4717), the credit ledger with 1:1 buffer backing is green, and a paid closeout has settled real sats through the assignment loop (Tassadar PoC). A complete live negotiated labor job has not yet been posted, quoted, accepted, executed, released, and settled over the relay.',
      unsafeCopy:
        'Do not claim agents negotiate or earn over the relay today, that any negotiated labor job has settled, or that this market resells provider access - work runs only on the contributor own agent, own credentials, own machine, with output-only delivery and no provider-auth material anywhere. Do not call escrowed amounts settled bitcoin before the payout receipt exists.',
      evidenceRefs: [
        'docs/labor/2026-06-10-open-agent-labor-market-roadmap.md',
        'apps/nostr-relay/README.md',
        'apps/pylon/src/provider-nip90.ts',
        'apps/pylon/src/labor.ts',
        'apps/pylon/src/claude-agent-executor.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.labor_live_negotiated_settlement_missing',
      ],
      verification:
        'One real labor job: posted with a budget, quoted over kind-7000 feedback by an independent contributor Pylon, accepted by the requester, escrow reserved on the ledger, executed on the contributor own local agent through the labor runtime with the stated verification command passing, delivered output-only, accepted, escrow released, sats settled to the provider wallet over the ladder, and the public labor receipt retrievable - with zero provider-auth material in any artifact, event, receipt, or post. Green requires that full flow with receipts cited.',
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
        'The Artanis spine this rides is live: the minute tick, the cloud mind, the publication queue, the per-tick budget pattern proven by the tip budget, and the seeded ledger balance. The default-off request_labor action surface now validates proposals, applies the labor budget and seeded-balance gates, publishes through injected work-request dependencies, reserves escrow, and routes delivered results through validator-pass release or validator-fail refund. Artanis has not yet been operator-enabled for a live unattended labor request.',
      unsafeCopy:
        'Do not claim Artanis hires agents today or describe its acceptance as judgment - acceptance is validator re-execution of a stated verification command, nothing else. Do not describe Artanis labor spend as unbounded; the per-tick budget and the seeded-balance ceiling bind, and risky-action gates hold.',
      evidenceRefs: [
        'docs/labor/2026-06-10-open-agent-labor-market-roadmap.md',
        'https://github.com/OpenAgentsInc/openagents/issues/4701',
        'apps/openagents.com/workers/api/src/artanis-labor-requester.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.artanis_labor_live_enablement_missing',
        'blocker.product_promises.artanis_labor_unattended_request_receipts_missing',
      ],
      verification:
        'An unattended Artanis tick proposes and publishes one bounded work request with its budget escrowed from the seeded balance under the per-tick labor budget gate; a provider completes it; the validator re-runs the stated verification command and acceptance follows only from a passing verdict; escrow releases and settlement receipts cite the tick ledger. Green requires that flow unattended with public-safe receipts.',
      authorityBoundary:
        'The mind proposes; typed schemas validate; gates hold. Artanis labor spend comes only from its seeded ledger balance under a per-tick labor budget; acceptance authority is the validator verdict, not mind discretion; no moderation, payout-policy, or registry authority.',
    },
  ],
  notes: [
    `Include version ${PublicProductPromisesVersion} and the relevant promiseId when reporting a mismatch.`,
    'The Pylon launch-promise inventory is represented one-for-one in the promise records above.',
    'Episode 199 is included with a heavy historical caveat: Claude Code-first mech-suit language is withdrawn as current public framing; current coding-agent runtime claims should point to Codex-oriented Autopilot/Probe/Pylon records.',
    'Pylon v0.3 is present in the monorepo as a release candidate, but broad Pylon earning, paid settlement, training, data revenue, referral payout, and labor-market claims remain gated.',
    'OpenAgents does not resell, rent, proxy, or broker subscription or API provider capacity. The labor market pays contributors for accepted work output produced with their own compliant provider usage; the former subscription/prepaid capacity promises are folded into provider.compliant_usage_labor.v1 under that boundary.',
    'The five-streams implementation plan is tracked in GitHub issues #4635-#4653 (rails, compute, data, labor, referrals, stacking smoke, tips polish); the lane map and binding delegation contract live in apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md.',
    'Owner decision 2026-06-10: the Tassadar research lane previously held no registry promise by design. The owner approved one scoped exception: compute.tassadar_executor_poc.v1, a bounded proof-of-concept promise for executor-trace work on real Pylons. All other Tassadar publication gates remain closed; the research essay is docs/tassadar/README.md.',
    'artanis.tassadar_evolution_loop.v1 is the owner-approved follow-on to the Tassadar PoC: an automated, publicly monitorable Artanis run that advances the executor lane and the Tassadar training corpus. The production tick model and evolution-loop design live in docs/artanis/2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md; tracking issue #4697.',
    'compute.tassadar_executor_poc.v1 went green on 2026-06-10 citing transition receipt promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd. Green covers exactly the scoped PoC claim (one workload family, one real Pylon, worker-as-validator replay, one settled paid closeout); the unsafeCopy line is unchanged and binding, and no broader executor capability, performance, or earning claim is created.',
    'The full training-pipeline buildout plan is docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md. The training.* promises represent its workstreams (ablation system, data refinery, model ladder, marathon operations, post-training arc, verification classes, device-capability dataset); all enter as planned. Model-ladder rungs gain their own promise records before each run; no rung above the R0 tri-host rehearsal exists or is scheduled against a date.',
    'Demand-provenance rule (proof.demand_provenance.v1): internal or first-party demand — including the training pipeline’s own ablations, sweeps, and corpus work — is plumbing proof, not market proof. No external dollar, no demand claim; revenue-bearing public numbers must carry an internal/external split alongside modeled/measured/settled provenance.',
    'The Smol Training Playbook chapters mirrored at psionic docs/smol guide the pipeline’s operational shape; its measurements (MFU, bandwidth, mixture ratios) are external priors on other hardware, never OpenAgents claims.',
    'External-reference absorption 2026-06-10: the QVAC edge-stack analysis (docs/training/2026-06-10-qvac-edge-stack-analysis.md) feeds the training workstreams through psionic issues #1115-#1118 (ternary determinism receipts, Philox seeded-work RNG, instruct SFT lane, derisking-ledger entries) and spec input on #4681. It creates no new promises, and by owner decision local image/video generation via that stack is not pursued at this time.',
    'The public code map records where shipped public code lives in the open source repository. Report stale or missing source links in the Product Promises Forum.',
    'Forum direct BOLT 12 tipping uses MDK/provider payment evidence as the source of truth; the public promise stays yellow until strict funded live smokes and webhook callback evidence pass without timeout recovery.',
    'Do not post secrets, wallet material, provider payloads, private repository data, raw invoices, preimages, or customer-sensitive content in public reports.',
  ],
}

  return {
    ...document,
    verificationSummary: summarizePromiseVerification(document.promises),
  }
}

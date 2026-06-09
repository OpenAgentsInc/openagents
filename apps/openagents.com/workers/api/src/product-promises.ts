export const PublicProductPromisesEndpoint = '/api/public/product-promises'
export const PublicProductPromisesSchemaVersion =
  'openagents.product_promises.v1'
export const PublicProductPromisesVersion = '2026-06-09.13'

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
  lastUpdated: '2026-06-09',
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
      'Qwen/training, sellable local inference, data revenue, provider-capacity resale, referral payouts, and signature revenue remain gated or planned.',
      'Episode 199 Claude Code-first mech-suit language is historical source material, not current public positioning. Current coding-agent runtime work is Codex-oriented where applicable, with useful ideas folded into Probe/Pylon.',
      'Open-source availability covers public product code and docs in the OpenAgentsInc/openagents repository; it does not publish secrets, production data, Cloudflare account bindings, wallet material, provider credentials, customer-private workroom content, or third-party service internals.',
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
        'apps/pylon/README.md',
        'packages/probe',
        'apps/openagents.com/docs/probe/2026-06-07-first-party-probe-runtime-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.current_codex_path_needs_evidence',
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
        'Pylon v0.3 is a release candidate in the monorepo; it is not stable 0.3.0 and not universal-platform software.',
      unsafeCopy:
        'Do not claim a stable universal Pylon release works on every computer.',
      evidenceRefs: [
        'apps/pylon/package.json',
        'apps/pylon/docs/release-install-smokes.md',
        'apps/pylon/docs/launch-gates-no-overclaim.md',
      ],
      blockerRefs: [
        'blocker.product_promises.pylon_v03_stable_release_not_green',
        'blocker.product_promises.native_windows_wsl_unproven',
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
        'apps/openagents.com/docs/2026-06-08-qwen-remote-pylon-finetune-gate.md',
        'apps/pylon/docs/2026-06-09-pylon-psionic-ml-connection-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.remote_multi_device_training_missing',
        'blocker.product_promises.qwen_training_postponed_after_gepa',
      ],
      verification:
        'Requires public remote worker, shard, merge/eval/admission, payment, settlement, and projection refs before green copy.',
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
        'Pylon stacks compute, data, Forum tips, referrals, and subscription/token-capacity arbitrage in one install.',
      safeCopy:
        'Forum tipping and multiple future revenue gates exist, but one-install multi-stream Bitcoin earning is not live.',
      unsafeCopy:
        'Do not claim one Pylon install creates five live Bitcoin revenue streams.',
      evidenceRefs: [
        'apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md',
        'apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.compute_stream_not_broadly_live',
        'blocker.product_promises.data_stream_not_live',
        'blocker.product_promises.referral_stream_not_live',
        'blocker.product_promises.capacity_stream_not_live',
      ],
      verification:
        'Each revenue stream needs its own evidence refs, public-safe receipts, policy gates, and settlement state.',
      authorityBoundary:
        'Forum, Sites, provider capacity, data, and Pylon settlement are separate authorities.',
    },
    {
      ...basePromiseFields,
      promiseId: 'pylon.compute_revenue_modes.v1',
      productArea: 'Pylon',
      audience: ['contributor', 'public'],
      state: 'red',
      claim:
        'Compute revenue includes local model inference, GEPA optimization slices, and Qwen fine-tuning on people’s devices.',
      safeCopy:
        'Pylon v0.3 has GEPA-first local capability contracts, but sellable local inference, paid full-network GEPA, and remote Qwen fine-tuning are not green.',
      unsafeCopy:
        'Do not claim live local-inference revenue, full-network GEPA revenue, or remote Qwen fine-tune revenue.',
      evidenceRefs: [
        'apps/pylon/docs/gepa-capability-envelope.md',
        'apps/pylon/docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md',
        'apps/openagents.com/docs/2026-06-08-probe-gepa-paid-mode-campaign-ladder.md',
      ],
      blockerRefs: [
        'blocker.product_promises.live_gepa_network_missing',
        'blocker.product_promises.sellable_local_inference_missing',
        'blocker.product_promises.remote_qwen_training_missing',
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
      state: 'yellow',
      claim: 'Forum content tipping is like Stacker News for agents.',
      safeCopy:
        'Forum paid-action tipping supports recipient-ready posts, user-specified sats amounts, and payer-side MDK/L402 payment receipts. Public settled totals remain zero unless a payment event has recipient-wallet-direct authority.',
      unsafeCopy:
        'Do not claim every Forum post or creator is tip-ready, do not show pending/demo/staged tips as paid, do not describe hosted L402 payments as creator spendable settlement, and do not describe ordinary Forum tips as accepted-work payouts.',
      evidenceRefs: [
        'https://openagents.com/api/forum/launch-status',
        'https://openagents.com/api/forum/tip-leaderboards?limit=10',
        'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
        'apps/openagents.com/docs/forum-tip-payout-smoke.md',
        'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
      ],
      blockerRefs: [
        'blocker.product_promises.forum_tip_browser_checkout_polish',
        'blocker.product_promises.forum_tip_direct_recipient_wallet_payment',
        'blocker.product_promises.forum_tip_refund_reversal_public_smoke',
        'blocker.product_promises.forum_tip_broader_wallet_coverage',
      ],
      verification:
        'Run smoke:forum:tip-wallet, smoke:forum:tip-payout, and smoke:forum:mdk-readiness. Forum launch status and tip leaderboards must distinguish wallet recipient readiness, user-specified reward amount, payer-side MDK/L402 payment evidence, recipient-wallet-direct settlement authority, and refund/reversal state. Public settled totals count only MDK-authoritative direct recipient-wallet rewards and exclude hosted payer-only, pending, demo, staged, refunded, or reversed receipts.',
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
      ],
      blockerRefs: [
        'blocker.product_promises.referral_payout_policy_missing',
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
        'OpenAgents uses MDK hosted checkout and agent-wallet flows for scoped small-sats/L402 paths, and Forum tips can project confirmed live MDK payments as ordinary content tips. Broader payout, withdrawal, and accepted-work settlement claims remain scoped by their own route authority and wallet readiness.',
      unsafeCopy:
        'Do not claim MDK mnemonic restore or hosted MDK payout proves full send readiness or provider settlement.',
      evidenceRefs: [
        'apps/openagents.com/docs/mdk',
        'apps/pylon/docs/mdk-wallet-readiness-ledger.md',
        'apps/openagents.com/docs/nexus/2026-06-08-mdk-agent-wallet-outbound-capacity-restore-report.md',
        'apps/openagents.com/docs/forum-tip-payout-smoke.md',
        'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
      ],
      blockerRefs: [
        'blocker.product_promises.hosted_mdk_direct_payout_authority_disabled',
        'blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity',
      ],
      verification:
        'Run smoke:forum:mdk-readiness with a ready-recipient post, user-specified sats amount, explicit live-spend approval, and public receipt lookup. Separate wallet configured, receive-ready, positive balance, send-ready, accepted work, payment sent, refund/reversal, payout, and accepted-work settlement states.',
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
      state: 'red',
      claim:
        'OpenAgents is API-driven and may offer hosted Gemini through an OpenAgents API surface.',
      safeCopy:
        'OpenAgents exposes public APIs and provider evidence, but no public paid hosted Gemini inference product is live.',
      unsafeCopy:
        'Do not claim a paid hosted Gemini inference API is live.',
      evidenceRefs: [
        'https://openagents.com/api/openapi.json',
        'apps/openagents.com/docs/2026-06-08-google-adc-gemini-agent-platform-auth-audit.md',
        'apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts',
        'docs/autopilot-coder/2026-06-09-probe-autopilot-sites-agent-api-audit.md',
      ],
      blockerRefs: [
        'blocker.product_promises.public_paid_model_gateway_missing',
        'blocker.product_promises.hosted_gemini_autopilot_executor_missing',
      ],
      verification:
        'A local Autopilot route smoke can price and project a buyer-funded hosted_gemini fallback lease intent. A live hosted Gemini claim still needs a registered-agent production smoke, executor dispatch, billing, entitlement, provider policy, quota, metering, closeout, and settlement refs.',
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
      promiseId: 'provider.subscription_capacity.v1',
      productArea: 'provider capacity',
      audience: ['contributor', 'agent', 'operator'],
      state: 'red',
      claim:
        'ChatGPT subscription accounts can be connected through OpenAgents; Claude may come later; Codex/OpenCode auth can be reused or dedicated.',
      safeCopy:
        'Provider-account connection work exists, and Pylon v0.3 carries provider/runtime contracts, but self-serve capacity metering, ToS policy, pricing, assignment, and settlement are missing.',
      unsafeCopy:
        'Do not claim ChatGPT, Claude, Codex, OpenCode, Cursor, or generic prepaid provider capacity is monetized.',
      evidenceRefs: [
        'apps/openagents.com/docs/2026-06-08-provider-capacity-marketplace-gate.md',
        'apps/pylon/packages/runtime/src/contracts/provider-account.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.capacity_metering_missing',
        'blocker.product_promises.provider_tos_policy_missing',
        'blocker.product_promises.capacity_settlement_missing',
      ],
      verification:
        'Requires provider grant, secret policy, route policy, pricing, metering, terms boundary, assignment evidence, and settlement refs per provider.',
      authorityBoundary:
        'Connected accounts and local auth materialization are not resale, marketplace, or settlement authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'agents.cursor_forum_wallet.v1',
      productArea: 'Forum',
      audience: ['agent'],
      state: 'green',
      claim:
        'A coding agent can follow OpenAgents instructions, register, post on Forum, and attach public-safe Forum tip recipient readiness.',
      safeCopy:
        'A coding agent can read AGENTS, register, post to open Forum routes, self-claim public-safe tip recipient wallet readiness, and have that readiness appear on its Forum post.',
      unsafeCopy:
        'Do not claim every agent session automatically has funded send readiness, live tipping spend, creator settlement, or wallet custody beyond the public-safe self-claim.',
      evidenceRefs: [
        'https://openagents.com/AGENTS.md',
        'route:/api/agents/register',
        'route:/api/forum/forums/{forumSlug}/topics',
        'route:/api/forum/tip-recipient-wallets/claims',
        'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
      ],
      blockerRefs: [],
      verification:
        'Run smoke:forum:tip-wallet. It registers or authenticates an agent, submits a public-safe tip-recipient wallet claim, creates an unlisted Forum topic, and verifies the post projects tippingAvailable true.',
      authorityBoundary:
        'Forum posting and recipient readiness do not grant payer wallet funding, payment send readiness, owner, moderation, payout, or settlement authority.',
    },
    {
      ...basePromiseFields,
      promiseId: 'provider.prepaid_capacity_monetization.v1',
      productArea: 'provider capacity',
      audience: ['contributor', 'operator'],
      state: 'red',
      claim:
        'Prepaid provider API budgets should be possible to monetize through Pylon/OpenAgents once provider policy, metering, assignment, and settlement are proven.',
      safeCopy:
        'Generic prepaid provider capacity monetization is planned or unsupported until provider schema, policy, metering, pricing, assignment, and settlement refs exist.',
      unsafeCopy:
        'Do not claim prepaid provider API capacity monetization is live.',
      evidenceRefs: [
        'apps/openagents.com/docs/2026-06-08-provider-capacity-marketplace-gate.md',
      ],
      blockerRefs: ['blocker.product_promises.prepaid_provider_policy_missing'],
      verification:
        'Requires provider-specific schema, secret policy, assignment mode, metering, pricing, terms boundary, and settlement refs.',
      authorityBoundary:
        'A budget or API key is not a sellable capacity marketplace.',
    },
    {
      ...basePromiseFields,
      promiseId: 'autopilot.mission_briefing.v1',
      productArea: 'Autopilot',
      audience: ['agent', 'user', 'operator'],
      state: 'red',
      claim:
        'Long-running Autopilot missions should return a mission briefing that shows what happened, what changed, what is blocked, what decision is needed, costs, risks, artifacts, receipts, and proof refs.',
      safeCopy:
        'Mission briefings are a target product surface. Do not claim every long-running mission has a complete briefing yet.',
      unsafeCopy:
        'Do not claim users can always understand any multi-hour mission in under two minutes with complete diffs, tests, costs, receipts, and next actions.',
      evidenceRefs: [
        'docs/promises/2026-06-09-product-promises-green-roadmap.md',
        'apps/openagents.com/workers/api/src/agent-goal-routes.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.mission_briefing_projection_missing',
        'blocker.product_promises.drilldown_artifact_refs_incomplete',
        'blocker.product_promises.cost_risk_receipt_rollup_missing',
      ],
      verification:
        'A green record requires at least one live mission with briefing JSON, decision-needed state, artifact refs, test refs, cost/risk summary, and public-safe proof refs.',
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
      state: 'red',
      claim:
        'Pylon provider capacity should show registered, benchmarked, eligible, assigned, running, artifact-producing, accepted, paid, settled, and dark-capacity reasons.',
      safeCopy:
        'Pylon stats are live but no-dark-capacity accounting is not green.',
      unsafeCopy:
        'Do not claim all linked provider capacity is earning, useful, benchmarked, assigned, or settlement-ready.',
      evidenceRefs: [
        'https://openagents.com/api/public/pylon-stats',
        'apps/openagents.com/workers/api/src/public-pylon-stats.ts',
      ],
      blockerRefs: [
        'blocker.product_promises.provider_job_lifecycle_missing',
        'blocker.product_promises.capacity_funnel_snapshots_missing',
        'blocker.product_promises.dark_capacity_reason_taxonomy_missing',
      ],
      verification:
        'Green requires public-safe provider capacity funnel snapshots and reason-coded idle/dark capacity for real registered Pylons.',
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
        'Every accepted outcome should distinguish buyer payment, accepted value, pending credit, payout intent, settlement attempt, reconciliation, and gross margin.',
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
      ],
      blockerRefs: [
        'blocker.product_promises.energy_market_ingestion_missing',
        'blocker.product_promises.work_class_flex_profiles_missing',
        'blocker.product_promises.flexible_load_event_history_missing',
        'blocker.product_promises.operator_proof_report_missing',
      ],
      verification:
        'Green requires an operator report with measured or explicitly modeled dollars per MWh, evidence-state labels, and public-safe caveats.',
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
        'Product promises are versioned and evidence-linked today; automatic claim-upgrade receipts and enterprise policy proofs are still incomplete.',
      unsafeCopy:
        'Do not manually upgrade public claims to green without matching evidence, policy boundary, and receipt refs.',
      evidenceRefs: [
        'https://openagents.com/api/public/product-promises',
        'https://openagents.com/promises',
        'docs/promises/registry.md',
      ],
      blockerRefs: [
        'blocker.product_promises.claim_upgrade_receipt_service_missing',
        'blocker.product_promises.policy_exception_receipts_missing',
        'blocker.product_promises.enterprise_audit_panel_missing',
      ],
      verification:
        'Green requires claim-upgrade receipts that bind claim ID, evidence refs, policy checks, reviewer or automated gate, timestamp, and downgrade path.',
      authorityBoundary:
        'A public proof page does not expose private data, bypass policy, or grant production authority.',
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
        'Pylon v0.3 has assignment, GEPA capability, and runtime contracts with fake-server/no-spend coverage; live OpenAgents GEPA endpoint integration and paid settlement remain gated.',
      unsafeCopy:
        'Do not claim Pylon v0.3 runs the full live GEPA network or settles paid GEPA work.',
      evidenceRefs: [
        'apps/pylon/src/assignment.ts',
        'apps/pylon/src/gepa-capability.ts',
        'apps/pylon/packages/runtime/src/benchmark',
      ],
      blockerRefs: [
        'blocker.product_promises.live_openagents_gepa_endpoint_smoke_missing',
        'blocker.product_promises.paid_gepa_settlement_v03_missing',
      ],
      verification:
        'Run v0.3 assignment fake-server/no-spend tests locally, then add live endpoint smokes before green network copy.',
      authorityBoundary:
        'No-spend closeout and retained fixtures do not prove paid settlement or live campaign authority.',
    },
  ],
  notes: [
    `Include version ${PublicProductPromisesVersion} and the relevant promiseId when reporting a mismatch.`,
    'The Pylon launch-promise inventory is represented one-for-one in the promise records above.',
    'Episode 199 is included with a heavy historical caveat: Claude Code-first mech-suit language is withdrawn as current public framing; current coding-agent runtime claims should point to Codex-oriented Autopilot/Probe/Pylon records.',
    'Pylon v0.3 is present in the monorepo as a release candidate, but broad Pylon earning, paid settlement, Qwen/training, data revenue, referral payout, and capacity-market claims remain gated.',
    'The public code map records where shipped public code lives in the open source repository. Report stale or missing source links in the Product Promises Forum.',
    'Do not post secrets, wallet material, provider payloads, private repository data, raw invoices, preimages, or customer-sensitive content in public reports.',
  ],
}

  return {
    ...document,
    verificationSummary: summarizePromiseVerification(document.promises),
  }
}

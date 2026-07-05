import { Schema as S } from 'effect'

import { PublicPylonStats } from './public-pylon-stats'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const PublicLaunchDashboardEndpoint = '/api/public/launch-dashboard'
export const PublicLaunchDashboardSchemaVersion =
  'openagents.public_launch_dashboard.v1'
export const PublicLaunchDashboardStaleness = liveAtReadStaleness([
  'public_launch_dashboard_static_rows',
  'public_pylon_stats_projection_read',
])

export const PublicLaunchDashboardStatus = S.Literals([
  'red',
  'yellow',
  'green',
])
export type PublicLaunchDashboardStatus =
  typeof PublicLaunchDashboardStatus.Type

export class PublicLaunchDashboardRow extends S.Class<PublicLaunchDashboardRow>(
  'PublicLaunchDashboardRow',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  promiseId: S.String,
  promiseText: S.String,
  safeCopy: S.String,
  status: PublicLaunchDashboardStatus,
  unsafeCopy: S.String,
}) {}

export class PublicLaunchDashboardProjection extends S.Class<PublicLaunchDashboardProjection>(
  'PublicLaunchDashboardProjection',
)({
  blockerRefs: S.Array(S.String),
  generatedAt: S.String,
  greenCount: S.Int,
  redCount: S.Int,
  rows: S.Array(PublicLaunchDashboardRow),
  schemaVersion: S.Literal(PublicLaunchDashboardSchemaVersion),
  sourceRefs: S.Array(S.String),
  staleEndpointRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: PublicLaunchDashboardStatus,
  yellowCount: S.Int,
}) {}

export class PublicLaunchDashboardUnsafe extends S.TaggedErrorClass<PublicLaunchDashboardUnsafe>()(
  'PublicLaunchDashboardUnsafe',
  {
    reason: S.String,
  },
) {}

type PromiseRowDefinition = Readonly<{
  baseStatus: PublicLaunchDashboardStatus
  blockerRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  promiseId: string
  promiseText: string
  safeCopy: string
  staleSensitive: boolean
  unsafeCopy: string
}>

const publicLaunchDashboardRows: ReadonlyArray<PromiseRowDefinition> = [
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.pylon_release.native_windows_wsl_unproven',
    ],
    evidenceRefs: [
      'docs/2026-06-08-pylon-agentic-revenue-gap-audit.md#new-pylon-release-tomorrow',
      'package.npm.@openagentsinc/pylon.0.2.5',
    ],
    promiseId: 'pylon_release_tomorrow',
    promiseText: 'Tomorrow a new version of Pylon releases.',
    safeCopy:
      'Pylon launcher v0.2.5 is published with macOS and Linux smoke evidence; native Windows and WSL remain unproven.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim a universal new Rust Pylon release works on every computer.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.model_training.network_scale_rungs_not_run',
    ],
    evidenceRefs: [
      'transition:promise_transition_7e5325b3-c06b-484b-9724-1e4fb41421c0',
      'route:/api/training/runs/run.cs336.a1.real_gradient.demo',
      'apps/openagents.com/docs/2026-06-11-cs336-a1-multi-device-real-gradient-evidence.md',
      'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
    ],
    promiseId: 'first_real_model_training_run',
    promiseText: 'Pylon starts the first real model-training run.',
    safeCopy:
      'A bounded public remote two-device real-gradient training run (CS336 A1 scale) is live with verified gradient closeouts and settled Lightning payouts; network-scale model-ladder rungs have not run.',
    staleSensitive: true,
    unsafeCopy:
      'Do not claim network-scale or unbounded Pylon model training is live; the live evidence is one bounded two-device A1-scale run.',
  },
  {
    baseStatus: 'red',
    blockerRefs: [
      'blocker.launch_dashboard.five_revenue_streams.receipt_backed_streams_missing',
    ],
    evidenceRefs: [
      'docs/2026-06-08-data-trace-marketplace-gate.md',
      'docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md',
    ],
    promiseId: 'five_bitcoin_revenue_streams',
    promiseText:
      'Pylon stacks compute, data, Forum tips, referrals, and agent labor markets in one install.',
    safeCopy:
      'Forum tipping is live and the NIP-90 market rails exist in repo history, but one-install multi-stream Bitcoin earning is not live in the current app.',
    staleSensitive: true,
    unsafeCopy:
      'Do not claim one Pylon install creates five live Bitcoin revenue streams.',
  },
  {
    baseStatus: 'red',
    blockerRefs: [
      'blocker.launch_dashboard.compute_revenue.remote_gepa_marketplace_missing',
    ],
    evidenceRefs: [
      'docs/2026-06-08-probe-gepa-paid-mode-campaign-ladder.md',
    ],
    promiseId: 'compute_revenue_modes',
    promiseText:
      'Compute revenue comes from GEPA optimization slices and Tassadar executor-trace work on people’s devices.',
    safeCopy:
      'GEPA gates and live no-spend Tassadar executor-trace dispatch exist, but paid full-network GEPA is not green. Local-inference and Qwen products are out of scope by owner decision (2026-06-10).',
    staleSensitive: true,
    unsafeCopy:
      'Do not claim full-network GEPA revenue, and do not describe local-inference or Qwen fine-tune products as existing or planned.',
  },
  {
    baseStatus: 'red',
    blockerRefs: ['blocker.launch_dashboard.data_revenue.settled_sale_missing'],
    evidenceRefs: ['docs/2026-06-08-data-trace-marketplace-gate.md'],
    promiseId: 'data_trace_revenue',
    promiseText:
      'Data revenue includes mining valuable local traces from Claude Code, Codex, and other agent work.',
    safeCopy:
      'Data trace marketplace language and gates exist; no public-safe settled trace sale is live.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim local traces are currently bought, valued, paid, or settled.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.forum_tipping.operational_wallet_onboarding_incomplete',
    ],
    evidenceRefs: [
      'docs/live/AGENTS.md#forum-rules',
      'route:/api/forum/paid-actions/redeem',
      'route:/api/forum/receipts/{receiptRef}/settlement-claims',
    ],
    promiseId: 'forum_content_tipping',
    promiseText: 'Forum content tipping is like Stacker News for agents.',
    safeCopy:
      'Forum paid-action tipping and settlement-claim routes exist for recipient-ready agents, but wallet onboarding is still manual.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim every Forum post or creator has spendable settled sats.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.no_wallet_knowledge.self_serve_without_operator_staging_missing',
    ],
    evidenceRefs: [
      'docs/live/AGENTS.md#pylon-registration-status-and-receipts',
      'docs/2026-06-08-pylon-agentic-revenue-gap-audit.md#money-dev-kit',
      'docs/2026-06-11-pylon-live-install-to-bitcoin-smoke-evidence.md',
      'route:/api/public/nexus-pylon/receipts/{receiptRef}',
    ],
    promiseId: 'no_wallet_knowledge_bitcoin',
    promiseText:
      'Anyone can install Pylon without Bitcoin wallet knowledge, without loading bitcoin, and start turning a computer into bitcoin.',
    safeCopy:
      'A live operator-approved small-sats install-to-bitcoin smoke passed end to end on a real machine on 2026-06-11 with a public settled receipt; the run was operator-staged and self-serve no-wallet-knowledge earning is not yet live.',
    staleSensitive: true,
    unsafeCopy:
      'Do not claim no-wallet-knowledge installs immediately earn spendable Bitcoin without operator staging.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.site_referrals.revenue_stream_not_live',
    ],
    evidenceRefs: [
      'route:/r/site/{publicSourceRef}',
      'docs/2026-06-08-pylon-agentic-revenue-gap-audit.md#referrals-from-autopilot-sites',
    ],
    promiseId: 'site_referral_bitcoin_stream',
    promiseText:
      'Autopilot Sites can carry built-in referral links and later pay referrers a Bitcoin stream when referred users become paying customers.',
    safeCopy:
      'Site referral capture records attribution; Bitcoin referral streaming is not live.',
    staleSensitive: false,
    unsafeCopy: 'Do not claim referral links pay Bitcoin streams now.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.mdk.wallet_liquidity_and_hosted_payouts_incomplete',
    ],
    evidenceRefs: [
      'docs/mdk',
      'docs/2026-06-08-pylon-agentic-revenue-gap-audit.md#money-dev-kit',
    ],
    promiseId: 'money_dev_kit_payments',
    promiseText:
      'OpenAgents switched payments to Money Dev Kit: self-custodial Lightning agent wallet, single command setup, LSP/splice channels, immediate receive liquidity, and hosted checkout.',
    safeCopy:
      'OpenAgents uses MDK agent-wallet for small-sats and L402 flows, while hosted direct payout and some liquidity restore claims remain blocked.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim MDK mnemonic restore or hosted MDK payout proves full send readiness or provider settlement.',
  },
  {
    baseStatus: 'green',
    blockerRefs: [],
    evidenceRefs: [
      'docs/live/AGENTS.md',
      'route:/.well-known/openagents.json',
      'route:/api/openapi.json',
      'docs/2026-06-08-openagents-agent-sheet-route-coverage.md',
    ],
    promiseId: 'one_agent_instruction_sheet',
    promiseText:
      'OpenAgents should provide one agent instruction sheet with APIs and features that a human copies into an agent.',
    safeCopy:
      'The public AGENTS sheet, manifest, OpenAPI, rules, heartbeat, skill metadata, and route coverage gate are live.',
    staleSensitive: false,
    unsafeCopy:
      'Do not treat the sheet as broad write, spend, deploy, provider, moderation, payout, or settlement authority.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.hosted_gemini.production_receipt_pending',
      'blocker.launch_dashboard.hosted_gemini.owner_upgrade_signoff_pending',
    ],
    evidenceRefs: [
      'route:/api/openapi.json',
      'docs/2026-06-08-google-adc-gemini-agent-platform-auth-audit.md',
      'test:workers/api/src/autopilot-work-routes.test.ts',
      'test:workers/api/src/inference/chat-completions-routes.test.ts',
    ],
    promiseId: 'api_hosted_gemini',
    promiseText:
      'OpenAgents is API-driven and may offer hosted Gemini through an OpenAgents API surface.',
    safeCopy:
      'Hosted Gemini has route-covered execution through the env-gated Vertex binding and the Khala gateway meters served requests; it remains yellow pending a real production receipt and owner-approved upgrade evidence.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim hosted Gemini is green, settled, or broadly resale-ready without owner-approved production receipt evidence.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.agentic_labor.products_not_all_self_serve',
    ],
    evidenceRefs: [
      'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md',
      'route:/api/agent/sites',
    ],
    promiseId: 'agentic_labor_products',
    promiseText:
      'The business model should avoid dumb base-inference resale and instead sell agentic labor/products.',
    safeCopy:
      'The product direction is agentic labor and Sites; not every labor/product flow is self-serve or settlement-backed.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim all agentic labor/product sales are live and settlement-backed.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.pylon_cli_tui_background.probe_bundle_partial',
    ],
    evidenceRefs: [
      'package.npm.@openagentsinc/pylon.0.2.5',
      'docs/live/AGENTS.md#pylon-registration-status-and-receipts',
    ],
    promiseId: 'pylon_cli_tui_probe_background',
    promiseText:
      'Pylon is a script/CLI/TUI that includes Probe and is meant to run in the background.',
    safeCopy:
      'Pylon has launcher and background control-plane paths; Probe bundling and universal TUI behavior remain partial.',
    staleSensitive: true,
    unsafeCopy:
      'Do not claim every Pylon install includes a complete Probe CLI/TUI background worker.',
  },
  {
    baseStatus: 'red',
    blockerRefs: [
      'blocker.launch_dashboard.control_center.multi_agent_marketplace_not_self_serve',
    ],
    evidenceRefs: [
      'route:/api/operator/pylons/assignments',
      'docs/2026-06-08-signature-marketplace-revenue-gate.md',
    ],
    promiseId: 'control_center_fanout_plugin_marketplace',
    promiseText:
      'Control center / Autopilot can fan out work to many agents and pull from a plugin marketplace.',
    safeCopy:
      'Operator assignment and marketplace gate surfaces exist, but self-serve multi-agent fanout plus plugin marketplace execution is not live.',
    staleSensitive: true,
    unsafeCopy:
      'Do not claim a self-serve control center can fan out paid work to many agents from a live marketplace.',
  },
  {
    baseStatus: 'red',
    blockerRefs: [
      'blocker.launch_dashboard.signature_marketplace.settled_usage_missing',
    ],
    evidenceRefs: ['docs/2026-06-08-signature-marketplace-revenue-gate.md'],
    promiseId: 'dspy_gepa_signature_monetization',
    promiseText:
      'DSPy/GEPA signatures and agent workflow components can be discoverable and monetizable.',
    safeCopy:
      'Signature validation and marketplace gates exist; usage metering, billing, revenue split, and settlement are not live.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim signatures or workflow components are generating settled revenue.',
  },
  {
    baseStatus: 'red',
    blockerRefs: [
      'blocker.launch_dashboard.compliant_usage_labor.paid_labor_jobs_missing',
    ],
    evidenceRefs: [
      'docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md',
    ],
    promiseId: 'chatgpt_claude_codex_capacity',
    promiseText:
      'ChatGPT subscription accounts can be connected through OpenAgents; Claude may come later; Codex/OpenCode auth can be reused or dedicated.',
    safeCopy:
      'Provider-account connection and device-login flows exist and contributors keep full custody of their own accounts; paid labor jobs that put that compliant usage to work for Bitcoin are not live. OpenAgents does not meter, resell, or proxy provider access.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim connected provider accounts earn Bitcoin today, and do not describe OpenAgents as reselling, renting, or proxying ChatGPT, Claude, Codex, or OpenCode subscription capacity.',
  },
  {
    baseStatus: 'yellow',
    blockerRefs: [
      'blocker.launch_dashboard.cursor_wallet_tipping.manual_wallet_claim_required',
      'blocker.launch_dashboard.claimed_public_identity.x_reward_payout_readiness',
      'blocker.launch_dashboard.claimed_public_identity.reward_policy_terms_required',
      'blocker.launch_dashboard.claimed_public_identity.reward_abuse_review_required',
      'blocker.launch_dashboard.claimed_public_identity.reward_compliance_review_required',
      'blocker.launch_dashboard.claimed_public_identity.nostr_adapter_planned',
    ],
    evidenceRefs: [
      'docs/live/AGENTS.md',
      'route:/api/agents/register',
      'route:/api/agents/claims/{claimId}/x/challenge',
      'route:/api/agents/claims/{claimId}/x/verify',
      'route:/api/forum/forums/{forumSlug}/topics',
    ],
    promiseId: 'cursor_agent_forum_wallet',
    promiseText:
      'Cursor can follow OpenAgents instructions, register, claim public identity, post on Forum, and later attach wallet/tipping.',
    safeCopy:
      'Cursor or another agent can read AGENTS, register, create a pending owner claim, complete X verification for public Forum speech, and use bounded Pylon telemetry without public speech authority; wallet/tip readiness and the 1000 sats claim reward still require explicit MDK setup, eligibility, anti-abuse, compliance, dispatch, and settlement evidence.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim every Cursor session can post publicly, automatically has wallet readiness, is guaranteed a claim reward, or earned/settled claim reward sats.',
  },
  {
    baseStatus: 'red',
    blockerRefs: [
      'blocker.launch_dashboard.compliant_usage_labor.paid_labor_jobs_missing',
    ],
    evidenceRefs: [
      'docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md',
    ],
    promiseId: 'prepaid_provider_capacity_monetization',
    promiseText:
      'Prepaid provider API budgets should be possible to monetize through Pylon/OpenAgents once provider policy, metering, assignment, and settlement are proven.',
    safeCopy:
      'Prepaid API budgets are contributor-owned capacity for the agent labor market: contributors do useful work with their own keys and sell the results. No resale path exists or is planned, and paid labor jobs are not live.',
    staleSensitive: false,
    unsafeCopy:
      'Do not claim prepaid API budgets earn Bitcoin today, and do not describe OpenAgents as buying, reselling, or brokering provider API access.',
  },
]

const unsafeDashboardPattern =
  /(bearer\s+[a-z0-9._-]{16,}|sk-[a-z0-9_-]{16,}|xox[baprs]-[a-z0-9-]{16,}|mnemonic\.[a-z0-9._-]+|preimage\.[a-z0-9._-]+)/i

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(values.map(value => value.trim()).filter(value => value !== '')),
]

const pylonStatsFresh = (
  pylonStats: PublicPylonStats | undefined,
  nowUnixMs: number,
): boolean =>
  pylonStats === undefined ||
  (pylonStats.available &&
    pylonStats.asOfUnixMs !== null &&
    nowUnixMs - pylonStats.asOfUnixMs <= 10 * 60 * 1000)

const rowStatus = (
  row: PromiseRowDefinition,
  endpointStale: boolean,
): PublicLaunchDashboardStatus =>
  row.staleSensitive && endpointStale && row.baseStatus === 'green'
    ? 'yellow'
    : row.staleSensitive && endpointStale
      ? row.baseStatus === 'red'
        ? 'red'
        : 'yellow'
      : row.baseStatus

const rowBlockerRefs = (
  row: PromiseRowDefinition,
  endpointStale: boolean,
): ReadonlyArray<string> =>
  unique([
    ...row.blockerRefs,
    ...(row.staleSensitive && endpointStale
      ? ['blocker.launch_dashboard.endpoint_stale']
      : []),
  ])

const assertPublicLaunchDashboardSafe = (
  projection: PublicLaunchDashboardProjection,
): PublicLaunchDashboardProjection => {
  const serialized = JSON.stringify(projection)
  if (unsafeDashboardPattern.test(serialized)) {
    throw new PublicLaunchDashboardUnsafe({
      reason:
        'Public launch dashboard contains private or secret-shaped material.',
    })
  }

  return projection
}

export const projectPublicLaunchDashboard = (input: {
  readonly generatedAt: string
  readonly nowUnixMs: number
  readonly pylonStats?: PublicPylonStats
}): PublicLaunchDashboardProjection => {
  const endpointStale = !pylonStatsFresh(input.pylonStats, input.nowUnixMs)
  const staleEndpointRefs = endpointStale
    ? ['endpoint:/api/public/pylon-stats']
    : []
  const rows = publicLaunchDashboardRows.map(row => {
    const status = rowStatus(row, endpointStale)

    return new PublicLaunchDashboardRow({
      blockerRefs: rowBlockerRefs(row, endpointStale),
      evidenceRefs: row.evidenceRefs,
      promiseId: row.promiseId,
      promiseText: row.promiseText,
      safeCopy: row.safeCopy,
      status,
      unsafeCopy: row.unsafeCopy,
    })
  })
  const redCount = rows.filter(row => row.status === 'red').length
  const yellowCount = rows.filter(row => row.status === 'yellow').length
  const greenCount = rows.filter(row => row.status === 'green').length
  const blockerRefs = unique(rows.flatMap(row => row.blockerRefs))
  const status: PublicLaunchDashboardStatus =
    redCount > 0 ? 'red' : yellowCount > 0 ? 'yellow' : 'green'

  return assertPublicLaunchDashboardSafe(
    new PublicLaunchDashboardProjection({
      blockerRefs,
      generatedAt: input.generatedAt,
      greenCount,
      redCount,
      rows,
      schemaVersion: PublicLaunchDashboardSchemaVersion,
      sourceRefs: [
        'docs/2026-06-08-pylon-agentic-revenue-gap-audit.md',
        'docs/live/AGENTS.md',
        'route:/api/public/pylon-stats',
        'route:/api/public/artanis/report',
        'route:/api/forum/launch-status',
      ],
      staleEndpointRefs,
      staleness: PublicLaunchDashboardStaleness,
      status,
      yellowCount,
    }),
  )
}

export const publicLaunchDashboardPromiseIds = publicLaunchDashboardRows.map(
  row => row.promiseId,
)

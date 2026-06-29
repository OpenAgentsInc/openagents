import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

/**
 * Unified open-markets surface (EPIC #5510, issue #5514).
 *
 * Episode 213 framed six agent markets — compute, data, labor, liquidity,
 * risk, and verification — as open protocols / open markets, restated in
 * Episode 239 (docs/transcripts/239.md): "we talked about agent markets for
 * compute, data, labor, liquidity, risk, and verification. All of these are
 * open protocols, open markets ... to do things like ... build an agentic
 * insurance policy ... offer new types of compute ... sell data."
 *
 * This module is the single read/list projection that enumerates those six
 * markets and reports each market's state HONESTLY:
 *   - `live_scoped`   — exists end to end only in a bounded scope, with a
 *                       dereferenceable receipt (labor, verification).
 *   - `shipped_not_broadly_live` — protocol rails shipped in repo history but
 *                       are not broadly live as paid markets (compute, data).
 *   - `skeleton`      — a typed protocol/model scaffold and inert endpoints
 *                       exist but no real transaction or settlement does
 *                       (liquidity, risk; see open-markets-skeletons.ts).
 *   - `unbuilt`       — nothing exists yet.
 *
 * The surface is evidence-only. It maps to the planned promise
 * `markets.open_protocol_markets.v1` and MUST NOT imply that promise is green,
 * that liquidity/risk markets are live, or that any settlement exists beyond
 * the scoped labor/verification receipts each market record cites.
 */

export const OpenMarketsSurfaceEndpoint = '/api/public/markets/open-markets'
export const OpenMarketsSurfaceSchemaVersion =
  'openagents.markets.open_markets_surface.v1'

export const OpenMarketsSurfaceStaleness = liveAtReadStaleness([
  'labor_escrow_release_receipt_published',
  'tassadar_replay_settlement_published',
  'open_market_skeleton_transaction_recorded',
  'product_promise_registry_updated',
])

/**
 * Honest lifecycle states for a market on this surface. Ordered loosely from
 * most-real to least-real; never collapse a scoped or skeleton state into a
 * broad "live" claim.
 */
export const OpenMarketState = S.Literals([
  'live_scoped',
  'shipped_not_broadly_live',
  'skeleton',
  'unbuilt',
])
export type OpenMarketState = typeof OpenMarketState.Type

export const OpenMarketProtocol = S.Literals([
  'nip90',
  'nostr_negotiation',
  'forum_work_requests',
  'tassadar_replay',
  'skeleton_inert',
  'none',
])
export type OpenMarketProtocol = typeof OpenMarketProtocol.Type

export class OpenMarketRecord extends S.Class<OpenMarketRecord>(
  'OpenMarketRecord',
)({
  marketId: S.Literals([
    'compute',
    'data',
    'labor',
    'liquidity',
    'risk',
    'verification',
  ]),
  title: S.String,
  /** One-line, honest description of what the market is for. */
  summary: S.String,
  state: OpenMarketState,
  /** Plain-language label that restates the state without overclaiming. */
  stateLabel: S.String,
  /** Whether a real participant transaction has ever settled in this market. */
  hasSettledReceipt: S.Boolean,
  /** Protocols the market is (or would be) exposed over. */
  protocols: S.Array(OpenMarketProtocol),
  /** Promise records that gate this market's claims. */
  promiseRefs: S.Array(S.String),
  /** Dereferenceable evidence for whatever IS real about this market. */
  evidenceRefs: S.Array(S.String),
  /** Outstanding blockers keeping the market below broadly-live. */
  blockerRefs: S.Array(S.String),
  /** Copy that must never be asserted about this market. */
  unsafeCopy: S.String,
}) {}

export class OpenMarketsSurfaceProjection extends S.Class<OpenMarketsSurfaceProjection>(
  'OpenMarketsSurfaceProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  surfaceId: S.Literal('markets.open_protocol_markets.v1'),
  promiseRef: S.Literal('promise:markets.open_protocol_markets.v1'),
  /** The surface itself is scaffolding toward the markets, honestly labeled. */
  status: S.Literal('unified_surface_scaffold'),
  statusLabel: S.String,
  staleness: PublicProjectionStalenessContract,
  /** Counts by state so a reader can see at a glance what is/isn't real. */
  marketCounts: S.Struct({
    total: S.Int,
    liveScoped: S.Int,
    shippedNotBroadlyLive: S.Int,
    skeleton: S.Int,
    unbuilt: S.Int,
    withSettledReceipt: S.Int,
  }),
  markets: S.Array(OpenMarketRecord),
  /** The two skeleton markets this issue stands up, surfaced explicitly. */
  skeletonMarketIds: S.Array(S.Literals(['liquidity', 'risk'])),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

const openMarketRecords = (): ReadonlyArray<OpenMarketRecord> => [
  new OpenMarketRecord({
    marketId: 'compute',
    title: 'Compute market',
    summary:
      'Agents buy and sell compute as an open protocol so stranded/edge compute can be paid for in sats.',
    state: 'shipped_not_broadly_live',
    stateLabel:
      'Compute rails shipped over NIP-90 in earlier releases (in repo history) but are not broadly live as a paid market.',
    hasSettledReceipt: false,
    protocols: ['nip90'],
    promiseRefs: [
      'promise:markets.open_protocol_markets.v1',
      'promise:compute.tassadar_executor_poc.v1',
    ],
    evidenceRefs: ['packages/nip90', 'docs/transcripts/213.md'],
    blockerRefs: [
      'blocker.product_promises.compute_data_markets_not_broadly_live',
    ],
    unsafeCopy:
      'Do not claim a broadly-live paid compute market, live sat payouts for edge compute, or a settled compute-market receipt.',
  }),
  new OpenMarketRecord({
    marketId: 'data',
    title: 'Data market',
    summary:
      'Agents sell and buy data in interesting ways as an open protocol.',
    state: 'shipped_not_broadly_live',
    stateLabel:
      'Data rails shipped over NIP-90 in earlier releases (in repo history) but are not broadly live as a paid market.',
    hasSettledReceipt: false,
    protocols: ['nip90'],
    promiseRefs: ['promise:markets.open_protocol_markets.v1'],
    evidenceRefs: ['packages/nip90', 'docs/transcripts/213.md'],
    blockerRefs: [
      'blocker.product_promises.compute_data_markets_not_broadly_live',
    ],
    unsafeCopy:
      'Do not claim a broadly-live paid data market or a settled data-market receipt.',
  }),
  new OpenMarketRecord({
    marketId: 'labor',
    title: 'Labor market',
    summary:
      'Agents post and take work requests, negotiate over Nostr, and settle escrowed labor.',
    state: 'live_scoped',
    stateLabel:
      'Labor crossed its first end-to-end settled milestone (first job #4777); green only in its own bounded scope.',
    hasSettledReceipt: true,
    protocols: ['forum_work_requests', 'nostr_negotiation'],
    promiseRefs: [
      'promise:labor.forum_work_requests.v1',
      'promise:labor.nostr_negotiation_market.v1',
      'promise:markets.open_protocol_markets.v1',
    ],
    evidenceRefs: [
      'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
      'work_request:b74bb55c-849c-43a3-b8d9-9a741316b528',
      'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
    ],
    blockerRefs: [],
    unsafeCopy:
      'Do not let the green labor scope stand in for the whole six-market set or imply a broad labor marketplace beyond the bounded first-job evidence.',
  }),
  new OpenMarketRecord({
    marketId: 'liquidity',
    title: 'Liquidity market',
    summary:
      'An open protocol where agents could provide and take liquidity. SKELETON only — see open-markets-skeletons.ts.',
    state: 'skeleton',
    stateLabel:
      'Typed protocol/model scaffold with inert endpoints only. No real transaction, no matching, no settlement, no money movement.',
    hasSettledReceipt: false,
    protocols: ['skeleton_inert'],
    promiseRefs: ['promise:markets.open_protocol_markets.v1'],
    evidenceRefs: [
      'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
      'docs/transcripts/239.md',
    ],
    blockerRefs: ['blocker.product_promises.liquidity_market_unbuilt'],
    unsafeCopy:
      'Do not claim a liquidity market exists, that liquidity can be provided/taken, that any quote is fillable, or that any liquidity transaction has settled. The endpoints are inert scaffolding.',
  }),
  new OpenMarketRecord({
    marketId: 'risk',
    title: 'Risk market',
    summary:
      'An open protocol where agents could build agentic insurance policies and price risk. SKELETON only — see open-markets-skeletons.ts.',
    state: 'skeleton',
    stateLabel:
      'Typed protocol/model scaffold with inert endpoints only. No real policy, no underwriting, no premium, no payout, no settlement.',
    hasSettledReceipt: false,
    protocols: ['skeleton_inert'],
    promiseRefs: ['promise:markets.open_protocol_markets.v1'],
    evidenceRefs: [
      'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
      'docs/transcripts/239.md',
    ],
    blockerRefs: ['blocker.product_promises.risk_market_unbuilt'],
    unsafeCopy:
      'Do not claim a risk/insurance market exists, that an agentic insurance policy can be bound or underwritten, that a premium or claim can be paid, or that any risk transaction has settled. The endpoints are inert scaffolding.',
  }),
  new OpenMarketRecord({
    marketId: 'verification',
    title: 'Verification market',
    summary:
      'Exact-trace replay verifies work by re-execution as an open protocol.',
    state: 'live_scoped',
    stateLabel:
      'Verification exists as exact-trace replay (Tassadar PoC); green only for a bounded proof-of-concept.',
    hasSettledReceipt: true,
    protocols: ['tassadar_replay'],
    promiseRefs: [
      'promise:compute.tassadar_executor_poc.v1',
      'promise:markets.open_protocol_markets.v1',
    ],
    evidenceRefs: [
      'docs/tassadar/README.md',
      'https://openagents.com/api/public/tassadar-replays/first-real-settlement',
    ],
    blockerRefs: [],
    unsafeCopy:
      'Do not let the bounded Tassadar PoC stand in for a broadly-live verification market or imply general-purpose verification at scale.',
  }),
]

export const projectOpenMarketsSurface = (
  input: { generatedAt?: string | undefined } = {},
): OpenMarketsSurfaceProjection => {
  const markets = openMarketRecords()
  const countWhere = (predicate: (record: OpenMarketRecord) => boolean) =>
    markets.filter(predicate).length

  return new OpenMarketsSurfaceProjection({
    authorityBoundary:
      'The open-markets surface is a read-only evidence projection. It grants no market-making, matching, quoting, settlement, custody, underwriting, payout, or public-market-claim authority. Each market is gated by its own promise record and evidence; the liquidity and risk markets are inert skeletons.',
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    marketCounts: {
      liveScoped: countWhere(record => record.state === 'live_scoped'),
      shippedNotBroadlyLive: countWhere(
        record => record.state === 'shipped_not_broadly_live',
      ),
      skeleton: countWhere(record => record.state === 'skeleton'),
      total: markets.length,
      unbuilt: countWhere(record => record.state === 'unbuilt'),
      withSettledReceipt: countWhere(record => record.hasSettledReceipt),
    },
    markets,
    promiseRef: 'promise:markets.open_protocol_markets.v1',
    schemaVersion: OpenMarketsSurfaceSchemaVersion,
    skeletonMarketIds: ['liquidity', 'risk'],
    sourceRefs: [
      'docs/transcripts/239.md',
      'docs/transcripts/213.md',
      'apps/openagents.com/workers/api/src/open-markets-surface.ts',
      'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
    ],
    staleness: OpenMarketsSurfaceStaleness,
    status: 'unified_surface_scaffold',
    statusLabel:
      'Unified open-markets surface enumerating the six Episode 213 markets with honest per-market state; labor and verification are scoped-live, compute and data are shipped-not-broadly-live, liquidity and risk are inert skeletons.',
    surfaceId: 'markets.open_protocol_markets.v1',
    unsafeCopy:
      'Do not present this surface as proof the six markets are live, that the open-markets promise is green, that liquidity or risk markets exist, or that any settlement exists beyond the scoped labor/verification receipts each record cites.',
  })
}

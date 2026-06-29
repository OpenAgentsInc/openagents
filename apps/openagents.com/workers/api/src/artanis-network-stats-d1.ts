// Artanis network-stats D1 reader (epic #6359).
//
// The Worker cannot reliably HTTP-fetch its OWN public zone
// (`https://openagents.com/api/public/...`) — a same-zone loopback subrequest
// from the openagents.com Worker comes back empty, which made Artanis see an
// all-zero token pace in production even though the public /stats endpoints are
// live. So in the Worker we read the SAME token-usage ledger the public stats
// routes read, directly from D1, and assemble the identical public-safe snapshot
// + pace block. Tests and non-Worker contexts can still use the HTTP
// `fetchArtanisNetworkStats` path in `artanis-token-pace.ts`.
//
// Named Effect->Promise bridge: the awareness reader and the get_network_stats
// tool are Promise-shaped, while the ledger reads are Effects, so this file runs
// the combined read once with `Effect.runPromise`. Read-only, fail-soft: any
// ledger read failure degrades that bucket to empty/zero, never an error.

import { Effect } from 'effect'

import {
  ARTANIS_TOKEN_PACE_TIMEZONE,
  type ArtanisNetworkStats,
  type ArtanisTokenHistoryPoint,
  assembleArtanisNetworkStats,
} from './artanis-token-pace'
import { currentIsoTimestamp } from './runtime-primitives'
import type { TokenUsageLedgerShape } from './token-usage-ledger'

export type LoadArtanisNetworkStatsFromLedgerConfig = Readonly<{
  nowIso?: (() => string) | undefined
  timezone?: string | undefined
  historyDays?: number | undefined
  modelMixWindow?: string | undefined
}>

// loadArtanisNetworkStatsFromLedger — read the live token-usage ledger (the same
// source the public /stats endpoints use) and assemble the Artanis network-stats
// snapshot with its pace block. Never rejects.
export const loadArtanisNetworkStatsFromLedger = (
  ledger: TokenUsageLedgerShape,
  config: LoadArtanisNetworkStatsFromLedgerConfig = {},
): Promise<ArtanisNetworkStats> => {
  const nowIso = (config.nowIso ?? currentIsoTimestamp)()
  const timezone = config.timezone ?? ARTANIS_TOKEN_PACE_TIMEZONE
  const modelMixWindow = config.modelMixWindow ?? '30d'

  const program = Effect.all({
    allTimeTokensServed: ledger.readPublicTokensServed().pipe(
      Effect.map(result => result.tokensServed),
      Effect.orElseSucceed(() => 0),
    ),
    modelMix: ledger
      .readPublicTokensServedModelMix({ window: modelMixWindow })
      .pipe(
        Effect.map(result =>
          result.groups.map(group => ({
            family: group.family as string,
            label: group.label,
            pct: group.pct,
            tokens: group.tokens,
          })),
        ),
        Effect.orElseSucceed(
          () =>
            [] as ReadonlyArray<
              Readonly<{
                family: string
                label: string
                tokens: number
                pct: number
              }>
            >,
        ),
      ),
    series: ledger.readPublicTokensServedHistory({ timezone }).pipe(
      Effect.map(result =>
        result.series.map(point => ({
          day: point.day,
          tokensServed: point.tokensServed,
        })),
      ),
      Effect.orElseSucceed(() => [] as ReadonlyArray<ArtanisTokenHistoryPoint>),
    ),
  })

  return Effect.runPromise(program).then(
    parts =>
      assembleArtanisNetworkStats({
        allTimeTokensServed: parts.allTimeTokensServed,
        historyDays: config.historyDays,
        modelMix: parts.modelMix,
        nowIso,
        series: parts.series,
        timezone,
      }),
    () =>
      assembleArtanisNetworkStats({
        allTimeTokensServed: 0,
        historyDays: config.historyDays,
        modelMix: [],
        nowIso,
        series: [],
        timezone,
      }),
  )
}

/**
 * Public relay health status route (openagents#4865):
 * `GET /api/public/relay-health`.
 *
 * Serves the current canonical market relay status plus retained probe
 * history and typed status transitions, so a short relay outage (the
 * Orrery spend-probe case: HTTP 530 + refused websockets, 20:33-20:35Z,
 * gone by follow-up time) stays publicly citable after recovery.
 *
 * Projection posture: composed from the retained probe rows at read time,
 * but the data itself is a stored snapshot series captured on the 5-minute
 * probe cadence, so the declared staleness contract is `stored_snapshot`
 * with a 7-minute bound (cadence plus execution slack). The payload
 * carries `generatedAt` and flags itself stale when the newest retained
 * probe exceeds the bound. Read-only monitoring evidence; grants no
 * relay-mutation, payout, settlement, or public-claim authority.
 */
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  projectionDataAgeSeconds,
  projectionStalenessExceeded,
  storedSnapshotStaleness,
} from './public-projection-staleness'
import {
  RELAY_HEALTH_PROBE_INTERVAL_MINUTES,
  makeD1RelayHealthStore,
  type RelayHealthProbeRecord,
  type RelayHealthStore,
  type RelayHealthTransitionEvent,
} from './relay-health'
import { currentIsoTimestamp } from './runtime-primitives'

export const PUBLIC_RELAY_HEALTH_PROJECTION_CONTRACT =
  'projection.relay_health.v1'

/** 24 hours of 5-minute probes. */
const PROBE_HISTORY_LIMIT = 288

/** Recent status transitions; retention bounds the rest. */
const TRANSITION_HISTORY_LIMIT = 50

/** Probe cadence (5 min) plus execution slack before the data is stale. */
const RELAY_HEALTH_MAX_STALENESS_SECONDS = 7 * 60

const stalenessContract = storedSnapshotStaleness(
  RELAY_HEALTH_MAX_STALENESS_SECONDS,
  ['relay_health_probe_recorded', 'relay_health_status_transition'],
)

const READ_ONLY_CAVEAT =
  'Relay health probes are read-only monitoring evidence for the canonical market relay. They grant no relay-mutation, payout, settlement, or public-claim authority.'

const probeProjection = (probe: RelayHealthProbeRecord) => ({
  id: probe.id,
  nip11: {
    httpStatus: probe.nip11.httpStatus,
    latencyMs: probe.nip11.latencyMs,
    outcome: probe.nip11.outcome,
    relayName: probe.nip11.relayName,
  },
  probedAt: probe.probedAt,
  status: probe.status,
  ws: {
    latencyMs: probe.ws.latencyMs,
    outcome: probe.ws.outcome,
  },
})

const transitionProjection = (event: RelayHealthTransitionEvent) => ({
  fromStatus: event.fromStatus,
  id: event.id,
  kind: event.kind,
  occurredAt: event.occurredAt,
  probeId: event.probeId,
  toStatus: event.toStatus,
})

export type PublicRelayHealthRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: () => string
  relayUrl: string
  store?: RelayHealthStore
}>

export const handlePublicRelayHealthApi = (
  request: Request,
  input: PublicRelayHealthRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const store =
    input.store ?? makeD1RelayHealthStore(input.OPENAGENTS_DB as D1Database)

  return Effect.promise(async () => {
    const [probes, transitions] = await Promise.all([
      store.listRecentProbes(input.relayUrl, PROBE_HISTORY_LIMIT),
      store.listRecentTransitions(input.relayUrl, TRANSITION_HISTORY_LIMIT),
    ])
    const latest = probes[0] ?? null
    const dataAgeSeconds = projectionDataAgeSeconds(
      latest?.probedAt ?? null,
      nowIso,
    )

    return noStoreJsonResponse({
      authorityCaveat: READ_ONLY_CAVEAT,
      contractVersion: PUBLIC_RELAY_HEALTH_PROJECTION_CONTRACT,
      current:
        latest === null
          ? null
          : {
              ...probeProjection(latest),
              dataAgeSeconds,
            },
      generatedAt: nowIso,
      history: {
        probes: probes.map(probeProjection),
        retentionPolicyRef:
          'retention.relay_health.probes_7d_transitions_30d',
      },
      kind: 'relay_health',
      probeCadenceMinutes: RELAY_HEALTH_PROBE_INTERVAL_MINUTES,
      publicSafe: true,
      relayUrl: input.relayUrl,
      staleExceeded: projectionStalenessExceeded(
        stalenessContract,
        dataAgeSeconds,
      ),
      staleness: stalenessContract,
      status: latest?.status ?? 'unknown',
      transitions: transitions.map(transitionProjection),
    })
  })
}

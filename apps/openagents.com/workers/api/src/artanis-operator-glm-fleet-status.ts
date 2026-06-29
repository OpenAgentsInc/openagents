// Artanis owner-scoped GLM inference-fleet readiness LOADER (iteration-7
// capability).
//
// This is the in-worker production seam behind the `get_glm_fleet_status` read
// tool (`artanis-operator-tools.ts`). It resolves the live GLM serving fleet's
// readiness into the bounded, public-safe `ArtanisGlmFleetStatus` projection so
// Artanis can GATE synthetic-load and Codex-dispatch decisions on healthy
// capacity instead of piling load onto a saturated/cold fleet and degrading real
// users.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It reuses the SAME projection the public
//     `GET /v1/gateway/glm-fleet/readiness` route serves
//     (`projectGlmFleetReadinessForEnv` over configured replica arming + the
//     latest in-memory/persisted heartbeat records). It never probes hosts,
//     mutates replica state, or touches the GLM serving / admission path.
//   - IN-WORKER. The Worker cannot reliably HTTP-fetch its OWN public zone, so
//     the operator tool reads this projection directly instead of an HTTP hop
//     (mirroring the get_network_stats ledger loader).
//   - PUBLIC-SAFE. Only the overall status and aggregate ready/total/warm
//     replica COUNTS are surfaced; no host origins, credentials, prompts,
//     completions, prices, or balances.

import type { ArtanisGlmFleetStatus } from './artanis-operator-tools'
import {
  type GlmFleetReadinessHeartbeatRecord,
  projectGlmFleetReadinessForEnv,
} from './inference/glm-fleet-readiness'
import { readPersistedGlmFleetReadinessHeartbeatRecords } from './inference/glm-fleet-readiness-routes'
import { glmPoolHeartbeatLatestRecordOracle } from './inference/glm-pool-heartbeat'

// The env shape the readiness projection needs (supply-lane credential presence
// + GLM replica config). Derived from the projection function so we never have
// to re-declare or export the internal env type.
type GlmFleetReadinessEnv = Parameters<typeof projectGlmFleetReadinessForEnv>[0]

export type ArtanisGlmFleetStatusLoaderDeps = Readonly<{
  // The worker env (supply-lane credential presence + GLM replica arming config).
  env: GlmFleetReadinessEnv
  // The D1 database for the latest persisted heartbeat records. Optional: with no
  // db wired the projection still resolves from configured arming + the in-memory
  // heartbeat oracle.
  db?: D1Database | undefined
}>

// Build the in-worker GLM fleet readiness loader for the `get_glm_fleet_status`
// tool. Returns the public-safe aggregate projection (overall status + ready /
// total / warm replica counts). Fail-soft: a persisted-heartbeat read failure
// falls back to configured arming + the in-memory heartbeat oracle, never a
// throw and never fabricated numbers.
export const makeArtanisGlmFleetStatusLoader = (
  deps: ArtanisGlmFleetStatusLoaderDeps,
): (() => Promise<ArtanisGlmFleetStatus>) => {
  return async (): Promise<ArtanisGlmFleetStatus> => {
    const persisted: ReadonlyArray<GlmFleetReadinessHeartbeatRecord> =
      deps.db === undefined
        ? []
        : await readPersistedGlmFleetReadinessHeartbeatRecords(deps.db).catch(
            () => [] as ReadonlyArray<GlmFleetReadinessHeartbeatRecord>,
          )
    const persistedByReplica = new Map(
      persisted.map(record => [record.replicaId, record] as const),
    )
    const projection = projectGlmFleetReadinessForEnv(
      deps.env,
      replicaId =>
        glmPoolHeartbeatLatestRecordOracle(replicaId) ??
        persistedByReplica.get(replicaId),
    )
    return {
      readyReplicas: projection.counts.readyReplicaCount,
      status: projection.status,
      totalReplicas: projection.counts.totalReplicaCount,
      warmReplicas: projection.counts.warmReplicaCount,
    }
  }
}

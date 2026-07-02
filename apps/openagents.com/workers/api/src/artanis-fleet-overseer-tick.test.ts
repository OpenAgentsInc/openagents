import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ArtanisFleetOverseerContext,
  runArtanisFleetOverseerTick,
  runArtanisFleetOverseerTickScheduled,
} from './artanis-fleet-overseer-tick'
import {
  ArtanisPersistenceTestStore,
  artanisPersistenceTestDb,
} from './test/artanis-persistence-fixture'

const nowIso = '2026-06-27T12:00:00.000Z'

const context: ArtanisFleetOverseerContext = {
  externalDemandTokens10m: 1200,
  heartbeatRunRefs: ['heartbeat.hydralisk.glm_52_reap_504b.20260627t1159'],
  readyReplicaCount: 2,
  reclaimedReplicaRefs: ['replica.hydralisk.glm_52_reap_504b.replica-b'],
  statusSpineActiveAssignmentCount: 1,
  statusSpineLiveRunnerCount: 2,
  statusSpineRetainedRunnerCount: 1,
  statusSpineSourceRefs: [
    'status.public.operator_fleet_status.spine',
    'event.public.operator.runner_status.codex_1',
  ],
  totalReplicaCount: 3,
  warmOrReadyMaxInflight: 2,
}

describe('Artanis fleet overseer tick', () => {
  test('stays disabled by default without writing rows', async () => {
    const store = new ArtanisPersistenceTestStore()
    const result = await Effect.runPromise(
      runArtanisFleetOverseerTickScheduled(artanisPersistenceTestDb(store), {
        enabled: false,
        geminiApiKey: null,
        nowIso,
      }),
    )

    expect(result).toEqual({
      approvalGateRef: null,
      decisionId: null,
      healthSnapshotRef: null,
      reason: 'fleet_overseer_disabled',
      state: 'skipped',
    })
    expect([...store.tables.values()].flat()).toHaveLength(0)
  })

  test('persists blocked decision and health snapshot for schema-invalid mind output', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)

    const result = await runArtanisFleetOverseerTick(db, {
      assembleContext: async () => context,
      geminiApiKey: 'test-key',
      mindComplete: async () => ({ text: '{"kind":"quarantine_now"}' }),
      nowIso,
    })

    expect(result).toMatchObject({
      approvalGateRef: null,
      reason: 'schema_invalid_mind_output',
      state: 'blocked',
    })
    expect(store.rows('artanis_fleet_overseer_decisions')).toHaveLength(1)
    expect(store.rows('artanis_fleet_overseer_decisions')[0]).toMatchObject({
      health_snapshot_ref:
        'health.public.artanis.snapshot.fleet_overseer.20260627t120000000z',
      state: 'blocked',
    })
    expect(
      JSON.parse(
        store.rows('artanis_fleet_overseer_decisions')[0]!.action_json!,
      ),
    ).toMatchObject({
      executionAllowed: false,
      kind: 'blocked',
      reason: 'schema_invalid_mind_output',
    })
    expect(store.rows('artanis_health_snapshots')).toHaveLength(1)
    expect(store.rows('pylon_agent_runner_status_events')).toHaveLength(1)
    const statusEvent = JSON.parse(
      store.rows('pylon_agent_runner_status_events')[0]!.event_json!,
    )
    expect(statusEvent).toMatchObject({
      runnerRef: 'runner.public.artanis.fleet_overseer',
      schemaVersion: 'openagents.pylon.agent_runner_status_event.v1',
      state: 'blocked',
    })
    expect(statusEvent.refs).toEqual(
      expect.arrayContaining([
        'status.public.artanis.fleet_overseer.on_status_spine',
        'status.public.operator_fleet_status.spine',
        'load.coding.codex.busy=1',
      ]),
    )
  })

  test('risky replica quarantine proposal creates pending approval only', async () => {
    const store = new ArtanisPersistenceTestStore()
    const db = artanisPersistenceTestDb(store)

    const result = await runArtanisFleetOverseerTick(db, {
      assembleContext: async () => context,
      geminiApiKey: 'test-key',
      mindComplete: async () => ({
        text: JSON.stringify({
          kind: 'request_replica_quarantine',
          rationale: 'Replica is reclaimed by watchdog.',
          replicaRef: 'replica.hydralisk.glm_52_reap_504b.replica-b',
        }),
      }),
      nowIso,
    })

    expect(result).toMatchObject({
      approvalGateRef:
        'gate.public.artanis.fleet_overseer.request_replica_quarantine.20260627t120000000z',
      state: 'approval_requested',
    })
    expect(store.rows('artanis_approval_gates')).toHaveLength(1)
    expect(store.rows('artanis_approval_gates')[0]).toMatchObject({
      active: 0,
      scope_ref: 'fleet_mutation',
      state: 'pending',
    })
    expect(store.rows('artanis_fleet_overseer_decisions')).toHaveLength(1)
    expect(
      JSON.parse(
        store.rows('artanis_fleet_overseer_decisions')[0]!.action_json!,
      ),
    ).toMatchObject({
      authorityRefs: expect.arrayContaining([
        'authority.public.artanis.fleet_overseer.decision_is_not_execution',
      ]),
      executionAllowed: false,
      kind: 'request_replica_quarantine',
    })
    expect(store.rows('pylon_agent_runner_status_events')).toHaveLength(1)
    const statusEvent = JSON.parse(
      store.rows('pylon_agent_runner_status_events')[0]!.event_json!,
    )
    expect(statusEvent).toMatchObject({
      runnerRef: 'runner.public.artanis.fleet_overseer',
      state: 'waiting',
    })
    expect(statusEvent.refs).toEqual(
      expect.arrayContaining([
        'status.public.artanis.fleet_overseer.decision_state.approval_requested',
        'capacity.coding.codex.ready=2',
      ]),
    )
  })
})

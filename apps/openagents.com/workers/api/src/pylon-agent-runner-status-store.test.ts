import { describe, expect, test } from 'vitest'

import type {
  PylonDispatchDiagnostic,
  PylonDispatchDiagnosticEvent,
} from './pylon-dispatch-store'
import {
  makeReadRoutedPylonAgentRunnerStatusReadStore,
  makePylonAgentRunnerStatusMirror,
  type PylonAgentRunnerStatusReadInput,
  type PylonAgentRunnerStatusReadResult,
  type PylonAgentRunnerStatusReadStore,
  type PylonAgentRunnerStatusEventRecord,
  type PylonAgentRunnerStatusMirrorInput,
  type PylonAgentRunnerStatusPostgresStore,
  type PylonAgentRunnerStatusRow,
  type PylonAgentRunnerStatusRetainInput,
} from './pylon-agent-runner-status-store'

const record: PylonAgentRunnerStatusEventRecord = {
  assignmentRef: 'assignment.public.issue_7878',
  createdAt: '2026-07-01T12:03:00.000Z',
  eventJson: '{"schemaVersion":"openagents.pylon.agent_runner_status_event.v1"}',
  eventRef: 'event.public.runner_status.1',
  ownerAgentUserId: 'agent_user.owner',
  pylonRef: 'pylon.public.codex',
  retainedAt: null,
  retentionState: 'live',
  runnerKind: 'codex_sdk',
  runnerRef: 'runner.public.codex.1',
  state: 'working',
  stateStartedAt: '2026-07-01T12:00:00.000Z',
  updatedAt: '2026-07-01T12:02:00.000Z',
}

const retain: PylonAgentRunnerStatusRetainInput = {
  eventRef: record.eventRef,
  ownerAgentUserId: record.ownerAgentUserId,
  retainedAt: record.stateStartedAt,
  runnerRef: record.runnerRef,
}

const input: PylonAgentRunnerStatusMirrorInput = {
  record,
  retain,
}

const statusRow: PylonAgentRunnerStatusRow = {
  assignment_ref: record.assignmentRef,
  event_json: record.eventJson,
  event_ref: record.eventRef,
  owner_agent_user_id: record.ownerAgentUserId,
  pylon_ref: record.pylonRef,
  retention_state: 'live',
  runner_kind: record.runnerKind,
  runner_ref: record.runnerRef,
  state: record.state,
  state_started_at: record.stateStartedAt,
  updated_at: record.updatedAt,
}

const makeMemoryStore = (): PylonAgentRunnerStatusPostgresStore & {
  retained: Array<PylonAgentRunnerStatusRetainInput>
  upserted: Array<PylonAgentRunnerStatusEventRecord>
} => {
  const retained: Array<PylonAgentRunnerStatusRetainInput> = []
  const upserted: Array<PylonAgentRunnerStatusEventRecord> = []
  return {
    retained,
    retainLiveRunnerEvents: async next => {
      retained.push(next)
    },
    upserted,
    upsertStatusEvent: async next => {
      upserted.push(next)
    },
  }
}

const makeReadStore = (
  result: PylonAgentRunnerStatusReadResult,
  listStatusRows: (
    input: PylonAgentRunnerStatusReadInput,
  ) => Promise<PylonAgentRunnerStatusReadResult> = async () => result,
): PylonAgentRunnerStatusReadStore & {
  calls: Array<PylonAgentRunnerStatusReadInput>
} => {
  const calls: Array<PylonAgentRunnerStatusReadInput> = []
  return {
    calls,
    listStatusRows: async next => {
      calls.push(next)
      return listStatusRows(next)
    },
  }
}

const makeLogSink = () => {
  const events: Array<{
    event: PylonDispatchDiagnosticEvent
    fields: PylonDispatchDiagnostic
  }> = []
  return {
    events,
    log: (
      event: PylonDispatchDiagnosticEvent,
      fields: PylonDispatchDiagnostic,
    ) => {
      events.push({ event, fields })
    },
  }
}

describe('Pylon agent runner status read routing', () => {
  test('d1 mode returns D1 rows without touching Postgres', async () => {
    const d1 = makeReadStore({
      rows: [statusRow],
      sourceRefs: ['d1:pylon_agent_runner_status_events'],
    })
    const postgres = makeReadStore({
      rows: [{ ...statusRow, state: 'blocked' }],
      sourceRefs: ['postgres:pylon_agent_runner_status_events'],
    })
    const store = makeReadRoutedPylonAgentRunnerStatusReadStore({
      d1,
      flags: { reads: 'd1' },
      postgres,
    })

    const result = await store.listStatusRows({
      limit: 200,
      scope: { kind: 'admin' },
    })

    expect(result.rows).toEqual([statusRow])
    expect(d1.calls).toHaveLength(1)
    expect(postgres.calls).toEqual([])
  })

  test('compare mode serves D1 and logs Postgres drift with shadow source refs', async () => {
    const sink = makeLogSink()
    const d1 = makeReadStore({
      rows: [statusRow],
      sourceRefs: ['d1:pylon_agent_runner_status_events'],
    })
    const postgres = makeReadStore({
      rows: [{ ...statusRow, state: 'blocked' }],
      sourceRefs: ['postgres:pylon_agent_runner_status_events'],
    })
    const store = makeReadRoutedPylonAgentRunnerStatusReadStore({
      d1,
      flags: { reads: 'compare' },
      log: sink.log,
      postgres,
      wait: async () => undefined,
    })

    const result = await store.listStatusRows({
      limit: 200,
      scope: { kind: 'agent', userId: 'agent_user.owner' },
    })

    expect(result.rows).toEqual([statusRow])
    expect(result.sourceRefs).toEqual([
      'd1:pylon_agent_runner_status_events',
      'postgres-shadow:pylon_agent_runner_status_events',
    ])
    expect(sink.events).toHaveLength(1)
    expect(sink.events[0]).toMatchObject({
      event: 'khala_sync_pylon_read_compare_mismatch',
      fields: {
        op: 'listAgentRunnerStatusRows',
        refs: ['agent:agent_user.owner'],
      },
    })
  })

  test('postgres mode serves Postgres rows without touching D1', async () => {
    const d1 = makeReadStore({
      rows: [statusRow],
      sourceRefs: ['d1:pylon_agent_runner_status_events'],
    })
    const postgresRow = { ...statusRow, state: 'blocked' }
    const postgres = makeReadStore({
      rows: [postgresRow],
      sourceRefs: ['postgres:pylon_agent_runner_status_events'],
    })
    const store = makeReadRoutedPylonAgentRunnerStatusReadStore({
      d1,
      flags: { reads: 'postgres' },
      postgres,
      wait: async () => undefined,
    })

    const result = await store.listStatusRows({
      limit: 200,
      scope: { kind: 'admin' },
    })

    expect(result.rows).toEqual([postgresRow])
    expect(result.sourceRefs).toEqual([
      'postgres:pylon_agent_runner_status_events',
    ])
    expect(d1.calls).toEqual([])
    expect(postgres.calls).toHaveLength(1)
  })

  test('postgres mode falls back to D1 after bounded retry diagnostics', async () => {
    let attempts = 0
    const sink = makeLogSink()
    const d1 = makeReadStore({
      rows: [statusRow],
      sourceRefs: ['d1:pylon_agent_runner_status_events'],
    })
    const postgres = makeReadStore(
      { rows: [], sourceRefs: ['postgres:pylon_agent_runner_status_events'] },
      async () => {
        attempts += 1
        throw new Error(`pg down ${attempts}`)
      },
    )
    const store = makeReadRoutedPylonAgentRunnerStatusReadStore({
      d1,
      flags: { reads: 'postgres' },
      log: sink.log,
      postgres,
      wait: async () => undefined,
    })

    const result = await store.listStatusRows({
      limit: 200,
      scope: { kind: 'agent', userId: 'agent_user.owner' },
    })

    expect(result.rows).toEqual([statusRow])
    expect(attempts).toBe(3)
    expect(sink.events.map(event => event.event)).toEqual([
      'khala_sync_pylon_postgres_read_failed',
      'khala_sync_pylon_postgres_read_failed',
      'khala_sync_pylon_postgres_read_fallback',
    ])
    expect(d1.calls).toHaveLength(1)
  })
})

describe('Pylon agent runner status mirror', () => {
  test('retains previous live rows before upserting the new status event', async () => {
    const postgres = makeMemoryStore()
    const mirror = makePylonAgentRunnerStatusMirror({
      flags: { dualWrite: true },
      postgres,
    })

    await mirror.recordStatusEvent(input)

    expect(postgres.retained).toEqual([retain])
    expect(postgres.upserted).toEqual([record])
  })

  test('Postgres mirror failures are fail-soft diagnostics', async () => {
    const sink = makeLogSink()
    const mirror = makePylonAgentRunnerStatusMirror({
      flags: { dualWrite: true },
      log: sink.log,
      postgres: {
        retainLiveRunnerEvents: () => Promise.reject(new Error('pg down')),
        upsertStatusEvent: () => Promise.reject(new Error('pg down')),
      },
    })

    await expect(mirror.recordStatusEvent(input)).resolves.toBeUndefined()

    expect(sink.events).toHaveLength(1)
    expect(sink.events[0]).toMatchObject({
      event: 'khala_sync_pylon_dual_write_failed',
      fields: {
        op: 'recordAgentRunnerStatusEvent',
        refs: [record.eventRef, record.runnerRef],
      },
    })
  })
})

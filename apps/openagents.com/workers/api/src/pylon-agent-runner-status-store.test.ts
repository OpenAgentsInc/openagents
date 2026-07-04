import { describe, expect, test } from 'vitest'

import type {
  PylonDispatchDiagnostic,
  PylonDispatchDiagnosticEvent,
} from './pylon-dispatch-store'
import {
  makePylonAgentRunnerStatusMirror,
  type PylonAgentRunnerStatusEventRecord,
  type PylonAgentRunnerStatusMirrorInput,
  type PylonAgentRunnerStatusPostgresStore,
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

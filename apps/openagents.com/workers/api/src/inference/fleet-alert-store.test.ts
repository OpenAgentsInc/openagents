import { describe, expect, test } from 'vitest'

import type {
  PylonDispatchDiagnostic,
  PylonDispatchDiagnosticEvent,
} from '../pylon-dispatch-store'
import {
  makeDualWriteFleetAlertWriteStore,
  type FleetAlertWriteRecord,
  type FleetAlertWriteStore,
} from './fleet-alert-store'

const alertRecord: FleetAlertWriteRecord = {
  activeAssignments: 2,
  alertRef: 'fleet_alert.2026-06-29T15:00:00.000Z.deadbeef',
  burnTokensWindow: 0,
  classification: 'stalled',
  createdAt: '2026-06-29T15:00:00.000Z',
  detectedAt: '2026-06-29T15:00:00.000Z',
  id: 'alert-row-1',
  queuedAssignments: 1,
  reasonRef: 'blocker.public.fleet.burn_stalled_with_active_work',
  recoveredLeaseCount: 1,
  recoveryActions: ['recovery.flushed_abandoned_leases.count=1'],
  stallThresholdTokens: 1_000_000,
  windowMinutes: 5,
}

const makeMemoryStore = (): FleetAlertWriteStore & {
  records: Array<FleetAlertWriteRecord>
} => {
  const records: Array<FleetAlertWriteRecord> = []
  return {
    records,
    insertAlert: async record => {
      records.push(record)
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

describe('fleet alert dual-write store', () => {
  test('mirrors D1-first fleet alert inserts to Postgres', async () => {
    const d1 = makeMemoryStore()
    const postgres = makeMemoryStore()
    const store = makeDualWriteFleetAlertWriteStore({
      d1,
      flags: { dualWrite: true },
      postgres,
    })

    await store.insertAlert(alertRecord)

    expect(d1.records).toEqual([alertRecord])
    expect(postgres.records).toEqual([alertRecord])
  })

  test('Postgres mirror failures are fail-soft diagnostics', async () => {
    const d1 = makeMemoryStore()
    const sink = makeLogSink()
    const store = makeDualWriteFleetAlertWriteStore({
      d1,
      flags: { dualWrite: true },
      log: sink.log,
      postgres: {
        insertAlert: () => Promise.reject(new Error('pg down')),
      },
    })

    await expect(store.insertAlert(alertRecord)).resolves.toBeUndefined()

    expect(d1.records).toEqual([alertRecord])
    expect(sink.events).toHaveLength(1)
    expect(sink.events[0]).toMatchObject({
      event: 'khala_sync_pylon_dual_write_failed',
      fields: {
        op: 'insertFleetAlert',
        refs: [alertRecord.alertRef, alertRecord.classification],
      },
    })
  })
})

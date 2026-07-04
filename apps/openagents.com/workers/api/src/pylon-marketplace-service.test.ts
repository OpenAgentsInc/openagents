import { describe, expect, test } from 'vitest'

import type {
  PylonDispatchDiagnostic,
  PylonDispatchDiagnosticEvent,
} from './pylon-dispatch-store'
import {
  makeDualWritePylonMarketplaceJobStore,
  type PylonMarketplaceJobStore,
  type PylonMarketplaceStoredAssignment,
  type PylonMarketplaceStoredIntake,
  type PylonMarketplaceStoredTriageAction,
} from './pylon-marketplace-service'

class MemoryMarketplaceStore implements PylonMarketplaceJobStore {
  readonly assignments: Array<PylonMarketplaceStoredAssignment> = []
  readonly intakes: Array<PylonMarketplaceStoredIntake> = []
  readonly triageActions: Array<PylonMarketplaceStoredTriageAction> = []

  insertAssignment = async (
    assignment: PylonMarketplaceStoredAssignment,
  ): Promise<void> => {
    this.assignments.push(assignment)
  }

  insertIntake = async (
    intake: PylonMarketplaceStoredIntake,
  ): Promise<void> => {
    this.intakes.push(intake)
  }

  insertTriageAction = async (
    action: PylonMarketplaceStoredTriageAction,
  ): Promise<void> => {
    this.triageActions.push(action)
  }

  listAssignments = async (): Promise<
    ReadonlyArray<PylonMarketplaceStoredAssignment>
  > => this.assignments

  listIntakes = async (): Promise<
    ReadonlyArray<PylonMarketplaceStoredIntake>
  > => this.intakes

  readIntakeByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<PylonMarketplaceStoredIntake | null> =>
    this.intakes.find(intake => intake.idempotencyKey === idempotencyKey) ??
      null

  readIntakeByRef = async (
    intakeRef: string,
  ): Promise<PylonMarketplaceStoredIntake | null> =>
    this.intakes.find(intake => intake.intakeRef === intakeRef) ?? null

  readTriageActionByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<PylonMarketplaceStoredTriageAction | null> =>
    this.triageActions.find(action =>
      action.idempotencyKey === idempotencyKey
    ) ?? null

  updateIntake = async (intake: PylonMarketplaceStoredIntake): Promise<void> => {
    const index = this.intakes.findIndex(row =>
      row.intakeRef === intake.intakeRef
    )
    if (index === -1) {
      this.intakes.push(intake)
      return
    }
    this.intakes[index] = intake
  }
}

const intake: PylonMarketplaceStoredIntake = {
  createdAtIso: '2026-06-07T06:30:00.000Z',
  idempotencyKey: 'intake-idem-1',
  intakeRef: 'intake.public.test.gepa',
  jobRef: 'job.public.test.gepa',
  record: {
    jobKind: 'coding_task',
    privacyClass: 'public',
    source: 'openagents_seeded',
  } as unknown as PylonMarketplaceStoredIntake['record'],
  requestHash: 'hash-intake',
  state: 'open',
  updatedAtIso: '2026-06-07T06:30:00.000Z',
}

const assignment: PylonMarketplaceStoredAssignment = {
  assignmentRef: 'assignment.public.test.gepa',
  createdAtIso: '2026-06-07T06:31:00.000Z',
  idempotencyKey: 'assignment-idem-1',
  intakeRef: intake.intakeRef,
  jobRef: intake.jobRef,
  payoutState: 'not_payable',
  record: {
    assignmentRef: 'assignment.public.test.gepa',
  } as unknown as PylonMarketplaceStoredAssignment['record'],
  requestHash: 'hash-assignment',
  state: 'proposed',
  updatedAtIso: '2026-06-07T06:31:00.000Z',
}

const triageAction: PylonMarketplaceStoredTriageAction = {
  createdAtIso: '2026-06-07T06:32:00.000Z',
  idempotencyKey: 'triage-idem-1',
  outcome: 'proposed_assignment',
  requestHash: 'hash-triage',
  response: {
    idempotent: false,
    liveDispatchAllowed: false,
  } as unknown as PylonMarketplaceStoredTriageAction['response'],
  targetIntakeRef: intake.intakeRef,
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

describe('Pylon marketplace dual-write store', () => {
  test('mirrors marketplace writes while reads stay D1-authoritative', async () => {
    const d1 = new MemoryMarketplaceStore()
    const postgres = new MemoryMarketplaceStore()
    const store = makeDualWritePylonMarketplaceJobStore({
      d1,
      flags: { dualWrite: true },
      postgres,
    })

    await store.insertIntake(intake)
    await store.insertAssignment(assignment)
    await store.insertTriageAction(triageAction)
    await store.updateIntake({ ...intake, state: 'triaged' })

    expect(d1.intakes).toEqual([{ ...intake, state: 'triaged' }])
    expect(d1.assignments).toEqual([assignment])
    expect(d1.triageActions).toEqual([triageAction])
    expect(postgres.intakes).toEqual([{ ...intake, state: 'triaged' }])
    expect(postgres.assignments).toEqual([assignment])
    expect(postgres.triageActions).toEqual([triageAction])
    expect(await store.listIntakes(10)).toEqual(d1.intakes)
  })

  test('Postgres marketplace mirror failures are fail-soft diagnostics', async () => {
    const d1 = new MemoryMarketplaceStore()
    const sink = makeLogSink()
    const store = makeDualWritePylonMarketplaceJobStore({
      d1,
      flags: { dualWrite: true },
      log: sink.log,
      postgres: {
        insertAssignment: () => Promise.reject(new Error('pg down')),
        insertIntake: () => Promise.reject(new Error('pg down')),
        insertTriageAction: () => Promise.reject(new Error('pg down')),
        updateIntake: () => Promise.reject(new Error('pg down')),
      },
    })

    await expect(store.insertIntake(intake)).resolves.toBeUndefined()
    await expect(store.insertAssignment(assignment)).resolves.toBeUndefined()
    await expect(store.insertTriageAction(triageAction)).resolves.toBeUndefined()
    await expect(
      store.updateIntake({ ...intake, state: 'triaged' }),
    ).resolves.toBeUndefined()

    expect(d1.intakes).toEqual([{ ...intake, state: 'triaged' }])
    expect(d1.assignments).toEqual([assignment])
    expect(d1.triageActions).toEqual([triageAction])
    expect(sink.events.map(event => event.fields.op)).toEqual([
      'insertMarketplaceIntake',
      'insertMarketplaceAssignment',
      'insertMarketplaceTriageAction',
      'updateMarketplaceIntake',
    ])
    expect(sink.events[0]?.fields.refs).toEqual([
      intake.intakeRef,
      intake.jobRef,
    ])
  })
})

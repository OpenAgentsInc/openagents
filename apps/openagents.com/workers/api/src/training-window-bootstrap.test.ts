import { describe, expect, it } from 'vitest'

import { buildPylonJoinLifecycleRecord } from './pylon-join-lifecycle'
import type {
  TrainingRunRecord,
  TrainingWindowRecord,
  TrainingWindowSealMetadata,
} from './training-run-window-authority'
import {
  TrainingBootstrapQueuedReasonCode,
  applyPylonJoinLifecycleTransitionUnderSealBarrier,
  decideTrainingWindowBootstrapGrant,
  selectLastDurableSealWindow,
  validateTrainingWindowBootstrapAcceptance,
} from './training-window-bootstrap'

const nowIso = '2026-06-12T10:00:00.000Z'

const makeRun = (
  overrides: Partial<TrainingRunRecord> = {},
): TrainingRunRecord => ({
  createdAt: nowIso,
  id: 'run-1',
  manifest: null,
  maxAllowedStale: 5,
  promiseRef: 'promise.training.4673',
  publicProjectionJson: '{}',
  receiptRefs: [],
  sealInFlightAt: null,
  sealPublicationCadenceWindows: 1,
  sourceRefs: [],
  state: 'active',
  trainingRunRef: 'training.run.4673',
  updatedAt: nowIso,
  ...overrides,
})

const sealMetadata = (
  checkpointDigestRef: string | undefined,
  windowRef = 'training.window.1',
): TrainingWindowSealMetadata => ({
  ...(checkpointDigestRef === undefined
    ? {}
    : {
        checkpointDigestRef,
        durableCheckpointSeal: {
          checkpointDigestRef,
          readbackReceipt: {
            objectKey: `checkpoints/${checkpointDigestRef}`,
            readbackDigestRef: checkpointDigestRef,
            receiptRef: `receipt.training.checkpoint_readback.${windowRef}`,
            sizeBytes: 1_048_576,
            storeClass: 'r2',
            storedDigestRef: checkpointDigestRef,
          },
          replicationFactor: 2,
          retrievalProofRef: `receipt.training.checkpoint_readback.${windowRef}`,
          retrievalVerified: true,
          sizeBytes: 1_048_576,
          storageClass: 'content_addressed_object_store',
          windowRef,
        },
      }),
  churn: {
    joinCount: 0,
    lossCount: 0,
    standbyPromotionCount: 0,
  },
  staleness: {
    contributionCount: 0,
    stepsBehindMax: 0,
    stepsBehindMin: 0,
    stepsBehindP50: 0,
    stepsBehindP90: 0,
  },
  verificationOverhead: {
    fraction: 0.2,
    ladderRungRef: 'ladder.rung.r1',
  },
})

const makeWindow = (
  overrides: Partial<TrainingWindowRecord> = {},
): TrainingWindowRecord => ({
  activatedAt: null,
  datasetRefs: [],
  homeworkKind: 'auto_starter',
  id: 'window-1',
  plannedAt: nowIso,
  priority: 0,
  publicProjectionJson: '{}',
  receiptRefs: [],
  reconciledAt: null,
  sealMetadata: null,
  sealedAt: null,
  sourceRefs: [],
  state: 'planned',
  trainingRunRef: 'training.run.4673',
  updatedAt: nowIso,
  windowRef: 'training.window.1',
  ...overrides,
})

const sealedWindow = (
  windowRef: string,
  sealedAt: string,
  checkpointDigestRef: string | undefined,
): TrainingWindowRecord =>
  makeWindow({
    sealMetadata: sealMetadata(checkpointDigestRef, windowRef),
    sealedAt,
    state: 'sealed',
    windowRef,
  })

describe('selectLastDurableSealWindow', () => {
  it('picks the most recently sealed window that carries a checkpoint digest', () => {
    const older = sealedWindow(
      'training.window.older',
      '2026-06-12T08:00:00.000Z',
      `sha256:${'6'.repeat(64)}`,
    )
    const newest = sealedWindow(
      'training.window.newest',
      '2026-06-12T09:30:00.000Z',
      `sha256:${'7'.repeat(64)}`,
    )

    expect(
      selectLastDurableSealWindow([older, newest])?.windowRef,
    ).toBe('training.window.newest')
  })

  it('skips sealed windows without a durably stored checkpoint digest', () => {
    const digestless = sealedWindow(
      'training.window.digestless',
      '2026-06-12T09:45:00.000Z',
      undefined,
    )
    const durable = sealedWindow(
      'training.window.durable',
      '2026-06-12T08:00:00.000Z',
      `sha256:${'8'.repeat(64)}`,
    )

    expect(
      selectLastDurableSealWindow([digestless, durable])?.windowRef,
    ).toBe('training.window.durable')
  })

  it('skips legacy digest-only seals and failed durability descriptors', () => {
    const legacyDigestOnly = makeWindow({
      sealMetadata: {
        ...sealMetadata(undefined),
        checkpointDigestRef: `sha256:${'a'.repeat(64)}`,
      },
      sealedAt: '2026-06-12T09:45:00.000Z',
      state: 'sealed',
      windowRef: 'training.window.legacy',
    })
    const failedDurability = makeWindow({
      sealMetadata: {
        ...sealMetadata(`sha256:${'b'.repeat(64)}`, 'training.window.failed'),
        durableCheckpointSeal: {
          checkpointDigestRef: `sha256:${'b'.repeat(64)}`,
          replicationFactor: 1,
          retrievalVerified: true,
          sizeBytes: 1_048_576,
          storageClass: 'content_addressed_object_store',
          windowRef: 'training.window.failed',
        },
      },
      sealedAt: '2026-06-12T09:50:00.000Z',
      state: 'sealed',
      windowRef: 'training.window.failed',
    })
    const durable = sealedWindow(
      'training.window.durable',
      '2026-06-12T08:00:00.000Z',
      `sha256:${'c'.repeat(64)}`,
    )

    expect(
      selectLastDurableSealWindow([legacyDigestOnly, failedDurability, durable])
        ?.windowRef,
    ).toBe('training.window.durable')
    expect(
      selectLastDurableSealWindow([legacyDigestOnly, failedDurability]),
    ).toBeUndefined()
  })

  it('counts reconciled windows as durable seals but never active or planned ones', () => {
    const reconciled = makeWindow({
      reconciledAt: '2026-06-12T09:00:00.000Z',
      sealMetadata: sealMetadata(
        `sha256:${'d'.repeat(64)}`,
        'training.window.reconciled',
      ),
      sealedAt: '2026-06-12T08:30:00.000Z',
      state: 'reconciled',
      windowRef: 'training.window.reconciled',
    })
    const active = makeWindow({
      activatedAt: '2026-06-12T09:50:00.000Z',
      state: 'active',
      windowRef: 'training.window.active',
    })

    expect(
      selectLastDurableSealWindow([active, reconciled])?.windowRef,
    ).toBe('training.window.reconciled')
    expect(selectLastDurableSealWindow([active, makeWindow()])).toBeUndefined()
  })
})

describe('decideTrainingWindowBootstrapGrant', () => {
  it('grants the last durable seal and pins its checkpoint digest', () => {
    const outcome = decideTrainingWindowBootstrapGrant({
      joinerReceiptRefs: [
        'receipt.joiner.qualification',
        'receipt.joiner.qualification',
      ],
      joinerRef: 'pylon.joiner.1',
      makeId: () => '0001',
      requestedAtIso: nowIso,
      run: makeRun(),
      windows: [
        sealedWindow(
          'training.window.older',
          '2026-06-12T08:00:00.000Z',
          `sha256:${'1'.repeat(64)}`,
        ),
        sealedWindow(
          'training.window.newest',
          '2026-06-12T09:30:00.000Z',
          `sha256:${'2'.repeat(64)}`,
        ),
      ],
    })

    expect(outcome).toMatchObject({
      grant: {
        checkpointDigestRef: `sha256:${'2'.repeat(64)}`,
        grantRef: 'training.bootstrap.grant.0001',
        joinerReceiptRefs: ['receipt.joiner.qualification'],
        joinerRef: 'pylon.joiner.1',
        sealedWindowRef: 'training.window.newest',
        trainingRunRef: 'training.run.4673',
      },
      kind: 'granted',
    })
  })

  it('refuses with a typed reason code when no durable seal exists', () => {
    const outcome = decideTrainingWindowBootstrapGrant({
      joinerRef: 'pylon.joiner.1',
      makeId: () => '0001',
      requestedAtIso: nowIso,
      run: makeRun(),
      windows: [
        makeWindow({ state: 'active', windowRef: 'training.window.live' }),
        sealedWindow(
          'training.window.digestless',
          '2026-06-12T09:00:00.000Z',
          undefined,
        ),
      ],
    })

    expect(outcome).toMatchObject({
      kind: 'refused',
      reasonCode: 'training.bootstrap.public.no_durable_seal',
    })
  })

  it('queues with the join-lifecycle deferral reason code while a seal is in flight', () => {
    const outcome = decideTrainingWindowBootstrapGrant({
      joinerRef: 'pylon.joiner.1',
      makeId: () => '0001',
      requestedAtIso: nowIso,
      run: makeRun({ sealInFlightAt: '2026-06-12T09:59:00.000Z' }),
      windows: [
        sealedWindow(
          'training.window.durable',
          '2026-06-12T09:00:00.000Z',
          `sha256:${'3'.repeat(64)}`,
        ),
      ],
    })

    expect(outcome).toEqual({
      joinerRef: 'pylon.joiner.1',
      kind: 'queued',
      reasonCode: 'join_lifecycle.public.join_deferred_seal_in_flight',
      trainingRunRef: 'training.run.4673',
    })
  })

  it('proceeds after the barrier clears, referencing the new last durable seal', () => {
    const windows = [
      sealedWindow(
        'training.window.before',
        '2026-06-12T09:00:00.000Z',
        `sha256:${'4'.repeat(64)}`,
      ),
    ]
    const queued = decideTrainingWindowBootstrapGrant({
      joinerRef: 'pylon.joiner.1',
      makeId: () => '0001',
      requestedAtIso: nowIso,
      run: makeRun({ sealInFlightAt: '2026-06-12T09:59:00.000Z' }),
      windows,
    })
    expect(queued.kind).toBe('queued')

    const replayed = decideTrainingWindowBootstrapGrant({
      joinerRef: 'pylon.joiner.1',
      makeId: () => '0002',
      requestedAtIso: nowIso,
      run: makeRun({ sealInFlightAt: null }),
      windows: [
        ...windows,
        sealedWindow(
          'training.window.after',
          '2026-06-12T10:00:00.000Z',
          `sha256:${'5'.repeat(64)}`,
        ),
      ],
    })

    expect(replayed).toMatchObject({
      grant: {
        checkpointDigestRef: `sha256:${'5'.repeat(64)}`,
        sealedWindowRef: 'training.window.after',
      },
      kind: 'granted',
    })
  })
})

describe('validateTrainingWindowBootstrapAcceptance', () => {
  const grant = {
    checkpointDigestRef: 'digest.checkpoint.newest',
    grantRef: 'training.bootstrap.grant.0001',
    joinerReceiptRefs: [],
    joinerRef: 'pylon.joiner.1',
    sealReceiptRefs: ['receipt.training.seal'],
    sealedAtDisplay: '30m ago',
    sealedWindowRef: 'training.window.newest',
    trainingRunRef: 'training.run.4673',
  }

  it('accepts an acceptance that echoes the granted checkpoint digest exactly', () => {
    expect(
      validateTrainingWindowBootstrapAcceptance(grant, {
        checkpointDigestRef: 'digest.checkpoint.newest',
        grantRef: 'training.bootstrap.grant.0001',
        joinerRef: 'pylon.joiner.1',
      }),
    ).toEqual({
      checkpointDigestRef: 'digest.checkpoint.newest',
      grantRef: 'training.bootstrap.grant.0001',
      kind: 'accepted',
      sealedWindowRef: 'training.window.newest',
    })
  })

  it('rejects a digest echo mismatch with a typed reason code', () => {
    expect(
      validateTrainingWindowBootstrapAcceptance(grant, {
        checkpointDigestRef: 'digest.checkpoint.other',
        grantRef: 'training.bootstrap.grant.0001',
        joinerRef: 'pylon.joiner.1',
      }),
    ).toMatchObject({
      kind: 'rejected',
      reasonCode: 'training.bootstrap.public.checkpoint_digest_echo_mismatch',
    })
  })

  it('rejects grant-ref and joiner-ref mismatches with typed reason codes', () => {
    expect(
      validateTrainingWindowBootstrapAcceptance(grant, {
        checkpointDigestRef: 'digest.checkpoint.newest',
        grantRef: 'training.bootstrap.grant.9999',
        joinerRef: 'pylon.joiner.1',
      }),
    ).toMatchObject({
      kind: 'rejected',
      reasonCode: 'training.bootstrap.public.grant_ref_mismatch',
    })
    expect(
      validateTrainingWindowBootstrapAcceptance(grant, {
        checkpointDigestRef: 'digest.checkpoint.newest',
        grantRef: 'training.bootstrap.grant.0001',
        joinerRef: 'pylon.joiner.2',
      }),
    ).toMatchObject({
      kind: 'rejected',
      reasonCode: 'training.bootstrap.public.joiner_ref_mismatch',
    })
  })
})

describe('applyPylonJoinLifecycleTransitionUnderSealBarrier', () => {
  const record = buildPylonJoinLifecycleRecord({
    capacityRef: 'capacity.pylon.1',
    nowIso,
  })

  it('queues the transition with the deferral reason code while a seal is in flight', () => {
    const outcome = applyPylonJoinLifecycleTransitionUnderSealBarrier({
      eventId: 'evt-1',
      nowIso,
      reasonCode: 'join_lifecycle.public.qualification_gate_passed',
      receiptRef: 'receipt.join.qualification',
      record,
      sealInFlight: true,
      toState: 'qualified',
    })

    expect(outcome).toEqual({
      capacityRef: 'capacity.pylon.1',
      kind: 'queued',
      reasonCode: TrainingBootstrapQueuedReasonCode,
    })
  })

  it('applies the identical replayed transition once the barrier clears', () => {
    const outcome = applyPylonJoinLifecycleTransitionUnderSealBarrier({
      eventId: 'evt-1',
      nowIso,
      reasonCode: 'join_lifecycle.public.qualification_gate_passed',
      receiptRef: 'receipt.join.qualification',
      record,
      sealInFlight: false,
      toState: 'qualified',
    })

    expect(outcome).toMatchObject({
      event: {
        fromState: 'registered',
        reasonCode: 'join_lifecycle.public.qualification_gate_passed',
        toState: 'qualified',
      },
      kind: 'applied',
      record: { state: 'qualified' },
    })
  })
})

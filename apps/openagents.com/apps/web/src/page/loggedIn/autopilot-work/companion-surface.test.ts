import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeCompanionSurfaceInput,
  projectForgeCompanionSurface,
} from './companion-surface'

const baseInput = {
  generatedAt: '2026-06-18T00:40:00.000Z',
  snapshotRef: 'companion-surface-snapshot.public.work_1',
  versionRef: 'companion-surface-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge companion surface projection', () => {
  test('projects public companion evidence as refs-only non-authoritative state', () => {
    const view = projectForgeCompanionSurface({
      ...baseInput,
      entries: [
        {
          artifactRefs: ['artifact.public.work_1.summary'],
          budgetRefs: ['budget.public.work_1.status'],
          closeoutRefs: ['closeout.public.work_1.summary'],
          companionRef: 'companion.public.mobile.work_1',
          cursorRefs: ['cursor.public.event_stream.42'],
          decisionRefs: ['decision.public.review_required'],
          deliveryTierRefs: ['delivery-tier.public.lossless'],
          freshness: 'fresh',
          notificationRefs: ['notification.public.review_required'],
          pairingRefs: ['pairing.public.mobile.browser'],
          policyRefs: ['policy.public.companion.read_only'],
          progressRefs: ['progress.public.work_1.latest'],
          runRefs: ['run.public.work_1'],
          sessionRefs: ['session.public.work_1'],
          state: 'ready',
          streamRefs: ['event-stream.public.work_1'],
          surfaceRefs: ['surface.public.mobile.status'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      offline: 0,
      readOnly: 0,
      ready: 1,
      total: 1,
      waiting: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      approvalResolveAuthority: false,
      cancelRunAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      instructionQueueAuthority: false,
      interruptRunAuthority: false,
      notificationSendAuthority: false,
      offlineActionQueueAuthority: false,
      pauseRunAuthority: false,
      privateLogStreamingAuthority: false,
      publicClaimAuthority: false,
      resumeRunAuthority: false,
      sessionMutationAuthority: false,
      settlementAuthority: false,
      spawnRunAuthority: false,
      terminalOpenAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing companion surface state as empty', () => {
    const view = projectForgeCompanionSurface({
      generatedAt: '2026-06-18T00:40:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale companion evidence', () => {
    const view = projectForgeCompanionSurface({
      ...baseInput,
      entries: [
        {
          companionRef: 'companion.public.stale',
          freshness: 'stale',
          policyRefs: ['policy.public.companion.read_only'],
          state: 'read_only',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.work_1:stale-companion-evidence:companion.public.stale',
    )
  })

  test('blocks lagged companion evidence', () => {
    const view = projectForgeCompanionSurface({
      ...baseInput,
      entries: [
        {
          companionRef: 'companion.public.lagged',
          freshness: 'lagged',
          lagRefs: ['lag.public.cursor_outside_retention'],
          state: 'read_only',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.work_1:lagged-companion-evidence:companion.public.lagged',
    )
  })

  test('blocks populated companion entries without snapshot refs', () => {
    const view = projectForgeCompanionSurface({
      entries: [
        {
          companionRef: 'companion.public.no_snapshot',
          freshness: 'fresh',
          state: 'read_only',
        },
      ],
      generatedAt: '2026-06-18T00:40:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.no_snapshot:missing-companion-surface-snapshot-ref',
    )
  })

  test('blocks action refs without capability policy pairing idempotency and receipts', () => {
    const view = projectForgeCompanionSurface({
      ...baseInput,
      entries: [
        {
          actionRefs: ['action.public.approve'],
          companionRef: 'companion.public.action_missing_boundary',
          freshness: 'fresh',
          state: 'waiting',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.work_1:companion-action-boundary-missing:companion.public.action_missing_boundary',
    )
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.work_1:companion-action-receipt-missing:companion.public.action_missing_boundary',
    )
  })

  test('blocks stream refs without cursor refs', () => {
    const view = projectForgeCompanionSurface({
      ...baseInput,
      entries: [
        {
          companionRef: 'companion.public.stream_without_cursor',
          freshness: 'fresh',
          state: 'ready',
          streamRefs: ['event-stream.public.work_1'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.work_1:companion-stream-cursor-missing:companion.public.stream_without_cursor',
    )
  })

  test('omits unsafe private companion material before projection', () => {
    const view = projectForgeCompanionSurface({
      ...baseInput,
      blockerRefs: [
        'companion-blocker.public.safe',
        'raw terminal /Users/christopher/terminal.log',
      ],
      entries: [
        {
          actionRefs: ['action.public.safe', 'raw action sk-private'],
          artifactRefs: ['artifact.public.safe', 'private artifact /Users/christopher/a.md'],
          blockerRefs: ['entry-companion-blocker.public.safe'],
          budgetRefs: ['budget.public.safe'],
          capabilityRefs: ['capability.public.safe'],
          closeoutRefs: ['closeout.public.safe'],
          companionRef: 'companion.public.safe',
          cursorRefs: ['cursor.public.safe'],
          decisionRefs: ['decision.public.safe', 'raw decision /Users/christopher/d.json'],
          deliveryTierRefs: ['delivery-tier.public.lossless'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.safe'],
          lagRefs: ['lag.public.safe'],
          notificationRefs: ['notification.public.safe', 'mobile payload private token'],
          pairingRefs: ['pairing.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          progressRefs: ['progress.public.safe', 'raw progress /Users/christopher/p.log'],
          receiptRefs: ['receipt.public.safe'],
          runRefs: ['run.public.safe'],
          sessionRefs: ['session.public.safe', 'terminal session /Users/christopher/s'],
          state: 'ready',
          streamRefs: ['event-stream.public.safe'],
          surfaceRefs: ['surface.public.safe', 'https://private.example/session'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.actionRefs).toEqual(['action.public.safe'])
    expect(view.entries[0]?.artifactRefs).toEqual(['artifact.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-companion-surface-blocker:work.public.work_1:unsafe-companion-surface-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw terminal')
    expect(payload).not.toContain('raw action')
    expect(payload).not.toContain('private artifact')
    expect(payload).not.toContain('raw decision')
    expect(payload).not.toContain('mobile payload')
    expect(payload).not.toContain('raw progress')
    expect(payload).not.toContain('terminal session')
    expect(payload).not.toContain('https://')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      companionSurface: {
        entries: [
          {
            companionRef: 'companion.public.work_2',
            freshness: 'fresh',
            policyRefs: ['policy.public.work_2'],
            state: 'read_only',
          },
        ],
        snapshotRef: 'companion-surface-snapshot.public.work_2',
        versionRef: 'companion-surface-version.public.v2',
      },
      generatedAt: '2026-06-18T00:41:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeCompanionSurfaceInput(work)).toEqual({
      entries: [
        {
          companionRef: 'companion.public.work_2',
          freshness: 'fresh',
          policyRefs: ['policy.public.work_2'],
          state: 'read_only',
        },
      ],
      generatedAt: '2026-06-18T00:41:00.000Z',
      snapshotRef: 'companion-surface-snapshot.public.work_2',
      versionRef: 'companion-surface-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})

import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeStructuredEventLogInput,
  projectForgeStructuredEventLog,
} from './structured-event-log'

const baseInput = {
  eventStreamRefs: ['event-stream.public.work_1'],
  generatedAt: '2026-06-18T02:40:00.000Z',
  policyRefs: ['policy.public.event_log.redacted'],
  snapshotRef: 'structured-event-log-snapshot.public.work_1',
  versionRef: 'structured-event-log-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge structured event log projection', () => {
  test('projects public event-log evidence as refs-only non-authoritative state', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      exportRefs: ['event-export.public.support_bundle'],
      projectionRefs: ['event-projection.public.run_detail'],
      replayRefs: ['event-replay.public.deterministic'],
      retentionRefs: ['retention.public.event_log'],
      events: [
        {
          actorRefs: ['actor.public.agent'],
          correlationRefs: ['correlation.public.work_1'],
          eventKind: 'status_transition',
          eventRef: 'event.public.work_1.1',
          exportRefs: ['event-export.public.event_1'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.event_1'],
          occurredAt: '2026-06-18T02:39:00.000Z',
          parentRefs: ['event-parent.public.root'],
          payloadSchemaVersionRefs: ['schema.public.event.status_transition.v1'],
          policyRefs: ['policy.public.event_log.redacted'],
          projectionRefs: ['event-projection.public.run_detail'],
          redactionClass: 'public_safe',
          replayRefs: ['event-replay.public.event_1'],
          retentionRefs: ['retention.public.event_log'],
          runRefs: ['run.public.work_1'],
          sequence: 1,
          sequenceRef: 'event-sequence.public.work_1.1',
          serviceRefs: ['service.public.autopilot'],
          status: 'appended',
          subjectRefs: ['work-order.public.work_1'],
          timestampRefs: ['timestamp.public.event_1'],
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      events: 1,
      failed: 0,
      privateEvents: 0,
      publicEvents: 1,
      stale: 0,
      teamEvents: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      deploymentAuthority: false,
      eventAppendAuthority: false,
      eventDeleteAuthority: false,
      eventTailAuthority: false,
      exportGenerationAuthority: false,
      projectionMutationAuthority: false,
      publicClaimAuthority: false,
      replayExecutionAuthority: false,
      retentionDeletionAuthority: false,
      schemaMigrationAuthority: false,
      settlementAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing structured event-log state as empty', () => {
    const view = projectForgeStructuredEventLog({
      generatedAt: '2026-06-18T02:40:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.events).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale event evidence', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.stale',
          freshness: 'stale',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:stale-event-evidence:event.public.stale',
    )
  })

  test('blocks event sequence gaps', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.seq_1',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.seq_3',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          sequence: 3,
          status: 'appended',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:event-sequence-gap:1:3',
    )
  })

  test('blocks duplicate event sequences', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.duplicate_a',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.duplicate_b',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:duplicate-event-sequence:1',
    )
  })

  test('blocks public events without public-safe redaction', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.private_ref',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'private_ref',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:public-event-redaction-missing:event.public.private_ref',
    )
  })

  test('blocks events missing schema and idempotency refs', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      events: [
        {
          eventKind: 'tool_result',
          eventRef: 'event.public.tool_result',
          freshness: 'fresh',
          redactionClass: 'public_safe',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:event-schema-version-missing:event.public.tool_result',
    )
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:event-idempotency-missing:event.public.tool_result',
    )
  })

  test('blocks replay and export refs without policy refs', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      policyRefs: [],
      replayRefs: ['event-replay.public.no_policy'],
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.replay',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          replayRefs: ['event-replay.public.event_1'],
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:event-replay-export-policy-missing:event.public.replay',
    )
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:event-log-replay-export-policy-missing',
    )
  })

  test('blocks populated event entries without snapshot refs', () => {
    const view = projectForgeStructuredEventLog({
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.team.no_snapshot',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.team.model_stream.v1'],
          redactionClass: 'team_ref',
          sequence: 1,
          status: 'appended',
          visibility: 'team',
        },
      ],
      generatedAt: '2026-06-18T02:40:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.no_snapshot:missing-structured-event-log-snapshot-ref',
    )
  })

  test('omits unsafe private event-log material before projection', () => {
    const view = projectForgeStructuredEventLog({
      ...baseInput,
      blockerRefs: [
        'event-log-blocker.public.safe',
        'raw event /Users/christopher/event.json',
      ],
      events: [
        {
          actorRefs: ['actor.public.safe', 'customer data private'],
          eventKind: 'status_transition',
          eventRef: 'event.public.safe',
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.safe'],
          payloadSchemaVersionRefs: [
            'schema.public.status_transition.v1',
            'raw event payload /Users/christopher/payload.json',
          ],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          redactionClass: 'public_safe',
          runRefs: ['run.public.safe'],
          sequence: 1,
          serviceRefs: ['service.public.safe', 'provider payload sk-private'],
          status: 'appended',
          subjectRefs: ['subject.public.safe', 'raw prompt /Users/christopher/prompt.md'],
          visibility: 'public',
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.events[0]?.actorRefs).toEqual(['actor.public.safe'])
    expect(view.events[0]?.subjectRefs).toEqual(['subject.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-structured-event-log-blocker:work.public.work_1:unsafe-structured-event-log-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw event')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('customer data')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      eventStreamRef: 'event-stream.public.work_2',
      generatedAt: '2026-06-18T02:40:00.000Z',
      structuredEventLog: {
        eventStreamRefs: ['event-stream.public.work_2.structured'],
        events: [
          {
            eventKind: 'model_stream',
            eventRef: 'event.public.work_2.1',
            freshness: 'fresh',
            payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
            redactionClass: 'public_safe',
            sequence: 1,
            status: 'appended',
            visibility: 'public',
          },
        ],
        generatedAt: '2026-06-18T02:41:00.000Z',
        policyRefs: ['policy.public.work_2'],
        snapshotRef: 'structured-event-log-snapshot.public.work_2',
        versionRef: 'structured-event-log-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeStructuredEventLogInput(work)).toEqual({
      eventStreamRefs: ['event-stream.public.work_2.structured'],
      events: [
        {
          eventKind: 'model_stream',
          eventRef: 'event.public.work_2.1',
          freshness: 'fresh',
          payloadSchemaVersionRefs: ['schema.public.model_stream.v1'],
          redactionClass: 'public_safe',
          sequence: 1,
          status: 'appended',
          visibility: 'public',
        },
      ],
      generatedAt: '2026-06-18T02:41:00.000Z',
      policyRefs: ['policy.public.work_2'],
      snapshotRef: 'structured-event-log-snapshot.public.work_2',
      versionRef: 'structured-event-log-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})

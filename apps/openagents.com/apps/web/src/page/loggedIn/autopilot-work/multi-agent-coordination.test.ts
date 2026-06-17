import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeMultiAgentCoordinationInput,
  projectForgeMultiAgentCoordination,
} from './multi-agent-coordination'

const baseInput = {
  generatedAt: '2026-06-18T01:20:00.000Z',
  parentRunRef: 'run.public.parent',
  planRef: 'coordination-plan.public.work_1',
  snapshotRef: 'multi-agent-coordination-snapshot.public.work_1',
  versionRef: 'multi-agent-coordination-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge multi-agent coordination projection', () => {
  test('projects public multi-lane evidence as refs-only non-authoritative state', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          acceptancePolicyRefs: ['acceptance-policy.public.all_mandatory'],
          adapterRefs: ['adapter.public.pylon.local'],
          artifactRefs: ['artifact.public.lane_1.summary'],
          assignmentRefs: ['assignment.public.lane_1'],
          budgetCapRefs: ['budget-cap.public.lane_1'],
          capabilityRefs: ['capability.public.repo_read'],
          closeoutRefs: ['closeout.public.lane_1'],
          criticality: 'mandatory',
          dependencyRefs: ['dependency.public.none'],
          freshness: 'fresh',
          kind: 'local',
          laneRef: 'coordination-lane.public.lane_1',
          policyRefs: ['policy.public.lane.local'],
          receiptRefs: ['receipt.public.lane_1.assigned'],
          state: 'completed',
        },
        {
          adapterRefs: ['adapter.public.pylon.hosted'],
          assignmentRefs: ['assignment.public.lane_2'],
          capabilityRefs: ['capability.public.test_run'],
          closeoutRefs: ['closeout.public.lane_2'],
          criticality: 'optional',
          freshness: 'fresh',
          kind: 'hosted',
          laneRef: 'coordination-lane.public.lane_2',
          policyRefs: ['policy.public.lane.hosted'],
          providerRefs: ['provider.public.hosted_pylon'],
          receiptRefs: ['receipt.public.lane_2.provider'],
          state: 'completed',
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      completed: 2,
      failedMandatory: 0,
      mandatory: 1,
      running: 0,
      total: 2,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      artifactMergeAuthority: false,
      assignmentMutationAuthority: false,
      deploymentAuthority: false,
      laneCancelAuthority: false,
      laneInboxAuthority: false,
      lanePauseAuthority: false,
      laneResumeAuthority: false,
      laneStartAuthority: false,
      marketProviderSelectionAuthority: false,
      planningAuthority: false,
      publicClaimAuthority: false,
      settlementAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing coordination state as empty', () => {
    const view = projectForgeMultiAgentCoordination({
      generatedAt: '2026-06-18T01:20:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale lane evidence', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          criticality: 'mandatory',
          freshness: 'stale',
          kind: 'local',
          laneRef: 'coordination-lane.public.stale',
          state: 'completed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:stale-lane-evidence:coordination-lane.public.stale',
    )
  })

  test('blocks active lanes without assignment capability and policy refs', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          criticality: 'mandatory',
          freshness: 'fresh',
          kind: 'local',
          laneRef: 'coordination-lane.public.active_missing_refs',
          state: 'running',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:active-lane-readiness-missing:coordination-lane.public.active_missing_refs',
    )
  })

  test('blocks failed mandatory lanes without closeout refs', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          assignmentRefs: ['assignment.public.failed'],
          capabilityRefs: ['capability.public.failed'],
          criticality: 'mandatory',
          freshness: 'fresh',
          kind: 'local',
          laneRef: 'coordination-lane.public.failed',
          policyRefs: ['policy.public.failed'],
          state: 'failed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:mandatory-lane-failed-without-closeout:coordination-lane.public.failed',
    )
  })

  test('blocks provider lanes without provider policy and receipt refs', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          assignmentRefs: ['assignment.public.market'],
          capabilityRefs: ['capability.public.market'],
          criticality: 'optional',
          freshness: 'fresh',
          kind: 'market',
          laneRef: 'coordination-lane.public.market',
          state: 'running',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:provider-lane-receipt-missing:coordination-lane.public.market',
    )
  })

  test('blocks conflict refs without merge strategy and policy refs', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          assignmentRefs: ['assignment.public.conflict'],
          capabilityRefs: ['capability.public.conflict'],
          conflictRefs: ['conflict.public.diff_overlap'],
          criticality: 'mandatory',
          freshness: 'fresh',
          kind: 'local',
          laneRef: 'coordination-lane.public.conflict',
          policyRefs: [],
          state: 'completed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:conflict-merge-policy-missing:coordination-lane.public.conflict',
    )
  })

  test('blocks lane inbox refs without steering receipts', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      entries: [
        {
          assignmentRefs: ['assignment.public.inbox'],
          capabilityRefs: ['capability.public.inbox'],
          criticality: 'optional',
          freshness: 'fresh',
          inboxRefs: ['lane-inbox.public.context_update'],
          kind: 'pylon',
          laneRef: 'coordination-lane.public.inbox',
          policyRefs: ['policy.public.lane_inbox'],
          state: 'running',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:lane-inbox-steering-receipt-missing:coordination-lane.public.inbox',
    )
  })

  test('blocks populated coordination entries without snapshot refs', () => {
    const view = projectForgeMultiAgentCoordination({
      entries: [
        {
          criticality: 'mandatory',
          freshness: 'fresh',
          kind: 'local',
          laneRef: 'coordination-lane.public.no_snapshot',
          state: 'completed',
        },
      ],
      generatedAt: '2026-06-18T01:20:00.000Z',
      planRef: 'coordination-plan.public.no_snapshot',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.no_snapshot:missing-multi-agent-coordination-snapshot-ref',
    )
  })

  test('omits unsafe private coordination material before projection', () => {
    const view = projectForgeMultiAgentCoordination({
      ...baseInput,
      blockerRefs: [
        'coordination-blocker.public.safe',
        'raw lane /Users/christopher/lane.log',
      ],
      entries: [
        {
          acceptancePolicyRefs: ['acceptance-policy.public.safe'],
          adapterRefs: ['adapter.public.safe'],
          artifactRefs: ['artifact.public.safe', 'raw artifact /Users/christopher/a.diff'],
          assignmentRefs: ['assignment.public.safe'],
          blockerRefs: ['lane-blocker.public.safe'],
          budgetCapRefs: ['budget-cap.public.safe'],
          capabilityRefs: ['capability.public.safe'],
          closeoutRefs: ['closeout.public.safe'],
          conflictRefs: ['conflict.public.safe'],
          criticality: 'mandatory',
          dependencyRefs: ['dependency.public.safe'],
          freshness: 'fresh',
          inboxRefs: ['lane-inbox.public.safe', 'raw message sk-private'],
          kind: 'local',
          laneRef: 'coordination-lane.public.safe',
          mergeStrategyRefs: ['merge-strategy.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          providerRefs: ['provider.public.safe', 'provider payload sk-private'],
          receiptRefs: ['receipt.public.safe'],
          state: 'completed',
          steeringReceiptRefs: ['steering-receipt.public.safe'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.artifactRefs).toEqual(['artifact.public.safe'])
    expect(view.entries[0]?.providerRefs).toEqual(['provider.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-multi-agent-coordination-blocker:work.public.work_1:unsafe-multi-agent-coordination-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw lane')
    expect(payload).not.toContain('raw artifact')
    expect(payload).not.toContain('raw message')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T01:21:00.000Z',
      multiAgentCoordination: {
        entries: [
          {
            criticality: 'mandatory',
            freshness: 'fresh',
            kind: 'local',
            laneRef: 'coordination-lane.public.work_2',
            state: 'completed',
          },
        ],
        parentRunRef: 'run.public.parent_2',
        planRef: 'coordination-plan.public.work_2',
        snapshotRef: 'multi-agent-coordination-snapshot.public.work_2',
        versionRef: 'multi-agent-coordination-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeMultiAgentCoordinationInput(work)).toEqual({
      entries: [
        {
          criticality: 'mandatory',
          freshness: 'fresh',
          kind: 'local',
          laneRef: 'coordination-lane.public.work_2',
          state: 'completed',
        },
      ],
      generatedAt: '2026-06-18T01:21:00.000Z',
      parentRunRef: 'run.public.parent_2',
      planRef: 'coordination-plan.public.work_2',
      snapshotRef: 'multi-agent-coordination-snapshot.public.work_2',
      versionRef: 'multi-agent-coordination-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})

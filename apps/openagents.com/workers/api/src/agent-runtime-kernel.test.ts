import { describe, expect, test } from 'vitest'
import { Schema as S } from 'effect'

import {
  AgentRuntimePublicRunProjection,
  MemoryAgentRuntimeEventRepository,
  agentRuntimeProjectionHasPrivateMaterial,
  ingestAgentRuntimeEvents,
  projectPublicAgentRuntimeRun,
  projectAgentRuntimeWorkroomStatus,
} from './agent-runtime-kernel'

const at = '2026-06-11T14:00:00.000Z'

const event = (
  runId: string,
  sequence: number,
  tag: string,
  input: Record<string, unknown> = {},
) => ({
  tag,
  eventId: `event.public.${runId}.${sequence}`,
  runId,
  sequence,
  generatedAt: at,
  visibility: 'public',
  redactionClass: 'public_ref',
  refs: [],
  blockerRefs: [],
  ...input,
})

const externalRun = (runId: string, adapterKind: 'codex' | 'claude_code') => [
  event(runId, 1, 'run.started'),
  event(runId, 2, 'external_agent.started', {
    externalInvocation: {
      invocationId: `external.public.${adapterKind}.1`,
      adapterKind,
      status: 'started',
      artifactRefs: [],
      blockerRefs: [],
    },
  }),
  event(runId, 3, 'external_agent.artifact_recorded', {
    externalInvocation: {
      invocationId: `external.public.${adapterKind}.1`,
      adapterKind,
      status: 'artifact_recorded',
      artifactRefs: [`artifact.public.${adapterKind}.closeout`],
      blockerRefs: [],
    },
  }),
  event(runId, 4, 'external_agent.completed', {
    externalInvocation: {
      invocationId: `external.public.${adapterKind}.1`,
      adapterKind,
      status: 'completed',
      artifactRefs: [`artifact.public.${adapterKind}.closeout`],
      blockerRefs: [],
    },
  }),
  event(runId, 5, 'run.completed'),
]

describe('Agent Runtime Kernel worker ingestion (RK4)', () => {
  test('ingests fixture, Codex, and Claude streams through one schema-decoded path', async () => {
    const repository = new MemoryAgentRuntimeEventRepository()
    const fixtureEvents = [
      event('run.public.fixture', 1, 'run.started'),
      event('run.public.fixture', 2, 'artifact.recorded', {
        artifact: {
          artifactRef: 'artifact.public.fixture.closeout',
          artifactKind: 'fixture',
          visibility: 'public',
        },
      }),
      event('run.public.fixture', 3, 'run.completed'),
    ]
    const codexEvents = externalRun('run.public.codex', 'codex')
    const claudeEvents = externalRun('run.public.claude', 'claude_code')

    await ingestAgentRuntimeEvents(repository, fixtureEvents)
    await ingestAgentRuntimeEvents(repository, codexEvents)
    await ingestAgentRuntimeEvents(repository, claudeEvents)

    const fixtureProjection = await projectPublicAgentRuntimeRun(
      repository,
      'run.public.fixture',
      at,
    )
    const codexProjection = await projectPublicAgentRuntimeRun(
      repository,
      'run.public.codex',
      at,
    )
    const claudeProjection = await projectPublicAgentRuntimeRun(
      repository,
      'run.public.claude',
      at,
    )

    expect(fixtureProjection).toMatchObject({
      state: 'completed',
      artifactRefs: ['artifact.public.fixture.closeout'],
    })
    expect(codexProjection.artifactRefs).toEqual(['artifact.public.codex.closeout'])
    expect(claudeProjection.artifactRefs).toEqual(['artifact.public.claude_code.closeout'])
    for (const projection of [fixtureProjection, codexProjection, claudeProjection]) {
      expect(() =>
        S.decodeUnknownSync(AgentRuntimePublicRunProjection)(projection),
      ).not.toThrow()
      expect(projection.generatedAt).toBe(at)
      expect(projection.staleness).toMatchObject({
        composition: 'rebuilt_on_transition',
        maxStalenessSeconds: 0,
      })
      expect(projection.authority).toEqual({
        acceptedWorkAuthority: false,
        payoutAuthority: false,
        publicClaimAuthority: false,
      })
    }
  })

  test('rejects malformed, duplicate, non-append, and public-unsafe events before persistence', async () => {
    const repository = new MemoryAgentRuntimeEventRepository()
    await ingestAgentRuntimeEvents(repository, [event('run.public.rejects', 1, 'run.started')])

    await expect(ingestAgentRuntimeEvents(repository, [
      event('run.public.rejects', 1, 'run.started'),
    ])).rejects.toThrow('already persisted')
    await expect(ingestAgentRuntimeEvents(repository, [
      event('run.public.rejects', 3, 'run.completed'),
    ])).rejects.toThrow('must append at 2')
    await expect(ingestAgentRuntimeEvents(repository, [
      { nope: true },
    ])).rejects.toThrow()
    await expect(ingestAgentRuntimeEvents(repository, [
      event('run.public.unsafe', 1, 'model.text_delta', {
        summary: 'raw_prompt: /Users/private/source',
      }),
    ])).rejects.toThrow('raw/private material')

    expect(await repository.eventsForRun('run.public.rejects')).toHaveLength(1)
    expect(await repository.eventsForRun('run.public.unsafe')).toHaveLength(0)
  })

  test('projects only public events while documenting the visibility split', async () => {
    const repository = new MemoryAgentRuntimeEventRepository()
    await ingestAgentRuntimeEvents(repository, [
      event('run.public.visibility', 1, 'run.started'),
      {
        ...event('run.public.visibility', 2, 'external_agent.event'),
        visibility: 'operator',
        redactionClass: 'operator_summary',
        summary: 'operator-only summary ref',
      },
      event('run.public.visibility', 3, 'run.completed'),
    ])

    const projection = await projectPublicAgentRuntimeRun(
      repository,
      'run.public.visibility',
      at,
    )

    expect(projection.eventCount).toBe(2)
    expect(projection.visibilitySplit).toEqual({
      storedEventVisibilities: ['public', 'operator'],
      projectedVisibility: 'public',
    })
    expect(agentRuntimeProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('projects the web workroom status row from the same public run projection', async () => {
    const repository = new MemoryAgentRuntimeEventRepository()
    await ingestAgentRuntimeEvents(repository, [
      event('run.public.workroom', 1, 'run.started'),
      event('run.public.workroom', 2, 'run.interrupted', {
        blockerRefs: ['blocker.agent_runtime.openagents_native.budget_stop'],
      }),
      event('run.public.workroom', 3, 'run.failed', {
        blockerRefs: ['blocker.agent_runtime.openagents_native.budget_stop'],
      }),
    ])

    const projection = await projectPublicAgentRuntimeRun(
      repository,
      'run.public.workroom',
      at,
    )
    const row = projectAgentRuntimeWorkroomStatus(projection)

    expect(row).toMatchObject({
      runId: 'run.public.workroom',
      status: 'failed',
      label: 'Failed',
      generatedAt: at,
      eventCount: 3,
      freshness: {
        generatedAt: at,
        maxStalenessSeconds: 0,
        transitionRefs: [
          'agent_runtime_event_ingested',
          'agent_runtime_run_state_transition',
        ],
      },
      blockerRefs: ['blocker.agent_runtime.openagents_native.budget_stop'],
      reviewActionRefs: [
        'review.public.agent_runtime.blocker.agent_runtime.openagents_native.budget_stop',
      ],
    })
  })
})

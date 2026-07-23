import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { CrmMcpCatalog } from './crm-mcp-routes'
import { makeSarahRuntimeTools } from './sarah-runtime-tools'

const sql = (() => Promise.resolve([])) as never
const env = { fixture: true }

const catalog = (
  calls: Array<Readonly<{ name: string; args: unknown }>>,
): CrmMcpCatalog<typeof env> => ({
  callTool: (_env, _request, _principal, name, args) => {
    calls.push({ args, name })
    if (name === 'khala.spawn') {
      const count =
        typeof (args as { count?: unknown }).count === 'number'
          ? (args as { count: number }).count
          : 2
      return Promise.resolve({
        content: [
          {
            text: JSON.stringify({ assignedCount: count, ok: true }),
            type: 'text',
          },
        ],
        structuredContent: {
          assignedCount: count,
          blockerRefs: [],
          children: Array.from({ length: count }, (_value, index) => ({
              assignmentRef: `assignment.fixture.${index + 1}`,
              ok: true,
              workerRef: `worker.fixture.${index + 1}`,
          })),
          ok: true,
          requestedCount: count,
          spawnRef: 'spawn.public.khala_coding.fixture',
        },
      })
    }
    return Promise.resolve({ content: [{ text: '{}', type: 'text' }] })
  },
  listResources: () => Promise.resolve([]),
  listTools: () => Promise.resolve([]),
  readResource: () => Promise.reject(new Error('unused')),
})

const authority = () =>
  Effect.succeed({
    allowed: true,
    receiptRef: 'receipt.authority.sarah.fixture',
  })

const projection = (state: 'running' | 'paused' = 'running') => ({
  observe: () =>
    Effect.succeed({
      projection: {
        generatedAt: '2026-07-18T20:00:00.000Z',
        privateMaterialExcluded: true as const,
        run: {
          accountRef: 'codex-local',
          doneCondition: 'All checks pass.',
          failedAttempts: 0,
          laneRef: 'codex-local',
          lastTransition: {
            actor: 'owner_ui' as const,
            at: '2026-07-18T20:00:00.000Z',
          },
          lifecycleState: state,
          objective: 'Implement Full Auto.',
          receiptSummary: null,
          rotationCount: 0,
          runRef: 'full_auto_run.fixture',
          startedAt: '2026-07-18T19:00:00.000Z',
          successfulAttempts: 2,
          threadRef: 'thread.fixture',
          turnCap: 20,
          updatedAt: '2026-07-18T20:00:00.000Z',
          workspaceLabel: 'openagents',
        },
        schema: 'full_auto_run.mobile_projection.v1' as const,
      },
    }),
  publish: () => Effect.die('unused'),
})

const control = (dispatched: Array<unknown>) => ({
  dispatch: (input: unknown) => {
    dispatched.push(input)
    return Effect.succeed({
      action: 'pause' as const,
      appliedAt: null,
      createdAt: '2026-07-18T20:00:01.000Z',
      idempotencyKey: 'idempotency.sarah.full_auto.turn.fixture.call.control',
      intentId: 'intent.sarah.full_auto.turn.fixture.call.control',
      rejectionReason: null,
      resultLifecycleState: null,
      runRef: 'full_auto_run.fixture',
      schema: 'full_auto_run.control_intent.v1' as const,
      status: 'pending' as const,
      surface: 'mobile' as const,
    })
  },
  list: () => Effect.succeed([]),
  reportOutcome: () => Effect.die('unused'),
})

const call = (name: string) => ({
  function: { arguments: '{}', name },
  id: `call.${name}`,
  type: 'function' as const,
})

describe('Sarah runtime tools', () => {
  test('queues a live Agent Computer turn before consulting Pylon fallback capacity', async () => {
    const calls: Array<Readonly<{ name: string; args: unknown }>> = []
    const cloudDispatches: Array<unknown> = []
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      dispatchCloudCoding: dispatchInput => {
        cloudDispatches.push(dispatchInput)
        return Effect.succeed({
          cloudTurnRef: 'turn.sarah_cloud.fixture',
          dispatchRef: 'dispatch.sarah_cloud.fixture',
          threadRef: 'thread.sarah_cloud.fixture',
          workContextRef: 'work_context.thread.sarah_cloud.fixture',
        })
      },
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog(calls),
      ownerUserId: 'owner.fixture',
      probeCloudCodingCapacity: () =>
        Promise.resolve({
          available: true,
          availableSlots: 1,
          capacityRef: 'capacity.agent_computer.control_plane.live',
        }),
      resolveRepositoryCommit: () => Promise.resolve('a'.repeat(40)),
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_start',
    )!

    const result = await Effect.runPromise(
      tool.execute(
        {
          count: 1,
          objective: 'Run issue #9191 on owned Agent Computer capacity.',
        },
        call('codex_workers_start'),
      ),
    )

    expect(result).toMatchObject({
      isError: false,
      summary:
        'Queued one live OpenAgents Agent Computer turn and started 1 of 1 requested coding worker.',
    })
    expect(cloudDispatches).toEqual([
      expect.objectContaining({
        commit: 'a'.repeat(40),
        repository: 'OpenAgentsInc/openagents',
      }),
    ])
    expect(calls.filter(entry => entry.name === 'khala.spawn')).toHaveLength(0)
    expect(result.resultRefs).toContain('turn.sarah_cloud.fixture')
  })

  test('falls back from unavailable Agent Computer capacity to Codex and then Claude Pylons', async () => {
    const calls: Array<Readonly<{ name: string; args: unknown }>> = []
    const fallbackCatalog: CrmMcpCatalog<typeof env> = {
      ...catalog([]),
      callTool: (_env, _request, _principal, name, args) => {
        calls.push({ args, name })
        const workflow = (args as { workflow?: string }).workflow
        if (name !== 'khala.spawn') {
          return Promise.resolve({
            content: [{ text: '{}', type: 'text' }],
          })
        }
        if (workflow === 'codex_agent_task') {
          return Promise.resolve({
            content: [{ text: '{"ok":false}', type: 'text' }],
            isError: true,
            structuredContent: {
              assignedCount: 0,
              blockerRefs: ['blocker.fixture.codex_unavailable'],
              children: [],
              requestedCount: 1,
              spawnRef: 'spawn.fixture.codex',
            },
          })
        }
        return Promise.resolve({
          content: [{ text: '{"ok":true}', type: 'text' }],
          structuredContent: {
            assignedCount: 1,
            blockerRefs: [],
            children: [
              {
                assignmentRef: 'assignment.fixture.claude',
                ok: true,
                workerRef: 'worker.fixture.claude',
              },
            ],
            requestedCount: 1,
            spawnRef: 'spawn.fixture.claude',
          },
        })
      },
    }
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: fallbackCatalog,
      ownerUserId: 'owner.fixture',
      probeCloudCodingCapacity: () =>
        Promise.resolve({
          available: false,
          availableSlots: 0,
          capacityRef:
            'capacity.agent_computer.control_plane.unavailable',
        }),
      resolveRepositoryCommit: () => Promise.resolve('a'.repeat(40)),
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_start',
    )!

    const result = await Effect.runPromise(
      tool.execute(
        { count: 1, objective: 'Run the fallback fixture.' },
        call('codex_workers_start'),
      ),
    )

    expect(
      calls
        .filter(entry => entry.name === 'khala.spawn')
        .map(entry => (entry.args as { workflow: string }).workflow),
    ).toEqual(['codex_agent_task', 'claude_agent_task'])
    expect(result).toMatchObject({
      isError: false,
      summary:
        'Started 1 of 1 requested coding worker through owner-linked fallback capacity.',
    })
    expect(result.resultRefs).toContain(
      'capacity.agent_computer.control_plane.unavailable',
    )
    expect(result.resultRefs).toContain('assignment.fixture.claude')
  })

  test('dispatches bounded Codex workers through the real owner-capacity broker with a pinned commit', async () => {
    const calls: Array<Readonly<{ name: string; args: unknown }>> = []
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog(calls),
      ownerUserId: 'owner.fixture',
      resolveRepositoryCommit: () => Promise.resolve('a'.repeat(40)),
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_start',
    )
    expect(tool).toBeDefined()
    const result = await Effect.runPromise(
      tool!.execute(
        {
          count: 2,
          maxParallel: 2,
          objective: 'Close the admitted Full Auto sprint.',
        },
        call('codex_workers_start'),
      ),
    )
    expect(result).toMatchObject({
      isError: false,
      summary:
        'Started 2 of 2 requested coding workers through owner-linked fallback capacity.',
    })
    expect(calls).toEqual([
      {
        args: expect.objectContaining({
          branch: 'main',
          commit: 'a'.repeat(40),
          count: 2,
          repo: 'OpenAgentsInc/openagents',
          verify: 'pnpm run check',
          workflow: 'codex_agent_task',
        }),
        name: 'khala.spawn',
      },
    ])
  })

  test('queues a Desktop-owned Full Auto control intent and reports pending honestly', async () => {
    const dispatched: Array<unknown> = []
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control(dispatched),
      fullAutoProjection: projection('running'),
      khalaCatalog: catalog([]),
      ownerUserId: 'owner.fixture',
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'full_auto_control',
    )
    const result = await Effect.runPromise(
      tool!.execute(
        { action: 'pause' },
        { ...call('full_auto_control'), id: 'call.control' },
      ),
    )
    expect(result.summary).toContain('Desktop has not applied it yet')
    expect(dispatched).toEqual([
      expect.objectContaining({
        ownerUserId: 'owner.fixture',
        request: expect.objectContaining({
          action: 'pause',
          runRef: 'full_auto_run.fixture',
        }),
      }),
    ])
  })

  test('does not call a target broker when exact Sarah authority is refused', async () => {
    const calls: Array<Readonly<{ name: string; args: unknown }>> = []
    const tools = makeSarahRuntimeTools({
      authorizeOperation: () =>
        Effect.succeed({
          allowed: false,
          receiptRef: 'receipt.authority.sarah.refused',
          refusalReason: 'action_not_granted',
        }),
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog(calls),
      ownerUserId: 'owner.fixture',
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_capacity',
    )
    const result = await Effect.runPromise(
      tool!.execute({}, call('codex_workers_capacity')),
    )
    expect(result).toMatchObject({ isError: true })
    expect(calls).toHaveLength(0)
  })

  test('never reserves more than eight worker dispatches in one Sarah turn', async () => {
    const calls: Array<Readonly<{ name: string; args: unknown }>> = []
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog(calls),
      ownerUserId: 'owner.fixture',
      resolveRepositoryCommit: () => Promise.resolve('b'.repeat(40)),
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_start',
    )!
    await Effect.runPromise(
      tool.execute(
        { count: 5, objective: 'First bounded worker packet.' },
        { ...call('codex_workers_start'), id: 'call.first' },
      ),
    )
    const refused = await Effect.runPromise(
      tool.execute(
        { count: 4, objective: 'Second bounded worker packet.' },
        { ...call('codex_workers_start'), id: 'call.second' },
      ),
    )
    expect(refused).toMatchObject({ isError: true })
    expect(refused.resultRefs).toContain(
      'blocker.sarah.codex_workers.turn_limit',
    )
    expect(calls.filter(entry => entry.name === 'khala.spawn')).toHaveLength(1)
  })

  test('submits terminal history to the independent harness gate and reports the released next-turn bundle', async () => {
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog([]),
      ownerUserId: 'owner.fixture',
      reviewHarness: () =>
        Effect.succeed({
          bundleDigest: `sha256:${'b'.repeat(64)}`,
          bundleRef: `harness.bundle.sarah.${'b'.repeat(24)}`,
          evaluation: {
            approved: true,
            privacyScore: 1,
            qualityScore: 0.9,
            rationale: 'Held-out owner feedback is better served.',
            regressionScore: 0.9,
            safetyScore: 1,
          },
          experienceCount: 12,
          heldOutExperienceCount: 3,
          latestReviewRef: `review.sarah.harness.${'c'.repeat(24)}`,
          latestReviewState: 'released',
          policy: {
            conversationInstructions: ['Be direct.'],
            dimensions: {
              contextAssembly: 'context',
              generationControl: 'generation',
              memoryManagement: 'memory',
              orchestration: 'orchestration',
              outputProcessing: 'output',
              toolInteraction: 'tools',
            },
            maxReplyWords: 80,
            schema: 'openagents.sarah.harness_policy.v1',
          },
          reviewRef: `review.sarah.harness.${'c'.repeat(24)}`,
          state: 'released',
          summary: 'More conversational and transparent.',
          trainingExperienceCount: 9,
        }),
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'sarah_harness_review_history',
    )
    const result = await Effect.runPromise(
      tool!.execute({}, call('sarah_harness_review_history')),
    )
    expect(result.summary).toContain('independent gate released')
    expect(result.resultRefs).toEqual([
      `review.sarah.harness.${'c'.repeat(24)}`,
      `harness.bundle.sarah.${'b'.repeat(24)}`,
    ])
  })

  // SARAH-PROACTIVE-1 (#9064)
  test('codex_workers_start records the (ownerUserId, threadRef) dispatch mapping for each admitted assignment', async () => {
    const inserts: Array<Readonly<{ text: string; values: ReadonlyArray<unknown> }>> = []
    const mappingSql = ((
      strings: TemplateStringsArray,
      ...values: Array<unknown>
    ) => {
      inserts.push({ text: strings.join('?'), values })
      return Promise.resolve([])
    }) as never
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog([]),
      ownerUserId: 'owner.fixture',
      resolveRepositoryCommit: () => Promise.resolve('a'.repeat(40)),
      sql: mappingSql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_start',
    )!
    await Effect.runPromise(
      tool.execute(
        { count: 2, objective: 'Dispatch mapping fixture packet.' },
        call('codex_workers_start'),
      ),
    )
    const mappingInserts = inserts.filter(entry =>
      entry.text.includes('sarah_worker_dispatch_mappings'),
    )
    expect(mappingInserts).toHaveLength(2)
    expect(mappingInserts.map(entry => entry.values)).toEqual([
      ['assignment.fixture.1', 'owner.fixture', 'thread.sarah.fixture', expect.any(String)],
      ['assignment.fixture.2', 'owner.fixture', 'thread.sarah.fixture', expect.any(String)],
    ])
  })

  const webCommsTools = () =>
    makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog([]),
      ownerUserId: 'owner.fixture',
      sql,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    }).find(item => item.definition.name === 'sarah_web_comms')!

  test('sarah_web_comms drafts a public-safe blog post with no delivery by default', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'draft',
          body: 'We shipped the owner-orchestrator reboot.',
          kind: 'blog',
          title: 'Sarah takes company command',
        },
        call('sarah_web_comms'),
      ),
    )
    expect(result.isError ?? false).toBe(false)
    const payload = JSON.parse(result.content) as {
      draft: { kind: string; title: string }
      delivery?: unknown
    }
    expect(payload.draft.kind).toBe('blog')
    expect(payload.delivery).toBeUndefined()
    expect(result.resultRefs).toContain('sarah.web_comms.blog_draft_ready')
  })

  test('sarah_web_comms returns a repository-delivery handoff when a blog draft asks to deliver', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'draft',
          body: 'Body text for the delivered document.',
          deliver: true,
          kind: 'document',
          title: 'Company command runbook',
        },
        call('sarah_web_comms'),
      ),
    )
    const payload = JSON.parse(result.content) as {
      delivery?: { channel: string; path: string; repository: string }
    }
    expect(payload.delivery).toBeDefined()
    expect(payload.delivery!.repository).toBe('OpenAgentsInc/openagents')
    expect(payload.delivery!.channel).toBe('repository_delivery')
    expect(payload.delivery!.path).toBe('docs/company-command-runbook.md')
    expect(result.resultRefs).toContain(
      'sarah.web_comms.blog_or_document_delivery_intent',
    )
  })

  test('sarah_web_comms drafts a forum post with no delivery handoff', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'draft',
          body: 'A loose product-promise report for the forum.',
          deliver: true,
          kind: 'forum',
          title: 'Promise gap report',
        },
        call('sarah_web_comms'),
      ),
    )
    const payload = JSON.parse(result.content) as { delivery?: unknown }
    expect(payload.delivery).toBeUndefined()
    expect(result.resultRefs).toContain('sarah.web_comms.forum_draft_ready')
  })

  test('sarah_web_comms queues a timeline post for owner review through the tweet queue', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'publish_outward',
          body: 'The releases keep moving. — an AI agent',
          channel: 'timeline',
          title: 'Weekly status',
        },
        call('sarah_web_comms'),
      ),
    )
    expect(result.isError ?? false).toBe(false)
    const payload = JSON.parse(result.content) as {
      queued: boolean
      channel: string
      delivery: { path: string; mode: string }
    }
    expect(payload.queued).toBe(true)
    expect(payload.channel).toBe('timeline')
    expect(payload.delivery.path).toBe('docs/sarah/SARAH_TWEET_QUEUE.md')
    expect(payload.delivery.mode).toBe('append')
    expect(result.resultRefs).toContain(
      'sarah.web_comms.queued_for_owner_review',
    )
  })

  test('sarah_web_comms drafts a Nostr post on the open channel without refusing', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'publish_outward',
          body: 'Live on Nostr through the owned relay.',
          channel: 'nostr',
          title: 'Nostr note',
        },
        call('sarah_web_comms'),
      ),
    )
    expect(result.isError ?? false).toBe(false)
    const payload = JSON.parse(result.content) as {
      channel: string
      delivery: { channel: string }
    }
    expect(payload.channel).toBe('nostr')
    expect(payload.delivery.channel).toBe('repository_delivery')
    expect(result.resultRefs).toContain('sarah.web_comms.nostr_draft_ready')
  })

  test('sarah_web_comms refuses animated-spoken publication until admission', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'publish_outward',
          body: 'This would be spoken by the avatar.',
          channel: 'animated_spoken',
          title: 'Spoken update',
        },
        call('sarah_web_comms'),
      ),
    )
    expect(result.isError).toBe(true)
    const payload = JSON.parse(result.content) as {
      error: string
      reason: string
    }
    expect(payload.error).toBe('web_comms_runtime_unavailable')
    expect(payload.reason).toBe('refuse_until_admission')
    expect(result.resultRefs).toContain(
      'blocker.sarah.web_comms.runtime_unavailable',
    )
  })

  test('sarah_web_comms rejects a body that contains secret-shaped material', async () => {
    const result = await Effect.runPromise(
      webCommsTools().execute(
        {
          action: 'draft',
          body: 'Here is the access_token: ghp_abcdefgh12345678 for the deploy.',
          kind: 'blog',
          title: 'Oops leaked a token',
        },
        call('sarah_web_comms'),
      ),
    )
    expect(result.isError).toBe(true)
    expect(result.resultRefs).toContain(
      'blocker.sarah.web_comms.unsafe_material',
    )
  })

  test('a failing dispatch-mapping write never fails codex_workers_start', async () => {
    const tools = makeSarahRuntimeTools({
      authorizeOperation: authority,
      env,
      fullAutoControl: control([]),
      fullAutoProjection: projection(),
      khalaCatalog: catalog([]),
      ownerUserId: 'owner.fixture',
      resolveRepositoryCommit: () => Promise.resolve('a'.repeat(40)),
      sql: (() => Promise.reject(new Error('postgres unavailable'))) as never,
      threadRef: 'thread.sarah.fixture',
      turnId: 'turn.fixture',
    })
    const tool = tools.find(
      item => item.definition.name === 'codex_workers_start',
    )!
    const result = await Effect.runPromise(
      tool.execute(
        { count: 1, objective: 'Fail-soft mapping write fixture.' },
        call('codex_workers_start'),
      ),
    )
    expect(result).toMatchObject({ isError: false })
  })
})

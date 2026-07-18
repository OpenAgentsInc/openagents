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
      return Promise.resolve({
        content: [
          {
            text: JSON.stringify({ assignedCount: 2, ok: true }),
            type: 'text',
          },
        ],
        structuredContent: {
          assignedCount: 2,
          blockerRefs: [],
          children: [
            {
              assignmentRef: 'assignment.fixture.1',
              ok: true,
              workerRef: 'worker.fixture.1',
            },
            {
              assignmentRef: 'assignment.fixture.2',
              ok: true,
              workerRef: 'worker.fixture.2',
            },
          ],
          ok: true,
          requestedCount: 2,
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
      summary: 'Started 2 of 2 requested Codex workers.',
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
})

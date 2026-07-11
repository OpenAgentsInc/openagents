import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'
import { decodeRuntimeInteraction } from '@openagentsinc/khala-sync'
import type { MutatorRegistry, SyncSql } from '@openagentsinc/khala-sync-server'
import {
  KHALA_SYNC_RUNTIME_INTERACTION_PATH,
  handleKhalaSyncRuntimeInteraction,
} from './khala-sync-runtime-interaction-routes'

const pending = decodeRuntimeInteraction({
  schema: 'openagents.runtime_interaction.v1',
  interactionRef: 'interaction.tool.1',
  threadId: 'thread.runtime.1',
  turnId: 'turn.runtime.1',
  requestedSequence: 3,
  requestedAt: '2026-07-11T22:00:00.000Z',
  expiresAt: '2026-07-11T22:05:00.000Z',
  source: { lane: 'claude_pylon', adapterKind: 'claude_code', surface: 'server' },
  visibility: 'private',
  redactionClass: 'private_ref',
  causalityRefs: ['event.runtime.2'],
  payload: {
    kind: 'tool_approval',
    displayText: 'Allow workspace write?',
    toolCallId: 'tool.call.1',
    toolName: 'workspaceWrite',
    authority: {
      authorityRef: 'authority.tool.1', policyRef: 'policy.tool.1',
      decisionRef: 'decision.pending.1', toolRef: 'tool.workspace.write',
      status: 'operator_escalation_required', allowed: false,
      blockerRefs: ['blocker.owner_approval'],
    },
  },
  lifecycle: { status: 'pending' },
})

const dependencies = (overrides: Record<string, unknown> = {}) => ({
  binding: { connectionString: 'postgresql://private' },
  registry: {} as MutatorRegistry,
  requireOperator: () => Promise.resolve(true),
  makeSqlClient: async () => ({ sql: {} as SyncSql, end: async () => {} }),
  ...overrides,
})

describe('trusted Pylon runtime interaction route', () => {
  test('executes the real named mutator as the explicit owner', async () => {
    const calls: Array<any> = []
    const response = await Effect.runPromise(handleKhalaSyncRuntimeInteraction(
      new Request(`https://openagents.com${KHALA_SYNC_RUNTIME_INTERACTION_PATH}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ ownerUserId: 'user.1', interaction: pending }),
      }),
      dependencies({
        executeMutation: async (input: any) => {
          calls.push(input)
          return { results: [{ mutationId: 1, status: 'applied' }] }
        },
      }),
    ))
    expect(response.status).toBe(200)
    expect(calls[0].userId).toBe('user.1')
    expect(calls[0].request.mutations[0].name).toBe('runtime.requestInteraction')
    expect(JSON.parse(calls[0].request.mutations[0].argsJson)).toMatchObject({ interactionRef: 'interaction.tool.1' })
  })

  test('reads only the exact owner/ref post-image', async () => {
    const reads: Array<unknown> = []
    const response = await Effect.runPromise(handleKhalaSyncRuntimeInteraction(
      new Request(`https://openagents.com${KHALA_SYNC_RUNTIME_INTERACTION_PATH}?ownerUserId=user.1&interactionRef=interaction.tool.1`),
      dependencies({
        readInteraction: async (_sql: SyncSql, input: unknown) => {
          reads.push(input)
          return { interaction: { ...pending, lifecycle: { status: 'expired', terminalAt: '2026-07-11T22:05:00.000Z', reasonRef: 'reason.deadline' } } }
        },
      }),
    ))
    expect(response.status).toBe(200)
    expect(reads).toEqual([{ ownerUserId: 'user.1', interactionRef: 'interaction.tool.1' }])
    expect(await response.json()).toMatchObject({ ok: true, interaction: { lifecycle: { status: 'expired' } } })
  })

  test('fails closed before storage for unauthorized or malformed requests', async () => {
    const unauthorized = await Effect.runPromise(handleKhalaSyncRuntimeInteraction(
      new Request(`https://openagents.com${KHALA_SYNC_RUNTIME_INTERACTION_PATH}`),
      dependencies({ requireOperator: () => Promise.resolve(false) }),
    ))
    expect(unauthorized.status).toBe(401)
    const malformed = await Effect.runPromise(handleKhalaSyncRuntimeInteraction(
      new Request(`https://openagents.com${KHALA_SYNC_RUNTIME_INTERACTION_PATH}?ownerUserId=bad/value&interactionRef=x`),
      dependencies(),
    ))
    expect(malformed.status).toBe(400)
  })
})

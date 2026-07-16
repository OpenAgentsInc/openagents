import type { OmniEventRecord } from './omni-runs'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ShareAccessService,
  type ShareProjectionRecord,
  type ShareViewer,
  audienceLabel,
  workroomPartFromEvent,
} from './share-projections'

const now = '2026-06-04T21:00:00.000Z'

const owner: ShareViewer = {
  email: 'owner@openagents.com',
  name: 'Owner',
  userId: 'github:owner',
}

const recipient: ShareViewer = {
  email: 'teammate@openagents.com',
  name: 'Teammate',
  userId: 'github:teammate',
}

const stranger: ShareViewer = {
  email: 'stranger@example.com',
  name: 'Stranger',
  userId: 'github:stranger',
}

const publicAudience = { _tag: 'Public' as const }

const teamAudience = {
  _tag: 'TeamMembers' as const,
  teamId: 'team_openagents_core',
  teamName: 'OpenAgents Core Team',
}

const usersAudience = {
  _tag: 'Users' as const,
  recipients: [
    {
      displayName: 'Teammate',
      email: recipient.email,
      userId: recipient.userId,
    },
  ],
}

const shareRecord = (
  audience: ShareProjectionRecord['audience'],
  overrides: Partial<ShareProjectionRecord> = {},
): ShareProjectionRecord => {
  const projection = {
    schemaVersion: 'openagents.share_projection.v1' as const,
    id: '123e4567-e89b-42d3-a456-426614174000',
    url: 'https://openagents.com/share/123e4567-e89b-42d3-a456-426614174000',
    audience,
    audienceLabel: audienceLabel(audience),
    title: 'Shared run',
    subtitle: 'openagents/autopilot-omega@main · completed',
    source: { kind: 'agent-run' as const, id: 'run_1' },
    status: 'active' as const,
    createdAt: now,
    updatedAt: now,
    messages: [],
    files: [],
    artifacts: [],
    approvals: [],
    receipts: [],
    metrics: {
      eventCount: 0,
      tokenTotal: 0,
      toolCallCount: 0,
    },
  }

  return {
    audience,
    canonicalUrl: projection.url,
    createdAt: now,
    expiresAt: null,
    id: projection.id,
    ownerUserId: owner.userId,
    projectId: null,
    projection,
    redactionPolicyId: 'default',
    revokedAt: null,
    source: projection.source,
    status: 'active',
    summary: null,
    teamId: null,
    title: projection.title,
    updatedAt: now,
    ...overrides,
  }
}

const authorizeView = (record: ShareProjectionRecord, viewer?: ShareViewer) =>
  Effect.gen(function* () {
    const access = yield* ShareAccessService

    return yield* access.authorizeView(
      viewer === undefined
        ? { db: {} as D1Database, record }
        : { db: {} as D1Database, record, viewer },
    )
  }).pipe(
    Effect.match({
      onFailure: left => ({ _tag: 'Left' as const, left }),
      onSuccess: right => ({ _tag: 'Right' as const, right }),
    }),
    Effect.provide(ShareAccessService.layer),
  )

describe('share projection audience labels', () => {
  test('formats public, team, and direct-recipient labels', () => {
    expect(audienceLabel(publicAudience)).toBe('Shared publicly')
    expect(audienceLabel(teamAudience)).toBe(
      'Shared with members of OpenAgents Core Team',
    )
    expect(audienceLabel(usersAudience)).toBe('Shared with Teammate')
    expect(audienceLabel(usersAudience, recipient)).toBe('Shared with you')
  })
})

describe('share projection access', () => {
  test('allows public shares without a signed-in viewer', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(publicAudience)),
    )

    expect(result._tag).toBe('Right')
    if (result._tag !== 'Right') {
      throw new Error('Expected public share authorization to succeed.')
    }
    expect(result.right.audienceLabel).toBe('Shared publicly')
  })

  test('requires auth before direct-recipient share access', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(usersAudience)),
    )

    expect(result._tag).toBe('Left')
    if (result._tag !== 'Left') {
      throw new Error('Expected direct-recipient share authorization to fail.')
    }
    expect(result.left._tag).toBe('ShareProjectionAuthenticationRequired')
  })

  test('labels matching direct-recipient shares as shared with you', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(usersAudience), recipient),
    )

    expect(result._tag).toBe('Right')
    if (result._tag !== 'Right') {
      throw new Error('Expected matching recipient authorization to succeed.')
    }
    expect(result.right.audienceLabel).toBe('Shared with you')
  })

  test('denies non-recipient direct share access', async () => {
    const result = await Effect.runPromise(
      authorizeView(shareRecord(usersAudience), stranger),
    )

    expect(result._tag).toBe('Left')
    if (result._tag !== 'Left') {
      throw new Error('Expected stranger authorization to fail.')
    }
    expect(result.left._tag).toBe('ShareProjectionForbidden')
  })

  test('marks revoked records before returning a projection', async () => {
    const result = await Effect.runPromise(
      authorizeView(
        shareRecord(publicAudience, {
          revokedAt: now,
          status: 'revoked',
        }),
      ),
    )

    expect(result._tag).toBe('Right')
    if (result._tag !== 'Right') {
      throw new Error('Expected revoked public projection to be returned.')
    }
    expect(result.right.status).toBe('revoked')
  })
})

// ---------------------------------------------------------------------------
// T14 (#8871): widened `WorkroomTimelinePart` classification.
// ---------------------------------------------------------------------------

const eventFixture = (
  overrides: Partial<OmniEventRecord> = {},
): OmniEventRecord => ({
  id: 'evt_1',
  parentId: 'run_1',
  sequence: 1,
  type: 'runner.event',
  summary: 'Runner event received.',
  status: null,
  source: 'openagents',
  payloadJson: null,
  artifactRefs: [],
  externalEventId: null,
  createdAt: '2026-07-16T00:00:00.000Z',
  ...overrides,
})

const withPayload = (payload: unknown): string => JSON.stringify(payload)

describe('workroomPartFromEvent classification', () => {
  test('classifies a Codex commandExecution payload as a command part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'commandExecution',
        status: 'completed',
        payloadJson: withPayload({
          type: 'commandExecution',
          command: 'pnpm test',
          cwd: '/work/openagents',
          exitCode: 0,
          durationMs: 1234,
          aggregatedOutput: 'ok\nall tests passed',
        }),
      }),
    )

    expect(part.kind).toBe('command')
    if (part.kind !== 'command') throw new Error('expected command part')
    expect(part.command).toBe('pnpm test')
    expect(part.cwd).toBe('/work/openagents')
    expect(part.exitCode).toBe(0)
    expect(part.durationMs).toBe(1234)
    expect(part.outputTail).toBe('ok\nall tests passed')
    expect(part.status).toBe('completed')
  })

  test('classifies a Claude SDK Bash tool_use block as a command part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'tool_use',
        payloadJson: withPayload({
          type: 'tool_use',
          name: 'Bash',
          input: { command: 'ls -la', cwd: '/tmp' },
        }),
      }),
    )

    expect(part.kind).toBe('command')
    if (part.kind !== 'command') throw new Error('expected command part')
    expect(part.command).toBe('ls -la')
    expect(part.cwd).toBe('/tmp')
  })

  test('classifies a Codex fileChange payload as a fileChange part with adds/dels', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'fileChange',
        status: 'completed',
        payloadJson: withPayload({
          type: 'fileChange',
          changes: [
            {
              path: 'src/retry.ts',
              kind: 'update',
              diff: '@@ -1,2 +1,3 @@\n-old\n+new\n+extra',
            },
          ],
        }),
      }),
    )

    expect(part.kind).toBe('fileChange')
    if (part.kind !== 'fileChange') throw new Error('expected fileChange part')
    expect(part.changes).toHaveLength(1)
    expect(part.changes[0]?.path).toBe('src/retry.ts')
    expect(part.changes[0]?.adds).toBe(2)
    expect(part.changes[0]?.dels).toBe(1)
  })

  test('classifies a Claude SDK Write tool_use block as a single-file fileChange part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'tool_use',
        payloadJson: withPayload({
          type: 'tool_use',
          name: 'Write',
          input: { file_path: 'src/new.ts', content: 'export const x = 1' },
        }),
      }),
    )

    expect(part.kind).toBe('fileChange')
    if (part.kind !== 'fileChange') throw new Error('expected fileChange part')
    expect(part.changes[0]?.path).toBe('src/new.ts')
    expect(part.changes[0]?.kind).toBe('add')
  })

  test('classifies an mcpToolCall payload as a toolCall part with args and result', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'mcpToolCall',
        status: 'completed',
        payloadJson: withPayload({
          type: 'mcpToolCall',
          server: 'github',
          tool: 'search_issues',
          arguments: { query: 'is:open' },
          result: { content: [{ text: '3 results' }] },
        }),
      }),
    )

    expect(part.kind).toBe('toolCall')
    if (part.kind !== 'toolCall') throw new Error('expected toolCall part')
    expect(part.callKind).toBe('mcp')
    expect(part.server).toBe('github')
    expect(part.tool).toBe('search_issues')
    expect(part.args).toEqual([{ key: 'query', value: 'is:open' }])
    expect(part.resultSnippet).toBe('3 results')
  })

  test('classifies a namespaced mcp__ Claude SDK tool_use block as an mcp toolCall part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'tool_use',
        payloadJson: withPayload({
          type: 'tool_use',
          name: 'mcp__github__search_issues',
          input: { query: 'is:open' },
        }),
      }),
    )

    expect(part.kind).toBe('toolCall')
    if (part.kind !== 'toolCall') throw new Error('expected toolCall part')
    expect(part.callKind).toBe('mcp')
    expect(part.server).toBe('github')
    expect(part.tool).toBe('search_issues')
  })

  test('classifies a generic Claude SDK tool_use block as a dynamic toolCall part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'tool_use',
        payloadJson: withPayload({
          type: 'tool_use',
          name: 'Grep',
          input: { pattern: 'TODO' },
        }),
      }),
    )

    expect(part.kind).toBe('toolCall')
    if (part.kind !== 'toolCall') throw new Error('expected toolCall part')
    expect(part.callKind).toBe('dynamic')
    expect(part.tool).toBe('Grep')
    expect(part.args).toEqual([{ key: 'pattern', value: 'TODO' }])
  })

  test('classifies a webSearch payload as a web toolCall part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'webSearch',
        status: 'completed',
        payloadJson: withPayload({
          type: 'webSearch',
          query: 'tanstack start routing',
          results: [{}, {}],
        }),
      }),
    )

    expect(part.kind).toBe('toolCall')
    if (part.kind !== 'toolCall') throw new Error('expected toolCall part')
    expect(part.callKind).toBe('web')
    expect(part.query).toBe('tanstack start routing')
    expect(part.resultCount).toBe(2)
  })

  test('classifies a reasoning payload as a reasoning part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'reasoning',
        payloadJson: withPayload({
          type: 'reasoning',
          summary: ['Traced the failing test', 'Found the root cause'],
        }),
      }),
    )

    expect(part.kind).toBe('reasoning')
    if (part.kind !== 'reasoning') throw new Error('expected reasoning part')
    expect(part.summary).toBe('Traced the failing test\nFound the root cause')
  })

  test('classifies a plan payload as a plan part with entries', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'plan',
        payloadJson: withPayload({
          type: 'plan',
          entries: [
            { step: 'Trace the bug', status: 'completed' },
            { step: 'Write the fix', status: 'in_progress' },
          ],
        }),
      }),
    )

    expect(part.kind).toBe('plan')
    if (part.kind !== 'plan') throw new Error('expected plan part')
    expect(part.entries).toEqual([
      { step: 'Trace the bug', status: 'completed' },
      { step: 'Write the fix', status: 'in_progress' },
    ])
  })

  test('classifies an approval-typed payload as an approval part with a normalized decision', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'commandExecution.approval',
        payloadJson: withPayload({
          type: 'commandExecution.approval',
          decision: 'approved',
          reason: 'Looked safe.',
        }),
      }),
    )

    expect(part.kind).toBe('approval')
    if (part.kind !== 'approval') throw new Error('expected approval part')
    expect(part.decision).toBe('approved')
    expect(part.detail).toBe('Looked safe.')
  })

  test('classifies a collabAgentToolCall payload as an agent part with per-child status', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'collabAgentToolCall',
        payloadJson: withPayload({
          type: 'collabAgentToolCall',
          tool: 'spawnAgent',
          prompt: 'Investigate the flaky test.',
          agentsStates: { child_1: { status: 'running' } },
        }),
      }),
    )

    expect(part.kind).toBe('agent')
    if (part.kind !== 'agent') throw new Error('expected agent part')
    expect(part.tool).toBe('spawnAgent')
    expect(part.children).toEqual([{ threadRef: 'child_1', status: 'running' }])
  })

  test('classifies a contextCompaction payload as a compaction part', () => {
    const part = workroomPartFromEvent(
      eventFixture({ type: 'contextCompaction', payloadJson: withPayload({ type: 'contextCompaction' }) }),
    )

    expect(part).toEqual({ kind: 'compaction' })
  })

  test('classifies a warning-typed event as a notice part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'runner.warning',
        summary: 'Model rerouted to a fallback.',
      }),
    )

    expect(part.kind).toBe('notice')
    if (part.kind !== 'notice') throw new Error('expected notice part')
    expect(part.severity).toBe('warning')
    expect(part.text).toBe('Model rerouted to a fallback.')
  })

  test('classifies a token-usage event as a meter part', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'thread.tokenUsage.updated',
        payloadJson: withPayload({
          usage: { inputTokens: 100, outputTokens: 40, totalTokens: 140 },
        }),
      }),
    )

    expect(part.kind).toBe('meter')
    if (part.kind !== 'meter') throw new Error('expected meter part')
    expect(part.inputTokens).toBe(100)
    expect(part.outputTokens).toBe(40)
    expect(part.totalTokens).toBe(140)
  })

  test('falls back to the generic tool part for an unrecognized event shape', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'some.unrecognized.event',
        summary: 'Something happened.',
        status: 'completed',
      }),
    )

    expect(part.kind).toBe('tool')
    if (part.kind !== 'tool') throw new Error('expected tool part')
    expect(part.title).toBe('Something happened.')
    expect(part.status).toBe('completed')
  })

  test('redacts command text containing provider secret material instead of leaking it', () => {
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'commandExecution',
        payloadJson: withPayload({
          type: 'commandExecution',
          command: 'export OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwx && pnpm run deploy',
        }),
      }),
    )

    expect(part.kind).toBe('command')
    if (part.kind !== 'command') throw new Error('expected command part')
    expect(part.command).toBe('[redacted]')
    expect(part.command).not.toContain('sk-abcdefghijklmnopqrstuvwx')
  })

  test('bounds and tail-truncates command output while flagging the cap', () => {
    const longOutput = `${'x'.repeat(4_100)}TAIL_MARKER`
    const part = workroomPartFromEvent(
      eventFixture({
        type: 'commandExecution',
        payloadJson: withPayload({
          type: 'commandExecution',
          command: 'pnpm run build',
          aggregatedOutput: longOutput,
        }),
      }),
    )

    expect(part.kind).toBe('command')
    if (part.kind !== 'command') throw new Error('expected command part')
    expect(part.outputTail?.length).toBeLessThanOrEqual(4_000)
    expect(part.outputTail?.endsWith('TAIL_MARKER')).toBe(true)
    expect(part.outputCapReached).toBe(true)
  })
})

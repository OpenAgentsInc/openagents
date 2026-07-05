// #8410 follow-up — agent-scope delegation e2e verification.
//
// Proves the fix for "agent identities can't write into a human owner's own
// Khala Sync thread": a chat thread/runtime turn created by a human's own
// browser/mobile session is owned by their userId
// (`khala_sync_chat_threads.owner_user_id` /
// `khala_sync_runtime_turns.owner_user_id`). An agent bearer (e.g. a Pylon's
// own registered credential) authenticates to a SEPARATE agent-user identity
// (`agent.user.id`), never the same id as any human — so before this fix,
// `ctx.userId` for an agent push was always the agent's own id, which could
// never match a human-owned thread's owner, and `ensureScopeOwner` correctly
// (and still correctly) rejected it as a foreign scope.
//
// `resolveKhalaSyncActorUserId` (`./index.ts`) resolves an agent actor to its
// LINKED human owner (`agent.credential.openauthUserId`, populated only by
// the owner-approved claim/link flow) when one exists, falling back to the
// agent's own identity otherwise. This suite drives the REAL
// `handleKhalaSyncPush` route (real Postgres, real migrations, the real
// production mutator registry) to prove:
//
//   1. A human creates a chat thread and starts a runtime turn as themselves.
//   2. An agent LINKED to that same human can post into that human's thread/
//      turn scope (the exact Pylon dispatch-consumer scenario).
//   3. An agent linked to a DIFFERENT human is still rejected — delegation
//      never widens to an arbitrary owner, only the one the link approves.
//   4. An UNLINKED agent (no `openauthUserId`) is still rejected — it falls
//      back to its own scope, exactly as before this change.
//
// Gated on local Postgres binaries (initdb/pg_ctl); machines without them
// skip instead of fail.

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { Effect } from 'effect'
import postgres from 'postgres'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import { KHALA_SYNC_PROTOCOL_VERSION } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  type LocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'

import { resolveKhalaSyncActorUserId, type AuthenticatedActor } from './index'
import { makeKhalaSyncWorkerMutatorRegistry } from './khala-sync-mutators'
import {
  handleKhalaSyncPush,
  KHALA_SYNC_PUSH_PATH,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'

const MIGRATIONS_DIR = join(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations',
)

const registry = makeKhalaSyncWorkerMutatorRegistry()

const iso = '2026-07-05T12:00:00.000Z'

// -----------------------------------------------------------------------
// Actor fixtures
// -----------------------------------------------------------------------

const humanActor = (userId: string): AuthenticatedActor => ({
  kind: 'human',
  user: {
    avatarUrl: '',
    email: `${userId}@example.com`,
    login: userId,
    name: userId,
    provider: 'github',
    userId,
  },
})

const agentActor = (input: {
  agentUserId: string
  openauthUserId?: string | null
}): AuthenticatedActor => ({
  agent: {
    credential: {
      id: `agent_credential.${input.agentUserId}`,
      lastUsedAt: iso,
      ...('openauthUserId' in input
        ? { openauthUserId: input.openauthUserId }
        : {}),
      profileMetadataJson: '{}',
      tokenPrefix: `oa_agent_${input.agentUserId}`,
    },
    user: {
      avatarUrl: null,
      createdAt: iso,
      displayName: `Test agent ${input.agentUserId}`,
      id: input.agentUserId,
      kind: 'agent',
      primaryEmail: null,
      status: 'active',
      updatedAt: iso,
    },
  },
  kind: 'agent',
})

// -----------------------------------------------------------------------
// Pure resolveKhalaSyncActorUserId unit coverage
// -----------------------------------------------------------------------

describe('resolveKhalaSyncActorUserId', () => {
  test('a human actor resolves to their own userId', () => {
    expect(resolveKhalaSyncActorUserId(humanActor('user-h1'))).toBe('user-h1')
  })

  test('an agent actor linked to a human owner resolves to that owner, not its own agent-user id', () => {
    expect(
      resolveKhalaSyncActorUserId(
        agentActor({ agentUserId: 'agent-1', openauthUserId: 'user-h1' }),
      ),
    ).toBe('user-h1')
  })

  test('an agent actor with no link (openauthUserId omitted) falls back to its own agent-user id', () => {
    expect(
      resolveKhalaSyncActorUserId(agentActor({ agentUserId: 'agent-1' })),
    ).toBe('agent-1')
  })

  test('an agent actor with an explicit null link falls back to its own agent-user id', () => {
    expect(
      resolveKhalaSyncActorUserId(
        agentActor({ agentUserId: 'agent-1', openauthUserId: null }),
      ),
    ).toBe('agent-1')
  })
})

// -----------------------------------------------------------------------
// Real Postgres + real push route e2e
// -----------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  '#8410 follow-up: agent-scope delegation against the real push route + real Postgres',
  () => {
    let pg: LocalPostgres
    let dbUrl: string
    let writerSql: ReturnType<typeof postgres>

    const makeRealSqlClient: MakeKhalaSyncPushSqlClient = async connectionString => {
      const sql = postgres(connectionString, {
        connect_timeout: 10,
        max: 1,
        onnotice: () => {},
        prepare: false,
      })
      return {
        end: () => sql.end({ timeout: 5 }),
        sql: sql as unknown as SyncSql,
      }
    }

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = postgres(pg.url, { max: 1, onnotice: () => {} })
      await admin.unsafe('CREATE DATABASE khala_sync_agent_delegation')
      await admin.end({ timeout: 5 })
      dbUrl = pg.urlFor('khala_sync_agent_delegation')

      const migrator = postgres(dbUrl, { max: 1, onnotice: () => {} })
      const files = readdirSync(MIGRATIONS_DIR)
        .filter(name => name.endsWith('.sql'))
        .sort()
      expect(files.length).toBeGreaterThan(0)
      for (const file of files) {
        const content = readFileSync(join(MIGRATIONS_DIR, file), 'utf8')
        await migrator.begin(async tx => {
          await tx.unsafe(content)
        })
      }
      await migrator.end({ timeout: 5 })

      writerSql = postgres(dbUrl, { max: 1, onnotice: () => {}, prepare: false })
    }, 120_000)

    afterAll(async () => {
      await writerSql?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    // Replicache-style protocol: mutationId is a PER-(clientGroupId, clientId)
    // monotonic sequence starting at 1, not a global counter — each distinct
    // actor identity below gets its own independent counter.
    const mutationCounters = new Map<string, number>()
    const nextMutationId = (clientKey: string): number => {
      const next = (mutationCounters.get(clientKey) ?? 0) + 1
      mutationCounters.set(clientKey, next)
      return next
    }

    type PushResultRow = { status: string; errorCode?: string; errorMessageSafe?: string }

    const pushAs = async (
      actor: AuthenticatedActor,
      mutations: ReadonlyArray<{ name: string; args: unknown }>,
    ): Promise<{
      status: number
      results: ReadonlyArray<PushResultRow> | undefined
      body: unknown
    }> => {
      const clientKey = actor.kind === 'human' ? actor.user.userId : actor.agent.user.id
      const body = {
        clientGroupId: `cg-8410-${clientKey}`,
        clientId: `c-8410-${clientKey}`,
        mutations: mutations.map(m => ({
          argsJson: JSON.stringify(m.args),
          mutationId: nextMutationId(clientKey),
          name: m.name,
        })),
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        schemaVersion: 1,
      }
      const response = await Effect.runPromise(
        handleKhalaSyncPush(
          new Request(`https://openagents.com${KHALA_SYNC_PUSH_PATH}`, {
            body: JSON.stringify(body),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          }),
          {
            authenticate: async () => ({ userId: resolveKhalaSyncActorUserId(actor) }),
            binding: { connectionString: dbUrl },
            makeSqlClient: makeRealSqlClient,
            registry,
          },
        ),
      )
      const parsed = (await response.json()) as {
        results?: ReadonlyArray<PushResultRow>
      }
      return { body: parsed, results: parsed.results, status: response.status }
    }

    test('a human-owned chat thread: a LINKED agent can append into it (the Pylon dispatch-consumer scenario); a differently-linked or unlinked agent cannot', async () => {
      const owner = 'user-8410-chat-owner'
      const threadId = 'thread.8410.chat-delegation'

      const created = await pushAs(humanActor(owner), [
        { args: { threadId, title: 'Delegation fixture thread' }, name: 'chat.createThread' },
      ])
      expect(created.results?.[0]?.status).toBe('applied')

      const linkedAppend = await pushAs(
        agentActor({ agentUserId: 'agent-linked', openauthUserId: owner }),
        [
          {
            args: { body: 'posted by the owner-linked agent', messageId: 'msg.8410.linked', threadId },
            name: 'chat.appendMessage',
          },
        ],
      )
      expect(linkedAppend.results?.[0]?.status).toBe('applied')

      const wrongLinkAppend = await pushAs(
        agentActor({ agentUserId: 'agent-wrong-owner', openauthUserId: 'user-8410-someone-else' }),
        [
          {
            args: { body: 'must be rejected', messageId: 'msg.8410.wrong-owner', threadId },
            name: 'chat.appendMessage',
          },
        ],
      )
      expect(wrongLinkAppend.results?.[0]?.status).toBe('rejected')
      expect(wrongLinkAppend.results?.[0]?.errorCode).toBe('unauthorized_scope')

      const unlinkedAppend = await pushAs(
        agentActor({ agentUserId: 'agent-unlinked' }),
        [
          {
            args: { body: 'must be rejected', messageId: 'msg.8410.unlinked', threadId },
            name: 'chat.appendMessage',
          },
        ],
      )
      expect(unlinkedAppend.results?.[0]?.status).toBe('rejected')
      expect(unlinkedAppend.results?.[0]?.errorCode).toBe('unauthorized_scope')

      // Sanity: the private message body from the linked agent write is
      // really there under the human owner, not silently dropped.
      const rows = await writerSql`
        SELECT author_user_id, body FROM khala_sync_chat_messages
        WHERE message_id = 'msg.8410.linked'
      `
      expect(rows).toHaveLength(1)
      expect(rows[0]?.author_user_id).toBe(owner)
      expect(rows[0]?.body).toBe('posted by the owner-linked agent')
    })

    test('a human-started runtime turn: a LINKED agent can post real runtime.recordEvent/turn.close progress into it; an unlinked agent cannot', async () => {
      const owner = 'user-8410-runtime-owner'
      const threadId = 'thread.8410.runtime-delegation'
      const turnId = 'turn.8410.runtime-delegation'

      const controlIntent = (input: {
        kind: 'turn.start' | 'turn.close'
        intentId: string
      }) => ({
        causalityRefs: [],
        createdAt: iso,
        idempotencyKey: `idem.${input.intentId}`,
        intentId: input.intentId,
        kind: input.kind,
        origin: { lane: 'khala_sync_mobile_control', surface: 'mobile' },
        redactionClass: 'private_ref',
        schema: 'openagents.khala_runtime_control_intent.v1',
        target: { adapterKind: 'codex', lane: 'codex_app_server' },
        threadId,
        turnId,
        visibility: 'private',
        ...(input.kind === 'turn.start' ? { bodyRef: 'chat_message.8410.runtime-msg' } : {}),
      })

      const started = await pushAs(humanActor(owner), [
        { args: controlIntent({ intentId: 'intent.8410.start', kind: 'turn.start' }), name: 'runtime.startTurn' },
      ])
      expect(started.results?.[0]?.status).toBe('applied')

      const linkedEvent = await pushAs(
        agentActor({ agentUserId: 'pylon-linked', openauthUserId: owner }),
        [
          {
            args: {
              causalityRefs: [],
              eventId: 'event.8410.linked',
              kind: 'turn.started',
              observedAt: iso,
              redactionClass: 'private_ref',
              schema: 'openagents.khala_runtime_event.v1',
              sequence: 1,
              source: { adapterKind: 'codex', lane: 'codex_app_server', surface: 'server' },
              threadId,
              turnId,
              visibility: 'private',
            },
            name: 'runtime.recordEvent',
          },
        ],
      )
      expect(linkedEvent.results?.[0]?.status).toBe('applied')

      const linkedClose = await pushAs(
        agentActor({ agentUserId: 'pylon-linked', openauthUserId: owner }),
        [
          { args: controlIntent({ intentId: 'intent.8410.close', kind: 'turn.close' }), name: 'runtime.closeTurn' },
        ],
      )
      expect(linkedClose.results?.[0]?.status).toBe('applied')

      const unlinkedEvent = await pushAs(
        agentActor({ agentUserId: 'pylon-unlinked' }),
        [
          {
            args: {
              causalityRefs: [],
              eventId: 'event.8410.unlinked',
              kind: 'turn.started',
              observedAt: iso,
              redactionClass: 'private_ref',
              schema: 'openagents.khala_runtime_event.v1',
              sequence: 2,
              source: { adapterKind: 'codex', lane: 'codex_app_server', surface: 'server' },
              threadId,
              turnId,
              visibility: 'private',
            },
            name: 'runtime.recordEvent',
          },
        ],
      )
      expect(unlinkedEvent.results?.[0]?.status).toBe('rejected')
      expect(unlinkedEvent.results?.[0]?.errorCode).toBe('unauthorized_scope')

      const turnRows = await writerSql`
        SELECT owner_user_id, status FROM khala_sync_runtime_turns WHERE turn_id = ${turnId}
      `
      expect(turnRows).toHaveLength(1)
      expect(turnRows[0]?.owner_user_id).toBe(owner)
      expect(turnRows[0]?.status).toBe('closed')

      const eventRows = (await writerSql`
        SELECT event_id FROM khala_sync_runtime_events WHERE turn_id = ${turnId} ORDER BY sequence ASC
      `) as unknown as ReadonlyArray<{ event_id: string }>
      expect(eventRows.map(r => r.event_id)).toEqual(['event.8410.linked'])
    })
  },
)

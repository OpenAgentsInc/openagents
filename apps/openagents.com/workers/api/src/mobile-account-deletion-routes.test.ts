/* eslint-disable @typescript-eslint/consistent-type-assertions */
import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import {
  hasMobileAccountDeletionReceipt,
  isMobileAccessTokenRevoked,
} from './auth/mobile-session'
import { makeMemoryAuthKvStore } from './auth/auth-kv'
import { makeKvOpenAuthStorage } from './auth/openauth-storage'
import {
  deleteKhalaSyncAccountDataWithSql,
  handleMobileAccountDeletionRequest,
} from './mobile-account-deletion-routes'
import {
  IDENTITY_AUTH_DOMAIN_D1_SCHEMA,
  makeSqliteD1,
} from './test/sqlite-d1'
import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'

type StoredValue = Readonly<{ expirationTtl?: number; value: string }>

const executionContext = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as never

const NOW = '2026-07-06T12:00:00.000Z'
const USER_ID = 'github:12345'
const ACCESS_TOKEN = 'opaque-access-token'

const agentBalancesSchema = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0)
);
`

const pushDeviceTokensSchema = `
CREATE TABLE push_device_tokens (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  expo_push_token TEXT NOT NULL,
  platform TEXT NOT NULL CHECK (platform IN ('ios', 'android')),
  access_token_revocation_key TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, device_id)
);
`

const makeMemoryKv = (): KVNamespace => {
  const values = new Map<string, StoredValue>()

  return {
    get: (key: string) => Promise.resolve(values.get(key)?.value ?? null),
    put: (
      key: string,
      value: string,
      options?: { expirationTtl?: number },
    ) => {
      values.set(
        key,
        options?.expirationTtl === undefined
          ? { value }
          : { expirationTtl: options.expirationTtl, value },
      )
      return Promise.resolve()
    },
  } as KVNamespace
}

const makeDb = () => {
  const sqlite = makeSqliteD1()
  sqlite.exec(IDENTITY_AUTH_DOMAIN_D1_SCHEMA)
  sqlite.exec(agentBalancesSchema)
  sqlite.exec(pushDeviceTokensSchema)
  sqlite.exec(
    `CREATE UNIQUE INDEX auth_identities_provider_subject_unique
       ON auth_identities(provider, provider_subject)`,
  )
  return sqlite
}

const seedAccountRows = async (db: D1Database) => {
  await db.batch([
    db
      .prepare(
        `INSERT INTO users
          (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
         VALUES (?, 'human', 'Octo', 'octo@example.com', '', 'active', ?, ?)`,
      )
      .bind(USER_ID, NOW, NOW),
    db
      .prepare(
        `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, provider_username, email, created_at, updated_at)
         VALUES ('auth_identity_github_12345', ?, 'github', '12345', 'octo', 'octo@example.com', ?, ?)`,
      )
      .bind(USER_ID, NOW, NOW),
    db
      .prepare(
        `INSERT INTO agent_balances
          (actor_ref, balance_msat, held_msat, usd_credit_msat, created_at, updated_at)
         VALUES (?, 120000, 20000, 100000, ?, ?)`,
      )
      .bind(`agent:${USER_ID}`, NOW, NOW),
    db
      .prepare(
        `INSERT INTO push_device_tokens
          (user_id, device_id, expo_push_token, platform, access_token_revocation_key, created_at, updated_at)
         VALUES (?, 'device-1', 'ExponentPushToken[abc]', 'ios', NULL, ?, ?)`,
      )
      .bind(USER_ID, NOW, NOW),
    db
      .prepare(
        `INSERT INTO github_write_connections
          (id, user_id, github_id, github_login, connection_ref, secret_ref, scopes_json,
           status, health, connected_at, last_status_at, created_at, updated_at)
         VALUES ('conn-1', ?, '12345', 'octo', 'github_write_connection.conn-1',
                 'secret/github/conn-1', '[]', 'connected', 'healthy', ?, ?, ?, ?)`,
      )
      .bind(USER_ID, NOW, NOW, NOW, NOW),
    db
      .prepare(
        `INSERT INTO github_write_auth_grants
          (id, connection_id, user_id, connection_ref, secret_ref, grant_ref, status,
           created_at, updated_at, expires_at)
         VALUES ('grant-1', 'conn-1', ?, 'github_write_connection.conn-1',
                 'secret/github/conn-1', 'grant-ref-1', 'issued', ?, ?, ?)`,
      )
      .bind(USER_ID, NOW, NOW, NOW),
  ])
}

const deleteAccountRequest = (body?: unknown): Request => {
  const init: RequestInit = {
    headers: {
      authorization: `Bearer ${ACCESS_TOKEN}`,
      ...(body === undefined ? {} : { 'content-type': 'application/json' }),
    },
    method: 'DELETE',
  }
  if (body !== undefined) {
    init.body = JSON.stringify(body)
  }
  return new Request('https://openagents.com/api/mobile/account', init)
}

describe('Khala mobile account deletion route', () => {
  test('deletes account-owned D1 rows, calls Khala Sync cleanup, forfeits credits, revokes bearer, and supports retry', async () => {
    const sqlite = makeDb()
    const kv = makeMemoryKv()
    const syncCalls: Array<{ binding: unknown; userId: string }> = []

    try {
      await seedAccountRows(sqlite.db)
      // CFG-3 (#8518): OpenAuth issuer state lives on the owned KvStore.
      const openAuthStorage = makeKvOpenAuthStorage(makeMemoryAuthKvStore())
      await openAuthStorage.set(['oauth:refresh', USER_ID, 'refresh-token'], {})
      const env = { AUTH_KV: kv, KHALA_SYNC_DB: { connectionString: 'postgres://khala-sync' }, OPENAGENTS_DB: sqlite.db }
      const dependencies = {
        authStorage: () => kv,
        db: () => sqlite.db,
        deleteKhalaSyncAccountData: async (binding: unknown, userId: string) => {
          syncCalls.push({ binding, userId })
          return { clientGroupsRemoved: 1, scopesRemoved: 2, threadScopesRemoved: 1 }
        },
        khalaSyncBinding: () => env.KHALA_SYNC_DB,
        nowIso: () => NOW,
        openAuthStorage: () => openAuthStorage,
        readBearerToken: () => ACCESS_TOKEN,
        requireUserBearerSession: async () => ({ user: { userId: USER_ID } }),
        userIdFromSession: (session: { user: { userId: string } }) => session.user.userId,
      }

      const response = await Effect.runPromise(
        handleMobileAccountDeletionRequest(
          dependencies,
          deleteAccountRequest({ refreshToken: `${USER_ID}:refresh-token` }),
          env,
          executionContext,
        ),
      )
      const body = (await response.json()) as {
        cleanup: {
          credits: { forfeitedBalanceMsat: number }
          github: { connectionsDisconnected: number; writeGrantsRevoked: number }
          openAuth: { refreshRevoked: boolean; storageRowsRemoved: number }
          push: { deviceTokensRemoved: number }
        }
        deleted: boolean
        ok: boolean
      }

      expect(response.status).toBe(200)
      expect(body).toMatchObject({
        cleanup: {
          credits: { forfeitedBalanceMsat: 120000 },
          github: { connectionsDisconnected: 1, writeGrantsRevoked: 1 },
          openAuth: { refreshRevoked: true, storageRowsRemoved: 1 },
          push: { deviceTokensRemoved: 1 },
        },
        deleted: true,
        ok: true,
      })
      expect(syncCalls).toEqual([{ binding: env.KHALA_SYNC_DB, userId: USER_ID }])
      await expect(isMobileAccessTokenRevoked(kv, ACCESS_TOKEN)).resolves.toBe(true)
      await expect(hasMobileAccountDeletionReceipt(kv, ACCESS_TOKEN)).resolves.toBe(true)

      const user = await sqlite.db
        .prepare(`SELECT status, deleted_at FROM users WHERE id = ?`)
        .bind(USER_ID)
        .first<{ status: string; deleted_at: string | null }>()
      expect(user).toEqual({ deleted_at: NOW, status: 'disabled' })
      const identity = await sqlite.db
        .prepare(`SELECT deleted_at FROM auth_identities WHERE user_id = ?`)
        .bind(USER_ID)
        .first<{ deleted_at: string | null }>()
      expect(identity?.deleted_at).toBe(NOW)
      const balance = await sqlite.db
        .prepare(
          `SELECT balance_msat, held_msat, usd_credit_msat
             FROM agent_balances WHERE actor_ref = ?`,
        )
        .bind(`agent:${USER_ID}`)
        .first<{ balance_msat: number; held_msat: number; usd_credit_msat: number }>()
      expect(balance).toEqual({ balance_msat: 0, held_msat: 0, usd_credit_msat: 0 })
      const pushRows = await sqlite.db
        .prepare(`SELECT COUNT(*) AS count FROM push_device_tokens WHERE user_id = ?`)
        .bind(USER_ID)
        .first<{ count: number }>()
      expect(pushRows?.count).toBe(0)
      await expect(
        openAuthStorage.get(['oauth:refresh', USER_ID, 'refresh-token']),
      ).resolves.toBeUndefined()
      const githubConnection = await sqlite.db
        .prepare(`SELECT status, health, deleted_at FROM github_write_connections WHERE id = 'conn-1'`)
        .first<{ deleted_at: string | null; health: string; status: string }>()
      expect(githubConnection).toEqual({
        deleted_at: NOW,
        health: 'requires_reauth',
        status: 'disconnected',
      })
      const grant = await sqlite.db
        .prepare(`SELECT status, revoked_at FROM github_write_auth_grants WHERE id = 'grant-1'`)
        .first<{ revoked_at: string | null; status: string }>()
      expect(grant).toEqual({ revoked_at: NOW, status: 'revoked' })

      const retryResponse = await Effect.runPromise(
        handleMobileAccountDeletionRequest(
          {
            ...dependencies,
            requireUserBearerSession: async () => undefined,
          },
          deleteAccountRequest(),
          env,
          executionContext,
        ),
      )
      expect(retryResponse.status).toBe(200)
      expect(await retryResponse.json()).toEqual({
        alreadyDeleted: true,
        deleted: true,
        ok: true,
      })
    } finally {
      sqlite.close()
    }
  })

  test('fails closed without Khala Sync storage and does not mutate local account rows', async () => {
    const sqlite = makeDb()
    const kv = makeMemoryKv()

    try {
      await seedAccountRows(sqlite.db)
      const openAuthStorage = makeKvOpenAuthStorage(makeMemoryAuthKvStore())
      await openAuthStorage.set(['oauth:refresh', USER_ID, 'refresh-token'], {})
      const env = { AUTH_KV: kv, KHALA_SYNC_DB: undefined, OPENAGENTS_DB: sqlite.db }
      const response = await Effect.runPromise(
        handleMobileAccountDeletionRequest(
          {
            authStorage: () => kv,
            db: () => sqlite.db,
            khalaSyncBinding: () => undefined,
            makeSqlClient: async () => {
              throw new Error('must not connect without a binding')
            },
            nowIso: () => NOW,
            openAuthStorage: () => openAuthStorage,
            readBearerToken: () => ACCESS_TOKEN,
            requireUserBearerSession: async () => ({ user: { userId: USER_ID } }),
            userIdFromSession: (session: { user: { userId: string } }) => session.user.userId,
          },
          deleteAccountRequest(),
          env,
          executionContext,
        ),
      )
      const pushRows = await sqlite.db
        .prepare(`SELECT COUNT(*) AS count FROM push_device_tokens WHERE user_id = ?`)
        .bind(USER_ID)
        .first<{ count: number }>()

      expect(response.status).toBe(503)
      expect(await response.json()).toMatchObject({
        error: 'storage_unavailable',
        retryable: true,
      })
      expect(pushRows?.count).toBe(1)
      await expect(isMobileAccessTokenRevoked(kv, ACCESS_TOKEN)).resolves.toBe(false)
    } finally {
      sqlite.close()
    }
  })
})

describe('Khala Sync account data deletion SQL plan', () => {
  test('removes the personal scope, owned thread scopes, client state, chat, runtime, changelog, and CVR rows', async () => {
    const calls: Array<string> = []
    const tx = (async (strings: TemplateStringsArray) => {
      const sql = strings.join('?')
      calls.push(sql)
      if (sql.includes('SELECT thread_id')) {
        return [{ thread_id: 'thread-1' }]
      }
      if (sql.includes('SELECT client_group_id')) {
        return [{ client_group_id: 'cg-1' }]
      }
      return []
    }) as SyncTransactionSql
    const sql = Object.assign(tx, {
      begin: async <A>(fn: (inner: SyncTransactionSql) => Promise<A>) => fn(tx),
    }) as SyncSql

    const outcome = await deleteKhalaSyncAccountDataWithSql(sql, USER_ID)

    expect(outcome).toEqual({
      clientGroupsRemoved: 1,
      scopesRemoved: 2,
      threadScopesRemoved: 1,
    })
    for (const table of [
      'khala_sync_runtime_events',
      'khala_sync_runtime_control_intents',
      'khala_sync_runtime_turns',
      'khala_sync_chat_messages',
      'khala_sync_chat_threads',
      'khala_sync_changelog',
      'khala_sync_cvrs',
      'khala_sync_capture_checkpoints',
      'khala_sync_scope_owners',
      'khala_sync_scopes',
      'khala_sync_mutations',
      'khala_sync_client_state',
    ]) {
      expect(calls.some(call => call.includes(`DELETE FROM ${table}`))).toBe(true)
    }
    expect(
      calls.some(
        call =>
          call.includes('DELETE FROM khala_sync_chat_messages') &&
          call.includes('thread_id = ANY'),
      ),
    ).toBe(true)
    expect(
      calls.some(
        call =>
          call.includes('DELETE FROM khala_sync_mutations') &&
          call.includes('scope = ANY'),
      ),
    ).toBe(true)
    expect(
      calls.some(
        call =>
          call.includes('DELETE FROM khala_sync_client_state') &&
          call.includes('user_id ='),
      ),
    ).toBe(true)
  })
})

// Shared test harness for driving the REAL `KhalaSyncHubDO` class outside
// the Workers runtime (KS-4.2 unit suite + the KS-4.4 stitch-seam suite).
//
// A `node:sqlite` database backs the DO's SQL storage with a cursor that
// reproduces Cloudflare's `SqlStorageCursor` semantics, and fake hibernation
// sockets record sent frames — the same real-storage idiom as
// `inference/durable-inference-real-do.test.ts`. The happy-path WebSocket
// upgrade (`WebSocketPair` + a 101 Response) is Workers-runtime-only (Node's
// Response rejects status 101), so socket behavior is driven through the
// REAL `attachSocket` accept/catch-up path.

import { DatabaseSync } from 'node:sqlite'

import {
  KhalaSyncHubDO,
  type HubSqlStorageLike,
  type HubWebSocketLike,
  type KhalaSyncHubEnv,
  type KhalaSyncHubStateLike,
} from '../khala-sync-hub-do'

export const cloudflareSql = (db: DatabaseSync): HubSqlStorageLike => ({
  exec<T = Record<string, unknown>>(query: string, ...bindings: Array<unknown>) {
    const rows = db.prepare(query).all(...(bindings as Array<never>)) as Array<T>
    return {
      toArray: () => rows,
      one: (): T | undefined => {
        if (rows.length !== 1) {
          throw new Error(
            `Expected exactly one result from SQL query, but got ${rows.length}.`,
          )
        }
        return rows[0]
      },
    }
  },
})

export class FakeWebSocket implements HubWebSocketLike {
  readonly sent: Array<string> = []
  closed: { code?: number | undefined; reason?: string | undefined } | undefined
  private attachment: unknown

  send(message: string): void {
    if (this.closed !== undefined) throw new Error('socket closed')
    this.sent.push(message)
  }

  close(code?: number, reason?: string): void {
    this.closed = { code, reason }
  }

  serializeAttachment(value: unknown): void {
    // Mirror the runtime: attachments survive via structured serialization.
    this.attachment = JSON.parse(JSON.stringify(value))
  }

  deserializeAttachment(): unknown {
    return this.attachment
  }

  cursor(): number | undefined {
    const raw = this.attachment as { cursor?: number } | undefined
    return raw?.cursor
  }

  frames(): Array<Record<string, unknown>> {
    return this.sent.map(text => JSON.parse(text) as Record<string, unknown>)
  }
}

export const makeHub = (env: KhalaSyncHubEnv = {}) => {
  const db = new DatabaseSync(':memory:')
  const sockets: Array<FakeWebSocket> = []
  const autoResponses: Array<unknown> = []
  const state: KhalaSyncHubStateLike = {
    acceptWebSocket: ws => sockets.push(ws as FakeWebSocket),
    blockConcurrencyWhile: fn => fn(),
    getWebSockets: () => sockets.filter(ws => ws.closed === undefined),
    setWebSocketAutoResponse: pair => autoResponses.push(pair),
    storage: { sql: cloudflareSql(db) },
  }
  const hub = new KhalaSyncHubDO(state, env)
  return { autoResponses, db, hub, sockets, state }
}

/** Reopen a NEW hub instance over the SAME database (isolate restart). */
export const reopenHub = (db: DatabaseSync, env: KhalaSyncHubEnv = {}) => {
  const sockets: Array<FakeWebSocket> = []
  const state: KhalaSyncHubStateLike = {
    acceptWebSocket: ws => sockets.push(ws as FakeWebSocket),
    blockConcurrencyWhile: fn => fn(),
    getWebSockets: () => sockets.filter(ws => ws.closed === undefined),
    storage: { sql: cloudflareSql(db) },
  }
  return { hub: new KhalaSyncHubDO(state, env), sockets }
}

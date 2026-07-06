import { canonicalJson, SyncScope } from "@openagentsinc/khala-sync"
import { Database } from "bun:sqlite"
import { afterEach, describe, expect, test } from "bun:test"
import { bunSqlDriver } from "../sqlite-store.js"
import { createKhalaSyncStoreCore } from "../store-core.js"
import {
  isStoreRequest,
  isStoreResponse,
  type StoreRequest,
  type StoreRequestBody,
  type StoreResponse,
} from "./protocol.js"
import {
  createKhalaSyncStoreWorkerServer,
  type KhalaSyncStoreWorkerServer,
  MALFORMED_REQUEST_ID,
} from "./worker-server.js"

/**
 * Storage-worker RPC server (KS-5.4): wire decode → core dispatch → wire
 * encode, with the typed error taxonomy transported by reason + message.
 * The core behind it is real SQL (`bun:sqlite` harness driver); full
 * store semantics through this server run in web-store.test.ts.
 */

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

const createServer = (): KhalaSyncStoreWorkerServer => {
  const db = new Database(":memory:")
  cleanups.push(() => db.close())
  return createKhalaSyncStoreWorkerServer(
    createKhalaSyncStoreCore(bunSqlDriver(db)),
  )
}

const scope = SyncScope.make("scope.team.alpha")

const request = (body: StoreRequestBody, id = 1): StoreRequest =>
  ({ ...body, id }) as StoreRequest

describe("khala-sync storage worker protocol guards", () => {
  test("classifies store request/response envelopes", () => {
    expect(isStoreRequest(request({ op: "cursor", scope }))).toBe(true)
    expect(isStoreRequest({ id: 1, op: "not-real", scope })).toBe(false)
    expect(isStoreResponse({ id: 1, ok: true })).toBe(true)
    expect(isStoreResponse({ ok: false, reason: "x" })).toBe(false)
  })
})

describe("createKhalaSyncStoreWorkerServer", () => {
  test("applies wire entries and reads them back as wire entities", () => {
    const server = createServer()

    const applied = server.handle(
      request(
        {
          op: "applyConfirmed",
          scope,
          entries: [
            {
              scope,
              version: 1,
              entityType: "task",
              entityId: "t1",
              op: "upsert",
              postImageJson: canonicalJson({ title: "one" }),
              committedAt: "2026-07-04T00:00:00.000Z",
            },
          ],
          cursor: 1,
        },
        7,
      ),
    )
    expect(applied).toEqual({ id: 7, ok: true, value: undefined })

    const read = server.handle(request({ op: "readEntities", scope }, 8))
    expect(read).toEqual({
      id: 8,
      ok: true,
      value: [
        {
          entityType: "task",
          entityId: "t1",
          postImageJson: canonicalJson({ title: "one" }),
          version: 1,
        },
      ],
    })

    const cursor = server.handle(request({ op: "cursor", scope }, 9))
    expect(cursor).toEqual({ id: 9, ok: true, value: 1 })
  })

  test("transports typed store errors by reason + public-safe message", () => {
    const server = createServer()
    const response = server.handle(
      request(
        {
          op: "enqueueMutation",
          mutation: { mutationId: 5, name: "task.create", argsJson: "{}" },
        },
        3,
      ),
    ) as Extract<StoreResponse, { ok: false }>
    expect(response.ok).toBe(false)
    expect(response.id).toBe(3)
    expect(response.reason).toBe("mutation_id_gap")
    expect(response.message).toContain("expected mutationId 1")
  })

  test("answers malformed frames without throwing", () => {
    const server = createServer()
    for (const frame of [null, 42, "nope", {}, { id: 1, op: "unknownOp" }]) {
      const response = server.handle(frame) as Extract<
        StoreResponse,
        { ok: false }
      >
      expect(response.ok).toBe(false)
      expect(response.id).toBe(MALFORMED_REQUEST_ID)
      expect(response.reason).toBe("storage_failure")
    }
  })

  test("identity round-trips through the wire", () => {
    const server = createServer()
    expect(server.handle(request({ op: "identity" }, 1))).toEqual({
      id: 1,
      ok: true,
      value: null,
    })
    const identity = {
      clientId: "client-1",
      clientGroupId: "group-1",
      schemaVersion: 1,
    }
    expect(
      server.handle(request({ op: "setIdentity", identity }, 2)),
    ).toEqual({ id: 2, ok: true, value: undefined })
    expect(server.handle(request({ op: "identity" }, 3))).toEqual({
      id: 3,
      ok: true,
      value: identity,
    })
  })
})

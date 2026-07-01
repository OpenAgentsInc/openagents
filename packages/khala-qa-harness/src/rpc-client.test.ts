import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  KhalaCodeRpcClient,
  KhalaCodeRpcMethodNames,
  type KhalaCodeRpcFetch,
  compareKhalaCodeRpcConsistency,
  decodeKhalaCodeRpcParametersOrFailure,
  decodeKhalaCodeRpcResultOrFailure,
} from "./rpc-client.js"

const jsonResponse = (payload: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(payload), ({
    status: init?.status ?? 200,
    headers: { "content-type": "application/json" },
    ...(init?.statusText === undefined ? {} : { statusText: init.statusText }),
  }))

describe("KhalaCodeRpcClient", () => {
  test("exposes a typed request helper for every desktop RPC method", () => {
    const client = new KhalaCodeRpcClient({
      fetch: (() => Promise.resolve(jsonResponse({}))) as KhalaCodeRpcFetch,
    })

    for (const method of KhalaCodeRpcMethodNames) {
      expect(client.request[method]).toBeFunction()
    }
  })

  test("posts to /rpc/<method> and decodes the response with the imported schema", async () => {
    const calls: string[] = []
    const client = new KhalaCodeRpcClient({
      baseUrl: "http://fixture.local",
      fetch: ((input, init) => {
        calls.push(`${init?.method ?? "GET"} ${String(input)} ${String(init?.body)}`)
        return Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
        }))
      }) as KhalaCodeRpcFetch,
    })

    const result = await Effect.runPromise(client.request.appInfo())

    expect(result.app).toBe("Khala Code Desktop")
    expect(calls).toEqual([
      'POST http://fixture.local/rpc/appInfo {"args":[]}',
    ])
  })

  test("returns a typed schema failure when a fixture bridge violates a response schema", async () => {
    const client = new KhalaCodeRpcClient({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
        }))) as KhalaCodeRpcFetch,
    })

    const exit = await Effect.runPromiseExit(client.callWithOracle("appInfo"))

    expect(exit._tag).toBe("Failure")
    if (exit._tag === "Failure") {
      const failure = String(exit.cause)
      expect(failure).toContain("KhalaCodeRpcSchemaFailure")
      expect(failure).toContain("observedAt")
    }
  })

  test("decodes bridge errors from non-2xx responses", async () => {
    const client = new KhalaCodeRpcClient({
      fetch: (() =>
        Promise.resolve(jsonResponse({
          error: "unknown_method",
          method: "appInfo",
          ok: false,
          tag: "rpc_unknown_method",
        }, { status: 404, statusText: "Not Found" }))) as KhalaCodeRpcFetch,
    })

    const exit = await Effect.runPromiseExit(client.callWithOracle("appInfo"))

    expect(exit._tag).toBe("Failure")
    if (exit._tag === "Failure") {
      const failure = String(exit.cause)
      expect(failure).toContain("KhalaCodeRpcHttpFailure")
      expect(failure).toContain("rpc_unknown_method")
    }
  })

  test("flags unknown response fields after schema decode", () => {
    const decoded = decodeKhalaCodeRpcResultOrFailure("appInfo", {
      app: "Khala Code Desktop",
      ok: true,
      observedAt: "2026-07-01T00:00:00.000Z",
      surprise: true,
    })

    expect(decoded.ok).toBe(true)
    if (decoded.ok) {
      expect(decoded.oracle.unknownFields).toEqual([
        { path: "surprise", value: true },
      ])
    }
  })

  test("returns typed request decode failures before transport", () => {
    const decoded = decodeKhalaCodeRpcParametersOrFailure("connectCodexAccount", [])

    expect(decoded.ok).toBe(false)
    if (!decoded.ok) {
      expect(decoded.failure._tag).toBe("KhalaCodeRpcSchemaFailure")
      if (decoded.failure._tag === "KhalaCodeRpcSchemaFailure") {
        expect(decoded.failure.phase).toBe("request")
      }
    }
  })

  test("compares two mode reads for consistency oracle consumers", () => {
    const result = compareKhalaCodeRpcConsistency({
      leftLabel: "rpc",
      rightLabel: "dom",
      left: { fleet: { ready: 2, accounts: ["codex", "codex-2"] } },
      right: { fleet: { ready: 1, accounts: ["codex", "codex-3"] } },
    })

    expect(result.ok).toBe(false)
    expect(result.mismatches.map((mismatch) => mismatch.path)).toContain("fleet.ready")
    expect(result.mismatches.map((mismatch) => mismatch.path)).toContain("fleet.accounts[1]")
  })
})

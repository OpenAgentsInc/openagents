import { describe, expect, it } from "bun:test"
import {
  decodePylonRpcReplyEnvelopeJson,
  decodePylonRpcRequestEnvelope,
  decodePylonRpcRequestEnvelopeJson,
  encodePylonRpcReplyEnvelope,
  encodePylonRpcRequestEnvelope,
  PylonRpcSchemaLiteral,
} from "./index.js"

// The RPC contract is an unconsumed seed (PY-2 wires it), so its tests assert
// the schema round-trips and rejects malformed envelopes end to end.
describe("pylon RPC contract", () => {
  it("round-trips a RunAssignment request envelope through JSON", () => {
    const envelope = {
      schema: PylonRpcSchemaLiteral,
      id: "req-1",
      request: {
        _tag: "RunAssignment" as const,
        objective: "fix the flaky test",
        workerKind: "codex" as const,
        fixture: true,
      },
    }
    const encoded = encodePylonRpcRequestEnvelope(envelope)
    const json = JSON.stringify(encoded)
    const decoded = decodePylonRpcRequestEnvelopeJson(json)
    expect(decoded.id).toBe("req-1")
    expect(decoded.request._tag).toBe("RunAssignment")
    if (decoded.request._tag === "RunAssignment") {
      expect(decoded.request.objective).toBe("fix the flaky test")
      expect(decoded.request.workerKind).toBe("codex")
    }
  })

  it("decodes each request variant by its tag", () => {
    const list = decodePylonRpcRequestEnvelope({
      schema: PylonRpcSchemaLiteral,
      id: "a",
      request: { _tag: "ListAccounts" },
    })
    expect(list.request._tag).toBe("ListAccounts")

    const health = decodePylonRpcRequestEnvelope({
      schema: PylonRpcSchemaLiteral,
      id: "b",
      request: { _tag: "GetAccountHealth", accountRefHash: "acct.hash.abc" },
    })
    expect(health.request._tag).toBe("GetAccountHealth")
  })

  it("round-trips a typed Error reply (replaces stderr scraping)", () => {
    const encoded = encodePylonRpcReplyEnvelope({
      schema: PylonRpcSchemaLiteral,
      id: "req-9",
      reply: {
        _tag: "Error" as const,
        code: "account_unavailable" as const,
        message: "no codex accounts online",
        blockerRefs: ["blocker.assignment.no_capacity"],
      },
    })
    const decoded = decodePylonRpcReplyEnvelopeJson(JSON.stringify(encoded))
    expect(decoded.reply._tag).toBe("Error")
    if (decoded.reply._tag === "Error") {
      expect(decoded.reply.code).toBe("account_unavailable")
    }
  })

  it("rejects an unknown request tag", () => {
    expect(() =>
      decodePylonRpcRequestEnvelope({
        schema: PylonRpcSchemaLiteral,
        id: "x",
        request: { _tag: "NotARealOperation" },
      }),
    ).toThrow()
  })

  it("rejects an envelope with a wrong schema literal", () => {
    expect(() =>
      decodePylonRpcRequestEnvelope({
        schema: "openagents.pylon.rpc.v0",
        id: "x",
        request: { _tag: "ListAccounts" },
      }),
    ).toThrow()
  })
})

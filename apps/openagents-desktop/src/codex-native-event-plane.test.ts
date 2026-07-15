import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexProtocolDecodeResult } from "@openagentsinc/codex-app-server-protocol/decode"
import { makeCodexNativeEventPlane } from "./codex-native-event-plane.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const decoded = (method: string, payload: unknown): CodexProtocolDecodeResult => ({
  _tag: "Decoded",
  lane: "bundled-0.144.1",
  direction: "server-notification",
  method,
  payload,
})

describe("Codex native event plane", () => {
  test("keeps exact causal identities and makes final item completion authoritative", () => {
    const plane = makeCodexNativeEventPlane()
    plane.accept({ generation: 1, decoded: decoded("item/agentMessage/delta", {
      threadId: "thread-1", turnId: "turn-1", itemId: "item-1", delta: "live",
    }) })
    plane.accept({ generation: 1, decoded: decoded("item/started", {
      threadId: "thread-1", turnId: "turn-1", item: { id: "item-1", type: "agentMessage", text: "" },
    }) })
    plane.accept({ generation: 1, decoded: decoded("item/completed", {
      threadId: "thread-1", turnId: "turn-1", item: { id: "item-1", type: "agentMessage", text: "final" },
    }) })
    plane.accept({ generation: 1, decoded: decoded("item/started", {
      threadId: "thread-1", turnId: "turn-1", item: { id: "item-1", type: "agentMessage", text: "stale" },
    }) })

    expect(plane.item("item-1")?.method).toBe("item/completed")
    expect(plane.envelopes({ itemId: "item-1" })).toHaveLength(4)
    expect(plane.envelopes({ method: "item/agentMessage/delta" })[0]?.retention).toBe("bounded-transient")
  })

  test("deduplicates visible compatibility receipts while retaining occurrence counts", () => {
    const visible: string[] = []
    const plane = makeCodexNativeEventPlane({ onCompatibilityReceipt: receipt => visible.push(receipt.method) })
    const failure: CodexProtocolDecodeResult = {
      _tag: "DecodeFailure",
      lane: "bundled-0.144.1",
      direction: "server-notification",
      method: "future/private-event",
      reason: "unknown_method",
      detail: "unknown",
    }
    plane.accept({ generation: 2, decoded: failure })
    plane.accept({ generation: 2, decoded: failure })
    expect(visible).toEqual(["future/private-event"])
    expect(plane.receipts()).toEqual([expect.objectContaining({ occurrences: 2 })])
  })

  test("bounds transient retention and journals only redacted semantic identity", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-codex-native-"))
    roots.push(root)
    const journalPath = join(root, "native.json")
    const plane = makeCodexNativeEventPlane({ journalPath, maxTransientEntries: 2 })
    for (let index = 0; index < 4; index += 1) {
      plane.accept({ generation: 3, decoded: decoded("process/outputDelta", {
        processId: "process-1", stream: "stdout", deltaBase64: `secret-${index}`,
      }) })
    }
    plane.accept({ generation: 3, requestId: "request-1", decoded: decoded("item/completed", {
      threadId: "thread-private",
      turnId: "turn-private",
      item: {
        id: "item-private",
        type: "commandExecution",
        status: "completed",
        aggregatedOutput: "workspace secret",
        cwd: "/Users/private/workspace",
        authorization: "Bearer private",
      },
    }) })

    expect(plane.envelopes({ method: "process/outputDelta" })).toHaveLength(2)
    const disk = readFileSync(journalPath, "utf8")
    expect(disk).toContain("thread-private")
    expect(disk).toContain("item-private")
    expect(disk).not.toContain("workspace secret")
    expect(disk).not.toContain("/Users/private")
    expect(disk).not.toContain("Bearer private")

    const reopened = makeCodexNativeEventPlane({ journalPath })
    expect(reopened.journal()).toEqual([expect.objectContaining({
      method: "item/completed",
      threadId: "thread-private",
      turnId: "turn-private",
      itemId: "item-private",
      status: "completed",
    })])
  })
})

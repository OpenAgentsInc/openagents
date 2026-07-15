import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "vite-plus/test"
import { openCodexDurableQueue } from "./codex-durable-queue.ts"

describe("Codex durable next-turn queue", () => {
  test("survives restart with stable intent/user-message identity and exact-once claim", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-codex-queue-")); const path = join(root, "queue.json")
    try {
      const first = openCodexDurableQueue(path)
      const one = first.enqueue("thread-1", "one", { intentRef: "intent-stable", clientUserMessageId: "user-stable" })
      expect(first.enqueue("thread-1", "lost ACK retry", { intentRef: "intent-stable", clientUserMessageId: "user-stable" })).toEqual(one)
      const two = first.enqueue("thread-1", "two")
      expect([one.position, two.position]).toEqual([1, 2])
      first.close()
      const second = openCodexDurableQueue(path)
      expect(second.list("thread-1").map(entry => entry.clientUserMessageId)).toEqual([one.clientUserMessageId, two.clientUserMessageId])
      const claimed = second.claimNext("thread-1", "turn-0:completed")!
      expect(claimed.queueRef).toBe(one.queueRef)
      expect(second.claimNext("thread-1", "turn-0:completed")?.queueRef).toBe(one.queueRef)
      expect(second.claimNext("thread-1", "turn-1:completed")?.queueRef).toBe(one.queueRef)
      expect(() => second.admitPromotion(one.queueRef, "thread-1", "wrong")).toThrow("not admitted")
      expect(second.admitPromotion(one.queueRef, "thread-1", one.clientUserMessageId).intentRef).toBe(one.intentRef)
      second.complete(one.queueRef, "provider-turn-1")
      expect(second.claimNext("thread-1", "turn-0:completed")).toBeNull()
      expect(second.claimNext("thread-1", "turn-1:completed")?.queueRef).toBe(two.queueRef)
      expect(second.list().find(entry => entry.queueRef === one.queueRef)).toMatchObject({ status: "promoted", providerTurnId: "provider-turn-1" })
      expect(statSync(path).mode & 0o777).toBe(0o600)
    } finally { rmSync(root, { recursive: true, force: true }) }
  })

  test("orders, edits, cancels, rejects stale edits, and records bounded failure", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-codex-queue-edit-")); const path = join(root, "queue.json")
    try {
      const queue = openCodexDurableQueue(path)
      const first = queue.enqueue("thread-1", "secret one")
      const second = queue.enqueue("thread-1", "secret two")
      const edited = queue.edit(second.queueRef, "edited", second.revision)
      expect(() => queue.edit(second.queueRef, "stale", second.revision)).toThrow("changed")
      queue.cancel(first.queueRef, first.revision)
      expect(queue.list("thread-1").find(entry => entry.queueRef === second.queueRef)).toMatchObject({ message: "edited", position: 1 })
      const claimed = queue.claimNext("thread-1", "terminal-1")!
      queue.fail(claimed.queueRef, "x".repeat(1_000))
      expect(queue.list().find(entry => entry.queueRef === claimed.queueRef)?.failure).toHaveLength(400)
      expect(readFileSync(path, "utf8")).toContain("edited")
      queue.close()
    } finally { rmSync(root, { recursive: true, force: true }) }
  })
})

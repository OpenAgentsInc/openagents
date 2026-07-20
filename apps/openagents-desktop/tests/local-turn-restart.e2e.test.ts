import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { CodexLocalRuntime } from "../src/codex-local-runtime.ts"
import { openLocalTurnJournal, type LocalTurnLane } from "../src/local-turn-journal.ts"
import { reconcileLocalTurns } from "../src/local-turn-recovery.ts"
import { makeThreadStore } from "../src/thread-store.ts"

const seedRuntimeA = (root: string, lane: LocalTurnLane) => {
  const journal = openLocalTurnJournal(path.join(root, "local-turns", "journal.json"))
  const store = makeThreadStore(path.join(root, "threads.json"))
  const thread = store.newThread()
  const key = { threadRef: thread.id, turnRef: `turn.${lane}.1`, lane }
  const userMessageKey = `${key.turnRef}-user`
  const assistantMessageKey = `${key.turnRef}-assistant`
  journal.accept({
    ...key,
    userMessageKey,
    assistantMessageKey,
    accountRef: lane === "codex-local" ? "codex-2" : "claude-pylon-b",
    model: lane === "codex-local" ? "gpt-5.6-sol" : "claude-fable-5",
  })
  store.upsert(thread.id, {
    key: userMessageKey,
    role: "user",
    text: "Explain the restart contract",
    timestamp: "11:00 PM",
  })
  journal.recordDispatch(key, lane === "codex-local" ? "codex-2" : "claude-pylon-b")
  journal.recordProviderSession(key, {
    accountRef: lane === "codex-local" ? "codex-2" : "claude-pylon-b",
    providerSessionRef: lane === "codex-local" ? "codex-thread-1" : "claude-session-1",
  })
  journal.appendAssistantText(key, "Hello ")
  store.upsert(thread.id, {
    key: assistantMessageKey,
    role: "assistant",
    text: "Hello ",
    timestamp: "11:01 PM",
  })
  return { key, threadRef: thread.id }
}

describe("local turn process restart", () => {
  test("Codex Runtime B resumes the exact durable account/thread once and converges without duplicates", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-local-turn-restart-codex-"))
    try {
      const seeded = seedRuntimeA(root, "codex-local")
      const journalB = openLocalTurnJournal(path.join(root, "local-turns", "journal.json"))
      const storeB = makeThreadStore(path.join(root, "threads.json"))
      let starts = 0
      const codex: Pick<CodexLocalRuntime, "runTurn"> = {
        runTurn: async input => {
          starts += 1
          expect(input.recovery).toEqual({ threadId: "codex-thread-1", accountRef: "codex-2" })
          expect(input.accountRef).toBe("codex-2")
          expect(input.message).not.toContain("Explain the restart contract")
          input.emit({ kind: "text_delta", text: "world" })
          return {
            ok: true,
            text: "Hello world",
            totalTokens: 2,
            accountRef: "codex-2",
            threadId: "codex-thread-1",
          }
        },
      }
      expect(await reconcileLocalTurns({ journal: journalB, store: storeB, codex })).toMatchObject([
        { key: seeded.key, state: "completed" },
      ])
      expect(starts).toBe(1)

      const journalC = openLocalTurnJournal(path.join(root, "local-turns", "journal.json"))
      expect(await reconcileLocalTurns({ journal: journalC, store: storeB, codex })).toEqual([])
      expect(starts).toBe(1)
      const notes = storeB.open(seeded.threadRef)!.notes
      expect(notes.filter(note => note.key === `${seeded.key.turnRef}-user`)).toHaveLength(1)
      const assistant = notes.filter(note => note.role === "assistant")
      expect(assistant).toHaveLength(2)
      expect(new Set(assistant.map(note => note.key)).size).toBe(2)
      expect(assistant.map(note => note.text).join("")).toBe("Hello world")
      expect(notes.filter(note => note.key === `${seeded.key.turnRef}-recovery`)).toHaveLength(1)
      expect(journalC.get(seeded.key)).toMatchObject({
        phase: "completed",
        disposition: "resumed_after_restart",
        recoveryGeneration: 1,
      })
      // A stale Runtime A callback after B settled is fenced by terminal state.
      expect(journalC.appendAssistantText(seeded.key, " stale")).toMatchObject({ assistantText: "Hello world" })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("Claude Runtime B records one explicit interruption and never silently starts another SDK query", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-local-turn-restart-claude-"))
    try {
      const seeded = seedRuntimeA(root, "claude-local")
      const journalB = openLocalTurnJournal(path.join(root, "local-turns", "journal.json"))
      const storeB = makeThreadStore(path.join(root, "threads.json"))
      let starts = 0
      const codex: Pick<CodexLocalRuntime, "runTurn"> = {
        runTurn: async () => {
          starts += 1
          throw new Error("Claude recovery must not dispatch Codex")
        },
      }
      expect(await reconcileLocalTurns({ journal: journalB, store: storeB, codex })).toMatchObject([
        { key: seeded.key, state: "interrupted" },
      ])
      expect(starts).toBe(0)
      expect(await reconcileLocalTurns({ journal: journalB, store: storeB, codex })).toEqual([])
      const notes = storeB.open(seeded.threadRef)!.notes
      expect(notes.filter(note => note.key === `${seeded.key.turnRef}-user`)).toHaveLength(1)
      expect(notes.filter(note => note.key === `${seeded.key.turnRef}-assistant`)).toHaveLength(1)
      expect(notes.filter(note => note.key === `${seeded.key.turnRef}-recovery`)).toHaveLength(1)
      expect(notes.find(note => note.key === `${seeded.key.turnRef}-recovery`)?.text)
        .toContain("Retry explicitly")
      expect(journalB.get(seeded.key)).toMatchObject({
        phase: "interrupted_by_restart",
        disposition: "interrupted_by_restart",
      })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

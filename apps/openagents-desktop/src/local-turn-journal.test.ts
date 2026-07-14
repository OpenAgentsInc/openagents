import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { LocalTurnJournalError, openLocalTurnJournal } from "./local-turn-journal.ts"

const key = { threadRef: "thread.1", turnRef: "turn.1", lane: "codex-local" as const }

describe("local turn recovery journal", () => {
  test("persists private accepted identity, provider attachment, and bounded streaming prefix across reopen", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-local-turn-journal-"))
    const file = path.join(root, "turns", "journal.json")
    try {
      const journal = openLocalTurnJournal(file, () => new Date("2026-07-13T05:00:00.000Z"))
      expect(journal.accept({
        ...key,
        userMessageKey: "turn.1-user",
        assistantMessageKey: "turn.1-assistant",
        accountRef: "codex-2",
        model: "gpt-5.6-sol",
      }).accepted).toBe(true)
      journal.recordDispatch(key, "codex-2")
      journal.recordProviderSession(key, { accountRef: "codex-2", providerSessionRef: "provider.thread.1" })
      journal.appendAssistantText(key, "first ")
      journal.appendAssistantText(key, "sentence")

      expect(statSync(file).mode & 0o777).toBe(0o600)
      expect(statSync(path.dirname(file)).mode & 0o777).toBe(0o700)
      expect(openLocalTurnJournal(file).get(key)).toMatchObject({
        accountRef: "codex-2",
        providerSessionRef: "provider.thread.1",
        phase: "streaming",
        persistedCursor: 2,
        assistantText: "first sentence",
      })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("accept is idempotent and recovery is claimed once before fail-honest terminalization", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-local-turn-recovery-"))
    const file = path.join(root, "journal.json")
    try {
      const journal = openLocalTurnJournal(file)
      const accepted = { ...key, userMessageKey: "turn.1-user", assistantMessageKey: "turn.1-assistant" }
      expect(journal.accept(accepted).accepted).toBe(true)
      expect(journal.accept(accepted).accepted).toBe(false)
      expect(journal.beginRecovery(key)).toMatchObject({ phase: "recovering", recoveryGeneration: 1 })

      const secondProcess = openLocalTurnJournal(file)
      expect(secondProcess.beginRecovery(key)).toMatchObject({
        phase: "interrupted_by_restart",
        disposition: "interrupted_by_restart",
        recoveryGeneration: 1,
      })
      expect(secondProcess.nonterminal()).toEqual([])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("invalid bytes fail closed instead of erasing nonterminal truth", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-local-turn-invalid-"))
    const file = path.join(root, "journal.json")
    try {
      writeFileSync(file, "not-json")
      expect(() => openLocalTurnJournal(file)).toThrow(LocalTurnJournalError)
      expect(() => openLocalTurnJournal(file)).toThrow("failed validation")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

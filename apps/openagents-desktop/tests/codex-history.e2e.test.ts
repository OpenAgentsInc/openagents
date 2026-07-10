import { describe, expect, test } from "bun:test"
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, truncateSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"
import { findRecentCodexThread } from "../src/codex-history.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"

const contractId = "openagents_desktop.chat.thread_first_content_under_1s.v1"

describe(contractId, () => {
  test("registry is valid and the performance contract is enforced", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(contract => contract.contractId === contractId)?.state).toBe("enforced")
  })

  test("projects first content from a 256 MiB rollout in under one second", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-large-rollout-"))
    try {
      const sessions = path.join(root, "sessions", "2026", "07", "10")
      mkdirSync(sessions, { recursive: true })
      const id = "019f4d20-5fce-7643-a993-995b8561b1a6"
      const file = path.join(sessions, `rollout-2026-07-10T18-00-00-${id}.jsonl`)
      writeFileSync(file, `${JSON.stringify({ timestamp: "2026-07-10T18:00:00.000Z", type: "session_meta", payload: { id, cwd: "/safe/repo" } })}\n`)
      truncateSync(file, 256 * 1024 * 1024)
      appendFileSync(file, `\n${JSON.stringify({ timestamp: "2026-07-10T18:01:00.000Z", type: "response_item", payload: { type: "message", role: "assistant", content: [{ type: "output_text", text: "Immediate bounded content" }] } })}\n`)

      const started = performance.now()
      const thread = findRecentCodexThread({ sessionsRoot: path.join(root, "sessions"), id, file, now: new Date("2026-07-10T18:02:00.000Z") })
      const elapsedMs = performance.now() - started

      expect(thread?.notes.at(-1)?.text).toBe("Immediate bounded content")
      expect(elapsedMs).toBeLessThan(1_000)
    } finally { rmSync(root, { recursive: true, force: true }) }
  })
})

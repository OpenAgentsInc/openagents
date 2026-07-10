import { describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { readRecentCodexHistory } from "../src/codex-history.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"

const now = new Date("2026-07-10T18:00:00.000Z")
const root = () => mkdtempSync(path.join(tmpdir(), "openagents-codex-history-"))
const write = (dir: string, name: string, rows: unknown[]) => {
  const nested = path.join(dir, "2026", "07", "10")
  mkdirSync(nested, { recursive: true })
  writeFileSync(path.join(nested, name), `${rows.map(row => JSON.stringify(row)).join("\n")}\n`)
}
const meta = (id: string, timestamp: string, extra: Record<string, unknown> = {}) => ({ timestamp, type: "session_meta", payload: { id, cwd: "/safe/repo", ...extra } })
const message = (timestamp: string, role: "user" | "assistant", text: string) => ({ timestamp, type: "response_item", payload: { type: "message", role, content: [{ type: "input_text", text }] } })

describe("openagents_desktop.seam.codex_recent_history_projection.v1", () => {
  test("records the app-owned enforced UX contract", () => {
    expect(openAgentsDesktopUxContractRegistry.contracts[0]?.contractId).toBe("openagents_desktop.seam.codex_recent_history_projection.v1")
    expect(openAgentsDesktopUxContractRegistry.contracts[0]?.state).toBe("enforced")
  })
  test("projects only recent top-level chats and their bounded conversational messages", () => {
    const sessions = root()
    write(sessions, "top.jsonl", [meta("top", "2026-07-10T17:00:00.000Z"), message("2026-07-10T17:01:00.000Z", "user", "Ship the Codex sidebar"), message("2026-07-10T17:02:00.000Z", "assistant", "Working on it")])
    write(sessions, "child.jsonl", [meta("child", "2026-07-10T17:30:00.000Z", { parent_thread_id: "top" }), message("2026-07-10T17:31:00.000Z", "user", "hidden child")])
    write(sessions, "old.jsonl", [meta("old", "2026-07-09T15:00:00.000Z"), message("2026-07-09T15:01:00.000Z", "user", "old")])
    const threads = readRecentCodexHistory({ sessionsRoot: sessions, now })
    expect(threads).toHaveLength(1)
    expect(threads[0]).toMatchObject({ id: "top", title: "Ship the Codex sidebar", cwd: "/safe/repo" })
    expect(threads[0]?.notes.map(note => note.text)).toEqual(["Ship the Codex sidebar", "Working on it"])
  })

  test("is empty and usable when the Codex history root is absent or malformed", () => {
    expect(readRecentCodexHistory({ sessionsRoot: path.join(tmpdir(), "missing-openagents-codex-history"), now })).toEqual([])
    const sessions = root(); write(sessions, "bad.jsonl", [{ unexpected: true }])
    expect(readRecentCodexHistory({ sessionsRoot: sessions, now })).toEqual([])
  })
})

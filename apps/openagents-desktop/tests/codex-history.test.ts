import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { readCodexHistoryPage, readRecentCodexHistory } from "../src/codex-history.ts"
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

describe("openagents_desktop.seam.codex_loss_accounted_history.v2 legacy compatibility", () => {
  test("records the app-owned enforced UX contract", () => {
    expect(
      openAgentsDesktopUxContractRegistry.contracts.some(
        ({ contractId }) => contractId === "openagents_desktop.seam.codex_loss_accounted_history.v2",
      ),
    ).toBe(true)
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

  test("keeps the sidebar projection metadata-only until a thread is selected", () => {
    const sessions = root()
    write(sessions, "top.jsonl", [meta("top", "2026-07-10T17:00:00.000Z"), message("2026-07-10T17:01:00.000Z", "user", "Only in detail")])
    const threads = readRecentCodexHistory({ sessionsRoot: sessions, now, includeMessages: false })
    expect(threads[0]?.notes).toEqual([])
  })

  test("transport context never becomes the legacy sidebar title", () => {
    const sessions = root()
    const environment = "<environment_context>\n  <cwd>/safe/repo</cwd>\n</environment_context>"
    write(sessions, "top.jsonl", [meta("top", "2026-07-10T17:00:00.000Z"), message("2026-07-10T17:01:00.000Z", "user", environment), message("2026-07-10T17:02:00.000Z", "user", "Continue the voice work")])
    expect(readRecentCodexHistory({ sessionsRoot: sessions, now })[0]?.title).toBe("Continue the voice work")
    const literal = root()
    write(literal, "literal.jsonl", [meta("literal", "2026-07-10T17:00:00.000Z"), message("2026-07-10T17:01:00.000Z", "user", "Show the literal <environment_context> tag")])
    expect(readRecentCodexHistory({ sessionsRoot: literal, now })[0]?.title).toBe("Show the literal <environment_context> tag")
  })
})

describe("typed WorkbenchItem history sidecar (#8859)", () => {
  test("tool-class rollout rows carry the structured item a renderer rebuilds typed cards from", () => {
    const sessions = root()
    write(sessions, "typed.jsonl", [
      meta("typed", "2026-07-10T17:00:00.000Z"),
      {
        timestamp: "2026-07-10T17:01:00.000Z",
        type: "event_msg",
        payload: {
          type: "item_completed",
          item: {
            id: "cmd-1",
            type: "command_execution",
            command: "pnpm test --filter desktop",
            cwd: "/safe/repo",
            exit_code: 0,
            duration_ms: 950,
            aggregated_output: "42 tests passed",
            status: "completed",
          },
        },
      },
      {
        timestamp: "2026-07-10T17:02:00.000Z",
        type: "event_msg",
        payload: {
          type: "item_completed",
          item: {
            id: "mcp-1",
            type: "mcp_tool_call",
            server: "stripe",
            tool: "createCharge",
            arguments: { amount: 42, token: "sk-abcdefghijklmnop" },
            duration_ms: 88,
            status: "completed",
          },
        },
      },
      message("2026-07-10T17:03:00.000Z", "assistant", "All green."),
    ])
    const page = readCodexHistoryPage({ sessionsRoot: sessions, threadRef: "typed", offset: 0, limit: 200 })!
    const command = page.items.find(item => item.item?.kind === "command")
    expect(command?.kind).toBe("tool_call")
    expect(command?.item).toEqual({
      kind: "command",
      source: "codex",
      command: "pnpm test --filter desktop",
      cwd: "/safe/repo",
      status: "completed",
      exitCode: 0,
      durationMs: 950,
      outputTail: "42 tests passed",
    })
    const mcp = page.items.find(item => item.item?.kind === "toolCall")
    expect(mcp?.item).toMatchObject({
      kind: "toolCall",
      callKind: "mcp",
      server: "stripe",
      tool: "createCharge",
      durationMs: 88,
      status: "completed",
    })
    // History redaction discipline applies to the typed sidecar too.
    const args = mcp?.item?.kind === "toolCall" ? mcp.item.args : []
    expect(args.find(entry => entry.key === "token")?.value).toBe("[REDACTED]")
    expect(args.find(entry => entry.key === "amount")?.value).toBe("42")
    // Non-tool rows stay sidecar-free.
    const assistant = page.items.find(item => item.kind === "assistant_message")
    expect(assistant?.item).toBeUndefined()
  })
})

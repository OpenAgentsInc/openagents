import { mkdtemp, readFile, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import {
  appendKhalaSessionModelItem,
  appendKhalaSessionToolEvent,
  createKhalaSessionRollout,
  forkKhalaSessionRollout,
  khalaSessionModelItems,
  khalaSessionRolloutPath,
  khalaSessionToolEvents,
  parseKhalaSessionRolloutText,
  readKhalaSessionRollout,
} from "./index.js"

async function makeStateDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-session-rollout-"))
}

describe("Khala session rollout", () => {
  test("persists model items and tool events as append-only JSONL", async () => {
    const stateDir = await makeStateDir()
    const session = await createKhalaSessionRollout({
      createdAt: "2026-06-30T00:00:00.000Z",
      sessionId: "session.test",
      stateDir,
    })

    await appendKhalaSessionModelItem(stateDir, session.sessionId, {
      body: "Inspect README.md",
      id: "msg.user.1",
      role: "user",
    }, { createdAt: "2026-06-30T00:00:01.000Z" })
    await appendKhalaSessionToolEvent(stateDir, session.sessionId, {
      eventId: "event.tool.1",
      invocationId: "call_1",
      kind: "tool_completed",
      payload: { publicSummary: "read: ok" },
      sessionId: session.sessionId,
    }, { createdAt: "2026-06-30T00:00:02.000Z" })

    const loaded = await readKhalaSessionRollout(stateDir, session.sessionId)
    const text = await readFile(khalaSessionRolloutPath(stateDir, session.sessionId), "utf8")

    expect(text.trim().split("\n")).toHaveLength(3)
    expect(loaded.records.map(record => record.sequence)).toEqual([0, 1, 2])
    expect(khalaSessionModelItems(loaded)).toEqual([
      { body: "Inspect README.md", id: "msg.user.1", role: "user" },
    ])
    expect(khalaSessionToolEvents(loaded)).toEqual([
      expect.objectContaining({ eventId: "event.tool.1", kind: "tool_completed" }),
    ])
  })

  test("tolerates a corrupt trailing line from an interrupted append", async () => {
    const stateDir = await makeStateDir()
    await createKhalaSessionRollout({ sessionId: "session.partial", stateDir })
    await appendKhalaSessionModelItem(stateDir, "session.partial", {
      body: "hello",
      id: "msg.user.1",
      role: "user",
    })
    const path = khalaSessionRolloutPath(stateDir, "session.partial")
    const current = await readFile(path, "utf8")
    await writeFile(path, `${current}{not-json`)

    const loaded = await readKhalaSessionRollout(stateDir, "session.partial")

    expect(loaded.corruptLineCount).toBe(1)
    expect(khalaSessionModelItems(loaded).map(item => item.body)).toEqual(["hello"])
  })

  test("forks into a new session with intact model and tool history", async () => {
    const stateDir = await makeStateDir()
    await createKhalaSessionRollout({ sessionId: "session.source", stateDir })
    await appendKhalaSessionModelItem(stateDir, "session.source", {
      body: "Read package.json",
      id: "msg.user.1",
      role: "user",
    })
    await appendKhalaSessionToolEvent(stateDir, "session.source", {
      eventId: "event.approval.1",
      invocationId: "call_approval",
      kind: "approval_answered",
      payload: { decision: "allow" },
      sessionId: "session.source",
    })

    const forked = await forkKhalaSessionRollout({
      createdAt: "2026-06-30T00:00:03.000Z",
      fromSessionId: "session.source",
      newSessionId: "session.fork",
      stateDir,
    })

    expect(forked.records.map(record => record.kind)).toEqual([
      "session_forked",
      "model_item",
      "tool_event",
    ])
    expect(forked.records.every(record => record.sessionId === "session.fork")).toBe(true)
    expect(forked.records[0]?.parentSessionId).toBe("session.source")
    expect(khalaSessionModelItems(forked).map(item => item.body)).toEqual(["Read package.json"])
    expect(khalaSessionToolEvents(forked).map(event => event.kind)).toEqual(["approval_answered"])
  })

  test("rejects corrupt non-trailing rollout lines", () => {
    expect(() => parseKhalaSessionRolloutText("{bad}\n{}\n", "test.jsonl")).toThrow(
      "Invalid Khala session rollout record",
    )
  })
})

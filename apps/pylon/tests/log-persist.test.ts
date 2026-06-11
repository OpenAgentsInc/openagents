import { describe, expect, test } from "bun:test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  createFeedLogWriter,
  feedLogFileName,
  feedLogRotatedFileName,
  parseFeedLogLine,
  readPersistedLogTail,
} from "../src/node/log-persist"
import type { PylonLogEntry } from "../src/node/state"

const entry = (n: number, level: PylonLogEntry["level"] = "info"): PylonLogEntry => ({
  at: new Date(n).toISOString(),
  level,
  message: `message ${n}`,
})

describe("feed log persistence", () => {
  test("append + tail round trip preserves order and content", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    const writer = createFeedLogWriter(dir)
    for (let i = 0; i < 5; i += 1) {
      await writer.append(entry(i))
    }
    const tail = await readPersistedLogTail(dir, 10)
    expect(tail).toHaveLength(5)
    expect(tail[0]?.message).toBe("message 0")
    expect(tail[4]?.message).toBe("message 4")
  })

  test("tail respects the max and returns the newest entries", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    const writer = createFeedLogWriter(dir)
    for (let i = 0; i < 20; i += 1) {
      await writer.append(entry(i))
    }
    const tail = await readPersistedLogTail(dir, 3)
    expect(tail.map((item) => item.message)).toEqual(["message 17", "message 18", "message 19"])
  })

  test("corrupt lines are skipped, not fatal", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    writeFileSync(
      join(dir, feedLogFileName),
      `${JSON.stringify(entry(1))}\n{broken json\n${JSON.stringify(entry(2))}\n{"at":"x","level":"nope","message":"bad level"}\n`,
    )
    const tail = await readPersistedLogTail(dir, 10)
    expect(tail).toHaveLength(2)
  })

  test("parseFeedLogLine validates shape", () => {
    expect(parseFeedLogLine("")).toBeNull()
    expect(parseFeedLogLine("not json")).toBeNull()
    expect(parseFeedLogLine(JSON.stringify(entry(5)))).toEqual(entry(5))
  })

  test("rotation renames the active file once it exceeds the cap", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    const writer = createFeedLogWriter(dir, { rotateBytes: 512 })
    // The size check runs every 64 appends; write enough to trip it.
    for (let i = 0; i < 130; i += 1) {
      await writer.append(entry(i))
    }
    const rotated = Bun.file(join(dir, feedLogRotatedFileName))
    expect(await rotated.exists()).toBe(true)
    // Both files together still yield a coherent tail.
    const tail = await readPersistedLogTail(dir, 5)
    expect(tail).toHaveLength(5)
    expect(tail[4]?.message).toBe("message 129")
  })

  test("write failures disable persistence without throwing", async () => {
    const errors: string[] = []
    // A path that cannot be a directory: a file stands where the dir should be.
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    const blocked = join(dir, "blocked")
    writeFileSync(blocked, "i am a file")
    const writer = createFeedLogWriter(join(blocked, "nested"), { onError: (message) => errors.push(message) })
    await writer.append(entry(1))
    await writer.append(entry(2))
    expect(errors.length).toBe(1)
  })
})

describe("session banner hygiene (no per-launch spam)", () => {
  test("transient entries are never persisted", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    const writer = createFeedLogWriter(dir)
    await writer.append({ ...entry(1), transient: true })
    await writer.append(entry(2))
    const tail = await readPersistedLogTail(dir, 10)
    expect(tail).toHaveLength(1)
    expect(tail[0]?.message).toBe("message 2")
  })

  test("legacy boot banners are dropped when restoring scrollback", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-feedlog-"))
    const banner = (message: string) => JSON.stringify({ at: new Date(0).toISOString(), level: "info", message })
    writeFileSync(
      join(dir, feedLogFileName),
      [
        banner("Pylon v0.3 ready. Logs are quiet by default - relaunch with --verbose for service detail."),
        banner("[Identity] Pylon Nostr npub: npub1xyz"),
        banner("Pylon node-core running headless. Attach with: pylon attach http://x"),
        banner("a real log line that should survive"),
        "",
      ].join("\n"),
    )
    const tail = await readPersistedLogTail(dir, 10)
    expect(tail).toHaveLength(1)
    expect(tail[0]?.message).toContain("should survive")
  })
})

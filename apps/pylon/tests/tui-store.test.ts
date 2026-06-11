import { beforeEach, describe, expect, test } from "bun:test"
import {
  appendChatFeedItem,
  appendRuntimeLogEntry,
  feedItems,
  maxFeedItems,
  resetViewState,
  setWalletState,
  walletState,
} from "../src/tui/store"

const entry = (level: "error" | "info" | "verbose", message: string) => ({
  at: new Date(0).toISOString(),
  level,
  message,
})

describe("tui view store", () => {
  beforeEach(() => {
    resetViewState()
  })

  test("verbose log entries are filtered out unless verbose mode is on", () => {
    appendRuntimeLogEntry(entry("verbose", "chatter"), false)
    appendRuntimeLogEntry(entry("info", "ready"), false)
    appendRuntimeLogEntry(entry("error", "boom"), false)
    expect(feedItems.length).toBe(2)
    expect(feedItems[0]?.markdown).toContain("ready")
    expect(feedItems[1]?.tone).toBe("logError")

    resetViewState()
    appendRuntimeLogEntry(entry("verbose", "chatter"), true)
    expect(feedItems.length).toBe(1)
  })

  test("feed is capped at maxFeedItems", () => {
    for (let i = 0; i < maxFeedItems + 25; i += 1) {
      appendRuntimeLogEntry(entry("info", `line ${i}`), false)
    }
    expect(feedItems.length).toBe(maxFeedItems)
    expect(feedItems[feedItems.length - 1]?.markdown).toContain(`line ${maxFeedItems + 24}`)
  })

  test("chat feed items stream updates in place and finish", () => {
    const handle = appendChatFeedItem("**OpenCode**: thinking", { streaming: true })
    expect(feedItems.length).toBe(1)
    expect(feedItems[0]?.streaming).toBe(true)
    handle.update("**OpenCode**: partial answer")
    expect(feedItems[0]?.markdown).toContain("partial answer")
    handle.finish()
    expect(feedItems[0]?.streaming).toBe(false)
  })

  test("wallet signal reflects the latest set", () => {
    setWalletState({ daemonOnline: true, balanceSats: 7, readiness: "receive-ready" })
    expect(walletState().balanceSats).toBe(7)
  })
})

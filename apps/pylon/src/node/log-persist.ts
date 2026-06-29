// Durable log feed persistence (issue #4739). Every runtime log entry is
// appended as one JSON line to `feed-log.jsonl` in the Pylon home directory,
// so the TUI is a window onto durable history (and attach mode, issue #4740,
// can replay scrollback). A size-capped rotation keeps disk usage bounded:
// when the active file exceeds the cap it is renamed to `.1` (replacing any
// previous rotation) and a fresh file starts.

import { appendFile, mkdir, rename, stat } from "node:fs/promises"
import { join } from "node:path"
import { isSessionBannerMessage, type PylonLogEntry, type PylonLogLevel } from "./state.js"

export const feedLogFileName = "feed-log.jsonl"
export const feedLogRotatedFileName = "feed-log.jsonl.1"
export const defaultRotateBytes = 5 * 1024 * 1024

export function feedLogPath(homeDir: string): string {
  return join(homeDir, feedLogFileName)
}

const validLevels = new Set<PylonLogLevel>(["error", "info", "verbose"])

export function parseFeedLogLine(line: string): PylonLogEntry | null {
  const trimmed = line.trim()
  if (!trimmed) return null
  try {
    const parsed = JSON.parse(trimmed) as Partial<PylonLogEntry>
    if (
      typeof parsed.at === "string" &&
      typeof parsed.message === "string" &&
      typeof parsed.level === "string" &&
      validLevels.has(parsed.level as PylonLogLevel)
    ) {
      return { at: parsed.at, level: parsed.level as PylonLogLevel, message: parsed.message }
    }
    return null
  } catch {
    // Corrupt lines (e.g. a partial write at crash) are skipped, never fatal.
    return null
  }
}

export interface FeedLogWriter {
  append: (entry: PylonLogEntry) => Promise<void>
}

// Serialized appender: writes are chained so entries land in order even when
// callers fire-and-forget, and a failed write disables persistence for the
// session instead of failing the node.
export function createFeedLogWriter(
  homeDir: string,
  options: { rotateBytes?: number; onError?: (message: string) => void } = {},
): FeedLogWriter {
  const rotateBytes = options.rotateBytes ?? defaultRotateBytes
  const path = feedLogPath(homeDir)
  const rotatedPath = join(homeDir, feedLogRotatedFileName)
  let chain: Promise<void> = Promise.resolve()
  let disabled = false
  let appendedSinceCheck = 0

  const writeOne = async (entry: PylonLogEntry) => {
    if (disabled) return
    try {
      await mkdir(homeDir, { recursive: true })
      await appendFile(path, `${JSON.stringify(entry)}\n`, "utf8")
      appendedSinceCheck += 1
      // Stat only periodically; rotation precision is not load-bearing.
      if (appendedSinceCheck >= 64) {
        appendedSinceCheck = 0
        const info = await stat(path)
        if (info.size > rotateBytes) {
          await rename(path, rotatedPath)
        }
      }
    } catch (error) {
      disabled = true
      options.onError?.(error instanceof Error ? error.message : String(error))
    }
  }

  return {
    append: (entry) => {
      // Session banners are shown live but never persisted - restored
      // scrollback must not accumulate one banner set per launch.
      if (entry.transient) return chain
      chain = chain.then(() => writeOne(entry))
      return chain
    },
  }
}

// Reads the last `max` entries from the persisted feed (rotated file first,
// then the active file). Bounded by the rotation cap, so reading the whole
// file is fine.
export async function readPersistedLogTail(homeDir: string, max: number): Promise<PylonLogEntry[]> {
  const entries: PylonLogEntry[] = []
  for (const name of [feedLogRotatedFileName, feedLogFileName]) {
    const file = Bun.file(join(homeDir, name))
    if (!(await file.exists())) continue
    const text = await file.text()
    for (const line of text.split("\n")) {
      const entry = parseFeedLogLine(line)
      if (entry && !isSessionBannerMessage(entry.message)) entries.push(entry)
    }
  }
  return entries.slice(-max)
}

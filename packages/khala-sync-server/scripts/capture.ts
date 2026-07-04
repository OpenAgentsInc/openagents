#!/usr/bin/env bun
/**
 * Khala Sync capture worker CLI (KS-4.1, #8294).
 *
 * Tails khala_sync_changelog over a DIRECT Postgres connection (LISTEN
 * khala_sync_changelog_append wake + short-poll fallback) and pushes
 * ordered whole-version-group batches to the per-scope KhalaSyncHubDO via
 * the deployed Worker's internal append route (admin bearer). Checkpoints
 * advance only on hub 2xx; delivery is at-least-once (the hub dedupes by
 * version).
 *
 * Usage:
 *   bun scripts/capture.ts [--once] [--database-url <url>]
 *     [--hub-append-url <url>] [--poll-interval-ms <n>] [--batch-versions <n>]
 *
 * Env (flags override): KHALA_SYNC_DATABASE_URL, KHALA_SYNC_HUB_APPEND_URL,
 * OPENAGENTS_ADMIN_API_TOKEN (env only — never a flag, so the token cannot
 * leak into process listings), KHALA_SYNC_CAPTURE_POLL_INTERVAL_MS,
 * KHALA_SYNC_CAPTURE_BATCH_VERSIONS.
 *
 * Modes:
 *   --once     one capture pass (resume from checkpoints, drain, exit).
 *              Exit 0 when every pending scope drained; 1 when any scope
 *              failed (cron/test mode).
 *   (default)  daemon: initial pass, then LISTEN-woken loop with poll
 *              fallback. SIGINT/SIGTERM stop it cleanly.
 *
 * See the README "Capture runbook" section for launchd supervision.
 */
import type { CaptureConfig, CapturePassResult } from "../src/capture.js"
import {
  captureConfigFromEnv,
  runCaptureOnce,
  startCaptureDaemon,
} from "../src/capture.js"

const USAGE = `Usage: bun scripts/capture.ts [--once] [--database-url <url>]
  [--hub-append-url <url>] [--poll-interval-ms <n>] [--batch-versions <n>]

Options:
  --once                  Run one capture pass and exit (cron/test mode).
  --database-url <url>    Direct Postgres URL (default: $KHALA_SYNC_DATABASE_URL).
  --hub-append-url <url>  Worker internal hub append URL
                          (default: $KHALA_SYNC_HUB_APPEND_URL).
  --poll-interval-ms <n>  Poll fallback interval (daemon mode; default 5000).
  --batch-versions <n>    Distinct versions per hub append batch (default 200).
  --help                  Show this help.

The admin bearer comes from $OPENAGENTS_ADMIN_API_TOKEN (env only).
`

const summarize = (result: CapturePassResult): string => {
  const pushed = result.scopes.reduce((sum, s) => sum + s.entriesPushed, 0)
  const lines = [
    `pass: ${result.scopes.length} pending scope(s), ` +
      `${pushed} entrie(s) pushed, ${result.failedScopes} failed`,
  ]
  for (const scope of result.scopes) {
    lines.push(
      `  ${scope.scope}: pushed_through ${scope.pushedThroughVersion} ` +
        `(+${scope.entriesPushed} entries in ${scope.batchesPushed} batch(es))` +
        (scope.error === undefined ? "" : ` — ERROR: ${scope.error}`),
    )
  }
  return lines.join("\n")
}

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  let once = false
  const overrides: {
    databaseUrl?: string
    hubAppendUrl?: string
    pollIntervalMs?: number
    batchVersions?: number
  } = {}

  const intFlag = (name: string, raw: string | undefined): number => {
    const value = raw === undefined ? Number.NaN : Number.parseInt(raw, 10)
    if (!Number.isSafeInteger(value) || value < 1) {
      throw new Error(`${name} requires a positive integer value`)
    }
    return value
  }

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    try {
      if (arg === "--once") {
        once = true
      } else if (arg === "--database-url") {
        overrides.databaseUrl = argv[++i]
        if (overrides.databaseUrl === undefined) {
          throw new Error("--database-url requires a value")
        }
      } else if (arg === "--hub-append-url") {
        overrides.hubAppendUrl = argv[++i]
        if (overrides.hubAppendUrl === undefined) {
          throw new Error("--hub-append-url requires a value")
        }
      } else if (arg === "--poll-interval-ms") {
        overrides.pollIntervalMs = intFlag("--poll-interval-ms", argv[++i])
      } else if (arg === "--batch-versions") {
        overrides.batchVersions = intFlag("--batch-versions", argv[++i])
      } else if (arg === "--help" || arg === "-h") {
        console.log(USAGE)
        return 0
      } else {
        throw new Error(`unknown argument ${JSON.stringify(arg)}`)
      }
    } catch (error) {
      console.error(`error: ${error instanceof Error ? error.message : error}\n`)
      console.error(USAGE)
      return 2
    }
  }

  let config: CaptureConfig
  try {
    const env = { ...process.env }
    if (overrides.databaseUrl !== undefined) {
      env["KHALA_SYNC_DATABASE_URL"] = overrides.databaseUrl
    }
    if (overrides.hubAppendUrl !== undefined) {
      env["KHALA_SYNC_HUB_APPEND_URL"] = overrides.hubAppendUrl
    }
    config = {
      ...captureConfigFromEnv(env),
      ...(overrides.pollIntervalMs === undefined
        ? {}
        : { pollIntervalMs: overrides.pollIntervalMs }),
      ...(overrides.batchVersions === undefined
        ? {}
        : { batchVersions: overrides.batchVersions }),
      log: (line) => console.log(line),
    }
  } catch (error) {
    console.error(`error: ${error instanceof Error ? error.message : error}\n`)
    console.error(USAGE)
    return 2
  }

  if (once) {
    const result = await runCaptureOnce(config)
    console.log(summarize(result))
    return result.failedScopes === 0 ? 0 : 1
  }

  console.log(
    `capture daemon starting (poll fallback ${config.pollIntervalMs ?? 5000}ms)`,
  )
  const daemon = startCaptureDaemon(config)
  let stopping = false
  const shutdown = (signal: string): void => {
    if (stopping) return
    stopping = true
    console.log(`capture daemon stopping (${signal})`)
    void daemon.stop()
  }
  process.on("SIGINT", () => shutdown("SIGINT"))
  process.on("SIGTERM", () => shutdown("SIGTERM"))
  await daemon.done
  console.log("capture daemon stopped")
  return 0
}

process.exit(await main(process.argv.slice(2)))

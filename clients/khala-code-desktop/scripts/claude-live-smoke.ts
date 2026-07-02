#!/usr/bin/env bun
import {
  CLAUDE_LIVE_SMOKE_DEFAULT_TIMEOUT_MS,
  CLAUDE_LIVE_SMOKE_HARNESS,
  runKhalaCodeClaudeLiveSmoke,
} from "../src/bun/claude-live-smoke.js"

const liveAllowed =
  Bun.env.KHALA_CODE_DESKTOP_LIVE_CLAUDE_SMOKE === "1" ||
  process.argv.includes("--allow-live")

if (!liveAllowed) {
  console.error([
    "Refusing to launch a live Claude turn without an explicit guard.",
    "Set KHALA_CODE_DESKTOP_LIVE_CLAUDE_SMOKE=1 or pass --allow-live.",
  ].join("\n"))
  process.exit(2)
}

const timeoutMs = positiveInteger(
  Bun.env.KHALA_CODE_DESKTOP_LIVE_CLAUDE_TIMEOUT_MS,
  CLAUDE_LIVE_SMOKE_DEFAULT_TIMEOUT_MS,
)

console.error(`[live-smoke] harness ${CLAUDE_LIVE_SMOKE_HARNESS}`)
console.error("[live-smoke] launching one Claude desktop runtime turn")
console.error(`[live-smoke] timeout ${timeoutMs}ms`)

const summary = await runKhalaCodeClaudeLiveSmoke({
  env: Bun.env,
  onEvent: event => {
    const label = event.type === "tool_event"
      ? `${event.event.kind} ${(event.event.payload as { readonly name?: string }).name ?? event.event.invocationId}`
      : event.type
    console.error(`[live-smoke] ${label}`)
  },
  timeoutMs,
})

console.log(JSON.stringify(summary, null, 2))

if (!summary.ok) {
  console.error(`[live-smoke] failed: ${summary.failures.join("; ")}`)
  process.exit(1)
}

console.error(
  `[live-smoke] ok: ${summary.approvalRequestCount} approval(s), ` +
    `${summary.exactTokenRows} exact token row(s), ${summary.totalTokens} token(s)`,
)

function positiveInteger(raw: string | undefined, fallback: number): number {
  if (raw === undefined || raw.trim().length === 0) return fallback
  const parsed = Number.parseInt(raw, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

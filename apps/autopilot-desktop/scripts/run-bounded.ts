// Hard wall-clock bound for a child smoke/proof process.
//
// Why: the Verse launch smoke (verse-launch-smoke.ts) spawns an Electrobun
// build + a headless Chrome via CDP. In headless / resource-contended
// environments those children can hang or get OOM-killed with no clean exit,
// which previously SIGKILLed the *whole* gate (DEPLOY_EXIT=137) before
// `wrangler deploy` ran. This wrapper makes the smoke fail fast and loud
// instead of hanging forever, and it kills the child's entire process group
// (Electrobun + Chrome) on timeout so nothing is left behind.
//
// Usage:
//   bun scripts/run-bounded.ts <timeoutMs> -- <command> [args...]
// Env override:
//   OA_VERSE_SMOKE_TIMEOUT_MS overrides <timeoutMs> when set.

const argv = process.argv.slice(2)
const separatorIndex = argv.indexOf("--")
if (separatorIndex === -1 || separatorIndex === argv.length - 1) {
  console.error(
    "run-bounded: usage: bun scripts/run-bounded.ts <timeoutMs> -- <command> [args...]",
  )
  process.exit(2)
}

const defaultTimeoutMs = Number.parseInt(argv[0] ?? "", 10)
const envTimeoutMs = Number.parseInt(process.env.OA_VERSE_SMOKE_TIMEOUT_MS ?? "", 10)
const timeoutMs = Number.isFinite(envTimeoutMs) && envTimeoutMs > 0
  ? envTimeoutMs
  : Number.isFinite(defaultTimeoutMs) && defaultTimeoutMs > 0
    ? defaultTimeoutMs
    : 600_000

const command = argv.slice(separatorIndex + 1)
const label = command.join(" ")

const startedAt = Date.now()
console.error(
  `[run-bounded] starting (hard timeout ${timeoutMs}ms): ${label}`,
)

const child = Bun.spawn({
  cmd: command,
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit",
  // New process group so we can signal Electrobun + Chrome descendants too.
  detached: true,
})

let timedOut = false

const killTree = (signal: NodeJS.Signals) => {
  // Negative pid targets the whole process group when detached.
  try {
    process.kill(-child.pid, signal)
  } catch {
    try {
      child.kill(signal)
    } catch {
      // already gone
    }
  }
}

const timer = setTimeout(() => {
  timedOut = true
  console.error(
    `[run-bounded] TIMEOUT after ${timeoutMs}ms — killing ${label} (SIGTERM, then SIGKILL).`,
  )
  killTree("SIGTERM")
  setTimeout(() => killTree("SIGKILL"), 5_000)
}, timeoutMs)

const exitCode = await child.exited
clearTimeout(timer)

const elapsedMs = Date.now() - startedAt

if (timedOut) {
  console.error(
    `[run-bounded] FAILED: ${label} exceeded its ${timeoutMs}ms wall-clock budget (ran ${elapsedMs}ms) and was killed. This is a fail-fast bound, not a hang.`,
  )
  process.exit(124)
}

if (exitCode === 0) {
  console.error(`[run-bounded] ok: ${label} (${elapsedMs}ms)`)
  process.exit(0)
}

console.error(
  `[run-bounded] FAILED: ${label} exited ${exitCode} (${elapsedMs}ms).`,
)
process.exit(exitCode ?? 1)

// ---------------------------------------------------------------------------
// Breez Spark SDK stdout guard.
//
// The bundled Breez Spark SDK (@breeztech/breez-sdk-spark) prints a storage
// banner to STDOUT at module-EVAL time (the first time the SDK module is
// evaluated):
//   "Breez SDK: Node.js storage automatically enabled"
// plus sibling storage WARNINGS if the default storage fails to load:
//   "Breez SDK: Failed to load Node.js storage: ..."
//   "Breez SDK: Storage operations may not work properly. ..."
//
// That single uncontrolled line corrupts every machine-readable `--json`
// command that loads the wallet (`wallet status --json`,
// `wallet backup-status --json`, ...): the first stdout line is no longer valid
// JSON, so any consumer parser fails. We cannot edit the third-party package.
//
// Why this lives in its OWN module that installs the guard as a TOP-LEVEL side
// effect (rc.33): an earlier version installed the guard at runtime inside the
// CLI's `main()`. ES module imports are HOISTED and evaluated before any
// top-level statement in the entry module -- and in the `bun --compile` binary
// the bundler can eagerly evaluate a transitively-reachable SDK module at
// startup, BEFORE `main()` runs. When that happened the banner escaped to
// stdout before the runtime guard was installed, so `wallet status --json`
// still leaked the banner in the compiled binary even though the source run
// (lazy dynamic import) did not. Installing the guard as the side effect of the
// FIRST import in the entry module makes it run before any sibling import is
// evaluated, so no Breez banner can reach stdout regardless of eval order. The
// Breez SDK itself remains lazily imported (see `loadBreezSparkModule`), so the
// guard is always in place before the SDK is ever evaluated.
//
// The guard drops ONLY the known Breez banner/storage-warning lines and
// re-routes them to stderr; all other stdout/console output is untouched.
// ---------------------------------------------------------------------------

const BREEZ_BANNER_RE =
  /^Breez SDK: (Node\.js storage automatically enabled|Failed to load Node\.js storage:|Storage operations may not work)/

/**
 * True iff `text` (after leading-whitespace trim) is one of the known Breez SDK
 * storage banner/warning lines that must not corrupt machine-readable stdout.
 * Exported so the guard's matching contract can be unit-tested directly.
 */
export function isBreezStdoutBanner(text: string): boolean {
  return BREEZ_BANNER_RE.test(text.trimStart())
}

let installed = false

/**
 * Install the Breez stdout guard. Idempotent: safe to call more than once (a
 * second call is a no-op, so re-installing from `main()` as belt-and-suspenders
 * cannot double-wrap the streams).
 *
 * The SDK emits its banner via `console.log(...)` at module-eval time, and Bun's
 * `console.*` does NOT route through `process.stdout.write` (it writes the fd
 * directly) -- so we override the `console` methods themselves AND, belt-and-
 * suspenders, `process.stdout.write`. The SDK's storage FAILURE siblings come
 * via `console.warn`, so `console.warn`/`console.info` are guarded too. Matching
 * lines are re-routed to stderr; everything else passes through unchanged.
 */
export function installBreezStdoutGuard(): void {
  if (installed) return
  installed = true

  for (const method of ["log", "info", "warn"] as const) {
    const original = (console[method] as (...parts: unknown[]) => void).bind(console)
    console[method] = ((...parts: unknown[]) => {
      const first = parts[0]
      if (typeof first === "string" && isBreezStdoutBanner(first)) {
        // Keep the diagnostic, just off stdout.
        console.error(...parts)
        return
      }
      original(...parts)
    }) as typeof console.log
  }

  // Belt-and-suspenders: also drop it if it ever arrives via stdout.write
  // (a future SDK build, or a fd-level write rather than console).
  const originalWrite = process.stdout.write.bind(process.stdout)
  process.stdout.write = ((chunk: unknown, ...rest: unknown[]) => {
    const text =
      typeof chunk === "string"
        ? chunk
        : chunk instanceof Uint8Array
          ? Buffer.from(chunk).toString("utf8")
          : ""
    if (text && isBreezStdoutBanner(text)) {
      process.stderr.write(chunk as string | Uint8Array)
      const cb = rest.find((a) => typeof a === "function") as ((...a: unknown[]) => void) | undefined
      if (cb) cb()
      return true
    }
    return (originalWrite as (...a: unknown[]) => boolean)(chunk, ...rest)
  }) as typeof process.stdout.write
}

// Install as a TOP-LEVEL side effect. Because this module is imported FIRST in
// the entry module, the guard is in place before any sibling import (or any
// eagerly-evaluated bundled SDK module in the compiled binary) can run.
installBreezStdoutGuard()

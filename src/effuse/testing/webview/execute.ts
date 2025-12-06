/**
 * Webview Test Executor
 *
 * Effect-native wrapper for spawning webview test subprocesses.
 * Compiles test HTML, spawns runner.ts, and parses results.
 */

import { Effect } from "effect"
import { resolve, dirname } from "node:path"
import { fileURLToPath } from "node:url"
import { WebviewTestError, type WebviewTestOptions } from "../errors.js"

const __dirname = dirname(fileURLToPath(import.meta.url))

/**
 * Result from a single test assertion.
 */
export interface TestResult {
  pass: boolean
  error?: string
  stack?: string
}

/**
 * Execute a test in a real webview-bun instance.
 *
 * Spawns a subprocess that:
 * 1. Creates a webview window (hidden by default)
 * 2. Loads the test HTML
 * 3. Runs tests and collects results
 * 4. Outputs results as JSON to stdout
 * 5. Exits when tests complete
 */
export const executeWebviewTest = (
  testHTML: string,
  options?: WebviewTestOptions
): Effect.Effect<TestResult[], WebviewTestError> =>
  Effect.gen(function* () {
    const headed = options?.headed ?? Bun.env.EFFUSE_HEADED === "1"
    const timeout = options?.timeout ?? parseInt(Bun.env.EFFUSE_TIMEOUT || "30000", 10)

    const runnerPath = resolve(__dirname, "runner.ts")

    // Spawn the subprocess
    const proc = Bun.spawn(["bun", "run", runnerPath], {
      stdout: "pipe",
      stderr: "pipe",
      env: {
        ...process.env,
        EFFUSE_TEST_HTML: testHTML,
        EFFUSE_HEADED: headed ? "1" : "0",
        EFFUSE_TIMEOUT: String(timeout),
      },
    })

    // Wait for process to complete and collect output
    const [stdout, stderr, exitCode] = yield* Effect.promise(async () => {
      const [stdoutText, stderrText] = await Promise.all([
        new Response(proc.stdout).text(),
        new Response(proc.stderr).text(),
      ])
      const code = await proc.exited
      return [stdoutText, stderrText, code] as const
    })

    // Check for process failure
    if (exitCode !== 0) {
      return yield* Effect.fail(
        new WebviewTestError({
          reason: "subprocess_failed",
          message: `Webview test subprocess exited with code ${exitCode}: ${stderr || stdout}`,
          exitCode,
        })
      )
    }

    // Parse results
    try {
      const results = JSON.parse(stdout.trim()) as TestResult[]
      return results
    } catch (e) {
      return yield* Effect.fail(
        new WebviewTestError({
          reason: "parse_failed",
          message: `Failed to parse test results: ${stdout}`,
        })
      )
    }
  })

/**
 * Execute a test and assert all results passed.
 * Fails with the first error if any test failed.
 */
export const executeAndAssert = (
  testHTML: string,
  options?: WebviewTestOptions
): Effect.Effect<void, WebviewTestError> =>
  Effect.gen(function* () {
    const results = yield* executeWebviewTest(testHTML, options)

    const failed = results.find((r) => !r.pass)
    if (failed) {
      return yield* Effect.fail(
        new WebviewTestError({
          reason: "execution_failed",
          message: failed.error || "Test failed",
        })
      )
    }
  })

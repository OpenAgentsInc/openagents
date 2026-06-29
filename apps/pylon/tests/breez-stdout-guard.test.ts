import { describe, expect, test } from "bun:test"
import { rmSync } from "node:fs"
import { join } from "node:path"

import { isBreezStdoutBanner } from "../src/breez-stdout-guard"

// rc.33 regression: the Breez Spark SDK prints
//   "Breez SDK: Node.js storage automatically enabled"
// to stdout at module-eval time. The rc.32 runtime guard (installed inside
// main()) fixed `wallet backup-status --json` but NOT `wallet status --json`
// in the COMPILED binary, because the bundled SDK module could be evaluated at
// startup BEFORE main() ran. The rc.33 fix installs the guard as a top-level
// side effect of the FIRST entry import, so the banner can never reach stdout.

describe("isBreezStdoutBanner", () => {
  test("matches the known Breez storage banner + warning siblings", () => {
    expect(isBreezStdoutBanner("Breez SDK: Node.js storage automatically enabled")).toBe(true)
    expect(isBreezStdoutBanner("Breez SDK: Failed to load Node.js storage: boom")).toBe(true)
    expect(isBreezStdoutBanner("Breez SDK: Storage operations may not work properly.")).toBe(true)
    // Leading whitespace (e.g. a newline-prefixed write) is still caught.
    expect(isBreezStdoutBanner("\n  Breez SDK: Node.js storage automatically enabled")).toBe(true)
  })

  test("does NOT match unrelated output (incl. legitimate JSON / other logs)", () => {
    expect(isBreezStdoutBanner('{"schema":"openagents.pylon.wallet_status.v0.3"}')).toBe(false)
    expect(isBreezStdoutBanner("Breez SDK: connected")).toBe(false)
    expect(isBreezStdoutBanner("some other line")).toBe(false)
  })
})

describe("installBreezStdoutGuard keeps stdout pure when the banner is emitted", () => {
  // Drive the SAME load order the entry module uses: import the guard module
  // (top-level side effect) FIRST, then emit the banner via console.log,
  // console.warn AND process.stdout.write, then print a JSON payload. Only the
  // JSON must reach stdout; the banners must be rerouted to stderr.
  test("banner via console.* and stdout.write never corrupts stdout JSON", async () => {
    const script = [
      `import "../src/breez-stdout-guard"`,
      `console.log("Breez SDK: Node.js storage automatically enabled")`,
      `console.warn("Breez SDK: Failed to load Node.js storage: x")`,
      `process.stdout.write("Breez SDK: Storage operations may not work properly.\\n")`,
      `console.info("Breez SDK: Node.js storage automatically enabled")`,
      `process.stdout.write(JSON.stringify({ ok: true }) + "\\n")`,
    ].join("\n")
    // Write the harness INSIDE the tests dir so its `../src/breez-stdout-guard`
    // import resolves to the real guard module (a temp-dir file could not).
    const file = join(import.meta.dir, ".breez-guard-harness.tmp.ts")
    await Bun.write(file, script)
    let proc
    try {
      proc = Bun.spawnSync(["bun", file], { cwd: import.meta.dir })
    } finally {
      rmSync(file, { force: true })
    }
    const stdout = proc.stdout.toString()
    const stderr = proc.stderr.toString()
    // stdout is PURE JSON (no Breez line anywhere).
    expect(stdout.includes("Breez SDK")).toBe(false)
    expect(stdout.trim().startsWith("{")).toBe(true)
    expect(() => JSON.parse(stdout)).not.toThrow()
    // The diagnostics were not dropped — they went to stderr.
    expect(stderr.includes("Breez SDK: Node.js storage automatically enabled")).toBe(true)
  })
})

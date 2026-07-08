import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  runRlProbe,
  writeRlProbeReceipt,
} from "./rate-limit-probe.ts"

describe("runRlProbe receipt shape", () => {
  test("writes schema-versioned receipt (uses real binary only if GROK_RL_LIVE=1)", async () => {
    if (process.env.GROK_RL_LIVE !== "1") {
      // Unit-level: ensure helper write path works with a synthetic receipt
      const dir = await mkdtemp(join(tmpdir(), "rl-"))
      try {
        const path = join(dir, "r.json")
        const fake = {
          schema: "openagents.grok_harness.rl_probe.v1" as const,
          plane: "cli_session" as const,
          marginalCostClass: "free" as const,
          binary: "grok",
          prompt: "ok",
          host: "test",
          measuredAt: new Date().toISOString(),
          concurrencies: [],
          maxFullSuccessConcurrency: 0,
          maxPartialSuccessConcurrency: 0,
          notes: [],
        }
        await writeRlProbeReceipt(fake, path)
        const raw = await Bun.file(path).text()
        expect(raw).toContain("openagents.grok_harness.rl_probe.v1")
      } finally {
        await rm(dir, { recursive: true, force: true })
      }
      return
    }

    const receipt = await runRlProbe({
      concurrencies: [1, 2],
      prompt: "Reply with only: ok",
    })
    expect(receipt.schema).toBe("openagents.grok_harness.rl_probe.v1")
    expect(receipt.concurrencies.length).toBe(2)
  })
})

import { describe, expect, test } from "bun:test"

import { createGrokHeadlessWorkerExecutor } from "./worker-executor.ts"

describe("GrokHeadlessWorkerExecutor", () => {
  test("runClaimedWork records claim pins and metering honesty", async () => {
    const executor = createGrokHeadlessWorkerExecutor({
      async runCommand(argv, cwd) {
        expect(argv).toContain("-p")
        expect(argv).toContain("--no-auto-update")
        expect(cwd).toBe("/tmp/work")
        const promptIdx = argv.indexOf("-p")
        const prompt = argv[promptIdx + 1] ?? ""
        expect(prompt).toContain("claimRef=claim-1")
        expect(prompt).toContain("workUnitRef=issue-9")
        return {
          code: 0,
          stdout: "done",
          stderr: "",
          wallClockMs: 12,
        }
      },
    })

    const closeout = await executor.runClaimedWork({
      pin: {
        claimRef: "claim-1",
        workUnitRef: "issue-9",
        runRef: "run-1",
        cwd: "/tmp/work",
        verifyCommand: "bun test",
      },
      prompt: "Fix the test",
      plane: "cli_session",
      marginalCostClass: "free",
    })

    expect(closeout.ok).toBe(true)
    expect(closeout.claimRef).toBe("claim-1")
    expect(closeout.text).toBe("done")
    expect(closeout.usage.metering).toBe("not_measured")
    expect(closeout.usage.marginalCostClass).toBe("free")
    expect(closeout.usage.plane).toBe("cli_session")
  })

  test("classifies rate limit failures", async () => {
    const executor = createGrokHeadlessWorkerExecutor({
      async runCommand() {
        return {
          code: 1,
          stdout: "",
          stderr: "Error: 429 rate limit exceeded",
          wallClockMs: 5,
        }
      },
    })

    const closeout = await executor.runClaimedWork({
      pin: {
        claimRef: "c",
        workUnitRef: "w",
        runRef: "r",
        cwd: "/tmp",
      },
      prompt: "x",
    })

    expect(closeout.ok).toBe(false)
    expect(closeout.failureClass).toBe("account_rate_limited")
  })
})

import { describe, expect, test } from "bun:test"
import {
  buildQaPrePushCommand,
  isUserFacingSurfaceChange,
  qaPrePushShouldRun,
  resolveQaPrePushTimeoutMs,
  runBoundedCommand,
  selectUserFacingSurfaceFiles,
} from "./qa-pre-push-smoke"

describe("qa pre-push smoke scoping", () => {
  test("recognizes user-facing surfaces", () => {
    expect(isUserFacingSurfaceChange("apps/openagents.com/apps/web/src/page/home.ts")).toBe(true)
    expect(isUserFacingSurfaceChange("apps/openagents.com/workers/api/src/index.ts")).toBe(true)
    expect(isUserFacingSurfaceChange("apps/qa-runner/src/byo.ts")).toBe(true)
    expect(isUserFacingSurfaceChange("packages/ui/src/button.ts")).toBe(true)
  })

  test("skips unrelated docs and reference-only changes", () => {
    expect(isUserFacingSurfaceChange("docs/notes/internal.md")).toBe(false)
    expect(isUserFacingSurfaceChange("projects/repos/opencode/README.md")).toBe(false)
    expect(isUserFacingSurfaceChange("README.md")).toBe(false)
  })

  test("dedupes and sorts surface files", () => {
    expect(
      selectUserFacingSurfaceFiles([
        "docs/notes/internal.md",
        "apps/qa-runner/src/byo.ts",
        "apps/openagents.com/apps/web/src/main.ts",
        "apps/qa-runner/src/byo.ts",
      ]),
    ).toEqual([
      "apps/openagents.com/apps/web/src/main.ts",
      "apps/qa-runner/src/byo.ts",
    ])
  })

  test("runs only for scoped files unless forced", () => {
    expect(qaPrePushShouldRun({ changedFiles: ["docs/notes/internal.md"] })).toEqual({
      run: false,
      surfaceFiles: [],
    })
    expect(
      qaPrePushShouldRun({
        changedFiles: ["docs/notes/internal.md"],
        force: true,
      }),
    ).toEqual({
      run: true,
      surfaceFiles: [],
    })
  })
})

describe("qa pre-push smoke command", () => {
  test("builds a deterministic fake-model qa-runner invocation", () => {
    const command = buildQaPrePushCommand("/tmp/qa-pre-push")
    expect(command).toContain("--fake-model")
    expect(command).toContain("--max-turns")
    expect(command).toContain("8")
    expect(command).toContain("/tmp/qa-pre-push")
    expect(command.join(" ")).toContain("apps/qa-runner qa run")
  })

  test("uses a positive timeout env override only", () => {
    expect(resolveQaPrePushTimeoutMs({ OA_QA_PRE_PUSH_TIMEOUT_MS: "2500" })).toBe(2500)
    expect(resolveQaPrePushTimeoutMs({ OA_QA_PRE_PUSH_TIMEOUT_MS: "0" })).toBe(60_000)
    expect(resolveQaPrePushTimeoutMs({ OA_QA_PRE_PUSH_TIMEOUT_MS: "nope" })).toBe(60_000)
  })

  test("bounds a hanging command as timed out", async () => {
    const result = await runBoundedCommand({
      command: ["bun", "-e", "setInterval(() => {}, 1000)"],
      cwd: process.cwd(),
      timeoutMs: 10,
    })
    expect(result.timedOut).toBe(true)
    expect(result.exitCode).toBe(124)
  })
})

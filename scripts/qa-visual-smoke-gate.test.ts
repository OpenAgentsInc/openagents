import { describe, expect, test } from "bun:test"
import {
  KHALA_VISUAL_SMOKE_GATE_HARD_FAIL_DATE,
  khalaVisualSmokeGateShouldRun,
  khalaVisualSmokeGateSteps,
  resolveKhalaVisualSmokeGateMode,
  runKhalaVisualSmokeGate,
  selectKhalaVisualSmokeGateFiles,
} from "./qa-visual-smoke-gate"

describe("Khala visual smoke gate scoping", () => {
  test("selects only Khala desktop and shared UI changes", () => {
    expect(
      selectKhalaVisualSmokeGateFiles([
        "docs/qa/khala-code-visual-smoke-gate.md",
        "clients/khala-code-desktop/src/ui/main.ts",
        "packages/ui/src/ai-elements/command-composer.ts",
        "apps/openagents.com/apps/web/src/page/home.ts",
      ]),
    ).toEqual([
      "clients/khala-code-desktop/src/ui/main.ts",
      "packages/ui/src/ai-elements/command-composer.ts",
    ])
  })

  test("skips non-desktop changes unless forced", () => {
    expect(
      khalaVisualSmokeGateShouldRun({
        changedFiles: ["docs/qa/notes.md", "apps/forum/src/index.ts"],
      }),
    ).toEqual({ run: false, visualFiles: [] })

    expect(
      khalaVisualSmokeGateShouldRun({
        changedFiles: ["docs/qa/notes.md"],
        force: true,
      }),
    ).toEqual({ run: true, visualFiles: [] })
  })

  test("runs the three required fixture visual smokes in order", () => {
    expect(khalaVisualSmokeGateSteps().map(step => step.command.join(" "))).toEqual([
      "bun run --cwd clients/khala-code-desktop smoke:part2-ui",
      "bun run --cwd clients/khala-code-desktop smoke:cockpit-visual",
      "bun run --cwd clients/khala-code-desktop smoke:composer-visual",
    ])
  })
})

describe("Khala visual smoke gate mode", () => {
  test("is warning-only before the dated hard-fail flip", () => {
    expect(resolveKhalaVisualSmokeGateMode({ OA_KHALA_VISUAL_SMOKE_GATE_TODAY: "2026-07-08" })).toBe(
      "warning-only",
    )
    expect(resolveKhalaVisualSmokeGateMode({ OA_KHALA_VISUAL_SMOKE_GATE_TODAY: KHALA_VISUAL_SMOKE_GATE_HARD_FAIL_DATE })).toBe(
      "hard-fail",
    )
  })

  test("allows explicit warning or hard-fail override", () => {
    expect(resolveKhalaVisualSmokeGateMode({ OA_KHALA_VISUAL_SMOKE_GATE_MODE: "warn" })).toBe("warning-only")
    expect(resolveKhalaVisualSmokeGateMode({ OA_KHALA_VISUAL_SMOKE_GATE_MODE: "enforce" })).toBe("hard-fail")
  })
})

describe("Khala visual smoke gate verdict", () => {
  test("invokes every smoke when a desktop UI change is clean", async () => {
    const commands: Array<string> = []
    const verdict = await runKhalaVisualSmokeGate({
      changedFiles: ["clients/khala-code-desktop/src/ui/styles.css"],
      env: { OA_KHALA_VISUAL_SMOKE_GATE_MODE: "enforce" },
      root: process.cwd(),
      runCommand: async input => {
        commands.push(input.command.join(" "))
        return { elapsedMs: 1, exitCode: 0, timedOut: false }
      },
    })

    expect(commands).toEqual(khalaVisualSmokeGateSteps().map(step => step.command.join(" ")))
    expect(verdict.status).toBe("passed")
    expect(verdict.exitCode).toBe(0)
  })

  test("catches the 2026-07-02 stale-smoke regression for landed UI changes", async () => {
    const commands: Array<string> = []
    const verdict = await runKhalaVisualSmokeGate({
      changedFiles: ["clients/khala-code-desktop/src/ui/main.ts"],
      env: { OA_KHALA_VISUAL_SMOKE_GATE_MODE: "enforce" },
      root: process.cwd(),
      runCommand: async input => {
        commands.push(input.command.join(" "))
        return { elapsedMs: 1, exitCode: 1, timedOut: false }
      },
    })

    expect(commands).toEqual(["bun run --cwd clients/khala-code-desktop smoke:part2-ui"])
    expect(verdict.status).toBe("failed")
    expect(verdict.exitCode).toBe(1)
    expect(verdict.visualFiles).toEqual(["clients/khala-code-desktop/src/ui/main.ts"])
  })

  test("keeps failures warning-only before the flip date", async () => {
    const verdict = await runKhalaVisualSmokeGate({
      changedFiles: ["packages/ui/src/ai-elements/command-composer.ts"],
      env: { OA_KHALA_VISUAL_SMOKE_GATE_TODAY: "2026-07-08" },
      root: process.cwd(),
      runCommand: async () => ({ elapsedMs: 1, exitCode: 1, timedOut: false }),
    })

    expect(verdict.status).toBe("failed")
    expect(verdict.mode).toBe("warning-only")
    expect(verdict.exitCode).toBe(0)
  })
})

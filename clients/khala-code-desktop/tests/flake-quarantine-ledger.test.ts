import { describe, expect, test } from "bun:test"

type StaticFlakeLedger = {
  readonly entries: ReadonlyArray<{
    readonly id: string
    readonly status: string
    readonly trackingIssue: string
    readonly resolution?: string
    readonly reproductionEvidence?: {
      readonly attemptedCommand?: string
      readonly cleanRuns?: number
      readonly finalSuiteShape?: {
        readonly files?: number
        readonly tests?: number
        readonly failures?: number
      }
    }
    readonly determinismGuard?: string
  }>
}

describe("Khala Code flake quarantine ledger", () => {
  test("records the #8044 desktop-suite flake as resolved with stress evidence", async () => {
    const ledger = JSON.parse(
      await Bun.file(new URL("../../../docs/qa/khala-code-flake-quarantine-ledger.json", import.meta.url)).text(),
    ) as StaticFlakeLedger

    const entry = ledger.entries.find(
      candidate => candidate.id === "khala.desktop_suite.single_test_error.2026-07-02",
    )

    expect(entry).toBeDefined()
    expect(entry).toMatchObject({
      status: "resolved",
      trackingIssue: "https://github.com/OpenAgentsInc/openagents/issues/8044",
      resolution: "not_reproduced_after_stress",
      reproductionEvidence: {
        attemptedCommand: "bun run --cwd clients/khala-code-desktop test",
        cleanRuns: 42,
        finalSuiteShape: {
          files: 67,
          tests: 526,
          failures: 0,
        },
      },
      determinismGuard:
        "clients/khala-code-desktop/tests/cockpit-visual-smoke.test.ts::rate-limit countdown repaints from an injected clock without sleeping",
    })
  })
})

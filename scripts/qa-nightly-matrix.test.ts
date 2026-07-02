import { mkdtemp, readFile, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"

import {
  QA_NIGHTLY_DEFAULT_RUNS,
  QA_NIGHTLY_DEFAULT_STEPS,
  buildQaNightlyFailureIssueBody,
  buildQaNightlySteps,
  runQaNightlyMatrix,
  type QaNightlyCommandRunner,
  type QaNightlyIssueFiler,
} from "./qa-nightly-matrix"

describe("qa nightly matrix plan", () => {
  test("runs the issue #8012 matrix in the required order", () => {
    const steps = buildQaNightlySteps({ artifactDir: "var/qa-nightly/run" })
    expect(steps.map(step => step.id)).toEqual([
      "harness-suite",
      "desktop-verify",
      "visual-part2-ui",
      "visual-cockpit",
      "visual-composer",
      "monkey-night",
      "model-based",
      "property-tier",
    ])
    expect(steps.find(step => step.id === "monkey-night")?.label).toContain(
      String(QA_NIGHTLY_DEFAULT_RUNS * QA_NIGHTLY_DEFAULT_STEPS),
    )
  })
})

describe("qa nightly matrix report", () => {
  test("writes public-safe JSON and markdown reports", async () => {
    const root = await mkdtemp(join(tmpdir(), "qa-nightly-"))
    const calls: string[] = []
    const commandRunner: QaNightlyCommandRunner = async step => {
      calls.push(step.id)
      return {
        durationMs: 7,
        exitCode: 0,
        stderr: "",
        stdout: `ok ${step.id}`,
      }
    }

    try {
      const report = await runQaNightlyMatrix({
        artifactRoot: join(root, "artifacts"),
        commandRunner,
        env: {},
        now: () => "2026-07-02T12:00:00.000Z",
        root,
      })

      expect(report.status).toBe("passed")
      expect(calls).toEqual([
        "harness-suite",
        "desktop-verify",
        "visual-part2-ui",
        "visual-cockpit",
        "visual-composer",
        "monkey-night",
        "model-based",
        "property-tier",
      ])

      const json = await readFile(join(root, report.reportJsonPath), "utf8")
      expect(json).toContain("openagents.khala_code.qa_nightly_matrix.v1")
      const markdown = await readFile(join(root, report.reportMarkdownPath), "utf8")
      expect(markdown).toContain("Khala Code QA Nightly Matrix")
      expect(markdown).toContain("| harness-suite | passed |")
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("files a strict bug issue body when the owned runner opt-in is armed", async () => {
    const root = await mkdtemp(join(tmpdir(), "qa-nightly-fail-"))
    const commandRunner: QaNightlyCommandRunner = async step => ({
      durationMs: 3,
      exitCode: step.id === "desktop-verify" ? 1 : 0,
      stderr: step.id === "desktop-verify" ? "fixture failure" : "",
      stdout: "",
    })
    const filed: string[] = []
    const issueFiler: QaNightlyIssueFiler = async input => {
      filed.push(input.title)
      const body = await readFile(input.bodyPath, "utf8")
      expect(body).toContain("### Affected surface")
      expect(body).toContain("desktop-verify")
      return {
        issueUrl: "https://github.com/OpenAgentsInc/openagents/issues/9999",
        status: "filed",
      }
    }

    try {
      const report = await runQaNightlyMatrix({
        artifactRoot: join(root, "artifacts"),
        commandRunner,
        env: { OA_QA_NIGHTLY_FILE_ISSUE: "1" },
        issueFiler,
        now: () => "2026-07-02T12:30:00.000Z",
        root,
      })

      expect(report.status).toBe("failed")
      expect(report.issueStatus).toEqual({
        issueUrl: "https://github.com/OpenAgentsInc/openagents/issues/9999",
        status: "filed",
      })
      expect(filed).toEqual([
        "[Bug]: Khala Code QA nightly failed khala-code-qa-nightly-2026-07-02t123000.000z",
      ])
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("failure issue body contains strict-form sections and public-safe refs", () => {
    const body = buildQaNightlyFailureIssueBody({
      artifactDir: "var/qa-nightly/run",
      coverageLedgerPath: "var/qa-nightly/run/monkey-night/monkey-night-coverage-ledger.json",
      generatedAt: "2026-07-02T12:00:00.000Z",
      reportJsonPath: "var/qa-nightly/run/qa-nightly-report.json",
      reportMarkdownPath: "var/qa-nightly/run/qa-nightly-report.md",
      runId: "khala-code-qa-nightly-2026-07-02",
      schema: "openagents.khala_code.qa_nightly_matrix.v1",
      status: "failed",
      steps: [
        {
          command: ["bun", "run", "x"],
          cwd: ".",
          durationMs: 1,
          exitCode: 1,
          id: "harness-suite",
          label: "Harness",
          logRef: "var/qa-nightly/run/logs/harness-suite.log",
          status: "failed",
        },
      ],
    })

    expect(body).toContain("### Reproduction steps")
    expect(body).toContain("### Safety and redaction")
    expect(body).not.toContain("/Users/")
    expect(body).not.toContain("Bearer ")
  })
})

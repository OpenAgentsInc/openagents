import { describe, expect, test } from "bun:test"

import { evaluateCommandSourceVerified } from "./command-execution-source-verified.js"
import { evaluateDiagnosisGrounding } from "./diagnosis-grounding.js"
import {
  authorizeCommandProposal,
  authorizeDiagnosisRemediation,
  authorizeFleetHealthyReport,
  authorizeIssueClose,
  authorizeMergeDeployLiveReport,
} from "./enforced-actions.js"
import {
  DEFAULT_WEDGE_THRESHOLD_MS,
  evaluateFleetLiveness,
} from "./fleet-liveness.js"
import { evaluateIssueCloseSafe } from "./issue-close-safe.js"
import { evaluateMergeDeployGate } from "./merge-deploy-gate.js"

describe("Blueprint enforced action authorizers", () => {
  const now = 2_000_000_000_000

  test("fleet healthy report is exposed only from PROVEN_ALIVE", () => {
    const partial = authorizeFleetHealthyReport(
      evaluateFleetLiveness({
        pidAlive: true,
        lastDispatchTime: now - 30_000,
        now,
      }),
    )
    expect(partial.ok).toBe(false)

    const proven = authorizeFleetHealthyReport(
      evaluateFleetLiveness({
        pidAlive: true,
        lastDispatchTime: now - 30_000,
        now,
        quotaLedgerSnapshot: { accounts: [] },
        heartbeatPayload: { last_dispatch_time: now - 30_000 },
      }),
    )
    expect(proven.ok).toBe(true)
    if (proven.ok) {
      expect(proven.action.kind).toBe("report_fleet_healthy")
    }
  })

  test("diagnosis remediation is exposed only from GROUNDED", () => {
    const blocked = authorizeDiagnosisRemediation(
      evaluateDiagnosisGrounding({
        claimedRootCause: "rate-limited",
        quotaLedgerSnapshot: {},
        supervisorDispatchLog: [{ outcome: "429" }],
        accountRateLimitHeaders: null,
      }),
      "cool down account",
    )
    expect(blocked.ok).toBe(false)

    const allowed = authorizeDiagnosisRemediation(
      evaluateDiagnosisGrounding({
        claimedRootCause: "rate-limited",
        quotaLedgerSnapshot: {},
        supervisorDispatchLog: [{ outcome: "429" }],
        accountRateLimitHeaders: { statusCode: 429, retryAfter: "60" },
      }),
      "cool down account",
    )
    expect(allowed.ok).toBe(true)
  })

  test("issue close action is exposed only from SAFE_TO_CLOSE", () => {
    const blocked = authorizeIssueClose(
      evaluateIssueCloseSafe({
        issueNumber: 7886,
        issueLabels: ["fable-roadmap"],
        parentEpicNumber: 7821,
        isLastOpenSubIssue: false,
        prNumber: 9001,
        prBody: "Closes #7886",
      }),
    )
    expect(blocked.ok).toBe(false)

    const allowed = authorizeIssueClose(
      evaluateIssueCloseSafe({
        issueNumber: 7886,
        issueLabels: ["fable-roadmap"],
        parentEpicNumber: null,
        prNumber: 9001,
        prBody: "Closes #7886",
      }),
    )
    expect(allowed.ok).toBe(true)
    if (allowed.ok) {
      expect(allowed.action.issueNumber).toBe(7886)
      expect(allowed.action.prNumber).toBe(9001)
    }
  })

  test("command proposal is exposed only from SAFE_TO_PROPOSE", () => {
    const blocked = authorizeCommandProposal(
      evaluateCommandSourceVerified({
        commandString: "bun scripts/tool.ts --flag",
        scriptPath: "scripts/tool.ts",
        expectedFlags: ["--flag"],
        sourceReadHash: "sha256:abc",
        declaredFlags: [],
        dryRunExitCode: 0,
      }),
    )
    expect(blocked.ok).toBe(false)

    const allowed = authorizeCommandProposal(
      evaluateCommandSourceVerified({
        commandString: "bun scripts/tool.ts --flag",
        scriptPath: "scripts/tool.ts",
        expectedFlags: ["--flag"],
        sourceReadHash: "sha256:abc",
        declaredFlags: ["--flag"],
        dryRunExitCode: 0,
      }),
    )
    expect(allowed.ok).toBe(true)
    if (allowed.ok) {
      expect(allowed.action.commandString).toBe("bun scripts/tool.ts --flag")
      expect(allowed.action.scriptPath).toBe("scripts/tool.ts")
    }
  })

  test("merge deploy live report is exposed only from LIVE", () => {
    const blocked = authorizeMergeDeployLiveReport(
      evaluateMergeDeployGate({
        prNumbers: [7886],
        mergeCommitHashes: ["abc123"],
        checkDeployExitCode: 0,
        checkDeployStdout: "EXIT=0",
        deployExitCode: null,
        smokeTestResults: [],
      }),
    )
    expect(blocked.ok).toBe(false)

    const allowed = authorizeMergeDeployLiveReport(
      evaluateMergeDeployGate({
        prNumbers: [7886],
        mergeCommitHashes: ["abc123"],
        checkDeployExitCode: 0,
        checkDeployStdout: "EXIT=0",
        deployExitCode: 0,
        smokeTestResults: [{ name: "home", passed: true }],
      }),
    )
    expect(allowed.ok).toBe(true)
    if (allowed.ok) {
      expect(allowed.action.prNumbers).toEqual([7886])
      expect(allowed.action.mergeCommitHashes).toEqual(["abc123"])
    }
  })

  test("stale liveness stays blocked even with evidence", () => {
    const blocked = authorizeFleetHealthyReport(
      evaluateFleetLiveness({
        pidAlive: true,
        lastDispatchTime: now - DEFAULT_WEDGE_THRESHOLD_MS - 1,
        now,
        quotaLedgerSnapshot: { accounts: [] },
        heartbeatPayload: { last_dispatch_time: now - DEFAULT_WEDGE_THRESHOLD_MS - 1 },
      }),
    )
    expect(blocked.ok).toBe(false)
  })
})

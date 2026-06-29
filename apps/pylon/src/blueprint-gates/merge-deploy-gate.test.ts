import { describe, expect, test } from "bun:test"

import {
  checkDeployIsGreen,
  evaluateMergeDeployGate,
  MERGE_DEPLOY_EVIDENCE,
  parseExitMarkers,
  type MergeDeployGateInputs,
} from "./merge-deploy-gate.js"

const liveInputs: MergeDeployGateInputs = {
  prNumbers: [6650],
  mergeCommitHashes: ["abc1234"],
  checkDeployExitCode: 0,
  checkDeployStdout: "all checks passed\nEXIT=0\n",
  deployExitCode: 0,
  smokeTestResults: [
    { name: "home", passed: true },
    { name: "asset", passed: true },
  ],
}

describe("merge-deploy-gate: exit-marker parsing (trailing-echo subtlety)", () => {
  test("parses EXIT markers from stdout", () => {
    expect(parseExitMarkers("step done\nEXIT=0\nwrapper\nEXIT=1\n")).toEqual([
      0, 1,
    ])
  })

  test("checkDeployIsGreen is false when a marker is nonzero even if exit is 0", () => {
    expect(checkDeployIsGreen(0, "EXIT=1\n")).toBe(false)
    expect(checkDeployIsGreen(0, "EXIT=0\n")).toBe(true)
    expect(checkDeployIsGreen(1, "EXIT=0\n")).toBe(false)
  })
})

describe("merge-deploy-gate: happy path", () => {
  test("a fully green batch reaches LIVE", () => {
    const result = evaluateMergeDeployGate(liveInputs)
    expect(result.state).toBe("LIVE")
    expect(result.isLive).toBe(true)
    expect(result.isRed).toBe(false)
    expect(result.blocksFurtherMerges).toBe(false)
    expect(result.satisfiedEvidence).toEqual([
      MERGE_DEPLOY_EVIDENCE.checkDeployPass,
      MERGE_DEPLOY_EVIDENCE.deployExitCode,
      MERGE_DEPLOY_EVIDENCE.smokeTests,
    ])
  })
})

describe("merge-deploy-gate: a nonzero check:deploy can never reach LIVE", () => {
  test("nonzero captured exit code goes RED, never LIVE", () => {
    const result = evaluateMergeDeployGate({
      ...liveInputs,
      checkDeployExitCode: 1,
      checkDeployStdout: "tsc errors\n",
    })
    expect(result.isLive).toBe(false)
    expect(result.state).toBe("RED")
    expect(result.failedGate).toBe("CHECK_DEPLOY_GREEN")
  })

  test("a green captured exit masking a nonzero EXIT marker still goes RED", () => {
    const result = evaluateMergeDeployGate({
      ...liveInputs,
      checkDeployExitCode: 0,
      checkDeployStdout: "real failure\nEXIT=1\nwrapper trailing echo\nEXIT=0\n",
    })
    expect(result.isLive).toBe(false)
    expect(result.state).toBe("RED")
    expect(result.failedGate).toBe("CHECK_DEPLOY_GREEN")
  })
})

describe("merge-deploy-gate: a red state blocks merges", () => {
  test("a RED gate blocks further merges until rollback evidence is presented", () => {
    const red = evaluateMergeDeployGate({
      ...liveInputs,
      deployExitCode: 1,
    })
    expect(red.state).toBe("RED")
    expect(red.failedGate).toBe("DEPLOYED")
    expect(red.blocksFurtherMerges).toBe(true)
    expect(red.missingEvidence).toContain(MERGE_DEPLOY_EVIDENCE.rollback)

    const recovered = evaluateMergeDeployGate({
      ...liveInputs,
      deployExitCode: 1,
      rollbackEvidenceRef: "evidence://deploy/rollback/abc1234",
    })
    expect(recovered.state).toBe("RED")
    expect(recovered.blocksFurtherMerges).toBe(false)
    expect(recovered.missingEvidence).not.toContain(
      MERGE_DEPLOY_EVIDENCE.rollback,
    )
  })

  test("merged but not deployed never claims LIVE", () => {
    const result = evaluateMergeDeployGate({
      ...liveInputs,
      deployExitCode: null,
    })
    expect(result.isLive).toBe(false)
    expect(result.failedGate).toBe("DEPLOYED")
  })

  test("a failed smoke goes RED and blocks merges", () => {
    const result = evaluateMergeDeployGate({
      ...liveInputs,
      smokeTestResults: [
        { name: "home", passed: true },
        { name: "asset", passed: false },
      ],
    })
    expect(result.state).toBe("RED")
    expect(result.failedGate).toBe("SMOKED")
    expect(result.blocksFurtherMerges).toBe(true)
  })

  test("mismatched PR / merge-commit counts go RED at MERGED", () => {
    const result = evaluateMergeDeployGate({
      ...liveInputs,
      prNumbers: [1, 2],
      mergeCommitHashes: ["only-one"],
    })
    expect(result.state).toBe("RED")
    expect(result.failedGate).toBe("MERGED")
  })
})

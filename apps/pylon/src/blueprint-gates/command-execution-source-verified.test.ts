import { describe, expect, test } from "bun:test"

import {
  COMMAND_SOURCE_VERIFIED_EVIDENCE,
  evaluateCommandSourceVerified,
  parseCommandFlags,
  type CommandSourceVerifiedInputs,
} from "./command-execution-source-verified.js"

const okInputs: CommandSourceVerifiedInputs = {
  commandString: "bun scripts/train-psion.ts --lane reference_pilot --json",
  scriptPath: "scripts/train-psion.ts",
  expectedFlags: ["--lane", "--json"],
  sourceReadHash: "sha256:abc123",
  declaredFlags: ["--lane", "--json", "--help"],
  dryRunExitCode: 0,
}

describe("command-execution-source-verified: flag parsing", () => {
  test("parses long and short flags and normalizes --flag=value", () => {
    expect(
      parseCommandFlags("tool --lane=ref -v --json -- --not-a-flag"),
    ).toEqual(["--lane", "-v", "--json"])
  })

  test("returns no flags for a bare command", () => {
    expect(parseCommandFlags("./mirrorcode run")).toEqual([])
  })
})

describe("command-execution-source-verified: happy path", () => {
  test("a verified command reaches SAFE_TO_PROPOSE", () => {
    const result = evaluateCommandSourceVerified(okInputs)
    expect(result.state).toBe("SAFE_TO_PROPOSE")
    expect(result.canPropose).toBe(true)
    expect(result.unknownFlags).toEqual([])
    expect(result.missingEvidence).toEqual([])
    expect(result.satisfiedEvidence).toEqual([
      COMMAND_SOURCE_VERIFIED_EVIDENCE.sourceRead,
      COMMAND_SOURCE_VERIFIED_EVIDENCE.flagVerification,
      COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck,
    ])
  })
})

describe("command-execution-source-verified: the fabricated-executable failure class", () => {
  test("a stub with zero declared flags cannot reach SAFE_TO_PROPOSE", () => {
    const result = evaluateCommandSourceVerified({
      commandString: "./mirrorcode --apply --pr 6376",
      scriptPath: "scripts/mirrorcode-stub.sh",
      expectedFlags: ["--apply", "--pr"],
      sourceReadHash: "sha256:stub",
      declaredFlags: [], // the stub declares no CLI flags at all
      dryRunExitCode: 0,
    })
    expect(result.canPropose).toBe(false)
    expect(result.state).not.toBe("SAFE_TO_PROPOSE")
    expect(result.state).toBe("SOURCE_READ")
    expect(result.lockedAt).toBe("FLAGS_VERIFIED")
    expect(result.unknownFlags).toEqual(["--apply", "--pr"])
    expect(result.missingEvidence).toContain(
      COMMAND_SOURCE_VERIFIED_EVIDENCE.flagVerification,
    )
  })

  test("a command with no flags cannot be flag-verified", () => {
    const result = evaluateCommandSourceVerified({
      commandString: "./mirrorcode run",
      scriptPath: "scripts/mirrorcode-stub.sh",
      expectedFlags: [],
      sourceReadHash: "sha256:stub",
      declaredFlags: [],
      dryRunExitCode: 0,
    })
    expect(result.canPropose).toBe(false)
    expect(result.lockedAt).toBe("FLAGS_VERIFIED")
  })
})

describe("command-execution-source-verified: ordered predicates", () => {
  test("missing source hash locks at SOURCE_READ", () => {
    const result = evaluateCommandSourceVerified({
      ...okInputs,
      sourceReadHash: null,
    })
    expect(result.canPropose).toBe(false)
    expect(result.state).toBe("UNVERIFIED")
    expect(result.lockedAt).toBe("SOURCE_READ")
  })

  test("an unknown proposed flag locks at FLAGS_VERIFIED", () => {
    const result = evaluateCommandSourceVerified({
      ...okInputs,
      commandString: "bun scripts/train-psion.ts --lane ref --bogus",
      expectedFlags: ["--lane", "--bogus"],
    })
    expect(result.canPropose).toBe(false)
    expect(result.lockedAt).toBe("FLAGS_VERIFIED")
    expect(result.unknownFlags).toEqual(["--bogus"])
  })

  test("a failed dry-run locks at RUNTIME_CONFIRMED", () => {
    const result = evaluateCommandSourceVerified({
      ...okInputs,
      dryRunExitCode: 2,
    })
    expect(result.canPropose).toBe(false)
    expect(result.state).toBe("FLAGS_VERIFIED")
    expect(result.lockedAt).toBe("RUNTIME_CONFIRMED")
    expect(result.missingEvidence).toEqual([
      COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck,
    ])
  })

  test("a never-run dry-run (null) locks at RUNTIME_CONFIRMED", () => {
    const result = evaluateCommandSourceVerified({
      ...okInputs,
      dryRunExitCode: null,
    })
    expect(result.canPropose).toBe(false)
    expect(result.lockedAt).toBe("RUNTIME_CONFIRMED")
  })
})

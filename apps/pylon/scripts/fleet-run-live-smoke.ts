#!/usr/bin/env bun

type LiveSmokePlan = {
  readonly ok: boolean
  readonly skipped: boolean
  readonly smoke: "smoke:fleet-run-live"
  readonly armingEnv: "PYLON_FLEET_RUN_LIVE_ARM=1"
  readonly targetWorkers: 2
  readonly repo?: string
  readonly branch?: string
  readonly commit?: string
  readonly pylonRef?: string
  readonly issues?: readonly number[]
  readonly verify?: string
  readonly expectedCloseout: readonly string[]
  readonly message: string
}

const env = Bun.env

function output(plan: LiveSmokePlan, exitCode: number): never {
  process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`)
  process.exit(exitCode)
}

function required(name: string): string {
  const value = env[name]?.trim()
  if (value === undefined || value === "") throw new Error(`${name} is required when PYLON_FLEET_RUN_LIVE_ARM=1`)
  return value
}

function parseIssues(value: string): readonly number[] {
  const issues = value.split(",").map((part) => Number(part.trim()))
  if (issues.length !== 2 || issues.some((issue) => !Number.isInteger(issue) || issue <= 0)) {
    throw new Error("PYLON_FLEET_RUN_LIVE_ISSUES must contain exactly two positive issue numbers")
  }
  if (new Set(issues).size !== issues.length) {
    throw new Error("PYLON_FLEET_RUN_LIVE_ISSUES must name two distinct issues")
  }
  return issues
}

if (env.PYLON_FLEET_RUN_LIVE_ARM !== "1") {
  output({
    ok: true,
    skipped: true,
    smoke: "smoke:fleet-run-live",
    armingEnv: "PYLON_FLEET_RUN_LIVE_ARM=1",
    targetWorkers: 2,
    expectedCloseout: [
      "two real workers accepted",
      "two distinct PRs reference distinct issues",
      "each closeout carries verify-green evidence",
      "zero duplicate live claims",
    ],
    message:
      "Skipped by default. Arm with PYLON_FLEET_RUN_LIVE_ARM=1 plus PYLON_FLEET_RUN_LIVE_PYLON_REF, PYLON_FLEET_RUN_LIVE_ISSUES, PYLON_FLEET_RUN_LIVE_REPO, PYLON_FLEET_RUN_LIVE_COMMIT, and PYLON_FLEET_RUN_LIVE_VERIFY.",
  }, 0)
}

try {
  const pylonRef = required("PYLON_FLEET_RUN_LIVE_PYLON_REF")
  const repo = required("PYLON_FLEET_RUN_LIVE_REPO")
  const branch = env.PYLON_FLEET_RUN_LIVE_BRANCH?.trim() || "main"
  const commit = required("PYLON_FLEET_RUN_LIVE_COMMIT")
  const verify = required("PYLON_FLEET_RUN_LIVE_VERIFY")
  const issues = parseIssues(required("PYLON_FLEET_RUN_LIVE_ISSUES"))
  required("OPENAGENTS_AGENT_TOKEN")

  if (!/^[0-9a-f]{40}$/iu.test(commit)) {
    throw new Error("PYLON_FLEET_RUN_LIVE_COMMIT must be a pinned 40-character commit SHA")
  }

  output({
    ok: true,
    skipped: false,
    smoke: "smoke:fleet-run-live",
    armingEnv: "PYLON_FLEET_RUN_LIVE_ARM=1",
    targetWorkers: 2,
    repo,
    branch,
    commit: commit.toLowerCase(),
    pylonRef,
    issues,
    verify,
    expectedCloseout: [
      `issue #${issues[0]} produces one ready non-draft PR with verify-green closeout`,
      `issue #${issues[1]} produces one ready non-draft PR with verify-green closeout`,
      "PR issue refs are distinct",
      "claim registry reports zero duplicate live claims",
    ],
    message:
      "Live smoke inputs are armed and public-safe. Dispatch remains operator-run; this script defines the skip-safe contract and target evidence for smoke:fleet-run-live.",
  }, 0)
} catch (error) {
  output({
    ok: false,
    skipped: false,
    smoke: "smoke:fleet-run-live",
    armingEnv: "PYLON_FLEET_RUN_LIVE_ARM=1",
    targetWorkers: 2,
    expectedCloseout: [
      "two real workers accepted",
      "two distinct PRs reference distinct issues",
      "each closeout carries verify-green evidence",
      "zero duplicate live claims",
    ],
    message: error instanceof Error ? error.message : String(error),
  }, 2)
}

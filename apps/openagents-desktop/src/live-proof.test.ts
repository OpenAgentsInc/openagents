/**
 * Live-proof driver configuration oracle (#8712, Episode 250).
 *
 * The live journey itself needs a real Electron window, a real pylon
 * registry, and real chat lanes — not unit-testable. What IS pinned here is
 * the step-runner contract the coordinator relies on: the EP250 step list and
 * order, which steps decide the exit code, every step being timeout-bounded,
 * output-dir resolution (env override vs userData default), and the
 * mutual-exclusion flag against smoke mode.
 */
import { describe, expect, test } from "bun:test"
import path from "node:path"

import {
  liveProofStepTimeoutMs,
  liveProofSteps,
  liveProofTurnMessage,
  requiredLiveProofSteps,
  resolveLiveProofConfig,
} from "./live-proof.ts"

describe("live-proof step list (EP250 journey)", () => {
  test("walks the journey in the episode order", () => {
    expect(liveProofSteps.map((step) => step.name)).toEqual([
      "shell-mounted",
      "fleet-workspace",
      "fleet-usage-check",
      "new-chat",
      "fable-chip",
      "fable-turn",
      "codex-chip",
      "codex-turn",
      "redaction-check",
      "summary",
    ])
  })

  test("exactly steps 1, 2, and 4 are required (shell, fleet, new chat)", () => {
    expect(requiredLiveProofSteps()).toEqual(["shell-mounted", "fleet-workspace", "new-chat"])
  })

  test("every step is bounded by a positive timeout", () => {
    for (const step of liveProofSteps) {
      expect(step.timeoutMs).toBeGreaterThan(0)
      expect(liveProofStepTimeoutMs(step.name)).toBe(step.timeoutMs)
    }
  })

  test("the fleet step outlasts the provider-accounts list spawn budget (120s)", () => {
    expect(liveProofStepTimeoutMs("fleet-workspace")).toBeGreaterThanOrEqual(120_000)
  })

  test("the usage check honors the episode's 60s bound", () => {
    expect(liveProofStepTimeoutMs("fleet-usage-check")).toBe(60_000)
  })

  test("the real turn message is the exact EP250 prompt", () => {
    expect(liveProofTurnMessage).toBe(
      "Episode 250 live proof: reply with one sentence confirming streaming works, then stop.",
    )
  })
})

describe("live-proof config resolution", () => {
  const userData = "/tmp/user-data-example"

  test("disabled unless OPENAGENTS_DESKTOP_LIVE_PROOF=1", () => {
    expect(resolveLiveProofConfig({}, userData).enabled).toBe(false)
    expect(resolveLiveProofConfig({ OPENAGENTS_DESKTOP_LIVE_PROOF: "true" }, userData).enabled).toBe(false)
    expect(resolveLiveProofConfig({ OPENAGENTS_DESKTOP_LIVE_PROOF: "1" }, userData).enabled).toBe(true)
  })

  test("defaults the output dir to userData/live-proof", () => {
    const config = resolveLiveProofConfig({ OPENAGENTS_DESKTOP_LIVE_PROOF: "1" }, userData)
    expect(config.outDir).toBe(path.join(userData, "live-proof"))
  })

  test("OPENAGENTS_DESKTOP_LIVE_PROOF_DIR overrides the output dir", () => {
    const config = resolveLiveProofConfig(
      { OPENAGENTS_DESKTOP_LIVE_PROOF: "1", OPENAGENTS_DESKTOP_LIVE_PROOF_DIR: "/tmp/ep250-shots" },
      userData,
    )
    expect(config.outDir).toBe(path.resolve("/tmp/ep250-shots"))
  })

  test("a blank dir override falls back to the default", () => {
    const config = resolveLiveProofConfig(
      { OPENAGENTS_DESKTOP_LIVE_PROOF: "1", OPENAGENTS_DESKTOP_LIVE_PROOF_DIR: "   " },
      userData,
    )
    expect(config.outDir).toBe(path.join(userData, "live-proof"))
  })

  test("live-proof and smoke are mutually exclusive (conflict flag)", () => {
    const both = resolveLiveProofConfig(
      { OPENAGENTS_DESKTOP_LIVE_PROOF: "1", OPENAGENTS_DESKTOP_SMOKE: "1" },
      userData,
    )
    expect(both.conflict).toBe(true)
    const liveOnly = resolveLiveProofConfig({ OPENAGENTS_DESKTOP_LIVE_PROOF: "1" }, userData)
    expect(liveOnly.conflict).toBe(false)
    const smokeOnly = resolveLiveProofConfig({ OPENAGENTS_DESKTOP_SMOKE: "1" }, userData)
    expect(smokeOnly.conflict).toBe(false)
    expect(smokeOnly.enabled).toBe(false)
  })
})

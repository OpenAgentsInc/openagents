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
  LIVE_PROOF_TURN_SETTLE_MS,
  LIVE_PROOF_TEXT_SETTLE_MS,
  liveProofTurnIsTerminal,
  liveProofTurnMessage,
  requiredLiveProofSteps,
  resolveLiveProofAccountRef,
  resolveLiveProofConfig,
} from "./live-proof.ts"

describe("live-proof step list (EP250 journey)", () => {
  test("walks the journey in the episode order (EP250 preflight is step 0)", () => {
    expect(liveProofSteps.map((step) => step.name)).toEqual([
      "account-preflight",
      "shell-mounted",
      "fleet-workspace",
      "fleet-usage-check",
      "new-chat",
      "fable-chip",
      "fable-turn",
      "codex-chip",
      "codex-turn",
      "interrupt-stop",
      "file-save",
      "git-review",
      "redaction-check",
      "summary",
    ])
  })

  test("the structural spine and both named-provider acceptance lanes are required", () => {
    expect(requiredLiveProofSteps()).toEqual([
      "account-preflight",
      "shell-mounted",
      "fleet-workspace",
      "new-chat",
      "fable-chip",
      "fable-turn",
      "codex-chip",
      "codex-turn",
    ])
  })

  test("the EP250 capability-eval steps (interrupt-stop/file-save/git-review) are optional and bounded", () => {
    for (const name of ["interrupt-stop", "file-save", "git-review"] as const) {
      const step = liveProofSteps.find((value) => value.name === name)
      expect(step).toBeDefined()
      expect(step?.required).toBe(false)
      expect(liveProofStepTimeoutMs(name)).toBeGreaterThan(0)
    }
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

  test("waits for the finalized thread snapshot instead of false-failing at first idle", () => {
    expect(liveProofTurnIsTerminal({
      turnPending: false,
      assistantGrew: false,
      activityObserved: true,
      idleForMs: LIVE_PROOF_TURN_SETTLE_MS - 1,
    })).toBe(false)
    expect(liveProofTurnIsTerminal({
      turnPending: false,
      assistantGrew: true,
      activityObserved: true,
      idleForMs: LIVE_PROOF_TEXT_SETTLE_MS - 1,
    })).toBe(false)
    expect(liveProofTurnIsTerminal({
      turnPending: false,
      assistantGrew: true,
      activityObserved: true,
      idleForMs: LIVE_PROOF_TEXT_SETTLE_MS,
    })).toBe(true)
    expect(liveProofTurnIsTerminal({
      turnPending: false,
      assistantGrew: false,
      activityObserved: true,
      idleForMs: LIVE_PROOF_TURN_SETTLE_MS,
    })).toBe(true)
    expect(liveProofTurnIsTerminal({
      turnPending: true,
      assistantGrew: true,
      activityObserved: true,
      idleForMs: LIVE_PROOF_TURN_SETTLE_MS,
    })).toBe(false)
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

  test("accepts only bounded exact named-account targets per harness", () => {
    const env = {
      OPENAGENTS_DESKTOP_LIVE_PROOF_FABLE_ACCOUNT_REF: " claude-pylon-3 ",
      OPENAGENTS_DESKTOP_LIVE_PROOF_CODEX_ACCOUNT_REF: "codex-5",
    }
    expect(resolveLiveProofAccountRef(env, "fable")).toBe("claude-pylon-3")
    expect(resolveLiveProofAccountRef(env, "codex")).toBe("codex-5")
    expect(resolveLiveProofAccountRef({
      OPENAGENTS_DESKTOP_LIVE_PROOF_FABLE_ACCOUNT_REF: "bad ref with spaces",
    }, "fable")).toBeNull()
    expect(resolveLiveProofAccountRef({}, "codex")).toBeNull()
  })
})

import { describe, expect, test } from "vite-plus/test"

import {
  beginCanary,
  promoteActivation,
  rollbackActivation,
  type CanaryPlan,
  type ReleaseChannel,
} from "@openagentsinc/dse"

import { buildOpenAgentsAppleFmPrompt } from "../apple-fm-prompt.ts"
import {
  buildCompiledAppleFmPrompt,
  honestChatRelease,
  resolveAppleFmPromptPlan,
  type AppleFmDseRelease,
} from "./release-channel.ts"
import { HONESTY_INSTRUCTION_MARKER } from "./fixtures.ts"

/**
 * AFS-09 activation-gate exit checks at the desktop provider seam: shadow serves
 * the hand-written baseline (no substitution), canary serves a bounded
 * population the compiled prompt, promotion serves the compiled prompt, and a
 * rollback restores the baseline without an app rebuild.
 */

const NOW = "2026-07-20T00:00:00.000Z"
const now = () => NOW
const turns = [{ role: "user" as const, text: "How do I read a file?" }]

const withChannel = (channel: ReleaseChannel): AppleFmDseRelease => ({ ...honestChatRelease, channel })

const canaryPlan: CanaryPlan = {
  schema: "openagents.dse.canary_plan.v1",
  populationFraction: 0.25,
  maxDurationMs: 3_600_000,
  abortErrorRate: 0.1,
  abortOnRegression: true,
}

describe("AFS-09 activation gate at the provider seam", () => {
  test("the checked-in default is shadow, so every request serves the hand-written baseline", () => {
    expect(honestChatRelease.channel.mode).toBe("shadow")
    for (const key of ["thread-a", "thread-b", "thread-c", "thread-d"]) {
      expect(resolveAppleFmPromptPlan({ release: honestChatRelease, requestKey: key })).toEqual({ kind: "baseline" })
    }
  })

  test("a promoted (active) channel serves the compiled preamble instead of the baseline", () => {
    const canaried = beginCanary({ channel: honestChatRelease.channel, plan: canaryPlan, reason: "canary", now })
    if (!canaried.ok) throw new Error(canaried.reason)
    const active = promoteActivation({ channel: canaried.channel, reason: "beats baseline", now })
    if (!active.ok) throw new Error(active.reason)

    const plan = resolveAppleFmPromptPlan({ release: withChannel(active.channel), requestKey: "thread-a" })
    expect(plan.kind).toBe("compiled")
    if (plan.kind !== "compiled") return

    const compiledPrompt = buildCompiledAppleFmPrompt(plan.program, turns)
    const baselinePrompt = buildOpenAgentsAppleFmPrompt(turns, [])
    // The compiled preamble replaces the hand-written one and stays within bound.
    expect(compiledPrompt).toContain(HONESTY_INSTRUCTION_MARKER)
    expect(baselinePrompt).not.toContain(HONESTY_INSTRUCTION_MARKER)
    expect(compiledPrompt).not.toBe(baselinePrompt)
    expect(compiledPrompt.length).toBeLessThanOrEqual(3900)
    // Both still carry the conversation history window.
    expect(compiledPrompt).toContain("How do I read a file?")
  })

  test("a canary serves a bounded population the compiled prompt", () => {
    const canaried = beginCanary({ channel: honestChatRelease.channel, plan: canaryPlan, reason: "canary", now })
    if (!canaried.ok) throw new Error(canaried.reason)
    const release = withChannel(canaried.channel)

    let compiled = 0
    const total = 400
    for (let index = 0; index < total; index += 1) {
      if (resolveAppleFmPromptPlan({ release, requestKey: `thread-${index}` }).kind === "compiled") compiled += 1
    }
    expect(compiled).toBeGreaterThan(total * 0.15)
    expect(compiled).toBeLessThan(total * 0.35)
  })

  test("rollback of a first release restores the baseline without a rebuild", () => {
    const canaried = beginCanary({ channel: honestChatRelease.channel, plan: canaryPlan, reason: "canary", now })
    if (!canaried.ok) throw new Error(canaried.reason)
    const active = promoteActivation({ channel: canaried.channel, reason: "ok", now })
    if (!active.ok) throw new Error(active.reason)
    const rolledBack = rollbackActivation({ channel: active.channel, reason: "regression", now })
    if (!rolledBack.ok) throw new Error(rolledBack.reason)

    expect(rolledBack.channel.mode).toBe("rolled_back")
    expect(resolveAppleFmPromptPlan({ release: withChannel(rolledBack.channel), requestKey: "thread-a" })).toEqual({
      kind: "baseline",
    })
  })

  test("each release carries an uncertainty record for its small dataset", () => {
    expect(honestChatRelease.uncertainty.method).toBe("small_sample_note")
    expect(honestChatRelease.uncertainty.holdoutDelta).toBeGreaterThan(0)
  })
})

import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  advanceFullAutoPlanFromTurn,
  applyFullAutoStepStatus,
  detectFullAutoPlanDrift,
  makeFullAutoPlan,
  nextActionableFullAutoStep,
  parseFullAutoStepMarkers,
  renderFullAutoPlanBrief,
  reorderFullAutoPlanSteps,
  validateFullAutoPlan,
} from "./full-auto-plan.ts"
import {
  compileFullAutoMissionPacket,
  renderFullAutoMissionPrompt,
} from "./full-auto-mission.ts"
import { type FullAutoRecord } from "./full-auto-registry.ts"
import { openFullAutoRunRegistry } from "./full-auto-run-registry.ts"

const clock = () => {
  let t = Date.parse("2026-07-22T00:00:00.000Z")
  return () => new Date((t += 1000))
}

const threeStepPlan = () =>
  makeFullAutoPlan({
    steps: [
      { stepRef: "read", title: "Read the rubric" },
      { stepRef: "deliver", title: "Implement the fix", dependsOn: ["read"] },
      { stepRef: "verify", title: "Run the named test", dependsOn: ["deliver"] },
    ],
    now: clock(),
  })

const lowLevelRecord = (): FullAutoRecord => ({
  threadRef: "thread.plan",
  enabled: true,
  continuationCount: 1,
  updatedAt: "2026-07-22T00:00:00.000Z",
  enabledAt: "2026-07-22T00:00:00.000Z",
  workspaceRef: "/workspace",
  profile: { lane: "codex-local" },
})

describe("HANDS-3 Full Auto plan", () => {
  test("nextActionableFullAutoStep respects dependency order", () => {
    const plan = threeStepPlan()
    expect(nextActionableFullAutoStep(plan)?.stepRef).toBe("read")
    const afterRead = applyFullAutoStepStatus(plan, { stepRef: "read", status: "done", now: clock() })
    expect(nextActionableFullAutoStep(afterRead)?.stepRef).toBe("deliver")
    const afterDeliver = applyFullAutoStepStatus(afterRead, { stepRef: "deliver", status: "done", now: clock() })
    expect(nextActionableFullAutoStep(afterDeliver)?.stepRef).toBe("verify")
  })

  test("an in-progress step is preferred over a later pending one", () => {
    const plan = applyFullAutoStepStatus(threeStepPlan(), { stepRef: "read", status: "done", now: clock() })
    const started = applyFullAutoStepStatus(plan, { stepRef: "deliver", status: "in_progress", now: clock() })
    expect(nextActionableFullAutoStep(started)?.stepRef).toBe("deliver")
  })

  test("status mutation bumps the revision; unknown step is a no-op", () => {
    const plan = threeStepPlan()
    const next = applyFullAutoStepStatus(plan, { stepRef: "read", status: "done", now: clock() })
    expect(next.revision).toBe(plan.revision + 1)
    const noop = applyFullAutoStepStatus(plan, { stepRef: "does-not-exist", status: "done", now: clock() })
    expect(noop.revision).toBe(plan.revision)
    expect(noop).toBe(plan)
  })

  test("reorder refuses a non-permutation and accepts a valid one", () => {
    const plan = threeStepPlan()
    expect(reorderFullAutoPlanSteps(plan, ["read", "deliver"]).ok).toBe(false)
    expect(reorderFullAutoPlanSteps(plan, ["read", "deliver", "read"]).ok).toBe(false)
    const reordered = reorderFullAutoPlanSteps(plan, ["verify", "read", "deliver"], clock())
    expect(reordered.ok).toBe(true)
    if (reordered.ok) {
      expect(reordered.plan.steps.map((s) => s.stepRef)).toEqual(["verify", "read", "deliver"])
      // Order changed but dependencies still gate: verify is not actionable.
      expect(nextActionableFullAutoStep(reordered.plan)?.stepRef).toBe("read")
    }
  })

  test("validation catches cycles, unknown deps, duplicates, self-deps", () => {
    const cyclic = makeFullAutoPlan({
      steps: [
        { stepRef: "a", title: "A", dependsOn: ["b"] },
        { stepRef: "b", title: "B", dependsOn: ["a"] },
      ],
      now: clock(),
    })
    expect(validateFullAutoPlan(cyclic).some((i) => i.kind === "dependency_cycle")).toBe(true)

    const unknown = makeFullAutoPlan({ steps: [{ stepRef: "a", title: "A", dependsOn: ["ghost"] }], now: clock() })
    expect(validateFullAutoPlan(unknown).some((i) => i.kind === "unknown_dependency")).toBe(true)

    const selfDep = makeFullAutoPlan({ steps: [{ stepRef: "a", title: "A", dependsOn: ["a"] }], now: clock() })
    expect(validateFullAutoPlan(selfDep).some((i) => i.kind === "self_dependency")).toBe(true)

    expect(validateFullAutoPlan(threeStepPlan())).toEqual([])
  })

  test("drift: all-terminal and deadlock are detected", () => {
    let plan = threeStepPlan()
    for (const stepRef of ["read", "deliver", "verify"]) {
      plan = applyFullAutoStepStatus(plan, { stepRef, status: "done", now: clock() })
    }
    const exhausted = detectFullAutoPlanDrift(plan)
    expect(exhausted.drifted).toBe(true)
    expect(exhausted.signals.some((s) => s.kind === "all_steps_terminal")).toBe(true)

    const blocked = applyFullAutoStepStatus(threeStepPlan(), { stepRef: "read", status: "blocked", now: clock() })
    const deadlock = detectFullAutoPlanDrift(blocked)
    expect(deadlock.signals.some((s) => s.kind === "deadlocked")).toBe(true)
  })

  test("brief names the current step and prior progress", () => {
    const plan = applyFullAutoStepStatus(threeStepPlan(), { stepRef: "read", status: "done", now: clock() })
    const brief = renderFullAutoPlanBrief(plan)
    expect(brief.currentStepRef).toBe("deliver")
    expect(brief.done).toBe(1)
    expect(brief.total).toBe(3)
    expect(brief.text).toContain("Read the rubric")
    expect(brief.text).toContain("CURRENT STEP (deliver)")
  })
})

describe("HANDS-3 plan carry across turns via the mission packet", () => {
  test("autonomy run carries the plan brief; a non-autonomy run is byte-identical", () => {
    const root = mkdtempSync(path.join(tmpdir(), "hands3-mission-"))
    try {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const started = runRegistry.startNew({
        title: "Autonomy run",
        objective: "Do the bounded thing.",
        doneCondition: "Merged and green on main.",
        objectiveSource: "user",
        workspaceRef: "/workspace",
        threadRef: "thread.plan",
        actor: "owner_ui",
        reason: "test",
      })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const runRef = started.run.runRef

      const record = lowLevelRecord()
      const base = { record, threadRef: "thread.plan", profile: record.profile, turnCap: 20, priorAcceptedOutcome: null, previousHandoff: null }

      // No autonomy: no planBrief, and the prompt equals the plain run's prompt.
      const plainRun = runRegistry.get(runRef)!
      const plainPacket = compileFullAutoMissionPacket({ run: plainRun, ...base })
      expect(plainPacket.planBrief).toBeUndefined()

      // Enable autonomy + attach a plan; the plan advances one step.
      runRegistry.setAutonomy(runRef, { enabled: true, plan: threeStepPlan() })
      const advanced = applyFullAutoStepStatus(threeStepPlan(), { stepRef: "read", status: "done", now: clock() })
      runRegistry.updatePlan(runRef, advanced)

      const autoRun = runRegistry.get(runRef)!
      const autoPacket = compileFullAutoMissionPacket({ run: autoRun, ...base })
      expect(autoPacket.planBrief).toBeDefined()
      expect(autoPacket.planBrief?.currentStepRef).toBe("deliver")
      expect(renderFullAutoMissionPrompt(autoPacket)).toContain("PERSISTENT PLAN")
      expect(renderFullAutoMissionPrompt(autoPacket)).toContain("CURRENT STEP (deliver)")

      // Turning autonomy off restores the exact non-autonomy prompt bytes.
      runRegistry.setAutonomy(runRef, { enabled: false })
      const offRun = runRegistry.get(runRef)!
      const offPacket = compileFullAutoMissionPacket({ run: offRun, ...base })
      expect(offPacket.planBrief).toBeUndefined()
      expect(renderFullAutoMissionPrompt(offPacket)).toBe(renderFullAutoMissionPrompt(plainPacket))
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("plan persists across a registry re-open (durable carry)", () => {
    const root = mkdtempSync(path.join(tmpdir(), "hands3-persist-"))
    try {
      const file = path.join(root, "runs.json")
      const registry = openFullAutoRunRegistry(file)
      const started = registry.startNew({
        title: "Persist",
        objective: "obj",
        doneCondition: "done",
        objectiveSource: "user",
        threadRef: "thread.persist",
        actor: "owner_ui",
        reason: "test",
      })
      if (!started.ok) throw new Error("start failed")
      registry.setAutonomy(started.run.runRef, { enabled: true, plan: threeStepPlan() })

      // A fresh process re-opening the same durable file sees the plan.
      const reopened = openFullAutoRunRegistry(file)
      const run = reopened.get(started.run.runRef)!
      expect(run.autonomy?.enabled).toBe(true)
      expect(run.autonomy?.plan?.steps.length).toBe(3)
      expect(nextActionableFullAutoStep(run.autonomy!.plan!)?.stepRef).toBe("read")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("HANDS-3 auto-advancement: parseFullAutoStepMarkers (#9174)", () => {
  test("parses STEP-DONE / STEP-START in line and paren forms, and a fenced block", () => {
    const markers = parseFullAutoStepMarkers(
      "did the reading.\nSTEP-DONE: read\n- STEP-START(deliver)\n```step-done\nverify\n```",
    )
    expect([...markers.completed].sort()).toEqual(["read", "verify"])
    expect(markers.started).toEqual(["deliver"])
  })

  test("ignores non-conforming step refs and free text (no NLP guess)", () => {
    const markers = parseFullAutoStepMarkers("I think I finished the read step, probably.\nSTEP-DONE: has spaces")
    expect(markers.completed).toEqual([])
    expect(markers.started).toEqual([])
  })
})

describe("HANDS-3 auto-advancement: advanceFullAutoPlanFromTurn (#9174)", () => {
  test("a completed turn with a STEP-DONE marker advances that step to done", () => {
    const plan = threeStepPlan()
    const result = advanceFullAutoPlanFromTurn(plan, { disposition: "completed", completedStepRefs: ["read"] })
    expect(result.advanced).toBe(true)
    expect(result.advancedStepRefs).toEqual(["read"])
    expect(result.plan.steps.find((s) => s.stepRef === "read")?.status).toBe("done")
    // The next actionable step is now the unblocked deliver step.
    expect(nextActionableFullAutoStep(result.plan)?.stepRef).toBe("deliver")
  })

  test("a FAILED turn never marks a step done (no fabricated progress)", () => {
    const plan = threeStepPlan()
    const result = advanceFullAutoPlanFromTurn(plan, { disposition: "failed", completedStepRefs: ["read"] })
    expect(result.advanced).toBe(false)
    expect(result.plan.steps.find((s) => s.stepRef === "read")?.status).toBe("pending")
  })

  test("a STEP-START marker moves a pending step to in_progress but does not count as advancing", () => {
    const plan = threeStepPlan()
    const result = advanceFullAutoPlanFromTurn(plan, { disposition: "completed", startedStepRefs: ["read"] })
    expect(result.advanced).toBe(false)
    expect(result.startedStepRefs).toEqual(["read"])
    expect(result.plan.steps.find((s) => s.stepRef === "read")?.status).toBe("in_progress")
  })

  test("verificationPassed advances a named verify step to done", () => {
    const started = advanceFullAutoPlanFromTurn(threeStepPlan(), {
      disposition: "completed",
      completedStepRefs: ["read", "deliver"],
    })
    const result = advanceFullAutoPlanFromTurn(started.plan, {
      disposition: "completed",
      verificationPassed: true,
      verifiedStepRef: "verify",
    })
    expect(result.advancedStepRefs).toEqual(["verify"])
    expect(result.plan.steps.every((s) => s.status === "done")).toBe(true)
  })

  test("an unknown or already-terminal step ref is a no-op (idempotent replay)", () => {
    const plan = advanceFullAutoPlanFromTurn(threeStepPlan(), { disposition: "completed", completedStepRefs: ["read"] }).plan
    const again = advanceFullAutoPlanFromTurn(plan, { disposition: "completed", completedStepRefs: ["read", "nope"] })
    expect(again.advanced).toBe(false)
    expect(again.plan.revision).toBe(plan.revision)
  })
})

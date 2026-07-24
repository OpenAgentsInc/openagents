import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  buildFullAutoTurnAction,
  classifyFullAutoTurnActionKind,
  detectFullAutoChurn,
  fullAutoChurnPauseReason,
  parseFullAutoChangedPaths,
  type FullAutoTurnAction,
} from "./full-auto-churn.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { openFullAutoRunRegistry } from "./full-auto-run-registry.ts"
import { reconcileFullAutoThreads } from "./full-auto-reconcile.ts"

let seq = 0
const action = (signature: string, advancedPlanStep = false): FullAutoTurnAction =>
  buildFullAutoTurnAction({
    turnRef: `turn.full-auto.${seq}`,
    resultHint: signature,
    advancedPlanStep,
    at: new Date(Date.parse("2026-07-22T00:00:00.000Z") + (seq++ * 1000)).toISOString(),
  })

describe("HANDS-4 churn taxonomy + detector", () => {
  test("classification is deterministic over structured signals", () => {
    expect(classifyFullAutoTurnActionKind({ turnRef: "t", verificationRan: true, at: "x" })).toBe("verify")
    expect(classifyFullAutoTurnActionKind({ turnRef: "t", changedPaths: ["a.ts"], at: "x" })).toBe("edit")
    expect(classifyFullAutoTurnActionKind({ turnRef: "t", readOnly: true, at: "x" })).toBe("recon")
    expect(classifyFullAutoTurnActionKind({ turnRef: "t", at: "x" })).toBe("setup")
  })

  test("repeated near-identical non-advancing turns are churn", () => {
    seq = 0
    const actions = [action("same"), action("same"), action("same")]
    const decision = detectFullAutoChurn({ actions })
    expect(decision.churn).toBe(true)
    expect(decision.consecutive).toBe(3)
    expect(fullAutoChurnPauseReason(decision)).toContain("low_value_churn:3")
  })

  test("a distinct signature resets the count (varied work is not churn)", () => {
    seq = 0
    const actions = [action("a"), action("a"), action("b")]
    expect(detectFullAutoChurn({ actions }).churn).toBe(false)
    expect(detectFullAutoChurn({ actions }).consecutive).toBe(1)
  })

  test("a plan-advancing turn is never churn even with identical signature", () => {
    seq = 0
    const actions = [action("same"), action("same"), action("same", true)]
    expect(detectFullAutoChurn({ actions }).churn).toBe(false)
  })

  test("anchorAt excludes pre-resume history", () => {
    seq = 0
    const older = action("same")
    const newer = [action("same"), action("same")]
    const decision = detectFullAutoChurn({ actions: [older, ...newer], anchorAt: older.at })
    expect(decision.consecutive).toBe(2) // older excluded
    expect(decision.churn).toBe(false)
  })
})

describe("HANDS-4 reconcile churn gate is opt-in and pauses the run", () => {
  test("churnSignal pauses; without it the loop dispatches as before", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "hands4-reconcile-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "registry.json"))
      registry.set("thread.churn", true, { workspaceRef: "/ws", profile: { lane: "codex-local" } })

      let paused: { reason: string; consecutiveChurnTurns: number } | null = null
      let dispatched = 0
      const churnActions = [action("same"), action("same"), action("same")]

      const result = await reconcileFullAutoThreads({
        registry,
        nonterminalThreadRefs: () => new Set<string>(),
        resolveWorkspaceRef: () => "/ws",
        journalHasNonterminalTurn: () => false,
        churnSignal: () => detectFullAutoChurn({ actions: churnActions }),
        onPausedLowValueChurn: (_thread, pause) => {
          paused = pause
        },
        dispatch: async () => {
          dispatched += 1
          return { ok: true }
        },
      })

      expect(dispatched).toBe(0)
      expect(result).toEqual([])
      expect(paused).not.toBeNull()
      expect(paused!.consecutiveChurnTurns).toBe(3)
      expect(registry.record("thread.churn")?.pausedReason).toContain("low_value_churn")

      // A fresh enabled thread with NO churnSignal dispatches normally.
      const registry2 = openFullAutoRegistry(path.join(root, "registry2.json"))
      registry2.set("thread.ok", true, { workspaceRef: "/ws", profile: { lane: "codex-local" } })
      let dispatched2 = 0
      await reconcileFullAutoThreads({
        registry: registry2,
        nonterminalThreadRefs: () => new Set<string>(),
        resolveWorkspaceRef: () => "/ws",
        journalHasNonterminalTurn: () => false,
        dispatch: async () => {
          dispatched2 += 1
          return { ok: true }
        },
      })
      expect(dispatched2).toBe(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("HANDS-4 changed-paths awareness (#9175)", () => {
  test("parses CHANGED / CHANGED-PATH lines and a fenced changed-paths block", () => {
    const paths = parseFullAutoChangedPaths(
      "did work.\nCHANGED: src/a.ts\nCHANGED-PATH: src/b.ts\n```changed-paths\nsrc/c.ts\nsrc/a.ts\n```",
    )
    expect([...paths].sort()).toEqual(["src/a.ts", "src/b.ts", "src/c.ts"])
  })

  test("ignores whitespace paths and free prose", () => {
    expect(parseFullAutoChangedPaths("I changed some files, honestly.")).toEqual([])
    expect(parseFullAutoChangedPaths("CHANGED: has a space here")).toEqual([])
  })

  test("distinct changed paths give distinct signatures, so varied edit turns are not churn", () => {
    const base = Date.parse("2026-07-22T00:00:00.000Z")
    const editTurn = (index: number, paths: ReadonlyArray<string>) =>
      buildFullAutoTurnAction({
        turnRef: `turn.full-auto.edit.${index}`,
        changedPaths: paths,
        advancedPlanStep: false,
        at: new Date(base + index * 1000).toISOString(),
      })
    const varied = [editTurn(0, ["src/a.ts"]), editTurn(1, ["src/b.ts"]), editTurn(2, ["src/c.ts"])]
    expect(detectFullAutoChurn({ actions: varied, threshold: 3 }).churn).toBe(false)
    const same = [editTurn(0, ["src/a.ts"]), editTurn(1, ["src/a.ts"]), editTurn(2, ["src/a.ts"])]
    expect(detectFullAutoChurn({ actions: same, threshold: 3 }).churn).toBe(true)
  })

  test("a REAL advancedPlanStep turn resets churn even with an identical signature", () => {
    const base = Date.parse("2026-07-22T00:00:00.000Z")
    const row = (index: number, advanced: boolean) =>
      buildFullAutoTurnAction({
        turnRef: `turn.full-auto.adv.${index}`,
        resultHint: "same-output-hash",
        advancedPlanStep: advanced,
        at: new Date(base + index * 1000).toISOString(),
      })
    // Three identical non-advancing turns would churn...
    expect(detectFullAutoChurn({ actions: [row(0, false), row(1, false), row(2, false)], threshold: 3 }).churn).toBe(true)
    // ...but a plan-advancing final turn is genuine progress, never churn.
    expect(detectFullAutoChurn({ actions: [row(0, false), row(1, false), row(2, true)], threshold: 3 }).churn).toBe(false)
  })
})

describe("HANDS-4 durable per-turn action persistence (#9175 record shape)", () => {
  test("recordTurnAction appends bounded rows and replaces a replayed turnRef in place", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "fa-turnactions-"))
    try {
      const registry = openFullAutoRunRegistry(path.join(dir, "runs.json"))
      const started = registry.startNew({
        title: "run",
        objective: "obj",
        doneCondition: "done",
        objectiveSource: "system_selected",
        workspaceRef: "/ws",
        actor: "control_api",
        reason: "t",
      })
      if (!started.ok) throw new Error("start failed")
      const runRef = started.run.runRef
      // Without an autonomy block, recordTurnAction is a no-op returning null.
      expect(registry.recordTurnAction(runRef, buildFullAutoTurnAction({ turnRef: "t1", advancedPlanStep: false, at: "2026-07-22T00:00:00.000Z" }))).toBeNull()
      expect(registry.get(runRef)!.autonomy).toBeUndefined()

      registry.setAutonomy(runRef, { enabled: true })
      registry.recordTurnAction(runRef, buildFullAutoTurnAction({ turnRef: "t1", resultHint: "a", advancedPlanStep: false, at: "2026-07-22T00:00:01.000Z" }))
      registry.recordTurnAction(runRef, buildFullAutoTurnAction({ turnRef: "t2", resultHint: "b", advancedPlanStep: true, at: "2026-07-22T00:00:02.000Z" }))
      // Replay t1 with different content -> replaced in place, not doubled.
      registry.recordTurnAction(runRef, buildFullAutoTurnAction({ turnRef: "t1", resultHint: "a2", advancedPlanStep: true, at: "2026-07-22T00:00:03.000Z" }))
      const stored = registry.get(runRef)!
      expect(stored.autonomy?.turnActions?.length).toBe(2)
      expect(stored.autonomy?.turnActions?.map((a) => a.turnRef)).toEqual(["t2", "t1"])
      expect(stored.autonomy?.turnActions?.find((a) => a.turnRef === "t1")?.advancedPlanStep).toBe(true)
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  })
})

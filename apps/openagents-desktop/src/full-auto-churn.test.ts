import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  buildFullAutoTurnAction,
  classifyFullAutoTurnActionKind,
  detectFullAutoChurn,
  fullAutoChurnPauseReason,
  type FullAutoTurnAction,
} from "./full-auto-churn.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
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

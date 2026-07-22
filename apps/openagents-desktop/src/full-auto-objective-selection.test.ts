import { describe, expect, test } from "vite-plus/test"

import {
  FULL_AUTO_COMPLETION_GATE,
  fullAutoObjectiveFromCandidate,
  rankFullAutoObjectiveCandidates,
  selectFullAutoObjective,
  validateFullAutoCandidateShape,
  type FullAutoCandidateSignal,
  type FullAutoObjectiveRecall,
} from "./full-auto-objective-selection.ts"
import { deriveFullAutoVerificationSpec } from "./full-auto-verification.ts"
import { nextActionableFullAutoStep } from "./full-auto-plan.ts"

const now = () => new Date("2026-07-22T00:00:00.000Z")

const wellFormed = (over: Partial<FullAutoCandidateSignal> = {}): FullAutoCandidateSignal => ({
  title: "Fix the flaky test",
  readTarget: "apps/openagents-desktop/src/x.test.ts",
  deliverable: "Make x.test.ts deterministic",
  verification: "pnpm --dir apps/openagents-desktop test -- src/x.test.ts",
  rationale: "Named in issue #9000; blocks the desktop check.",
  surface: "desktop",
  citedRefs: ["#9000"],
  ...over,
})

describe("HANDS-1 objective selection", () => {
  test("shape validation flags missing owner-shape fields", () => {
    expect(validateFullAutoCandidateShape(wellFormed())).toEqual([])
    expect(validateFullAutoCandidateShape(wellFormed({ verification: " " }))).toContain("missing_verification")
    expect(validateFullAutoCandidateShape(wellFormed({ citedRefs: [] }))).toContain("missing_citation")
    expect(validateFullAutoCandidateShape(wellFormed({ readTarget: "" }))).toContain("missing_read_target")
  })

  test("ranking orders by owner-priority surface and drops shape-invalid signals", () => {
    const selection = rankFullAutoObjectiveCandidates({
      signals: [
        wellFormed({ title: "analysis doc", surface: "analysis" }),
        wellFormed({ title: "desktop fix", surface: "desktop" }),
        wellFormed({ title: "malformed", verification: "" }),
      ],
      now,
    })
    expect(selection.candidates.map((c) => c.title)).toEqual(["desktop fix", "analysis doc"])
    expect(selection.rejectedCount).toBe(1)
    // Every ranked candidate ends at the fixed completion gate.
    for (const candidate of selection.candidates) {
      expect(candidate.completionGate).toBe(FULL_AUTO_COMPLETION_GATE)
      expect(candidate.score).toBeGreaterThan(0)
    }
  })

  test("priorityHint contributes but surface dominates", () => {
    const selection = rankFullAutoObjectiveCandidates({
      signals: [
        wellFormed({ title: "high-hint other", surface: "other", priorityHint: 1 }),
        wellFormed({ title: "desktop no-hint", surface: "desktop", priorityHint: 0 }),
      ],
      now,
    })
    expect(selection.candidates[0]?.title).toBe("desktop no-hint")
  })

  test("consumes an injected recall seam and merges its cited signals", async () => {
    const recall: FullAutoObjectiveRecall = async () => [
      {
        title: "Recalled sandbox hardening",
        readTarget: "docs/sandbox/notes.md",
        deliverable: "Tighten the sandbox default deny",
        verification: "pnpm --dir apps/openagents-desktop test -- src/sandbox.test.ts",
        surface: "sandbox",
        citedRefs: ["corpus:abc123"],
        priorityHint: 0.9,
      },
    ]
    const selection = await selectFullAutoObjective({
      runRef: "run.1",
      workspaceRef: "/ws",
      directSignals: [wellFormed({ title: "direct desktop", surface: "desktop" })],
      recall,
      now,
    })
    expect(selection.usedRecall).toBe(true)
    expect(selection.candidates.map((c) => c.title)).toContain("Recalled sandbox hardening")
  })

  test("a throwing recall seam fails soft to direct signals only", async () => {
    const recall: FullAutoObjectiveRecall = async () => {
      throw new Error("recall unavailable")
    }
    const selection = await selectFullAutoObjective({
      runRef: "run.1",
      workspaceRef: "/ws",
      directSignals: [wellFormed()],
      recall,
      now,
    })
    expect(selection.usedRecall).toBe(false)
    expect(selection.candidates.length).toBe(1)
  })

  test("a chosen candidate projects to a run objective + verifiable done condition + starter plan", () => {
    const selection = rankFullAutoObjectiveCandidates({ signals: [wellFormed()], now })
    const top = selection.candidates[0]!
    const projected = fullAutoObjectiveFromCandidate(top, now)
    expect(projected.objective).toContain("Make x.test.ts deterministic")
    // The done condition embeds a verify: marker HANDS-2 can extract.
    const spec = deriveFullAutoVerificationSpec(projected.doneCondition)
    expect(spec.kind).toBe("command")
    if (spec.kind === "command") expect(spec.command).toContain("src/x.test.ts")
    // The starter plan is a real read -> deliver -> verify decomposition.
    expect(projected.plan.steps.map((s) => s.stepRef)).toEqual(["read", "deliver", "verify"])
    expect(nextActionableFullAutoStep(projected.plan)?.stepRef).toBe("read")
  })
})

import { describe, expect, test } from "vite-plus/test"

import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  FULL_AUTO_COMPLETION_GATE,
  FULL_AUTO_DEFAULT_NAMED_VERIFICATION,
  fullAutoObjectiveFromCandidate,
  proposeFullAutoObjectiveCandidates,
  rankFullAutoObjectiveCandidates,
  roadmapCandidateToRecallSignal,
  roadmapRecallToSignals,
  selectFullAutoObjective,
  validateFullAutoCandidateShape,
  type FullAutoCandidateSignal,
  type FullAutoObjectiveRecall,
} from "./full-auto-objective-selection.ts"
import { deriveFullAutoVerificationSpec } from "./full-auto-verification.ts"
import { nextActionableFullAutoStep } from "./full-auto-plan.ts"
import type { FullAutoRoadmapRecallResult } from "./full-auto-recall.ts"

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

const roadmapResult = (over: Partial<FullAutoRoadmapRecallResult> = {}): FullAutoRoadmapRecallResult => ({
  runRef: "run.1",
  recallRef: "recall.1",
  tier: "deterministic",
  status: "completed",
  reason: null,
  label: "cited-candidate",
  verified: false,
  corpusRef: "folder:docs",
  contentDigest: "d".repeat(64),
  candidates: [
    { entryRef: "docs/transcripts/001.md#p3", sourceFile: "docs/transcripts/001.md", excerpt: "The product should schedule overnight work across coding agents." },
  ],
  synthesis: null,
  citationCount: 1,
  capsHit: [],
  usage: { modelCalls: 0, inputTokens: 0, outputTokens: 0, totalTokens: 0 },
  usageRows: [],
  ...over,
})

describe("HANDS-1 -> HANDS-5 real roadmap recall binding (#9172)", () => {
  test("roadmapCandidateToRecallSignal maps a cited excerpt into an owner-shape signal", () => {
    const signal = roadmapCandidateToRecallSignal(roadmapResult().candidates[0]!)
    expect(signal.readTarget).toBe("docs/transcripts/001.md")
    expect(signal.verification).toBe(FULL_AUTO_DEFAULT_NAMED_VERIFICATION)
    expect(signal.surface).toBe("roadmap")
    // The corpus citation is preserved, never invented.
    expect(signal.citedRefs).toContain("docs/transcripts/001.md#p3")
    // The mapped signal is a valid owner-shape candidate (deliverable + verification present).
    expect(validateFullAutoCandidateShape({ ...signal, deliverable: signal.deliverable ?? "", verification: signal.verification ?? "", rationale: signal.rationale ?? "" })).toEqual([])
  })

  test("roadmapRecallToSignals is empty for a refused/failed recall (fail-soft)", () => {
    expect(roadmapRecallToSignals(roadmapResult({ status: "refused", candidates: [] }))).toEqual([])
    expect(roadmapRecallToSignals(roadmapResult({ status: "failed", candidates: [] }))).toEqual([])
  })

  test("selectFullAutoObjective surfaces a roadmap-recalled candidate the owner can endorse", async () => {
    const recall: FullAutoObjectiveRecall = async () =>
      roadmapResult().candidates.map(roadmapCandidateToRecallSignal)
    const selection = await selectFullAutoObjective({
      runRef: "run.1",
      workspaceRef: "/ws",
      directSignals: [],
      recall,
    })
    expect(selection.usedRecall).toBe(true)
    expect(selection.candidates.length).toBe(1)
    expect(selection.candidates[0]!.surface).toBe("roadmap")
    expect(selection.candidates[0]!.completionGate).toBe(FULL_AUTO_COMPLETION_GATE)
  })

  test("a throwing recall degrades to direct signals only (fail-soft, never blocks)", async () => {
    const recall: FullAutoObjectiveRecall = async () => {
      throw new Error("corpus unavailable")
    }
    const selection = await selectFullAutoObjective({
      runRef: "run.1",
      workspaceRef: "/ws",
      directSignals: [wellFormed({ title: "direct only" })],
      recall,
    })
    expect(selection.usedRecall).toBe(false)
    expect(selection.candidates.map((c) => c.title)).toEqual(["direct only"])
  })

  test("fullAutoObjectiveFromCandidate attributes a system_selected objective source", () => {
    const [candidate] = rankFullAutoObjectiveCandidates({ signals: [wellFormed()] }).candidates
    const projected = fullAutoObjectiveFromCandidate(candidate!, now)
    expect(projected.objectiveSource).toBe("system_selected")
  })

  test("proposeFullAutoObjectiveCandidates runs the REAL folder recall end to end (Tier D, no spend)", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "hands1-corpus-"))
    try {
      writeFileSync(
        path.join(root, "notes.md"),
        ["# Notes", "The product should schedule overnight work across several coding agents.", "We need a durable run monitor."].join("\n\n"),
        "utf8",
      )
      const selection = await proposeFullAutoObjectiveCandidates({
        runRef: "run.real",
        workspaceRef: "/ws",
        corpus: { rootDir: root, scopeLabel: "notes", minEntryChars: 12 } as never,
        limit: 5,
      })
      // The real recall ran (usedRecall true) and the call resolved fail-soft.
      expect(selection.usedRecall).toBe(true)
      expect(selection.schema).toBe("openagents.desktop.full_auto_objective_selection.v1")
      // Any surfaced candidate is in the owner shape with the repo green gate.
      for (const candidate of selection.candidates) {
        expect(candidate.verification).toBe(FULL_AUTO_DEFAULT_NAMED_VERIFICATION)
        expect(candidate.citedRefs.length).toBeGreaterThan(0)
      }
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

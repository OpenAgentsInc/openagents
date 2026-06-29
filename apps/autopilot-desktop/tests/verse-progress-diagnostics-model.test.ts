import { describe, expect, test } from "bun:test"

import {
  projectVersePortraitChips,
  projectVerseRunStepProgress,
  projectVerseWebglDiagnostics,
} from "../src/shared/verse-progress-diagnostics-model"

describe("Verse progress and diagnostics model", () => {
  test("maps run progress through assignment, trace, replay, verdict, and settle", () => {
    expect(projectVerseRunStepProgress({
      assignmentRef: "assignment.run.1",
      traceRef: "trace.run.1",
      replayRef: "replay.run.1",
    })).toEqual([
      {
        kind: "assignment",
        label: "Assignment",
        status: "done",
        progress: 1,
        sourceRef: "assignment.run.1",
      },
      {
        kind: "trace",
        label: "Trace",
        status: "done",
        progress: 1,
        sourceRef: "trace.run.1",
      },
      {
        kind: "replay",
        label: "Replay",
        status: "done",
        progress: 1,
        sourceRef: "replay.run.1",
      },
      {
        kind: "verdict",
        label: "Verdict",
        status: "current",
        progress: 0.5,
        sourceRef: null,
      },
      {
        kind: "settle",
        label: "Settle",
        status: "blocked",
        progress: 0,
        sourceRef: null,
      },
    ])
  })

  test("marks failed run steps without fabricating later progress", () => {
    const progress = projectVerseRunStepProgress({
      assignmentRef: "assignment.run.2",
      traceRef: "trace.run.2",
      failedStep: "replay",
    })

    expect(progress.map(step => [step.kind, step.status])).toEqual([
      ["assignment", "done"],
      ["trace", "done"],
      ["replay", "failed"],
      ["verdict", "blocked"],
      ["settle", "blocked"],
    ])
  })

  test("projects portrait chips with HiDPI backing pixels and overscan stability", () => {
    const [wide] = projectVersePortraitChips({
      avatars: [{ avatarRef: "avatar.alpha", label: "Alpha", anchorX: 0.5, anchorY: 0.25 }],
      viewportCssWidth: 1200,
      viewportCssHeight: 800,
      devicePixelRatio: 2.5,
      chipCssSize: 48,
      overscanRatio: 0.25,
    })
    const [narrow] = projectVersePortraitChips({
      avatars: [{ avatarRef: "avatar.alpha", label: "Alpha", anchorX: 0.5, anchorY: 0.25 }],
      viewportCssWidth: 600,
      viewportCssHeight: 400,
      devicePixelRatio: 2.5,
      chipCssSize: 48,
      overscanRatio: 0.25,
    })

    expect(wide).toMatchObject({
      avatarRef: "avatar.alpha",
      cssX: 576,
      cssY: 176,
      cssSize: 48,
      backingPx: 120,
      overscanPx: 12,
      visible: true,
    })
    expect(narrow).toMatchObject({
      avatarRef: "avatar.alpha",
      cssX: 276,
      cssY: 76,
      cssSize: 48,
      backingPx: 120,
      overscanPx: 12,
      visible: true,
    })
  })

  test("keeps portrait chips bounded by overscan near viewport edges", () => {
    const [chip] = projectVersePortraitChips({
      avatars: [{ avatarRef: "avatar.edge", label: "Edge", anchorX: -0.1, anchorY: 1.1 }],
      viewportCssWidth: 320,
      viewportCssHeight: 180,
      chipCssSize: 40,
      overscanRatio: 0.2,
    })

    expect(chip?.cssX).toBe(-8)
    expect(chip?.cssY).toBe(148)
    expect(chip?.visible).toBe(false)
  })

  test("emits development and smoke WebGL diagnostics without production overlay noise", () => {
    const development = projectVerseWebglDiagnostics({
      mode: "development",
      enabled: true,
      frameTimesMs: [16, 17, 33, 20],
      drawCalls: 42,
      entityCount: 9,
      sourceRefs: ["scene.verse.demo"],
    })
    const smoke = projectVerseWebglDiagnostics({
      mode: "smoke",
      enabled: true,
      frameTimesMs: [10, 20],
      drawCalls: 7,
    })
    const production = projectVerseWebglDiagnostics({
      mode: "production",
      enabled: true,
      frameTimesMs: [16],
      drawCalls: 1,
    })

    expect(development.showOverlay).toBe(true)
    expect(development.artifact).toMatchObject({
      drawCalls: 42,
      entityCount: 9,
      frameCount: 4,
      frameMsAverage: 21.5,
      frameMsP95: 33,
      frameMsMax: 33,
      fpsEstimate: 46.5,
      sourceRefs: ["scene.verse.demo"],
    })
    expect(smoke.showOverlay).toBe(false)
    expect(smoke.artifact?.drawCalls).toBe(7)
    expect(production).toEqual({
      available: false,
      showOverlay: false,
      artifact: null,
    })
  })

  test("keeps perf artifacts public-safe by filtering raw-looking refs", () => {
    const diagnostics = projectVerseWebglDiagnostics({
      mode: "smoke",
      enabled: true,
      frameTimesMs: [16],
      drawCalls: 1,
      sourceRefs: [
        "world.scene.public",
        "private prompt with spaces",
        "token=abc123 secret",
      ],
    })

    expect(JSON.stringify(diagnostics)).toContain("world.scene.public")
    expect(JSON.stringify(diagnostics)).not.toContain("private prompt")
    expect(JSON.stringify(diagnostics)).not.toContain("token=abc123")
  })
})

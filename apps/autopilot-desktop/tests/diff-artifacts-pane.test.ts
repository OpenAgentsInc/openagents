// VCODE-10 (#5927): Diff/Artifacts pane for Verse code mode.
//
// Covers the public-safe projection boundary, selected-file preservation, and
// stable scroll-key render contract for stream refreshes.

import { describe, expect, test } from "bun:test"

import type { NodeStateMessage, SessionArtifactStats, SessionEventRow } from "../src/shared/rpc"
import { projectDiffArtifactsPanel } from "../src/ui/diff-artifacts-projection"
import { initialModel, Model } from "../src/ui/model"
import {
  GotNodeState,
  SelectedDiffFile,
  SelectedSession,
  SelectedSessionDetailView,
} from "../src/ui/message"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const renderHtml = (node: unknown): string => {
  if (!isVNodeLike(node)) return ""
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, on]) => on)
    .map(([c]) => c)
    .join(" ")
  const attrStr = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes ? [["class", classes] as const] : []),
  ]
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join("")
  const tag = node.sel ?? "node"
  const children = (node.children ?? [])
    .map((c) => (typeof c === "string" ? c : renderHtml(c)))
    .join("")
  return `<${tag}${attrStr}>${node.text ?? ""}${children}</${tag}>`
}

const event = (input: Partial<SessionEventRow> = {}): SessionEventRow => ({
  eventIndex: input.eventIndex ?? 0,
  phase: input.phase ?? "progress",
  state: input.state ?? "running",
  observedAt: input.observedAt ?? "2026-06-21T21:00:00.000Z",
  detail: input.detail ?? "",
  ...(input.full !== undefined ? { full: input.full } : {}),
})

const proofStats = (): SessionArtifactStats => ({
  kind: "proof",
  outcome: "completed",
  editedFileCount: 2,
  commandCount: 4,
  totalTokens: 1800,
  detail: {
    schema: "schema.pylon.proof.v1",
    objectiveDigestRef: "digest.objective.safe",
    verifyRef: "verify.ref.safe",
    responseDigestRef: "digest.response.safe",
    externalSessionRef: "session.external.safe",
    executionPathRef: "control_session.composer",
    executionMode: "local_bounded",
    sandboxMode: "workspace-write",
    permissionMode: "on-request",
    devCheckState: "passed",
    deviationRefs: ["deviation.safe"],
    redactionState: "clean",
    errorClass: null,
    errorDigestRef: null,
    workspaceRef: "/Users/private/worktree",
  },
})

const sessionRef = "session.pylon.codex.diff"

const nodeWithEvents = (events: readonly SessionEventRow[]): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [
    {
      sessionRef,
      adapter: "codex",
      state: "completed",
      accountRefHash: null,
      updatedAt: "2026-06-21T21:00:00.000Z",
    },
  ],
  events: { [sessionRef]: [...events] },
  artifacts: { [sessionRef]: proofStats() },
})

describe("Diff/Artifacts pane projection (#5927)", () => {
  test("keeps relative file refs and filters raw absolute paths/log fragments", () => {
    const projection = projectDiffArtifactsPanel({
      sessionRef,
      events: [
        event({
          eventIndex: 1,
          detail: "edited src/safe.ts (+2 -0)",
          full: "edited /Users/private/openagents/src/safe.ts (+2 -0)",
        }),
        event({ eventIndex: 2, detail: "edited /Users/private/secret.ts (+9 -0)" }),
        event({ eventIndex: 3, detail: "completed: update apps/web/page.ts, add C:\\tmp\\secret.ts" }),
      ],
      stats: proofStats(),
      expandedFiles: ["/Users/private/secret.ts", "src/safe.ts"],
      selectedFilePath: "src/safe.ts",
    })

    expect(projection.changeSet.files.map((file) => file.path)).toEqual([
      "apps/web/page.ts",
      "src/safe.ts",
    ])
    expect(projection.expandedFiles).toEqual(["src/safe.ts"])
    expect(projection.selectedFilePath).toBe("src/safe.ts")
    expect(JSON.stringify(projection)).not.toContain("/Users/private")
    expect(JSON.stringify(projection)).not.toContain("C:\\tmp")
    expect(projection.receiptRefs).toContain("digest.response.safe")
    expect(projection.checkRefs).toContain("verify.ref.safe")
    expect(projection.proofLinks).toContain("session.external.safe")
  })

  test("falls back to the first current changed file if the selected file disappears", () => {
    const projection = projectDiffArtifactsPanel({
      sessionRef,
      events: [event({ detail: "edited src/new.ts (+1 -0)" })],
      stats: null,
      expandedFiles: [],
      selectedFilePath: "src/old.ts",
    })
    expect(projection.selectedFilePath).toBe("src/new.ts")
  })
})

describe("Diff/Artifacts pane UI state (#5927)", () => {
  test("selected diff file survives GotNodeState stream refreshes", () => {
    let model = Model.make({ ...initialModel, pane: "session-detail" })
    ;[model] = update(model, GotNodeState({ node: nodeWithEvents([event({ detail: "edited src/a.ts (+1 -0)" })]) }))
    ;[model] = update(model, SelectedSession({ sessionRef }))
    ;[model] = update(model, SelectedSessionDetailView({ view: "diff-artifacts" }))
    ;[model] = update(model, SelectedDiffFile({ path: "src/a.ts" }))

    const [afterPoll] = update(
      model,
      GotNodeState({
        node: nodeWithEvents([
          event({ detail: "edited src/a.ts (+2 -0)" }),
          event({ eventIndex: 2, detail: "edited src/b.ts (+1 -1)" }),
        ]),
      }),
    )

    expect(afterPoll.selectedDiffFilePath).toBe("src/a.ts")
    expect(afterPoll.sessionDetailView).toBe("diff-artifacts")
  })

  test("rendered pane carries stable scroll key and selected file controls", () => {
    let model = Model.make({ ...initialModel, pane: "session-detail" })
    ;[model] = update(
      model,
      GotNodeState({
        node: nodeWithEvents([
          event({ detail: "edited src/a.ts (+1 -0)" }),
          event({ eventIndex: 2, detail: "edited docs/readme.md (+3 -1)" }),
        ]),
      }),
    )
    ;[model] = update(model, SelectedSession({ sessionRef }))
    ;[model] = update(model, SelectedSessionDetailView({ view: "diff-artifacts" }))
    ;[model] = update(model, SelectedDiffFile({ path: "docs/readme.md" }))

    const rendered = renderHtml((view(model) as { body: unknown }).body)
    expect(rendered).toContain(`data-autopilot-diff-artifacts-panel="${sessionRef}"`)
    expect(rendered).toContain(`data-autopilot-diff-scroll-key="diff-artifacts:${sessionRef}"`)
    expect(rendered).toContain('data-autopilot-scroll-retained="diff-artifacts"')
    expect(rendered).toContain('data-autopilot-diff-file-index=""')
    expect(rendered).toContain('data-autopilot-diff-file-select="docs/readme.md"')
    expect(rendered).toContain('data-autopilot-selected-diff-file="docs/readme.md"')
    expect(rendered).toContain('data-autopilot-diff-artifact-ref-group="checks"')
    expect(rendered).toContain('data-autopilot-diff-artifact-ref-group="receipts"')
    expect(rendered).not.toContain("/Users/private")
  })

  test("partial proof artifacts do not crash the artifact browser", () => {
    const projection = projectDiffArtifactsPanel({
      sessionRef,
      events: [event({ detail: "edited src/smoke.ts (+1 -0)" })],
      stats: {
        kind: "proof",
        outcome: "completed",
        editedFileCount: 1,
        commandCount: 1,
        totalTokens: 20,
        detail: {
          schema: "schema.pylon.proof.v1",
          objectiveDigestRef: "digest.objective.partial",
          verifyRef: "verify.partial",
          responseDigestRef: "digest.response.partial",
          externalSessionRef: "session.external.partial",
          executionPathRef: "control_session.composer",
          executionMode: "local_bounded",
          sandboxMode: "workspace-write",
          permissionMode: "on-request",
          devCheckState: "passed",
          redactionState: "clean",
          errorClass: null,
          errorDigestRef: null,
        } as unknown as SessionArtifactStats["detail"],
      },
      expandedFiles: [],
      selectedFilePath: null,
    })

    expect(projection.artifactSections.length).toBeGreaterThan(0)
    expect(JSON.stringify(projection)).toContain("digest.response.partial")
  })
})

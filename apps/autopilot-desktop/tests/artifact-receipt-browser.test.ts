// #5470: session-detail diff fidelity + artifact & receipt browser tests.
//
// Covers the three pieces of #5470 without a live node:
//   1. artifactBrowserSections — derive the redaction-safe, ref-only browser
//      rows from a retained `session.artifact` projection (proof / failure).
//   2. the update reducer for the new pure toggles (per-file diff expand,
//      unified/split layout, artifact-browser open).
//   3. a session-detail render through view(): the diff card carries the file
//      tree + per-file expand strip, and the artifact browser surfaces the refs
//      while NEVER rendering a raw seed/token/path.

import { describe, expect, test } from "bun:test"

import { artifactBrowserSections } from "../src/ui/helpers"
import { initialModel, Model } from "../src/ui/model"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import {
  GotNodeState,
  SelectedSession,
  ToggledArtifactBrowser,
  ToggledDiffFile,
  ToggledDiffViewMode,
} from "../src/ui/message"
import type { NodeStateMessage, SessionArtifactStats } from "../src/shared/rpc"

const proofStats = (over: Partial<SessionArtifactStats> = {}): SessionArtifactStats => ({
  kind: "proof",
  outcome: "completed",
  editedFileCount: 2,
  commandCount: 3,
  totalTokens: 1200,
  detail: {
    schema: "schema.pylon.control_session.proof.v1",
    objectiveDigestRef: "digest.objective.aaaa",
    verifyRef: "ref.verify.bbbb",
    responseDigestRef: "digest.response.cccc",
    externalSessionRef: "session.external.dddd",
    executionPathRef: "control_session.composer",
    executionMode: "local_bounded",
    sandboxMode: "workspace-write",
    permissionMode: "on-request",
    devCheckState: "passed",
    deviationRefs: [],
    redactionState: "clean",
    errorClass: null,
    errorDigestRef: null,
    workspaceRef: "workspace.pylon.eeee",
  },
  ...over,
})

// A serializer that flattens the foldkit vnode tree to a string (attrs + text)
// so we can assert what session-detail rendered without a DOM.
type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: { attrs?: Record<string, unknown>; props?: Record<string, unknown>; class?: Record<string, boolean> }
}>
const isVNodeLike = (value: unknown): value is VNodeLike => typeof value === "object" && value !== null
const renderHtml = (node: unknown): string => {
  if (!isVNodeLike(node)) return ""
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, on]) => on)
    .map(([c]) => c)
    .join(" ")
  const attrStr = [...Object.entries(attrs), ...Object.entries(props), ...(classes ? [["class", classes]] : [])]
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join("")
  const tag = node.sel ?? "node"
  const children = (node.children ?? []).map((c) => (typeof c === "string" ? c : renderHtml(c))).join("")
  return `<${tag}${attrStr}>${node.text ?? ""}${children}</${tag}>`
}

describe("artifactBrowserSections (#5470)", () => {
  test("proof artifact → proof + receipt sections, refs only", () => {
    const sections = artifactBrowserSections(proofStats())
    const proof = sections.find((s) => s.id === "proof")
    const receipts = sections.find((s) => s.id === "receipts")
    expect(proof?.title).toBe("Proof artifact")
    const labels = proof?.rows.map((r) => r.label) ?? []
    expect(labels).toContain("objective")
    expect(labels).toContain("verify")
    expect(labels).toContain("execution path")
    expect(labels).toContain("redaction")
    // every value is a ref/digest/enum string, never empty
    for (const row of proof?.rows ?? []) expect(row.value.length).toBeGreaterThan(0)
    expect(receipts?.title).toBe("Receipt refs")
    expect(receipts?.rows.map((r) => r.label)).toContain("response digest")
  })

  test("failure artifact surfaces error class + digest, no proof-only fields", () => {
    const sections = artifactBrowserSections(
      proofStats({
        kind: "failure",
        outcome: null,
        detail: {
          ...proofStats().detail!,
          devCheckState: "failed",
          responseDigestRef: null,
          errorClass: "error.pylon.control_session.dev_check_not_passed",
          errorDigestRef: "digest.error.ffff",
          deviationRefs: ["deviation.pylon.control_session.dev_check_not_passed"],
        },
      }),
    )
    const failure = sections.find((s) => s.id === "failure")
    expect(failure?.title).toBe("Failure artifact")
    const byLabel = Object.fromEntries((failure?.rows ?? []).map((r) => [r.label, r.value]))
    expect(byLabel["error class"]).toContain("dev_check_not_passed")
    expect(byLabel["error digest"]).toBe("digest.error.ffff")
    expect(failure?.rows.some((r) => r.label === "deviation")).toBe(true)
  })

  test("no detail (older projection) still shows kind/outcome only", () => {
    const sections = artifactBrowserSections({
      kind: "proof",
      outcome: "completed",
      editedFileCount: null,
      commandCount: null,
      totalTokens: null,
    })
    const proof = sections.find((s) => s.id === "proof")
    expect(proof?.rows.map((r) => r.label)).toEqual(["kind", "outcome"])
    // no receipts section without detail refs
    expect(sections.find((s) => s.id === "receipts")).toBeUndefined()
  })

  test("null stats → no sections", () => {
    expect(artifactBrowserSections(null)).toEqual([])
    expect(artifactBrowserSections(undefined)).toEqual([])
  })
})

describe("update reducer toggles (#5470)", () => {
  test("ToggledDiffFile adds then removes a path", () => {
    const [open] = update(initialModel, ToggledDiffFile({ path: "src/a.ts" }))
    expect(open.expandedDiffFiles).toEqual(["src/a.ts"])
    const [closed] = update(open, ToggledDiffFile({ path: "src/a.ts" }))
    expect(closed.expandedDiffFiles).toEqual([])
  })

  test("ToggledDiffViewMode flips unified <-> split", () => {
    expect(initialModel.diffViewMode).toBe("unified")
    const [split] = update(initialModel, ToggledDiffViewMode())
    expect(split.diffViewMode).toBe("split")
    const [back] = update(split, ToggledDiffViewMode())
    expect(back.diffViewMode).toBe("unified")
  })

  test("ToggledArtifactBrowser flips open", () => {
    expect(initialModel.artifactBrowserOpen).toBe(false)
    const [opened] = update(initialModel, ToggledArtifactBrowser())
    expect(opened.artifactBrowserOpen).toBe(true)
  })
})

describe("session-detail render (#5470)", () => {
  const ref = "session.pylon.control.zzz"
  const node: NodeStateMessage = {
    ok: true,
    schema: "openagents.pylon.control.v0.3",
    sessions: [
      {
        sessionRef: ref,
        adapter: "codex",
        state: "completed",
        accountRefHash: null,
        updatedAt: "2026-06-19T12:00:00.000Z",
      } as NodeStateMessage["sessions"][number],
    ],
    events: {
      [ref]: [
        {
          eventIndex: 1,
          phase: "progress",
          state: "running",
          observedAt: "2026-06-19T12:00:01.000Z",
          detail: "edited src/health.ts (+12 −0)",
        },
        {
          eventIndex: 2,
          phase: "progress",
          state: "running",
          observedAt: "2026-06-19T12:00:02.000Z",
          detail: "added docs/readme.md (+3 −0)",
        },
      ],
    },
    artifacts: { [ref]: proofStats() },
  }

  const detailModel = (over: Partial<Model> = {}): Model => {
    const [withNode] = update(initialModel, GotNodeState({ node }))
    const [selected] = update(withNode, SelectedSession({ sessionRef: ref }))
    return Model.make({ ...selected, pane: "session-detail", ...over })
  }

  test("diff card renders the file tree grouped by directory + the view toggle", () => {
    const rendered = renderHtml((view(detailModel()) as { body: unknown }).body)
    expect(rendered).toContain('data-autopilot-diff-tree=""')
    expect(rendered).toContain('data-autopilot-diff-tree-dir="src"')
    expect(rendered).toContain('data-autopilot-diff-tree-dir="docs"')
    // the public-safe file refs are present, grouped under their dirs
    expect(rendered).toContain('data-autopilot-diff-file="src/health.ts"')
    expect(rendered).toContain('data-autopilot-diff-file="docs/readme.md"')
    // side-by-side toggle control is present (flips diffViewMode)
    expect(rendered).toContain('data-autopilot-diff-view-toggle="unified"')
    // event-derived diffs carry no raw hunk bodies, so no per-file expand strip
    // is offered (nothing to expand) — the strip only appears when hunks exist.
    expect(rendered).not.toContain('data-autopilot-diff-file-toggle=')
  })

  test("split layout flag threads into the shared DiffReview view mode", () => {
    const rendered = renderHtml((view(detailModel({ diffViewMode: "split" }) ) as { body: unknown }).body)
    expect(rendered).toContain('data-autopilot-diff-view-mode="split"')
    expect(rendered).toContain('data-autopilot-diff-view-toggle="split"')
  })

  test("artifact browser is collapsed by default and expands refs on open", () => {
    const collapsed = renderHtml((view(detailModel()) as { body: unknown }).body)
    expect(collapsed).toContain('data-autopilot-artifact-browser-toggle="closed"')
    expect(collapsed).not.toContain('data-autopilot-artifact-browser=""')

    const open = renderHtml((view(detailModel({ artifactBrowserOpen: true })) as { body: unknown }).body)
    expect(open).toContain('data-autopilot-artifact-browser=""')
    expect(open).toContain("digest.objective.aaaa")
    expect(open).toContain("ref.verify.bbbb")
    expect(open).toContain('data-autopilot-artifact-section="proof"')
    expect(open).toContain('data-autopilot-artifact-section="receipts"')
  })

  test("redaction-safety: no raw seed/token/path string leaks into the browser", () => {
    const open = renderHtml((view(detailModel({ artifactBrowserOpen: true })) as { body: unknown }).body)
    // none of these should ever appear; the browser is refs/digests/enums only.
    expect(open).not.toContain("BEGIN PRIVATE KEY")
    expect(open).not.toContain("/Users/")
    expect(open).not.toContain("sk-")
    expect(open).not.toContain("mnemonic")
  })
})

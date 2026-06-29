import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import type { Artifact, Receipt } from "../src/artifacts"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { ArtifactList, ReceiptList } = await import("../src/artifacts")

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

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(" ")
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [["class", classes] as const]),
  ]

  return pairs
    .filter(([, value]) => value !== false && value !== undefined && value !== null)
    .map(([name, value]) => (value === true ? ` ${name}` : ` ${name}="${String(value)}"`))
    .join("")
}

const renderHtml = (html: Html): string => {
  if (html === null) return ""
  if (!isVNodeLike(html)) return ""
  const tag = html.sel ?? "node"
  const children = (html.children ?? [])
    .map((child) => (typeof child === "string" ? child : renderHtml(child)))
    .join("")
  const text = html.text ?? ""

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe("Autopilot artifact and receipt components", () => {
  test("renders artifact names, digest refs, and metadata chips without raw payloads", () => {
    const artifacts = [
      {
        name: "summary.md",
        digestRef: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        contentType: "text/markdown",
        payload: "DO NOT RENDER RAW ARTIFACT PAYLOAD",
      },
      {
        name: "run-log.json",
        digestRef: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        content: "DO NOT RENDER RAW ARTIFACT CONTENT",
      },
    ] as ReadonlyArray<Artifact & { payload?: string; content?: string }>

    const rendered = renderHtml(ArtifactList({ artifacts }))

    expect(rendered).toContain('data-autopilot-artifact-list=""')
    expect(rendered).toContain("summary.md")
    expect(rendered).toContain("run-log.json")
    expect(rendered).toContain("sha256:aaaaaaaaaaa...aaaaaaaaaa")
    expect(rendered).toContain("sha256:ccccccccccc...cccccccccc")
    expect(rendered).toContain("text/markdown / size: ref-only")
    expect(rendered).toContain("content-type: unknown / size: ref-only")
    expect(rendered).not.toContain("DO NOT RENDER RAW ARTIFACT PAYLOAD")
    expect(rendered).not.toContain("DO NOT RENDER RAW ARTIFACT CONTENT")
  })

  test("renders receipt kinds, digest refs, and status chips without raw payloads", () => {
    const receipts = [
      {
        kind: "payment",
        digestRef: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        status: "ok",
        rawPayload: "DO NOT RENDER RAW RECEIPT PAYLOAD",
      },
      {
        kind: "verification",
        digestRef: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        status: "failed",
      },
    ] as ReadonlyArray<Receipt & { rawPayload?: string }>

    const rendered = renderHtml(ReceiptList({ receipts }))

    expect(rendered).toContain('data-autopilot-receipt-list=""')
    expect(rendered).toContain("payment")
    expect(rendered).toContain("verification")
    expect(rendered).toContain("sha256:bbbbbbbbbbb...bbbbbbbbbb")
    expect(rendered).toContain("sha256:ddddddddddd...dddddddddd")
    expect(rendered).toContain('data-autopilot-receipt-status="ok"')
    expect(rendered).toContain('data-autopilot-receipt-status="failed"')
    expect(rendered).not.toContain("DO NOT RENDER RAW RECEIPT PAYLOAD")
  })
})

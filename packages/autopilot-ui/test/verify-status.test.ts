import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { VerifyStatus } = await import("../src/verify-status")

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

describe("Autopilot verify status", () => {
  test("renders a passed fixture", () => {
    const rendered = renderHtml(
      VerifyStatus({
        command: ["bun", "test", "packages/autopilot-ui/test/verify-status.test.ts"],
        status: "passed",
        requiredArtifacts: [
          { ref: "artifact.verify.junit", present: true },
          { ref: "artifact.verify.coverage", present: true },
        ],
      }),
    )

    expect(rendered).toContain('data-autopilot-verify-status="passed"')
    expect(rendered).toContain('data-autopilot-verify-state="passed"')
    expect(rendered).toContain("passed")
    expect(rendered).toContain('data-autopilot-verify-command=""')
    expect(rendered).toContain("bun test packages/autopilot-ui/test/verify-status.test.ts")
    expect(rendered).toContain('data-autopilot-artifact-ref="artifact.verify.junit"')
    expect(rendered).toContain('data-autopilot-artifact-ref="artifact.verify.coverage"')
    expect(rendered).toContain('data-autopilot-artifact-status="present"')
    expect(rendered).toContain("artifact.verify.junit")
    expect(rendered).toContain("artifact.verify.coverage")
    expect(rendered).toContain("present")
  })

  test("renders a failed fixture with a missing artifact", () => {
    const rendered = renderHtml(
      VerifyStatus({
        command: ["bun", "run", "typecheck"],
        status: "failed",
        requiredArtifacts: [
          { ref: "artifact.verify.tsc-log", present: false },
          { ref: "artifact.verify.stdout", present: true },
        ],
      }),
    )

    expect(rendered).toContain('data-autopilot-verify-status="failed"')
    expect(rendered).toContain('data-autopilot-verify-state="failed"')
    expect(rendered).toContain("failed")
    expect(rendered).toContain("bun run typecheck")
    expect(rendered).toContain('data-autopilot-artifact-ref="artifact.verify.tsc-log"')
    expect(rendered).toContain('data-autopilot-artifact-ref="artifact.verify.stdout"')
    expect(rendered).toContain('data-autopilot-artifact-status="missing"')
    expect(rendered).toContain("artifact.verify.tsc-log")
    expect(rendered).toContain("artifact.verify.stdout")
    expect(rendered).toContain("missing")
  })
})

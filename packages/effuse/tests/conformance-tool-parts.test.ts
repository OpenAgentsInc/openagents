import { describe, expect, it } from "vitest"
import { Effect } from "effect"
import { boundText, renderToString, renderToolPart, html } from "../src/index.ts"

describe("conformance: tool part rendering + BlobRef discipline", () => {
  it("boundText truncates and returns a BlobRef when payload is large", async () => {
    const stored = new Map<string, string>()
    let puts = 0

    const putText = ({ text }: { readonly text: string }) =>
      Effect.sync(() => {
        puts++
        const id = `blob-${puts}`
        stored.set(id, text)
        return { id, hash: id, size: text.length }
      })

    const tiny = await Effect.runPromise(
      boundText({
        text: "ok",
        maxChars: 10,
        putText,
        mime: "text/plain",
      })
    )

    expect(tiny.truncated).toBe(false)
    expect(tiny.blob).toBeUndefined()
    expect(puts).toBe(0)

    const big = await Effect.runPromise(
      boundText({
        text: "x".repeat(50),
        maxChars: 10,
        putText,
        mime: "text/plain",
      })
    )

    expect(big.truncated).toBe(true)
    expect(big.preview.length).toBeGreaterThan(10)
    expect(big.preview.startsWith("x".repeat(10))).toBe(true)
    expect(big.blob?.id).toBe("blob-1")
    expect(stored.get("blob-1")).toBe("x".repeat(50))
  })

  it("renderToolPart includes required schema fields and a view-full affordance for blobs", () => {
    const tool = renderToolPart({
      status: "tool-result",
      toolName: "search",
      toolCallId: "call_123",
      summary: "Ran tool",
      details: {
        extra: html`<div data-extra="1">meta</div>`,
        input: {
          preview: "{\\n  \\\"q\\\": \\\"hello\\\"\\n}",
          truncated: false,
        },
        output: {
          preview: "{\\n  \\\"result\\\": \\\"...\\\"\\n}\\n… (truncated)",
          truncated: true,
          blob: { id: "b1", hash: "b1", size: 999, mime: "application/json" },
        },
      },
    })

    const out = renderToString(tool)
    expect(out).toContain("tool-result")
    expect(out).toContain("search")
    expect(out).toContain("call_123")
    expect(out).toContain("Ran tool")
    expect(out).toContain("<details")
    expect(out).toContain("data-ez=\"effuse.blob.view\"")
    expect(out).toContain("data-effuse-blob-id=\"b1\"")
    expect(out).toContain("data-effuse-blob-hash=\"b1\"")
    expect(out).toContain("data-effuse-blob-size=\"999\"")
    expect(out).toContain("data-effuse-blob-mime=\"application/json\"")
  })

  it("renderToolPart always renders a visible card for tool-error", () => {
    const tool = renderToolPart({
      status: "tool-error",
      toolName: "writeFile",
      toolCallId: "call_err",
      summary: "Tool failed",
      details: {
        error: { preview: "Permission denied", truncated: false },
      },
    })

    const out = renderToString(tool)
    expect(out).toContain("tool-error")
    expect(out).toContain("writeFile")
    expect(out).toContain("call_err")
    expect(out).toContain("Tool failed")
    expect(out).toContain("Permission denied")
  })
})


/**
 * Chat markdown projector (#8712, EP250 owner fix 4): bounded markdown subset
 * -> typed catalog Markdown/CodeBlock/Divider views. Pure function; text
 * nodes only (no HTML injection is constructible); streaming-safe on
 * unterminated markers.
 */
import { describe, expect, test } from "vite-plus/test"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"

import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import { chatMarkdownBody, parseChatInlineMarkdown, parseChatMarkdown } from "./markdown.ts"

describe("openagents_desktop.chat.markdown_rendering.v1", () => {
  test("registers the enforced markdown-rendering contract", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(
      (contract) => contract.contractId === "openagents_desktop.chat.markdown_rendering.v1",
    )?.state).toBe("enforced")
  })
})

describe("parseChatInlineMarkdown", () => {
  test("bold, italics, and inline code become typed inline nodes", () => {
    expect(parseChatInlineMarkdown("a **bold** b *it* c `code` d")).toEqual([
      { kind: "text", text: "a " },
      { kind: "strong", children: [{ kind: "text", text: "bold" }] },
      { kind: "text", text: " b " },
      { kind: "emphasis", children: [{ kind: "text", text: "it" }] },
      { kind: "text", text: " c " },
      { kind: "code", text: "code" },
      { kind: "text", text: " d" },
    ])
  })

  test("links render as safe text (label + href), never a link node", () => {
    expect(parseChatInlineMarkdown("see [docs](https://example.com/a) now")).toEqual([
      { kind: "text", text: "see " },
      { kind: "text", text: "docs" },
      { kind: "text", text: " (https://example.com/a)" },
      { kind: "text", text: " now" },
    ])
    // A hostile scheme is inert: it is only ever text.
    const hostile = parseChatInlineMarkdown("[x](javascript:alert(1))")
    expect(hostile.every((node) => node.kind === "text")).toBe(true)
  })

  test("MID-STREAM: an unterminated ** renders literally as plain text until closed", () => {
    expect(parseChatInlineMarkdown("Fable local **streaming")).toEqual([
      { kind: "text", text: "Fable local **streaming" },
    ])
    // …and once the closing marker arrives, it becomes strong.
    expect(parseChatInlineMarkdown("Fable local **streaming** proof.")).toEqual([
      { kind: "text", text: "Fable local " },
      { kind: "strong", children: [{ kind: "text", text: "streaming" }] },
      { kind: "text", text: " proof." },
    ])
  })

  test("MID-STREAM: an unterminated backtick renders literally", () => {
    expect(parseChatInlineMarkdown("run `bun tes")).toEqual([
      { kind: "text", text: "run `bun tes" },
    ])
  })
})

describe("parseChatMarkdown", () => {
  test("headings keep their full 1-6 levels", () => {
    const segments = parseChatMarkdown("# One\n\n###### Six")
    expect(segments).toEqual([{
      kind: "markdown",
      blocks: [
        { kind: "heading", level: 1, children: [{ kind: "text", text: "One" }] },
        { kind: "heading", level: 6, children: [{ kind: "text", text: "Six" }] },
      ],
    }])
  })

  test("fenced code becomes a code segment with its language", () => {
    const segments = parseChatMarkdown("before\n\n```ts\nconst x = 1\n```\n\nafter")
    expect(segments).toEqual([
      { kind: "markdown", blocks: [{ kind: "paragraph", children: [{ kind: "text", text: "before" }] }] },
      { kind: "code", language: "ts", code: "const x = 1" },
      { kind: "markdown", blocks: [{ kind: "paragraph", children: [{ kind: "text", text: "after" }] }] },
    ])
  })

  test("MID-STREAM: an unterminated fence renders as a growing code segment (no crash)", () => {
    const segments = parseChatMarkdown("```ts\nconst x = 1")
    expect(segments).toEqual([{ kind: "code", language: "ts", code: "const x = 1" }])
  })

  test("ordered and unordered lists, blockquotes, and rules parse", () => {
    const segments = parseChatMarkdown("- a\n- b\n\n1. one\n2. two\n\n> quoted\n\n---")
    expect(segments).toEqual([
      {
        kind: "markdown",
        blocks: [
          {
            kind: "list",
            ordered: false,
            items: [
              [{ kind: "paragraph", children: [{ kind: "text", text: "a" }] }],
              [{ kind: "paragraph", children: [{ kind: "text", text: "b" }] }],
            ],
          },
          {
            kind: "list",
            ordered: true,
            items: [
              [{ kind: "paragraph", children: [{ kind: "text", text: "one" }] }],
              [{ kind: "paragraph", children: [{ kind: "text", text: "two" }] }],
            ],
          },
          { kind: "blockquote", children: [{ kind: "paragraph", children: [{ kind: "text", text: "quoted" }] }] },
        ],
      },
      { kind: "rule" },
    ])
  })

  test("plain prose is a single paragraph and empty input never throws", () => {
    expect(parseChatMarkdown("just a line")).toEqual([
      { kind: "markdown", blocks: [{ kind: "paragraph", children: [{ kind: "text", text: "just a line" }] }] },
    ])
    expect(parseChatMarkdown("")).toEqual([
      { kind: "markdown", blocks: [{ kind: "paragraph", children: [] }] },
    ])
  })
})

describe("chatMarkdownBody (segments -> catalog views)", () => {
  test("lowers to Markdown, CodeBlock (plain tokens), and Divider catalog views", () => {
    const views = chatMarkdownBody("m", "# H\n\n```js\na\nb\n```\n\n---\n\ntail")
    expect(views.map((view) => view._tag)).toEqual(["Markdown", "CodeBlock", "Divider", "Markdown"])
    const code = views[1] as unknown as { lines: Array<{ tokens: Array<{ kind: string; text: string }> }>; language?: string }
    expect(code.language).toBe("js")
    expect(code.lines).toEqual([
      { tokens: [{ kind: "plain", text: "a" }] },
      { tokens: [{ kind: "plain", text: "b" }] },
    ])
    // Keys are stable and derived from the prefix (streaming re-render safe).
    expect(views.map((view) => view.key)).toEqual(["m-md-0", "m-code-1", "m-rule-2", "m-md-3"])
  })

  test("re-rendering per streamed append is stable and never throws mid-token", () => {
    const stream = ["Fable ", "local ", "**stream", "ing** ", "proof.\n", "```", "ts\nconst ", "x = 1\n```"]
    let text = ""
    for (const delta of stream) {
      text += delta
      expect(() => chatMarkdownBody("s", text)).not.toThrow()
    }
    const final = chatMarkdownBody("s", text)
    expect(final.map((view) => view._tag)).toEqual(["Markdown", "CodeBlock"])
  })
})

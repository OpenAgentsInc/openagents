import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import fc from "fast-check"
import { Window } from "happy-dom"

import {
  diffElement,
  markdownElement,
  renderMessageBody,
} from "../src/ui/transcript-render"
import type { KhalaCodeDesktopCodexItemCard } from "../src/shared/rpc"

const hostileLiteral = fc.constantFrom(
  "<script>alert(1)</script>",
  "<img src=x onerror=alert(1)>",
  "<svg onload=alert(1)>x</svg>",
  "<a href=\"javascript:alert(1)\">bad</a>",
)

const hostileInput = fc
  .tuple(fc.string({ maxLength: 80 }), hostileLiteral, fc.string({ maxLength: 80 }))
  .map(([before, hostile, after]) => `${before}${hostile}${after}`)

const diffInput = fc.record({
  file: fc.stringMatching(/[a-z][a-z0-9_-]{0,8}\.ts/),
  before: hostileInput,
  after: hostileInput,
}).map(({ file, before, after }) =>
  `--- a/${file}\n+++ b/${file}\n@@ -1 +1 @@\n-${before}\n+${after}\n`)

const toolCard: KhalaCodeDesktopCodexItemCard = {
  itemId: "item-property",
  itemType: "commandExecution",
  status: "completed",
  title: "Ran command",
}

let previousDocument: typeof globalThis.document | undefined
let previousWindow: typeof globalThis.window | undefined
let previousNavigator: typeof globalThis.navigator | undefined
let previousRequestAnimationFrame: typeof globalThis.requestAnimationFrame | undefined

const installDom = (): void => {
  const window = new Window()
  previousDocument = globalThis.document
  previousWindow = globalThis.window
  previousNavigator = globalThis.navigator
  previousRequestAnimationFrame = globalThis.requestAnimationFrame
  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { clipboard: { writeText: async () => undefined } },
  })
  Object.defineProperty(globalThis, "requestAnimationFrame", {
    configurable: true,
    value: (callback: FrameRequestCallback) => {
      callback(0)
      return 0
    },
  })
}

const restoreDom = (): void => {
  Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
  Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: previousNavigator })
  Object.defineProperty(globalThis, "requestAnimationFrame", {
    configurable: true,
    value: previousRequestAnimationFrame,
  })
}

const expectEscaped = (nodes: readonly HTMLElement[]): void => {
  const root = document.createElement("div")
  root.append(...nodes.map(node => node.cloneNode(true)))
  expect(root.querySelector("script")).toBeNull()
  expect(root.querySelector("img")).toBeNull()
  for (const element of root.querySelectorAll("*")) {
    for (const attribute of element.getAttributeNames()) {
      expect(attribute.toLowerCase().startsWith("on")).toBe(false)
      expect(element.getAttribute(attribute)?.toLowerCase().startsWith("javascript:")).toBe(false)
    }
  }
}

describe("transcript renderer properties", () => {
  beforeAll(installDom)
  afterAll(restoreDom)

  test("arbitrary markdown input never crashes or injects raw HTML", () => {
    fc.assert(
      fc.property(hostileInput, (markdown) => {
        const node = markdownElement({ markdown })
        expect(node.textContent).toContain("<")
        expectEscaped([node])
      }),
      { numRuns: 150 },
    )
  })

  test("arbitrary diff input never crashes or injects raw HTML", () => {
    fc.assert(
      fc.property(diffInput, (patch) => {
        const node = diffElement({ patch })
        expect(node.textContent).toContain("<")
        expectEscaped([node])
      }),
      { numRuns: 150 },
    )
  })

  test("message body rendering escapes markdown, diff, tool, and card bodies", () => {
    fc.assert(
      fc.property(hostileInput, diffInput, (markdown, patch) => {
        expectEscaped(renderMessageBody(markdown, "assistant"))
        expectEscaped(renderMessageBody(patch, "assistant"))
        expectEscaped(renderMessageBody(markdown, "tool"))
        expectEscaped(renderMessageBody(markdown, "tool", toolCard))
      }),
      { numRuns: 100 },
    )
  })
})

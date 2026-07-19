import { readFileSync } from "node:fs"
import path from "node:path"

import { Window } from "happy-dom"
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { IdePathIndexGenerationSchema } from "../src/ide/project-contract.ts"
import {
  IdePathNodeRefSchema,
  IdePathScanRefSchema,
  IdePierreTreeProjectionSchema,
  type IdeExplorerCommand,
} from "../src/ide/path-index-contract.ts"
import { PierreWorkspaceTree } from "../src/renderer/ide/pierre-tree-adapter.tsx"

const roots = new Set<Root>()
const restores: Array<() => void> = []

const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  const domRect = window.DOMRect as typeof DOMRect
  Object.defineProperty(domRect, "fromRect", {
    configurable: true,
    value: (rect: Partial<DOMRect> = {}) =>
      new window.DOMRect(rect.x ?? 0, rect.y ?? 0, rect.width ?? 0, rect.height ?? 0),
  })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    CSS: window.CSS,
    CSSStyleSheet: window.CSSStyleSheet,
    customElements: window.customElements,
    Node: window.Node,
    Text: window.Text,
    Document: window.Document,
    Range: window.Range,
    DOMRect: domRect,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    HTMLDivElement: window.HTMLDivElement,
    HTMLInputElement: window.HTMLInputElement,
    HTMLStyleElement: window.HTMLStyleElement,
    HTMLTemplateElement: window.HTMLTemplateElement,
    SVGElement: window.SVGElement,
    ShadowRoot: window.ShadowRoot,
    Event: window.Event,
    FocusEvent: window.FocusEvent,
    InputEvent: window.InputEvent,
    CompositionEvent: window.CompositionEvent,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
    MutationObserver: window.MutationObserver,
    ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
    IS_REACT_ACT_ENVIRONMENT: true,
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  restores.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) Reflect.deleteProperty(globalThis, name)
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div")
  window.document.body.appendChild(container)
  return { window, container: container as unknown as HTMLDivElement }
}

const settle = async (): Promise<void> => {
  await new Promise(resolve => setTimeout(resolve, 0))
}

afterEach(async () => {
  await act(async () => {
    for (const root of roots) root.unmount()
    roots.clear()
    await settle()
  })
  while (restores.length > 0) restores.pop()?.()
})

const projection = (scanning = false) => IdePierreTreeProjectionSchema.make({
  schemaVersion: "openagents.desktop.pierre-tree-projection.v1",
  indexGeneration: IdePathIndexGenerationSchema.make(7),
  state: scanning
    ? { _tag: "Scanning", scanRef: IdePathScanRefSchema.make("ide.path-scan.accessibility"), progress: { discoveredDirectories: 1, scannedDirectories: 0, discoveredNodes: 3, admittedNodes: 3, pendingDirectories: 1, sourceEpoch: 4 }, reason: "initial" }
    : { _tag: "Ready", sourceEpoch: 4, nodeCount: 3 },
  nodes: [
    { nodeRef: IdePathNodeRefSchema.make("ide.path-node.1"), pathRef: "src", kind: "directory", revisionRef: "revision-src", badgeLabels: [], pendingLabel: null },
    { nodeRef: IdePathNodeRefSchema.make("ide.path-node.2"), pathRef: "README.md", kind: "file", revisionRef: "revision-readme", badgeLabels: ["Git modified"], pendingLabel: null },
    { nodeRef: IdePathNodeRefSchema.make("ide.path-node.3"), pathRef: "src/index.ts", kind: "file", revisionRef: "revision-index", badgeLabels: ["2 error diagnostics", "Unsaved changes"], pendingLabel: null },
  ],
  expandedNodeRefs: [IdePathNodeRefSchema.make("ide.path-node.1")],
  selectedNodeRef: IdePathNodeRefSchema.make("ide.path-node.3"),
  focusedNodeRef: IdePathNodeRefSchema.make("ide.path-node.3"),
  scrollAnchorNodeRef: IdePathNodeRefSchema.make("ide.path-node.3"),
  stickyAncestorNodeRefs: [IdePathNodeRefSchema.make("ide.path-node.1")],
  truncated: false,
})

describe("IDE Explorer accessibility journeys", () => {
  test("keyboard", async () => {
    const { window, container } = installDom()
    const intents: IdeExplorerCommand[] = []
    const root = createRoot(container)
    roots.add(root)
    await act(async () => {
      root.render(<PierreWorkspaceTree projection={projection()} onIntent={intent => intents.push(intent)} />)
      await settle()
    })
    const host = container.querySelector<HTMLElement>('[data-oa-pierre-tree="true"]')
    const row = host?.shadowRoot?.querySelector<HTMLElement>('[data-item-path="src/index.ts"]')
    expect(row).not.toBeNull()
    await act(async () => {
      row?.focus()
      row?.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Home", bubbles: true }))
      row?.dispatchEvent(new window.KeyboardEvent("keydown", { key: " ", code: "Space", ctrlKey: true, bubbles: true }))
      await settle()
    })
    expect(intents.some(intent => intent._tag === "Open" && intent.pathRef === "src")).toBe(true)
    expect(host?.shadowRoot?.querySelector('[role="tree"]')).not.toBeNull()
  })

  test("screen-reader", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    roots.add(root)
    await act(async () => {
      root.render(<PierreWorkspaceTree projection={projection(true)} onIntent={() => undefined} />)
      await settle()
    })
    const host = container.querySelector<HTMLElement>('[data-oa-pierre-tree="true"]')
    expect(host?.getAttribute("aria-label")).toBe("Workspace files")
    expect(host?.getAttribute("aria-busy")).toBe("true")
    expect(host?.getAttribute("aria-describedby")).toBe("oa-workspace-index-status")
    expect(host?.dataset.oaIndexGeneration).toBe("7")
    const selected = host?.shadowRoot?.querySelector<HTMLElement>('[data-item-path="src/index.ts"]')
    expect(selected?.getAttribute("aria-level")).toBe("2")
    expect(selected?.getAttribute("aria-selected")).toBe("true")
  })

  test("reduced-motion", () => {
    const source = readFileSync(path.resolve(import.meta.dirname, "../src/renderer/app.css"), "utf8")
    expect(source).toContain("@media (prefers-reduced-motion: reduce)")
    expect(source).toContain(':root[data-en-reduce-motion="true"]')
    expect(source).toContain("scroll-behavior: auto !important")
  })

  test("zoom-200", async () => {
    const { container } = installDom()
    container.style.width = "240px"
    container.style.fontSize = "200%"
    const root = createRoot(container)
    roots.add(root)
    await act(async () => {
      root.render(<PierreWorkspaceTree projection={projection()} onIntent={() => undefined} />)
      await settle()
    })
    const host = container.querySelector<HTMLElement>('[data-oa-pierre-tree="true"]')
    expect(host).not.toBeNull()
    expect(host?.shadowRoot?.querySelector('[data-item-path="src/index.ts"]')).not.toBeNull()
    expect(host?.getAttribute("aria-label")).toBe("Workspace files")
  })

  test("pointer context and drag/drop have keyboard-equivalent typed commands", () => {
    const source = readFileSync(path.resolve(import.meta.dirname, "../src/renderer/ide/pierre-tree-adapter.tsx"), "utf8")
    expect(source).toContain("dragAndDrop: {")
    expect(source).toContain("expectedRevisionRef: node.revisionRef")
    expect(source).toContain("renderContextMenu=")
    expect(source).toContain("model.startRenaming")
    expect(source).toContain('role="menuitem"')
    expect(source).toContain('_tag: "OpenTerminal"')
    expect(source).toContain('_tag: "Compare"')
  })
})

import { Effect } from "@effect-native/core/effect"
import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Window } from "happy-dom"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { initialDesktopShellState } from "./shell.ts"
import { emptyWorkspaceEditorState, WorkspaceEditorTabSchema } from "./workspace-editor.ts"

const restores: Array<() => void> = []
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    customElements: window.customElements,
    Node: window.Node,
    Element: window.Element,
    SVGElement: window.SVGElement,
    CSSStyleSheet: window.CSSStyleSheet,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    Event: window.Event,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
    ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  restores.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
  return container
}

afterEach(async () => {
  await new Promise((resolve) => setTimeout(resolve, 0))
  restores.splice(0).reverse().forEach((restore) => restore())
})

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 20))
const recorder = () => {
  const received: Array<{ name: string; payload: unknown }> = []
  const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
  return { received, report }
}

const editorStateWithFile = (pathRef: string) => {
  const tab = WorkspaceEditorTabSchema.make({
    pathRef,
    phase: "loading",
    document: null,
    externalDocument: null,
    draft: "",
    selection: { start: 0, end: 0 },
    selectionVersion: 0,
    undo: [],
    redo: [],
    saveState: "idle",
    reason: null,
    findQuery: "",
    findMatches: [],
    findIndex: 0,
  })
  return { ...emptyWorkspaceEditorState(), tabs: [tab], activePathRef: pathRef }
}

describe("Editor agent rail (AFS-05)", () => {
  test("mounts the agent context tray and cursor rail while the active file stays visible", async () => {
    const container = installDom()
    const { ReactWorkspaceEditor } = await import("./react-workspace-surfaces.tsx")
    const state = {
      ...initialDesktopShellState("electron/darwin"),
      workspace: "files" as const,
      workspaceEditor: editorStateWithFile("src/active.ts"),
    }
    const root = createRoot(container)
    root.render(<ReactWorkspaceEditor state={state} report={recorder().report} />)
    await settle()
    // The active file remains visible in the editor toolbar.
    expect(container.textContent).toContain("src/active.ts")
    // The agent context disclosure tray is mounted in the production Editor rail.
    expect(container.querySelector('[aria-label="Agent context disclosure"]')).not.toBeNull()
    // The cursor "AI editing" rail is present for in-editor ask/change work.
    expect(container.querySelector('[aria-label="AI editing"]')).not.toBeNull()
    root.unmount()
  })
})

import type { IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Window } from "happy-dom"
import { act, createRef } from "react"
import { createRoot } from "react-dom/client"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"

import { emptyGitPanelState } from "./git-panel.ts"
import { initialDesktopShellState } from "./shell.ts"

const previous = new Map<string, PropertyDescriptor | undefined>()
let window: Window
let container: HTMLDivElement

beforeAll(() => {
  window = new Window({ url: "http://localhost/" })
  class ResizeObserverStub { observe(): void {} unobserve(): void {} disconnect(): void {} }
  Object.defineProperty(window, "matchMedia", { configurable: true, value: () => ({
    matches: false, media: "(min-width: 1120px)", addEventListener: () => {}, removeEventListener: () => {},
  }) })
  const values = {
    window, document: window.document, navigator: window.navigator, Node: window.Node,
    Element: window.Element, HTMLElement: window.HTMLElement, HTMLButtonElement: window.HTMLButtonElement,
    Event: window.Event, KeyboardEvent: window.KeyboardEvent, MouseEvent: window.MouseEvent,
    MutationObserver: window.MutationObserver, ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  }
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
})

afterAll(async () => {
  await new Promise(resolve => setTimeout(resolve, 40))
  for (const [name, descriptor] of previous) {
    if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
    else Object.defineProperty(globalThis, name, descriptor)
  }
})

describe("React narrow repository review", () => {
  test("uses the shadcn sheet and renders typed refusal copy", async () => {
    const { ReviewSurface } = await import("./react-review.tsx")
    const base = initialDesktopShellState("electron/darwin")
    const state = {
      ...base,
      git: { ...emptyGitPanelState(), phase: "ready" as const, reviewFailure: "secret_diff" as const },
    }
    const report: IntentReporter = () => Effect.void
    const root = createRoot(container)
    await act(async () => {
      root.render(<ReviewSurface state={state} report={report} open onOpenChange={() => {}} triggerRef={createRef()} />)
    })
    expect(window.document.querySelector("[data-slot='sheet-content']")).not.toBeNull()
    expect(window.document.body.textContent).toContain("Potentially sensitive diff content was withheld.")
    await act(async () => { root.unmount() })
  })
})

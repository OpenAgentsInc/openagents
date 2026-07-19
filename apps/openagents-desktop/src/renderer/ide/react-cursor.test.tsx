import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Window } from "happy-dom"
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  IdeCursorSnapshotSchema,
  emptyIdeCursorSnapshot,
} from "../../ide/cursor-contract.ts"
import {
  ideCursorFixtureCandidate,
  ideCursorFixtureDisclosure,
  ideCursorFixtureRequest,
} from "../../ide/cursor-fixture.ts"
import { IdeCursorRendererStateSchema } from "./cursor.ts"
import { initialDesktopShellState } from "../shell.ts"

const restores: Array<() => void> = []
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLInputElement: window.HTMLInputElement,
    Event: window.Event,
    KeyboardEvent: window.KeyboardEvent,
    MouseEvent: window.MouseEvent,
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
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
  return { window, container }
}

afterEach(async () => {
  await new Promise(resolve => setTimeout(resolve, 0))
  restores.splice(0).reverse().forEach(restore => restore())
})

const recorder = () => {
  const received: Array<{ name: string; payload: unknown }> = []
  const report: IntentReporter = (ref, payload) =>
    Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
  return { received, report }
}

describe("IDE-09 rendered cursor surface", () => {
  test("exposes every request, partial-accept, review, comparison, rejection, undo, and disclosure control", async () => {
    const { container } = installDom()
    const { ReactIdeCursor } = await import("./react-cursor.tsx")
    const request = ideCursorFixtureRequest()
    const candidate = ideCursorFixtureCandidate(request)
    const state = {
      ...initialDesktopShellState("electron/darwin"),
      ideCursor: IdeCursorRendererStateSchema.make({
        snapshot: IdeCursorSnapshotSchema.make({
          ...emptyIdeCursorSnapshot(),
          latestSequence: request.sequence,
          activeRequestRef: request.requestRef,
          activeAttemptRef: request.attemptRef,
          candidates: [candidate],
          finalDisclosure: ideCursorFixtureDisclosure(),
          state: "complete",
        }),
        activeRequest: request,
        selectedCandidateRef: candidate.candidateRef,
        prompt: "change the selection",
        notice: null,
        invalidation: null,
      }),
    }
    const { received, report } = recorder()
    const root = createRoot(container)
    await act(async () => {
      root.render(<ReactIdeCursor state={state} report={report} />)
      await new Promise(resolve => setTimeout(resolve, 20))
    })
    const labels = [...container.querySelectorAll("button")].map(button => button.textContent?.trim())
    expect(labels).toEqual(expect.arrayContaining([
      "Complete", "Next edit", "Ask", "Change", "Generate", "Cancel",
      "Accept word", "Accept line", "Accept all", "Compare", "Retry", "Reject", "Undo",
    ]))
    expect(container.querySelector('[aria-label="Ask or change code"]')).not.toBeNull()
    expect(container.textContent).toContain("No remote index dependency")
    const compare = [...container.querySelectorAll("button")].find(button => button.textContent === "Compare")
    await act(async () => {
      compare?.click()
      await new Promise(resolve => setTimeout(resolve, 20))
    })
    expect(received).toContainEqual({
      name: "IdeCursorDecisionRequested",
      payload: { action: "compare", candidateRef: candidate.candidateRef },
    })
    root.unmount()
  })

  test("does not cancel during IME composition and does cancel the exact active request on Escape", async () => {
    const { window, container } = installDom()
    const { ReactIdeCursor } = await import("./react-cursor.tsx")
    const request = ideCursorFixtureRequest("ime")
    const candidate = ideCursorFixtureCandidate(request)
    const { received, report } = recorder()
    const root = createRoot(container)
    await act(async () => {
      root.render(<ReactIdeCursor state={{
        ...initialDesktopShellState("electron/darwin"),
        ideCursor: IdeCursorRendererStateSchema.make({
          snapshot: IdeCursorSnapshotSchema.make({
            ...emptyIdeCursorSnapshot(),
            latestSequence: request.sequence,
            activeRequestRef: request.requestRef,
            activeAttemptRef: request.attemptRef,
            candidates: [candidate],
            state: "running",
          }),
          activeRequest: request,
          selectedCandidateRef: candidate.candidateRef,
          prompt: "",
          notice: null,
          invalidation: null,
        }),
      }} report={report} />)
      await new Promise(resolve => setTimeout(resolve, 20))
    })
    const region = container.querySelector('[aria-label="AI editing"]')
    if (region === null) throw new Error("AI editing region did not render")
    await act(async () => {
      region.dispatchEvent(new window.KeyboardEvent("keydown", {
        key: "Escape",
        bubbles: true,
        isComposing: true,
      }) as unknown as Event)
      await new Promise(resolve => setTimeout(resolve, 20))
    })
    expect(received).toHaveLength(0)
    await act(async () => {
      region.dispatchEvent(new window.KeyboardEvent("keydown", {
        key: "Escape",
        bubbles: true,
      }) as unknown as Event)
      await new Promise(resolve => setTimeout(resolve, 20))
    })
    expect(received).toContainEqual({
      name: "IdeCursorDecisionRequested",
      payload: { action: "cancel", candidateRef: candidate.candidateRef },
    })
    root.unmount()
  })
})

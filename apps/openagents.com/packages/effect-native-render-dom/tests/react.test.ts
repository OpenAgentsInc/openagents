import { afterEach, describe, expect, test } from "vitest"
import { Effect, Stream } from "effect"
import { Window } from "happy-dom"
import { Button, Icon, IntentRef, Stack, Text, type IntentReporter } from "@effect-native/core"
import { makeReactDomRenderer, makeReactViewStore } from "../src/react"

const restoreGlobals: Array<() => void> = []
const installDom = () => {
  const window = new Window()
  const document = window.document as unknown as Document
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries({
    window, document, navigator: window.navigator, Node: window.Node,
    Element: window.Element, HTMLElement: window.HTMLElement,
    Event: window.Event, MouseEvent: window.MouseEvent
  })) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  restoreGlobals.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = document.createElement("main")
  document.body.appendChild(container)
  return { container, document }
}

afterEach(async () => {
  await new Promise((resolve) => setTimeout(resolve, 0))
  restoreGlobals.splice(0).reverse().forEach((restore) => restore())
})

const noopReport: IntentReporter = () => Effect.void

describe("React DOM projection boundary", () => {
  test("compatibility and React are mutually exclusive whole-surface backends", async () => {
    const { container, document } = installDom()
    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      const surface = yield* makeReactDomRenderer({ document, backend: "compatibility" }).mount(
        container,
        Stream.make(Stack({ key: "root", direction: "column" }, [
          Text({ key: "message", content: "Compatibility", variant: "body" })
        ])),
        noopReport
      )
      expect(surface.backend).toBe("compatibility")
      expect(container.querySelectorAll("[data-en-react-backend]")).toHaveLength(1)
      expect(container.querySelector('[data-en-react-backend="react"]')).toBeNull()
      yield* surface.unmount
      expect(container.childElementCount).toBe(0)
    })))
  })

  test("snapshot identity is stable and Strict Mode cannot reopen the Effect source", async () => {
    let opens = 0
    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      const store = yield* makeReactViewStore(Stream.suspend(() => {
        opens += 1
        return Stream.make(Stack({ key: "root", direction: "column" }, []))
      }))
      yield* store.firstCommit
      const first = store.getSnapshot()
      expect(store.getSnapshot()).toBe(first)
      store.subscribe(() => {})()
      store.subscribe(() => {})()
      expect(opens).toBe(1)
    })))
  })

  test("projects application state without introducing a second authority", async () => {
    const state = { selectedSessionRef: "thread-1", title: "First thread" } as const
    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      const store = yield* makeReactViewStore(Stream.make(state))
      yield* store.firstCommit
      const snapshot = store.getSnapshot()
      expect(snapshot.status).toBe("ready")
      if (snapshot.status === "ready") expect(snapshot.view).toBe(state)
    })))
  })

  test("semantic lowerings preserve keys, a11y, styles, and exact-once intents", async () => {
    const { container, document } = installDom()
    const received: Array<string> = []
    const report: IntentReporter = (ref) => Effect.sync(() => received.push(ref.name))
    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      const surface = yield* makeReactDomRenderer({ document, backend: "react" }).mount(
        container,
        Stream.make(Stack({ key: "root", direction: "column", gap: "2" }, [
          Text({ key: "heading", content: "Codex workbench", variant: "heading", a11y: { label: "Workbench" } }),
          Button({ key: "run", label: "Run", onPress: IntentRef("workbench.run") })
        ])),
        report
      )
      expect(container.querySelectorAll("[data-en-react-backend]")).toHaveLength(1)
      expect(container.querySelector('[data-en-key="heading"]')?.getAttribute("aria-label")).toBe("Workbench")
      ;(container.querySelector('[data-en-key="run"]') as HTMLButtonElement).click()
      yield* Effect.promise(() => new Promise((resolve) => setTimeout(resolve, 0)))
      expect(received).toEqual(["workbench.run"])
      yield* surface.unmount
      expect(surface.activeReactSubscribers()).toBe(0)
    })))
  })

  test("unsupported tags fail visibly without mounting a second backend", async () => {
    const { container, document } = installDom()
    const previousError = console.error
    console.error = () => {}
    try {
      await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
        const surface = yield* makeReactDomRenderer({ document, backend: "react" }).mount(
          container,
          Stream.make(Icon({ key: "unsupported", name: "Agent", label: "Agent" })),
          noopReport
        )
        expect(container.querySelector('[data-en-react-state="incompatible"]')?.getAttribute("role")).toBe("alert")
        expect(container.querySelector('[data-en-react-backend="compatibility"]')).toBeNull()
        yield* surface.unmount
      })))
    } finally {
      console.error = previousError
    }
  })
})

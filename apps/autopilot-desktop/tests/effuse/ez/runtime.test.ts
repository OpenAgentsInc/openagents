import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import {
  EffuseLive,
  html,
  makeEzRegistry,
  mountEzRuntimeWith,
} from "../../../src/effuse/index.js"

const waitFor = (ms: number) =>
  Effect.promise<void>(
    () => new Promise((resolve) => setTimeout(resolve, ms))
  )

const makeRoot = () =>
  Effect.gen(function* () {
    const root = document.createElement("div")
    document.body.appendChild(root)
    yield* Effect.addFinalizer(() => Effect.sync(() => root.remove()))
    return root
  })

describe("Ez runtime", () => {
  it.live("uses default triggers based on element type", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const calls = { button: 0, input: 0, form: 0 }
        const registry = makeEzRegistry([
          [
            "action.button",
            () =>
              Effect.sync(() => {
                calls.button += 1
              }),
          ],
          [
            "action.input",
            () =>
              Effect.sync(() => {
                calls.input += 1
              }),
          ],
          [
            "action.form",
            () =>
              Effect.sync(() => {
                calls.form += 1
              }),
          ],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `
          <button id="btn" data-ez="action.button">Tap</button>
          <input id="field" name="field" value="x" data-ez="action.input" />
          <form id="form" data-ez="action.form"></form>
        `

        yield* mountEzRuntimeWith(root, registry)

        root.querySelector("#btn")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )
        root.querySelector("#field")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )
        root.querySelector("#field")?.dispatchEvent(
          new Event("change", { bubbles: true })
        )
        root.querySelector("#form")?.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true })
        )

        yield* waitFor(0)

        yield* Effect.sync(() => {
          expect(calls.button).toBe(1)
          expect(calls.input).toBe(1)
          expect(calls.form).toBe(1)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("resolves targets for this/closest/find/selectors", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const registry = makeEzRegistry([
          ["action.this", () => Effect.succeed(html`<span>self</span>`)],
          ["action.closest", () => Effect.succeed(html`<span>closest</span>`)],
          ["action.find", () => Effect.succeed(html`<span>find</span>`)],
          ["action.selector", () => Effect.succeed(html`<span>selector</span>`)],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `
          <button id="self" data-ez="action.this">Self</button>
          <div class="wrap" id="wrap">
            <button id="closest" data-ez="action.closest" data-ez-target="closest(.wrap)">Closest</button>
          </div>
          <div id="find-root" data-ez="action.find" data-ez-target="find(.child)">
            <div class="child">Old</div>
          </div>
          <button id="selector" data-ez="action.selector" data-ez-target="#target">Run</button>
        `

        const target = document.createElement("div")
        target.id = "target"
        document.body.appendChild(target)
        yield* Effect.addFinalizer(() =>
          Effect.sync(() => {
            target.remove()
          })
        )

        yield* mountEzRuntimeWith(root, registry)

        root.querySelector("#self")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )
        root.querySelector("#closest")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )
        root.querySelector("#find-root")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )
        root.querySelector("#selector")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )

        yield* waitFor(0)

        const wrap = root.querySelector("#wrap") as HTMLElement
        const child = root.querySelector(".child") as HTMLElement
        const selectorTarget = document.querySelector("#target") as HTMLElement

        yield* Effect.sync(() => {
          expect((root.querySelector("#self") as HTMLElement).innerHTML).toContain(
            "self"
          )
          expect(wrap.innerHTML).toContain("closest")
          expect(child.innerHTML).toContain("find")
          expect(selectorTarget.innerHTML).toContain("selector")
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("defaults swap mode to inner", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const registry = makeEzRegistry([
          ["action.swap", () => Effect.succeed(html`<span>new</span>`)],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `<div id="swap" data-ez="action.swap">old</div>`
        yield* mountEzRuntimeWith(root, registry)

        root.querySelector("#swap")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )

        yield* waitFor(0)

        const target = root.querySelector("#swap") as HTMLElement
        yield* Effect.sync(() => {
          expect(target.innerHTML).toContain("new")
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("collects params from submit/input and data-ez-vals", () =>
    Effect.scoped(
      Effect.gen(function* () {
        let submitParams: Record<string, string> | null = null
        let inputParams: Record<string, string> | null = null
        let valsParams: Record<string, string> | null = null

        const registry = makeEzRegistry([
          [
            "action.submit",
            ({ params }) =>
              Effect.sync(() => {
                submitParams = params
              }),
          ],
          [
            "action.input",
            ({ params }) =>
              Effect.sync(() => {
                inputParams = params
              }),
          ],
          [
            "action.vals",
            ({ params }) =>
              Effect.sync(() => {
                valsParams = params
              }),
          ],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `
          <form id="form" data-ez="action.submit">
            <input name="first" value="Ada" />
          </form>
          <input id="query" name="q" value="effuse" data-ez="action.input" />
          <button id="vals" data-ez="action.vals" data-ez-vals='{"static":"x","obj":{"a":1}}'>Vals</button>
        `

        yield* mountEzRuntimeWith(root, registry)

        root.querySelector("#form")?.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true })
        )
        root.querySelector("#query")?.dispatchEvent(
          new Event("change", { bubbles: true })
        )
        root.querySelector("#vals")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )

        yield* waitFor(0)

        yield* Effect.sync(() => {
          expect(submitParams).toEqual({ first: "Ada" })
          expect(inputParams).toEqual({ q: "effuse" })
          expect(valsParams).toEqual({
            static: "x",
            obj: JSON.stringify({ a: 1 }),
          })
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("skips action when data-ez-confirm returns false", () =>
    Effect.scoped(
      Effect.gen(function* () {
        let called = false
        const registry = makeEzRegistry([
          [
            "action.confirm",
            () =>
              Effect.sync(() => {
                called = true
              }),
          ],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `<button id="confirm" data-ez="action.confirm" data-ez-confirm="sure?">Confirm</button>`
        yield* mountEzRuntimeWith(root, registry)

        const original = window.confirm
        window.confirm = () => false

        root.querySelector("#confirm")?.dispatchEvent(
          new MouseEvent("click", { bubbles: true })
        )

        yield* waitFor(0)

        window.confirm = original

        yield* Effect.sync(() => {
          expect(called).toBe(false)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("disables and restores action elements when data-ez-disable is present", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const registry = makeEzRegistry([
          [
            "action.disable",
            () => Effect.sleep("40 millis").pipe(Effect.as(html`ok`)),
          ],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `
          <button id="btn" data-ez="action.disable" data-ez-disable>Run</button>
          <button id="btn2" data-ez="action.disable" data-ez-disable disabled>Run</button>
        `

        yield* mountEzRuntimeWith(root, registry)

        const btn = root.querySelector("#btn") as HTMLButtonElement
        const btn2 = root.querySelector("#btn2") as HTMLButtonElement

        btn.dispatchEvent(new MouseEvent("click", { bubbles: true }))
        btn2.dispatchEvent(new MouseEvent("click", { bubbles: true }))

        yield* waitFor(0)

        yield* Effect.sync(() => {
          expect(btn.disabled).toBe(true)
          expect(btn.getAttribute("aria-disabled")).toBe("true")
          expect(btn2.disabled).toBe(true)
        })

        yield* waitFor(60)

        yield* Effect.sync(() => {
          expect(btn.disabled).toBe(false)
          expect(btn.getAttribute("aria-disabled")).toBe(null)
          expect(btn2.disabled).toBe(true)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )

  it.live("interrupts in-flight actions on re-trigger", () =>
    Effect.scoped(
      Effect.gen(function* () {
        let runs = 0
        let completes = 0
        let interrupts = 0

        const registry = makeEzRegistry([
          [
            "action.long",
            () =>
              Effect.sync(() => {
                runs += 1
              }).pipe(
                Effect.zipRight(Effect.sleep("60 millis")),
                Effect.tap(() =>
                  Effect.sync(() => {
                    completes += 1
                  })
                ),
                Effect.onInterrupt(() =>
                  Effect.sync(() => {
                    interrupts += 1
                  })
                )
              ),
          ],
        ])

        const root = yield* makeRoot()
        root.innerHTML = `<button id="long" data-ez="action.long">Go</button>`
        yield* mountEzRuntimeWith(root, registry)

        const button = root.querySelector("#long") as HTMLButtonElement
        button.dispatchEvent(new MouseEvent("click", { bubbles: true }))
        button.dispatchEvent(new MouseEvent("click", { bubbles: true }))

        yield* waitFor(120)

        yield* Effect.sync(() => {
          expect(runs).toBe(2)
          expect(completes).toBe(1)
          expect(interrupts).toBe(1)
        })
      }).pipe(Effect.provide(EffuseLive))
    )
  )
})

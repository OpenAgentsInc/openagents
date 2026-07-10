import { describe, expect, test } from "bun:test"
import { Window } from "../../../apps/web/node_modules/happy-dom"
import {
  Button,
  Icon,
  IntentRef,
  Stack,
  UnknownIntentError,
  iconNames,
  type IntentReporter
} from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"
import { Deferred } from "effect"

import { makeDomRenderer } from "../src/index"

const nextTask = Effect.promise<void>(
  () => new Promise((resolve) => setTimeout(resolve, 0))
)

describe("DOM renderer host boundaries", () => {
  test("resolves every closed icon and lowers Compose through the ChatCompose asset", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const report: IntentReporter = () => Effect.succeed(undefined)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Stack(
              { key: "icons", direction: "row" },
              iconNames.map((name) => Icon({ key: `icon-${name}`, name }))
            )),
            report
          )

          expect(root.querySelectorAll("[data-en-icon] svg")).toHaveLength(iconNames.length)
          const compose = root.querySelector('[data-en-icon="Compose"] svg')
          expect(compose?.outerHTML).toContain("M12 4.5")
        })
      )
    )
  })

  test("deduplicates atomic CSS while retaining the exact declaration value", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const view = Stack(
      {
        key: "outer",
        direction: "column",
        style: { backgroundColor: "accent" }
      },
      [
        Stack({
          key: "inner",
          direction: "column",
          style: { backgroundColor: "accent" }
        })
      ]
    )
    const report: IntentReporter = () => Effect.succeed(undefined)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const surface = yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(view),
            report
          )
          const outer = root.querySelector('[data-en-key="outer"]')
          const inner = root.querySelector('[data-en-key="inner"]')
          expect(outer?.className).toBe(inner?.className)

          const stylesheet = yield* surface.stylesheetText
          expect(stylesheet.match(/\.en-[a-z0-9]+\{background-color:var\(--en-color-accent\);\}/g)).toHaveLength(1)
        })
      )
    )
  })

  test("keeps a failing intent callback total while executing its effect", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const attempted: Array<string> = []
    const report: IntentReporter = (ref) =>
      Effect.sync(() => {
        attempted.push(ref.name)
      }).pipe(
        Effect.andThen(Effect.fail(new UnknownIntentError({ name: ref.name })))
      )

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Button({
              key: "failing-button",
              label: "Fail safely",
              variant: "primary",
              onPress: IntentRef("FailSafely")
            })),
            report
          )
          const button = root.querySelector("button") as HTMLButtonElement | null
          expect(button).not.toBeNull()
          expect(() => button?.click()).not.toThrow()
          yield* nextTask
          expect(attempted).toEqual(["FailSafely"])
        })
      )
    )
  })

  test("interrupts an in-flight intent when the mounted surface unmounts", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const started = yield* Deferred.make<void>()
          const interrupted = yield* Deferred.make<void>()
          const report: IntentReporter = () =>
            Deferred.succeed(started, undefined).pipe(
              Effect.andThen(Effect.never),
              Effect.onInterrupt(() =>
                Effect.sleep("1 millis").pipe(
                  Effect.andThen(Deferred.succeed(interrupted, undefined))
                )
              )
            )
          const surface = yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Button({
              key: "in-flight-button",
              label: "Start",
              variant: "primary",
              onPress: IntentRef("StartInFlight")
            })),
            report
          )
          const button = root.querySelector("button") as HTMLButtonElement | null
          expect(button).not.toBeNull()
          button?.click()
          yield* Deferred.await(started)

          yield* surface.unmount

          yield* Deferred.await(interrupted)
          expect(surface.root.isConnected).toBe(false)
        })
      )
    )
  })
})

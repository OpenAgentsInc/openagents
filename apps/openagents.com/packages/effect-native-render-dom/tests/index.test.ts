import { describe, expect, test } from "bun:test"
import { Window } from "../../../apps/web/node_modules/happy-dom"
import {
  Button,
  IntentRef,
  Stack,
  UnknownIntentError,
  type IntentReporter
} from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"

import { makeDomRenderer } from "../src/index"

const nextTask = Effect.promise<void>(
  () => new Promise((resolve) => setTimeout(resolve, 0))
)

describe("DOM renderer host boundaries", () => {
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
})

import { describe, expect, test } from "bun:test"
import {
  Button,
  IntentRef,
  Stack,
  UnknownIntentError,
  type IntentReporter,
  type View
} from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"

import {
  createEffectNativeSurface,
  makeReactNativeRenderer,
  renderReactNativeView,
  type ReactElementLike,
  type ReactNativeDependencies,
  type ReactNodeLike
} from "./index"

const createElement = (
  type: unknown,
  props: Record<string, unknown> | null = null,
  ...children: ReadonlyArray<ReactNodeLike>
): ReactElementLike => ({
  type,
  key: typeof props?.key === "string" ? props.key : null,
  props: {
    ...(props ?? {}),
    ...(children.length === 0
      ? {}
      : { children: children.length === 1 ? children[0] : children })
  }
})

const reactNative = {
  View: "View",
  Text: "Text",
  Pressable: "Pressable",
  TextInput: "TextInput",
  FlatList: "FlatList",
  SectionList: "SectionList",
  Image: "Image",
  Modal: "Modal"
}

const nextTask = Effect.promise<void>(
  () => new Promise((resolve) => setTimeout(resolve, 0))
)

describe("React Native renderer host boundaries", () => {
  test("keeps a failing intent callback total while executing its effect", async () => {
    const attempted: Array<string> = []
    const report: IntentReporter = (ref) =>
      Effect.sync(() => {
        attempted.push(ref.name)
      }).pipe(
        Effect.andThen(Effect.fail(new UnknownIntentError({ name: ref.name })))
      )
    const element = renderReactNativeView(
      Button({
        key: "failing-button",
        label: "Fail safely",
        variant: "primary",
        onPress: IntentRef("FailSafely")
      }),
      { React: { createElement }, ReactNative: reactNative },
      report
    )
    const onPress = element.props.onPress
    if (typeof onPress !== "function") {
      throw new Error("expected an onPress callback")
    }

    expect(() => onPress()).not.toThrow()
    await Effect.runPromise(nextTask)
    expect(attempted).toEqual(["FailSafely"])
  })

  test("interrupts the view stream through total React unmount cleanup", async () => {
    let cleanup: (() => void) | undefined
    let finalized = false
    const dependencies: ReactNativeDependencies = {
      React: {
        createElement,
        useState: <State>(initial: State | (() => State)) => [
          typeof initial === "function" ? (initial as () => State)() : initial,
          () => undefined
        ],
        useEffect: (effect) => {
          const finalizer = effect()
          cleanup = typeof finalizer === "function" ? finalizer : undefined
        }
      },
      ReactNative: reactNative
    }
    const Surface = createEffectNativeSurface(dependencies)
    const report: IntentReporter = () => Effect.succeed(undefined)
    const initialView: View = Stack({ key: "root", direction: "column" })

    Surface({
      viewStream: Stream.never.pipe(
        Stream.ensuring(Effect.sync(() => {
          finalized = true
        }))
      ),
      report,
      initialView
    })
    expect(typeof cleanup).toBe("function")
    expect(() => cleanup?.()).not.toThrow()
    await Effect.runPromise(nextTask)
    expect(finalized).toBe(true)
  })

  test("updates viewport callbacks and removes the native subscription on unmount", async () => {
    let listener: ((event: { readonly window?: { readonly width: number; readonly height: number } }) => void) | undefined
    let removed = false
    const dependencies: ReactNativeDependencies = {
      React: { createElement },
      ReactNative: {
        ...reactNative,
        Dimensions: {
          get: () => ({ width: 320, height: 640 }),
          addEventListener: (_type, nextListener) => {
            listener = nextListener
            return { remove: () => { removed = true } }
          }
        }
      }
    }
    const report: IntentReporter = () => Effect.succeed(undefined)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const surface = yield* makeReactNativeRenderer({ dependencies }).mount(
            undefined,
            Stream.succeed(Stack({ key: "root", direction: "column" })),
            report
          )
          expect(listener).toBeDefined()
          expect(() => listener?.({ window: { width: 768, height: 1024 } })).not.toThrow()
          yield* nextTask
          expect(yield* surface.currentViewport).toMatchObject({ width: 768, height: 1024 })
          yield* surface.unmount
          expect(removed).toBe(true)
        })
      )
    )
  })
})

import { describe, expect, test } from "bun:test"
import {
  Button,
  Composer,
  IntentRef,
  Stack,
  UnknownIntentError,
  type IntentReporter,
  type View
} from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"
import { Deferred, FiberSet } from "effect"

import {
  createEffectNativeSurface,
  makeReactNativeRenderer,
  renderReactNativeView,
  type ReactElementLike,
  type ExpoUiSwiftUiRuntime,
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
  test("lowers a glass composer through renderer-owned SwiftUI on iOS 26 with typed change and submit parity", async () => {
    const cleanups: Array<() => void> = []
    let observedNativeState: { readonly get: () => string; readonly set: (value: string) => void } | undefined
    const dependencies: ReactNativeDependencies = {
      React: {
        createElement,
        useState: <State>(initial: State | (() => State)) => {
          let value = typeof initial === "function"
            ? (initial as () => State)()
            : initial
          return [value, (next: State | ((current: State) => State)) => {
            value = typeof next === "function"
              ? (next as (current: State) => State)(value)
              : next
          }] as const
        },
        useEffect: (effect) => {
          const cleanup = effect()
          if (typeof cleanup === "function") cleanups.push(cleanup)
        }
      },
      ReactNative: {
        ...reactNative,
        Platform: { OS: "ios", Version: 26 }
      }
    }
    const expoUi: ExpoUiSwiftUiRuntime = {
      Host: "SwiftHost",
      HStack: "SwiftHStack",
      VStack: "SwiftVStack",
      Button: "SwiftButton",
      Image: "SwiftImage",
      Text: "SwiftText",
      Spacer: "SwiftSpacer",
      TextField: "SwiftTextField",
      useNativeState: <Value>(initialValue: Value) => {
        let value = initialValue
        const state = {
          get: () => value,
          set: (next: Value) => { value = next }
        }
        observedNativeState = state as unknown as typeof observedNativeState
        return state
      },
      modifiers: {
        glassEffect: (value) => ({ kind: "glass", value }),
        foregroundStyle: (value) => ({ kind: "foreground", value }),
        frame: (value) => ({ kind: "frame", value })
      }
    }
    const events: Array<readonly [string, unknown]> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => {
      events.push([ref.name, payload])
    })
    const element = renderReactNativeView(
      Composer({
        key: "composer",
        doc: [{ kind: "text", text: "draft" }],
        mode: "normal",
        placeholder: "Message",
        clearOnSubmit: true,
        onChange: IntentRef("Changed"),
        onSubmit: IntentRef("Submitted"),
        style: { surface: "glass" }
      }),
      dependencies,
      report,
      { expoUi, platform: "ios" }
    )
    expect(element.props.testID).toBe("en-composer:normal")
    const lifecycleRef = element.props.ref
    if (typeof lifecycleRef === "function") lifecycleRef({})
    const host = element.props.children as ReactElementLike
    expect(host.type).toBe("SwiftHost")
    const component = host.props.children as ReactElementLike
    if (typeof component.type !== "function") throw new Error("expected native composer component")
    const native = component.type(component.props) as ReactElementLike
    const nativeChildren = native.props.children as ReadonlyArray<ReactElementLike>
    const field = nativeChildren[0]!
    const submit = nativeChildren[1]!
    expect(field.type).toBe("SwiftTextField")
    expect(submit.type).toBe("SwiftButton")
    if (observedNativeState === undefined) throw new Error("expected native state")
    const nativeState = observedNativeState
    nativeState.set("edited")
    ;(field.props.onTextChange as (value: string) => void)("edited")
    ;(submit.props.onPress as () => void)()
    await Effect.runPromise(nextTask)
    expect(events).toEqual([["Changed", "edited"], ["Submitted", "edited"]])
    expect(nativeState.get()).toBe("")
    if (typeof lifecycleRef === "function") lifecycleRef(null)
    for (const cleanup of cleanups) cleanup()
  })

  test("keeps the iOS glass composer component identity stable across draft emissions", () => {
    const dependencies: ReactNativeDependencies = {
      React: {
        createElement,
        useEffect: () => undefined,
        useState: <State>(initial: State | (() => State)) => [
          typeof initial === "function" ? (initial as () => State)() : initial,
          () => undefined,
        ],
      },
      ReactNative: { ...reactNative, Platform: { OS: "ios", Version: 26 } }
    }
    const expoUi: ExpoUiSwiftUiRuntime = {
      Host: "SwiftHost",
      HStack: "SwiftHStack",
      VStack: "SwiftVStack",
      Button: "SwiftButton",
      Image: "SwiftImage",
      Text: "SwiftText",
      Spacer: "SwiftSpacer",
      TextField: "SwiftTextField",
      useNativeState: <Value>(initialValue: Value) => ({
        get: () => initialValue,
        set: () => undefined
      }),
      modifiers: {
        glassEffect: (value) => ({ kind: "glass", value }),
        foregroundStyle: (value) => ({ kind: "foreground", value }),
        frame: (value) => ({ kind: "frame", value })
      }
    }
    const render = (text: string): ReactElementLike => renderReactNativeView(
      Composer({
        key: "composer",
        doc: [{ kind: "text", text }],
        mode: "normal",
        placeholder: "Message",
        onChange: IntentRef("Changed"),
        onSubmit: IntentRef("Submitted"),
        style: { surface: "glass" }
      }),
      dependencies,
      () => Effect.succeed(undefined),
      { expoUi, platform: "ios" }
    )
    const firstHost = render("a").props.children as ReactElementLike
    const secondHost = render("ab").props.children as ReactElementLike
    const firstComposer = firstHost.props.children as ReactElementLike
    const secondComposer = secondHost.props.children as ReactElementLike

    expect(firstComposer.key).toBe("composer")
    expect(secondComposer.key).toBe("composer")
    expect(secondComposer.type).toBe(firstComposer.type)
    expect(secondComposer.props.view).not.toBe(firstComposer.props.view)
  })

  test("imperatively clears SwiftUI and rejects stale native echoes while the controlled clear commits", async () => {
    const hookValues: unknown[] = []
    let hookIndex = 0
    let nativeValue = ""
    let nativeInitialized = false
    const scheduledNativeWrites: string[] = []
    const nativeState = {
      get: () => nativeValue,
      // This matches @expo/ui useNativeState: JS writes are scheduled to the
      // native UI thread and get() continues to expose the old value until the
      // scheduled update is applied.
      set: (value: string) => { scheduledNativeWrites.push(value) },
    }
    const dependencies: ReactNativeDependencies = {
      React: {
        createElement,
        useEffect: effect => { effect() },
        useState: <State>(initial: State | (() => State)) => {
          const index = hookIndex++
          if (!(index in hookValues)) {
            hookValues[index] = typeof initial === "function"
              ? (initial as () => State)()
              : initial
          }
          return [hookValues[index] as State, (next: State | ((current: State) => State)) => {
            const current = hookValues[index] as State
            hookValues[index] = typeof next === "function"
              ? (next as (value: State) => State)(current)
              : next
          }] as const
        },
      },
      ReactNative: { ...reactNative, Platform: { OS: "ios", Version: 26 } },
    }
    const expoUi: ExpoUiSwiftUiRuntime = {
      Host: "SwiftHost",
      HStack: "SwiftHStack",
      VStack: "SwiftVStack",
      Button: "SwiftButton",
      Image: "SwiftImage",
      Text: "SwiftText",
      Spacer: "SwiftSpacer",
      TextField: "SwiftTextField",
      useNativeState: <Value>(initialValue: Value) => {
        if (!nativeInitialized) {
          nativeValue = initialValue as string
          nativeInitialized = true
        }
        return nativeState as unknown as {
          readonly get: () => Value
          readonly set: (value: Value) => void
        }
      },
      modifiers: {
        glassEffect: value => ({ kind: "glass", value }),
        foregroundStyle: value => ({ kind: "foreground", value }),
        frame: value => ({ kind: "frame", value }),
      },
    }
    const events: Array<readonly [string, unknown]> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => {
      events.push([ref.name, payload])
    })
    const renderNative = (text: string): ReactElementLike => {
      hookIndex = 0
      const rendered = renderReactNativeView(
        Composer({
          key: "composer",
          doc: text === "" ? [] : [{ kind: "text", text }],
          mode: "normal",
          placeholder: "Message",
          clearOnSubmit: true,
          onChange: IntentRef("Changed"),
          onSubmit: IntentRef("Submitted"),
          style: { surface: "glass" },
        }),
        dependencies,
        report,
        { expoUi, platform: "ios" },
      )
      const host = rendered.props.children as ReactElementLike
      const component = host.props.children as ReactElementLike
      if (typeof component.type !== "function") throw new Error("expected native composer component")
      return component.type(component.props) as ReactElementLike
    }

    const first = renderNative("submitted draft")
    const firstChildren = first.props.children as ReadonlyArray<ReactElementLike>
    const field = firstChildren[0]!
    const submit = firstChildren[1]!
    const fieldRef = field.props.ref as { current: { clear: () => Promise<void> } | null }
    let imperativeClearCount = 0
    fieldRef.current = {
      clear: async () => {
        imperativeClearCount += 1
        nativeValue = ""
      },
    }
    nativeValue = "submitted draft"
    ;(submit.props.onPress as () => void)()
    await Effect.runPromise(nextTask)
    expect(nativeValue).toBe("")
    expect(imperativeClearCount).toBe(1)
    expect(events).toEqual([["Submitted", "submitted draft"]])

    // A delayed native change can arrive from the already-committed TextField
    // closure before React has rendered the app's cleared controlled value.
    // It must neither restore app state nor remain visible in SwiftUI.
    nativeValue = "submitted draft"
    ;(field.props.onTextChange as (value: string) => void)("submitted draft")
    await Effect.runPromise(nextTask)
    expect(nativeValue).toBe("")
    expect(imperativeClearCount).toBe(2)
    expect(events).toEqual([["Submitted", "submitted draft"]])

    // The app's Effect has not emitted its cleared controlled draft yet. This
    // render used to copy the stale submitted text back into SwiftUI.
    renderNative("submitted draft")
    expect(nativeValue).toBe("")

    const cleared = renderNative("")
    expect(nativeValue).toBe("")

    // A late clear notification is also internal synchronization, not a new
    // user edit. A distinct subsequent edit ends suppression and propagates.
    const clearedField = (cleared.props.children as ReadonlyArray<ReactElementLike>)[0]!
    ;(clearedField.props.onTextChange as (value: string) => void)("")
    ;(clearedField.props.onTextChange as (value: string) => void)("n")
    await Effect.runPromise(nextTask)
    expect(events).toEqual([
      ["Submitted", "submitted draft"],
      ["Changed", "n"],
    ])

    // Applying queued ObservableState writes after those callbacks cannot
    // invalidate the proof: the imperative field clear already happened in
    // the native view, and the next controlled emission owns the new draft.
    expect(scheduledNativeWrites).toContain("")
  })

  test("keeps the accessible RN composer fallback on Android", () => {
    const element = renderReactNativeView(
      Composer({
        key: "composer",
        doc: [],
        mode: "normal",
        placeholder: "Message",
        onSubmit: IntentRef("Submitted"),
        style: { surface: "glass" }
      }),
      {
        React: { createElement },
        ReactNative: { ...reactNative, Platform: { OS: "android", Version: 35 } }
      },
      () => Effect.succeed(undefined),
      { platform: "android" }
    )
    const structure = JSON.stringify(element)
    expect(structure).toContain('"type":"TextInput"')
    expect(structure).toContain('"testID":"en-composer-submit"')
    expect(structure).toContain('"accessibilityLabel":"Send message"')
    expect(structure).not.toContain("SwiftHost")
  })

  test("keeps labelled structural stacks out of the screen-reader focus order", () => {
    const element = renderReactNativeView(
      Stack(
        {
          key: "application-root",
          direction: "column",
          a11y: { role: "region", label: "Application root" }
        },
        [Button({
          key: "open-navigation",
          label: "Open navigation",
          variant: "ghost",
          onPress: IntentRef("OpenNavigation")
        })]
      ),
      {
        React: { createElement },
        ReactNative: { ...reactNative, Platform: { OS: "android", Version: 35 } }
      },
      () => Effect.succeed(undefined),
      { platform: "android" }
    )
    expect(element.props.accessibilityLabel).toBe("Application root")
    expect(element.props.accessible).toBe(false)
    expect(element.props.importantForAccessibility).toBe("no")
    const child = element.props.children as ReactElementLike
    expect(child.props.accessibilityRole).toBe("button")
    expect(JSON.stringify(child)).toContain("Open navigation")
  })

  test("keeps an unavailable-target draft editable while disabling Send", () => {
    const element = renderReactNativeView(
      Composer({
        key: "composer-unavailable-target",
        doc: [{ kind: "text", text: "offline draft" }],
        mode: "normal",
        placeholder: "Continue conversation",
        onChange: IntentRef("Changed")
      }),
      {
        React: { createElement },
        ReactNative: { ...reactNative, Platform: { OS: "android", Version: 35 } }
      },
      () => Effect.succeed(undefined),
      { platform: "android" }
    )
    const structure = JSON.stringify(element)
    expect(structure).toContain('"editable":true')
    expect(structure).toContain('"accessibilityLabel":"Send unavailable"')
    expect(structure).toContain('"disabled":true')
  })

  test("keeps a failing intent callback total while executing its effect", async () => {
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
          const runFiber = yield* FiberSet.makeRuntime<never, void, never>()
          const element = renderReactNativeView(
            Button({
              key: "failing-button",
              label: "Fail safely",
              variant: "primary",
              onPress: IntentRef("FailSafely")
            }),
            { React: { createElement }, ReactNative: reactNative },
            report,
            { runEffect: (effect) => { runFiber(effect) } }
          )
          const onPress = element.props.onPress
          if (typeof onPress !== "function") {
            throw new Error("expected an onPress callback")
          }

          expect(() => onPress()).not.toThrow()
          yield* nextTask
        })
      )
    )
    expect(attempted).toEqual(["FailSafely"])
  })

  test("owns direct-render intent effects through the root ref lifecycle", async () => {
    const started = await Effect.runPromise(Deferred.make<void>())
    const interrupted = await Effect.runPromise(Deferred.make<void>())
    const report: IntentReporter = () =>
      Deferred.succeed(started, undefined).pipe(
        Effect.andThen(Effect.never),
        Effect.onInterrupt(() =>
          Effect.sleep("1 millis").pipe(
            Effect.andThen(Deferred.succeed(interrupted, undefined))
          )
        )
      )
    const element = renderReactNativeView(
      Button({
        key: "runtime-required",
        label: "No daemon",
        variant: "primary",
        onPress: IntentRef("RuntimeRequired")
      }),
      { React: { createElement }, ReactNative: reactNative },
      report
    )
    const onPress = element.props.onPress
    if (typeof onPress !== "function") {
      throw new Error("expected an onPress callback")
    }
    const ref = element.props.ref
    if (typeof ref !== "function") {
      throw new Error("expected a lifecycle ref callback")
    }

    expect(() => onPress()).not.toThrow()
    await Effect.runPromise(Deferred.await(started))
    expect(() => ref(null)).not.toThrow()
    await Effect.runPromise(Deferred.await(interrupted))
  })

  test("interrupts the view stream through total React unmount cleanup", async () => {
    // The surface registers multiple effects (view-stream runtime + the
    // host-driver runtime dispose); real React runs EVERY cleanup on unmount.
    const cleanups: Array<() => void> = []
    const cleanup = () => {
      for (const finalizer of cleanups) {
        finalizer()
      }
    }
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
          if (typeof finalizer === "function") {
            cleanups.push(finalizer)
          }
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
    expect(cleanups.length).toBeGreaterThan(0)
    expect(() => cleanup()).not.toThrow()
    await Effect.runPromise(nextTask)
    expect(finalized).toBe(true)
  })

  test("interrupts an in-flight intent through React unmount cleanup", async () => {
    const cleanups: Array<() => void> = []
    const cleanup = () => {
      for (const finalizer of cleanups) {
        finalizer()
      }
    }
    const started = await Effect.runPromise(Deferred.make<void>())
    const interrupted = await Effect.runPromise(Deferred.make<void>())
    const dependencies: ReactNativeDependencies = {
      React: {
        createElement,
        useState: <State>(initial: State | (() => State)) => [
          typeof initial === "function" ? (initial as () => State)() : initial,
          () => undefined
        ],
        useEffect: (effect) => {
          const finalizer = effect()
          if (typeof finalizer === "function") {
            cleanups.push(finalizer)
          }
        }
      },
      ReactNative: reactNative
    }
    const Surface = createEffectNativeSurface(dependencies)
    const report: IntentReporter = () =>
      Deferred.succeed(started, undefined).pipe(
        Effect.andThen(Effect.never),
        Effect.onInterrupt(() =>
          Effect.sleep("1 millis").pipe(
            Effect.andThen(Deferred.succeed(interrupted, undefined))
          )
        )
      )
    const element = Surface({
      viewStream: Stream.never,
      report,
      initialView: Button({
        key: "in-flight-button",
        label: "Start",
        variant: "primary",
        onPress: IntentRef("StartInFlight")
      })
    })
    if (typeof element !== "object" || element === null) {
      throw new Error("expected a rendered button")
    }
    const onPress = element.props.onPress
    if (typeof onPress !== "function") {
      throw new Error("expected an onPress callback")
    }
    onPress()
    await Effect.runPromise(Deferred.await(started))

    expect(() => cleanup()).not.toThrow()

    await Effect.runPromise(Deferred.await(interrupted))
  })

  test("an old async cleanup cannot clear a restarted hook runtime", async () => {
    const stateSlots: Array<unknown> = []
    let stateIndex = 0
    let renderCleanups: Array<() => void> = []
    const dependencies: ReactNativeDependencies = {
      React: {
        createElement,
        useState: <State>(initial: State | (() => State)) => {
          const index = stateIndex
          stateIndex += 1
          if (!(index in stateSlots)) {
            stateSlots[index] = typeof initial === "function" ? (initial as () => State)() : initial
          }
          return [stateSlots[index] as State, () => undefined]
        },
        useEffect: (effect) => {
          const finalizer = effect()
          if (typeof finalizer === "function") {
            renderCleanups.push(finalizer)
          }
        }
      },
      ReactNative: reactNative
    }
    const Surface = createEffectNativeSurface(dependencies)
    const view = Button({
      key: "restart-button",
      label: "Restart",
      variant: "primary",
      onPress: IntentRef("AfterRestart")
    })
    let secondRuntimeDispatches = 0

    stateIndex = 0
    Surface({
      viewStream: Stream.never.pipe(Stream.ensuring(Effect.sleep("10 millis"))),
      report: () => Effect.succeed(undefined),
      initialView: view
    })
    await Effect.runPromise(nextTask)
    const firstCleanups = renderCleanups
    for (const finalizer of firstCleanups) {
      finalizer()
    }

    renderCleanups = []
    stateIndex = 0
    const restartedElement = Surface({
      viewStream: Stream.never,
      report: () => Effect.sync(() => {
        secondRuntimeDispatches += 1
      }),
      initialView: view
    })
    const secondCleanups = renderCleanups
    await Effect.runPromise(Effect.sleep("20 millis"))
    if (typeof restartedElement !== "object" || restartedElement === null) {
      throw new Error("expected a restarted button")
    }
    const onPress = restartedElement.props.onPress
    if (typeof onPress !== "function") {
      throw new Error("expected an onPress callback")
    }
    onPress()
    await Effect.runPromise(nextTask)

    expect(secondRuntimeDispatches).toBe(1)
    for (const finalizer of secondCleanups) {
      finalizer()
    }
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

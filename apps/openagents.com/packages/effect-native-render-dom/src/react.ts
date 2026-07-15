/** React-owned Effect Native DOM surface with an explicit whole-surface backend. */
import { Component, StrictMode, createElement, useLayoutEffect, useSyncExternalStore, type ErrorInfo, type ReactElement, type ReactNode } from "react"
import { createRoot, type Root } from "react-dom/client"
import { Deferred, Effect, Exit, Fiber, Scope, Stream } from "effect"
import type { IntentReporter, MountedSurface, RendererAdapter, View } from "@effect-native/core"
import { defaultTheme } from "@effect-native/tokens"
import {
  makeDomRenderer,
  mountDomThemeStyleSheet,
  type DomMountedSurface,
  type DomRendererOptions,
  type DomThemeStyleSheet
} from "./index.js"
import { renderReactDomView } from "./react-lowering.js"
import { makeReactViewStore, type ReactViewSnapshot, type ReactViewStore } from "./react-store.js"

export const packageName = "@effect-native/render-dom/react" as const

export type ReactDomBackend = "react" | "compatibility"

export interface EffectNativeReactDomSurfaceProps extends DomRendererOptions {
  readonly viewStore: ReactViewStore
  readonly report: IntentReporter
  readonly onError?: (error: unknown) => void
  readonly onCommit?: (snapshot: ReactViewSnapshot) => void
}

export type ReactDomMountedSurface = MountedSurface & {
  readonly reactRoot: Root
  readonly backend: ReactDomBackend
  readonly activeReactSubscribers: () => number
} & (
  | { readonly backend: "react"; readonly stylesheet: DomThemeStyleSheet }
  | { readonly backend: "compatibility"; readonly domSurface: DomMountedSurface }
)

interface BoundaryProps {
  readonly children?: ReactNode
  readonly resetKey: number
  readonly onError?: (error: unknown) => void
  readonly onSettled?: () => void
}

interface BoundaryState {
  readonly error: Error | undefined
  readonly resetKey: number
}

export class ReactSurfaceErrorBoundary extends Component<BoundaryProps, BoundaryState> {
  override state: BoundaryState = { error: undefined, resetKey: this.props.resetKey }

  static getDerivedStateFromError(error: Error): Partial<BoundaryState> {
    return { error }
  }

  static getDerivedStateFromProps(props: BoundaryProps, state: BoundaryState): Partial<BoundaryState> | null {
    return props.resetKey === state.resetKey ? null : { error: undefined, resetKey: props.resetKey }
  }

  override componentDidCatch(error: Error, _info: ErrorInfo): void {
    this.props.onError?.(error)
    this.props.onSettled?.()
  }

  override componentDidMount(): void {
    if (this.state.error === undefined) this.props.onSettled?.()
  }

  override componentDidUpdate(previous: BoundaryProps): void {
    if (previous.resetKey !== this.props.resetKey && this.state.error === undefined) {
      this.props.onSettled?.()
    }
  }

  override render(): ReactNode {
    if (this.state.error !== undefined) {
      return createElement("section", {
        role: "alert",
        "data-en-react-state": "incompatible",
        "data-en-react-error": this.state.error.name
      }, "This surface is not available in the React renderer yet.")
    }
    return this.props.children
  }
}

const ReactLoweredView = (props: {
  readonly view: View
  readonly report: IntentReporter
  readonly theme: EffectNativeReactDomSurfaceProps["theme"]
}): ReactElement => renderReactDomView(props.view, { report: props.report, theme: props.theme ?? defaultTheme })

const ReactStatus = (props: {
  readonly state: "loading" | "failed"
  readonly onCommit?: () => void
}): ReactElement => {
  useLayoutEffect(() => props.onCommit?.(), [props.onCommit])
  return props.state === "loading"
    ? createElement("div", { role: "status", "data-en-react-state": "loading" }, "Loading…")
    : createElement("section", { role: "alert", "data-en-react-state": "failed" }, "The surface stopped updating.")
}

const ReactViewProjection = (props: EffectNativeReactDomSurfaceProps): ReactElement => {
  const snapshot = useSyncExternalStore(
    props.viewStore.subscribe,
    props.viewStore.getSnapshot,
    props.viewStore.getServerSnapshot
  )
  if (snapshot.status === "loading") {
    return createElement(ReactStatus, { state: "loading" })
  }
  if (snapshot.status === "failed") {
    return createElement(ReactStatus, {
      state: "failed",
      ...(props.onCommit === undefined ? {} : { onCommit: () => props.onCommit?.(snapshot) })
    })
  }
  return createElement(ReactSurfaceErrorBoundary, {
    resetKey: snapshot.revision,
    ...(props.onError === undefined ? {} : { onError: props.onError }),
    ...(props.onCommit === undefined ? {} : {
      onSettled: () => queueMicrotask(() => props.onCommit?.(snapshot))
    })
  }, createElement(ReactLoweredView, { view: snapshot.view, report: props.report, theme: props.theme }))
}

export const EffectNativeReactDomSurface = (
  props: EffectNativeReactDomSurfaceProps
): ReactElement => createElement(ReactViewProjection, props)

const mountCompatibilityBackend = (
  container: Element,
  viewStream: Stream.Stream<View>,
  report: IntentReporter,
  options: DomRendererOptions
): Effect.Effect<ReactDomMountedSurface, never, Scope.Scope> =>
  Effect.gen(function*() {
    const ready = yield* Deferred.make<DomMountedSurface, unknown>()
    const attached = yield* Deferred.make<void>()
    const reactRoot = yield* Effect.sync(() => createRoot(container))
    const host = options.document?.createElement("div") ?? container.ownerDocument.createElement("div")
    host.setAttribute("data-en-react-surface", "true")
    host.setAttribute("data-en-react-backend", "compatibility")
    yield* Effect.sync(() => reactRoot.render(createElement("div", {
      ref: (element: HTMLDivElement | null) => {
        if (element !== null && host.parentNode === null) {
          element.appendChild(host)
          Effect.runFork(Deferred.succeed(attached, undefined))
        }
      }
    })))
    yield* Deferred.await(attached)
    const backendScope = yield* Scope.make()
    const mountFiber = Effect.runFork(Scope.provide(backendScope)(
      makeDomRenderer(options).mount(host, viewStream, report)
    ))
    void Effect.runPromise(Fiber.join(mountFiber)).then(
      (surface) => Effect.runFork(Deferred.succeed(ready, surface)),
      (error) => Effect.runFork(Deferred.fail(ready, error))
    )
    const domSurface = yield* Deferred.await(ready).pipe(Effect.orDie)
    let disposed = false
    const unmount = Effect.suspend(() => {
      if (disposed) return Effect.void
      disposed = true
      return Effect.andThen(
        Scope.close(backendScope, Exit.void),
        Effect.sync(() => reactRoot.unmount())
      )
    })
    yield* Effect.addFinalizer(() => unmount)
    return {
      reactRoot,
      backend: "compatibility" as const,
      domSurface,
      activeReactSubscribers: () => 0,
      unmount
    }
  })

const mountReactBackend = (
  container: Element,
  viewStream: Stream.Stream<View>,
  report: IntentReporter,
  options: DomRendererOptions
): Effect.Effect<ReactDomMountedSurface, never, Scope.Scope> =>
  Effect.gen(function*() {
    const document = options.document ?? container.ownerDocument
    const viewStore = yield* makeReactViewStore(viewStream)
    const committed = yield* Deferred.make<void>()
    const stylesheet = yield* Effect.sync(() => mountDomThemeStyleSheet(document, options.theme))
    const reactRoot = yield* Effect.sync(() => createRoot(container))
    let disposed = false
    const unmount = Effect.sync(() => {
      if (disposed) return
      disposed = true
      reactRoot.unmount()
      stylesheet.dispose()
    })
    yield* Effect.addFinalizer(() => unmount)
    yield* Effect.sync(() => reactRoot.render(createElement(StrictMode, null,
      createElement("div", {
        "data-effect-native-surface": "dom",
        "data-en-react-surface": "true",
        "data-en-react-backend": "react"
      }, createElement(EffectNativeReactDomSurface, {
        ...options,
        viewStore,
        report,
        onCommit: (snapshot) => {
          if (snapshot.status === "ready" || snapshot.status === "failed") {
            Effect.runFork(Deferred.succeed(committed, undefined))
          }
        }
      }))
    )))
    yield* viewStore.firstCommit
    yield* Deferred.await(committed)
    return {
      reactRoot,
      backend: "react" as const,
      stylesheet,
      activeReactSubscribers: viewStore.activeSubscribers,
      unmount
    }
  })

/** Selects one backend for the lifetime of one authoritative surface. */
export const makeReactDomRenderer = (
  options: DomRendererOptions & { readonly backend?: ReactDomBackend } = {}
): RendererAdapter<Element, ReactDomMountedSurface> => ({
  mount: (container, viewStream, report) => {
    const { backend = "compatibility", ...domOptions } = options
    return backend === "react"
      ? mountReactBackend(container, viewStream, report, domOptions)
      : mountCompatibilityBackend(container, viewStream, report, domOptions)
  }
})

export type { DomMountedSurface, ReactViewSnapshot, ReactViewStore }
export { makeReactViewStore, renderReactDomView }

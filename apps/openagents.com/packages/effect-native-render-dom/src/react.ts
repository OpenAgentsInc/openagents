/**
 * React-owned Effect Native DOM surface.
 *
 * React owns the application root and stream subscription. During the first
 * migration phase the existing DOM renderer remains the compatibility
 * lowering for the catalog itself, preserving every Effect Native component,
 * host driver, intent, accessibility attribute, and CSS contract while React
 * applications adopt this entrypoint. Native React component lowerings can be
 * moved across this boundary incrementally without changing application code.
 */
import { Deferred, Effect, Exit, Fiber, Scope, Stream } from "effect"
import { createElement, useEffect, useRef, type ReactElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import type { IntentReporter, MountedSurface, RendererAdapter, View } from "@effect-native/core"
import {
  makeDomRenderer,
  type DomMountedSurface,
  type DomRendererOptions
} from "./index.js"

export const packageName = "@effect-native/render-dom/react" as const

export interface EffectNativeReactDomSurfaceProps extends DomRendererOptions {
  readonly viewStream: Stream.Stream<View>
  readonly report: IntentReporter
}

export interface ReactDomMountedSurface extends MountedSurface {
  readonly reactRoot: Root
  readonly domSurface: DomMountedSurface
}

interface InternalSurfaceProps extends EffectNativeReactDomSurfaceProps {
  readonly onReady?: (surface: DomMountedSurface) => void
  readonly onError?: (error: unknown) => void
}

/**
 * A React component that owns an Effect Native DOM surface.
 *
 * The bridge is deliberately explicit: React owns mounting and cleanup while
 * `makeDomRenderer` performs the catalog lowering. This gives React/Electron
 * applications a stable integration point without duplicating renderer state
 * or weakening the typed Effect Native view and intent contracts.
 */
const ReactDomSurfaceHost = (
  props: InternalSurfaceProps
): ReactElement => {
  const hostRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const host = hostRef.current
    if (host === null) return

    const scope = Effect.runSync(Scope.make())
    const { viewStream, report, onReady, onError, ...rendererOptions } = props
    const renderer = makeDomRenderer(rendererOptions)
    const mountFiber = Effect.runFork(Scope.provide(scope)(
      renderer.mount(host, viewStream, report)
    ))
    void Effect.runPromise(Fiber.join(mountFiber)).then(
      (surface) => onReady?.(surface),
      (error) => onError?.(error)
    )

    return () => {
      void Effect.runPromise(Scope.close(scope, Exit.void))
      void Effect.runPromise(Fiber.interrupt(mountFiber))
    }
  }, [
    props.clipboard,
    props.document,
    props.hostDrivers,
    props.onError,
    props.onReady,
    props.reducedMotion,
    props.report,
    props.theme,
    props.viewStream,
    props.viewport
  ])

  return createElement("div", {
    ref: hostRef,
    "data-en-react-surface": "hybrid"
  })
}

export const EffectNativeReactDomSurface = (
  props: EffectNativeReactDomSurfaceProps
): ReactElement => createElement(ReactDomSurfaceHost, props)

/**
 * RendererAdapter for applications that want React to own the DOM root.
 *
 * The returned surface is scope-bound just like `makeDomRenderer`: closing
 * the mounting scope unmounts the React root and the nested DOM renderer.
 */
export const makeReactDomRenderer = (
  options: DomRendererOptions = {}
): RendererAdapter<Element, ReactDomMountedSurface> => ({
  mount: (container, viewStream, report) =>
    Effect.gen(function*() {
      const ready = yield* Deferred.make<DomMountedSurface, unknown>()
      const reactRoot = yield* Effect.sync(() => createRoot(container))
      let unmounted = false
      const unmountRoot = (): void => {
        if (unmounted) return
        unmounted = true
        reactRoot.unmount()
      }
      yield* Effect.addFinalizer(() =>
        Effect.sync(unmountRoot)
      )
      yield* Effect.sync(() => {
        reactRoot.render(createElement(ReactDomSurfaceHost, {
          ...options,
          viewStream,
          report,
          onReady: (surface) => {
            Effect.runFork(Deferred.succeed(ready, surface))
          },
          onError: (error) => {
            Effect.runFork(Deferred.fail(ready, error))
          }
        }))
      })
      // Match makeDomRenderer's readiness contract: callers do not proceed
      // until the first View has committed beneath the React-owned root.
      const domSurface = yield* Deferred.await(ready).pipe(Effect.orDie)

      const surface: ReactDomMountedSurface = {
        reactRoot,
        domSurface,
        unmount: Effect.sync(unmountRoot)
      }
      return surface
    })
})

export type { DomMountedSurface }

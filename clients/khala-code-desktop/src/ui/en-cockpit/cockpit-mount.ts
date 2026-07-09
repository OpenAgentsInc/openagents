import {
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type IntentHandlers,
  type IntentReporter,
} from "@effect-native/core"
import { Effect, Scope, SubscriptionRef } from "@effect-native/core/effect"
import { makeDomRenderer } from "@effect-native/render-dom"
import type { KhalaFleetIntent } from "@openagentsinc/khala-fleet-intents"

import type { KhalaCodeDesktopFleetStatus } from "../../shared/rpc"
import {
  approvalToFleetIntent,
  cockpitIntents,
  runControlToFleetIntent,
  workerSelectToFleetIntent,
  type EnCockpitIntentContext,
} from "./cockpit-intents"
import {
  buildEnCockpitProjection,
  type EnCockpitState,
} from "./cockpit-projection"
import { enCockpitView } from "./cockpit-view"
import { khalaCockpitTheme } from "./theme"

// ---------------------------------------------------------------------------
// EN cockpit mount (MH-7 / EN-5)
//
// Mounts the typed cockpit tree through the REAL Effect Native DOM renderer,
// exactly like EN-1's `/stage1` route. The desktop shell is an Electrobun
// webview — i.e. a live DOM host — so the DOM renderer is the natural fit.
//
// Mount decision: DOM renderer, NOT `@effect-native/platform-desktop`'s
// `runMainDesktop` / `DesktopBridge`. Two honest reasons:
//   1. `platform-desktop` is not part of the effect-native snapshot EN-1
//      vendored into this monorepo (only core / tokens / render-dom / render-rn
//      are), so using it would mean re-vendoring a newer snapshot.
//   2. `DesktopBridge` abstracts native menu / window / deep-link / single-
//      instance concerns over its own request/event schema; Electrobun already
//      owns those via `Electroview`, and this cockpit needs none of them — it
//      needs a DOM container and a typed intent sink. Forcing the cockpit
//      through `DesktopBridge` would be a bad fit, so we mount the DOM renderer
//      directly (the same choice `/stage1` made) and note it here.
// ---------------------------------------------------------------------------

export type EnCockpitFleetIntentSink = (
  intent: KhalaFleetIntent,
) => void | Promise<void>

export type MountEnCockpitOptions = Readonly<{
  initialStatus: KhalaCodeDesktopFleetStatus
  // Called with the fully-typed, decoded shared fleet intent for every control
  // the operator dispatches. This is where a real host wires the intent into
  // Khala Sync / the fleet supervisor RPC; the proof just needs the value.
  onFleetIntent?: EnCockpitFleetIntentSink
  intentContext?: EnCockpitIntentContext
}>

export type MountedEnCockpit = Readonly<{
  state: SubscriptionRef.SubscriptionRef<EnCockpitState>
  unmount: Effect.Effect<void>
  // Every KhalaFleetIntent produced by a dispatched control, in order. The
  // mount records these regardless of whether `onFleetIntent` was supplied.
  dispatchedIntents: ReadonlyArray<KhalaFleetIntent>
  // Push a fresh live fleet status into the mounted surface (re-projects and
  // re-renders through the same EN view stream).
  applyStatus: (status: KhalaCodeDesktopFleetStatus) => Effect.Effect<void>
}>

export const mountEnCockpitSurface = (
  container: HTMLElement,
  options: MountEnCockpitOptions,
): Effect.Effect<MountedEnCockpit, never, Scope.Scope> =>
  Effect.gen(function* () {
    const dispatchedIntents: KhalaFleetIntent[] = []

    const emit = (intent: KhalaFleetIntent) =>
      Effect.promise(async () => {
        dispatchedIntents.push(intent)
        await options.onFleetIntent?.(intent)
      })

    const state = yield* SubscriptionRef.make(
      buildEnCockpitProjection(options.initialStatus),
    )
    const program = makeViewProgramFromState(state, enCockpitView)

    const ctx = options.intentContext ?? {}
    const handlers: IntentHandlers<typeof cockpitIntents> = {
      CockpitRunControl: (payload) => emit(runControlToFleetIntent(payload, ctx)),
      CockpitApprovalDecision: (payload) =>
        emit(approvalToFleetIntent(payload, ctx)),
      CockpitWorkerSelect: (payload) =>
        emit(workerSelectToFleetIntent(payload, ctx)),
    }

    const registry = yield* makeIntentRegistry(cockpitIntents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))

    const surface = yield* makeDomRenderer({ theme: khalaCockpitTheme }).mount(
      container,
      program.viewStream,
      report,
    )

    return {
      state,
      unmount: surface.unmount,
      dispatchedIntents,
      applyStatus: (status: KhalaCodeDesktopFleetStatus) =>
        SubscriptionRef.set(state, buildEnCockpitProjection(status)),
    }
  })

// Import effect through the @effect-native/core/effect bridge so this module
// uses the SAME effect instance the vendored @effect-native/* packages pin
// (effect beta.94), not the repo catalog effect (beta.70). Mixing the two
// effect copies makes SubscriptionRef/Effect/Schema types fail to unify across
// the adapter boundary. (Same bridge the web Effect Native surfaces use.)
import { Effect, Schema, type Stream, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  Card,
  defineIntent,
  type IntentHandlers,
  IntentRef,
  type IntentReporter,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"

/**
 * OpenAgents mobile (#8597) — the PURE half of the Home screen. This module
 * imports only `@effect-native/core` (+ its effect bridge), never `react` or
 * `react-native`, so the same typed view program is host-agnostic: it renders
 * on mobile through `@effect-native/render-rn` and would render unchanged
 * through the DOM/desktop adapters. One catalog, many hosts.
 *
 * The `.tsx` screen mounts this program inside the Expo shell via
 * `EffectNativeHost` (`@effect-native/render-rn`).
 */

export interface HomeState {
  readonly pings: number
  readonly glassTaps: number
}

export const initialHomeState: HomeState = { pings: 0, glassTaps: 0 }

/** Visible JS-bundle tag, rendered on the Home card. Bump this string when
 * publishing an OTA so the owner can SEE the over-the-air bundle swap land
 * (embedded build 106 ships the tag below; a published OTA with a bumped tag
 * should appear within ~3s via the temporary poll loop and reload). */
export const BUNDLE_TAG = "2026-07-09.embedded-106"

/** One typed intent, proving the intent -> handler -> state -> re-render loop
 * runs end-to-end through the RN adapter (not just static rendering). */
export const HomePinged = defineIntent(
  "HomePinged",
  Schema.Struct({ amount: Schema.Number }),
)

/** Typed intent dispatched when the SwiftUI Liquid Glass island's button is
 * tapped (SwiftUI event -> shell -> this intent -> state -> re-render of BOTH
 * the Effect Native tree and the island's props). This is the intent half of
 * the SwiftUI renderer seam per
 * docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md. */
export const GlassPinged = defineIntent(
  "GlassPinged",
  Schema.Struct({ amount: Schema.Number }),
)

/** The IntentRef the shell dispatches for a SwiftUI glass tap — the typed
 * contract between the native island and this program. */
export const glassPingedRef = IntentRef("GlassPinged", StaticPayload({ amount: 1 }))

export const homeIntentDefinitions = [HomePinged, GlassPinged] as const

/** Serializable props for the SwiftUI island, derived from program state —
 * the props half of the seam. The catalog has NO SwiftUI host kind yet
 * (closed `hostKinds` registry; demand register D-MB-02), so the island
 * mounts at the shell boundary per the audit's interop case 2 and this
 * projection is the typed data contract it renders. */
export interface GlassIslandProps {
  readonly title: string
  readonly subtitle: string
  readonly buttonLabel: string
  readonly tapCount: number
}

export const glassIslandProps = (state: HomeState): GlassIslandProps => ({
  title: "Liquid Glass",
  subtitle: "SwiftUI island driven by the Effect Native program",
  buttonLabel: "Dispatch typed intent from SwiftUI",
  tapCount: state.glassTaps,
})

/** The OpenAgents shell authored as a typed Effect Native view tree —
 * Stack/Text/Card/Spacer/Button from the shared component catalog, styled with
 * typed color/spacing tokens the renderer resolves against the Protoss-blue
 * `khalaTheme` (no class strings, no parallel palette). */
export const renderHomeView = (state: HomeState): View =>
  Stack(
    {
      key: "home-root",
      direction: "column",
      gap: "4",
      padding: "5",
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    [
      Text({
        key: "home-title",
        content: "OpenAgents",
        variant: "heading",
        color: "textPrimary",
      }),
      Text({
        key: "home-subtitle",
        content:
          "Greenfield OpenAgents mobile. This screen is a typed Effect Native view program from the shared component catalog, rendered by @effect-native/render-rn.",
        variant: "body",
        color: "textMuted",
      }),
      Card(
        {
          key: "home-card",
          padding: "4",
          radius: "lg",
          style: {
            width: "full",
            backgroundColor: "surface",
            borderColor: "border",
            borderWidth: 1,
          },
        },
        [
          Text({
            key: "home-app-label",
            content: "Application",
            variant: "label",
            color: "accent",
          }),
          Text({
            key: "home-app-value",
            content: "com.openagents.app",
            variant: "body",
            color: "textPrimary",
          }),
          Spacer({ key: "home-card-space-1", size: "2" }),
          Text({
            key: "home-renderer-label",
            content: "Renderer",
            variant: "label",
            color: "accent",
          }),
          Text({
            key: "home-renderer-value",
            content: "@effect-native/render-rn (React Native host)",
            variant: "body",
            color: "textPrimary",
          }),
          Spacer({ key: "home-card-space-2", size: "2" }),
          Text({
            key: "home-bundle-label",
            content: "Bundle",
            variant: "label",
            color: "accent",
          }),
          Text({
            key: "home-bundle-value",
            content: BUNDLE_TAG,
            variant: "body",
            color: "textPrimary",
          }),
          Spacer({ key: "home-card-space-3", size: "2" }),
          Text({
            key: "home-pings-label",
            content: "Typed intents dispatched",
            variant: "label",
            color: "accent",
          }),
          Text({
            key: "home-pings-value",
            content: String(state.pings),
            variant: "title",
            color: "textPrimary",
          }),
        ],
      ),
      Button({
        key: "home-ping",
        label: "Dispatch a typed intent",
        variant: "primary",
        onPress: IntentRef("HomePinged", StaticPayload({ amount: 1 })),
      }),
      Spacer({ key: "home-glass-space", size: "2" }),
      Text({
        key: "home-glass-section-label",
        content: "SwiftUI via Effect Native — test",
        variant: "label",
        color: "accent",
      }),
      Text({
        key: "home-glass-taps",
        content: `Glass intents received: ${state.glassTaps}`,
        variant: "body",
        color: "textMuted",
      }),
    ],
  )

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
): IntentHandlers<typeof homeIntentDefinitions> => ({
  HomePinged: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      pings: current.pings + payload.amount,
    })),
  GlassPinged: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      ...current,
      glassTaps: current.glassTaps + payload.amount,
    })),
})

export interface HomeProgramHandle {
  readonly viewStream: Stream.Stream<View>
  readonly report: IntentReporter
  /** State changes stream — the shell subscribes to derive the SwiftUI
   * island's serializable props from the same single source of truth the
   * Effect Native tree renders from. */
  readonly stateChanges: Stream.Stream<HomeState>
  /** The SwiftUI island's tap event handler: dispatches the typed
   * `GlassPinged` intent through the SAME registry the renderer's reporter
   * uses. Fire-and-forget with soft failure (an intent error must never crash
   * the native event path). */
  readonly dispatchGlassTap: () => void
}

/** Builds the runnable program: a `SubscriptionRef` of state, an intent
 * registry bound to the handlers, and a closure-captured `IntentReporter`
 * (R = never, so the RN surface runs it with no additional context). Runs
 * synchronously — `makeIntentRegistry`/`SubscriptionRef.make` need no Scope. */
export const buildHomeProgram = (): HomeProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<HomeState>(initialHomeState)
      const registry = yield* makeIntentRegistry(
        homeIntentDefinitions,
        makeHomeHandlers(state),
      )
      const report: IntentReporter = (ref, runtimeValue) =>
        registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const dispatchGlassTap = (): void => {
        Effect.runFork(Effect.exit(registry.dispatch(resolveIntentRef(glassPingedRef))))
      }
      const program = makeViewProgramFromState(state, renderHomeView)
      return {
        viewStream: program.viewStream,
        report,
        stateChanges: SubscriptionRef.changes(state),
        dispatchGlassTap,
      }
    }),
  )

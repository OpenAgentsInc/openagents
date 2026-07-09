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
}

export const initialHomeState: HomeState = { pings: 0 }

/** One typed intent, proving the intent -> handler -> state -> re-render loop
 * runs end-to-end through the RN adapter (not just static rendering). */
export const HomePinged = defineIntent(
  "HomePinged",
  Schema.Struct({ amount: Schema.Number }),
)

export const homeIntentDefinitions = [HomePinged] as const

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
    ],
  )

export const makeHomeHandlers = (
  state: SubscriptionRef.SubscriptionRef<HomeState>,
): IntentHandlers<typeof homeIntentDefinitions> => ({
  HomePinged: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      pings: current.pings + payload.amount,
    })),
})

export interface HomeProgramHandle {
  readonly viewStream: Stream.Stream<View>
  readonly report: IntentReporter
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
      const program = makeViewProgramFromState(state, renderHomeView)
      return { viewStream: program.viewStream, report }
    }),
  )

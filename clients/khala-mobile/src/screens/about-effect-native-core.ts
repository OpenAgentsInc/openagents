// Import effect through the @effect-native/core/effect bridge so this module
// uses the SAME effect instance the vendored @effect-native/* packages pin
// (effect beta.94), not the app's catalog effect (beta.70). Mixing the two
// effect copies makes SubscriptionRef/Effect/Schema types fail to unify across
// the adapter boundary. (Same bridge EN-1's web /stage1 surface uses.)
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
 * EN-3 (#8568) — the PURE half of the first Khala-mobile screen authored with
 * the Effect Native component set (renderer adapter #1). It imports only
 * `@effect-native/core` + `effect`, never `react`/`react-native`, so it renders
 * deterministically through the RN renderer in tests with no native host. The
 * `.tsx` screen mounts this program inside the Expo/React-Navigation shell via
 * `EffectNativeHost` (`@effect-native/render-rn`).
 */

export interface AboutState {
  readonly pings: number
}

export const initialAboutState: AboutState = { pings: 0 }

/** One typed intent, proving the intent -> handler -> state -> re-render loop
 * runs through the RN adapter (not just static rendering). */
export const AboutPinged = defineIntent(
  "AboutPinged",
  Schema.Struct({ amount: Schema.Number }),
)

export const aboutIntentDefinitions = [AboutPinged] as const

/** The screen authored as a typed Effect Native view tree — Stack/Text/Card/
 * Spacer/Button from the shared component catalog, styled with Protoss-blue
 * theme tokens (resolved by the renderer against `khalaEffectNativeTheme`). */
export const renderAboutView = (state: AboutState): View =>
  Stack(
    {
      key: "about-root",
      direction: "column",
      gap: "4",
      padding: "5",
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    [
      Text({
        key: "about-title",
        content: "Effect Native",
        variant: "heading",
        color: "textPrimary",
      }),
      Text({
        key: "about-subtitle",
        content:
          "This screen is authored with the Effect Native component set and rendered by @effect-native/render-rn — renderer adapter #1 for Khala mobile. Zero new native work: it maps to real React Native host components.",
        variant: "body",
        color: "textMuted",
      }),
      Card(
        {
          key: "about-card",
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
            key: "about-renderer-label",
            content: "Renderer",
            variant: "label",
            color: "accent",
          }),
          Text({
            key: "about-renderer-value",
            content: "@effect-native/render-rn (React Native host)",
            variant: "body",
            color: "textPrimary",
          }),
          Spacer({ key: "about-card-space", size: "2" }),
          Text({
            key: "about-pings-label",
            content: "Typed intents dispatched",
            variant: "label",
            color: "accent",
          }),
          Text({
            key: "about-pings-value",
            content: String(state.pings),
            variant: "title",
            color: "textPrimary",
          }),
        ],
      ),
      Button({
        key: "about-ping",
        label: "Dispatch a typed intent",
        variant: "primary",
        onPress: IntentRef("AboutPinged", StaticPayload({ amount: 1 })),
      }),
    ],
  )

export const makeAboutHandlers = (
  state: SubscriptionRef.SubscriptionRef<AboutState>,
): IntentHandlers<typeof aboutIntentDefinitions> => ({
  AboutPinged: (payload) =>
    SubscriptionRef.update(state, (current) => ({
      pings: current.pings + payload.amount,
    })),
})

export interface AboutProgramHandle {
  readonly viewStream: Stream.Stream<View>
  readonly report: IntentReporter
}

/** Builds the runnable program: a `SubscriptionRef` of state, an intent
 * registry bound to the handlers, and a closure-captured `IntentReporter`
 * (R = never, so the RN surface runs it with no additional context). Runs
 * synchronously — `makeIntentRegistry`/`SubscriptionRef.make` need no Scope. */
export const buildAboutProgram = (): AboutProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<AboutState>(initialAboutState)
      const registry = yield* makeIntentRegistry(
        aboutIntentDefinitions,
        makeAboutHandlers(state),
      )
      const report: IntentReporter = (ref, runtimeValue) =>
        registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const program = makeViewProgramFromState(state, renderAboutView)
      return { viewStream: program.viewStream, report }
    }),
  )

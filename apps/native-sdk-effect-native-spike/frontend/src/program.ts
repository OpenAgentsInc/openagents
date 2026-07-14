import {
  Badge,
  Button,
  Card,
  defineIntent,
  Divider,
  IntentRef,
  makeIntentRegistry,
  makeViewProgramFromState,
  Stack,
  Text,
  type View,
} from "@effect-native/core";
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect";

import { adoptionCounts, nativeSdkComponentAdoption } from "./native-sdk-component-adoption.ts";

export interface SpikeState {
  readonly effectCount: number;
  readonly lastAction: string;
}

export const IncrementEffectCount = defineIntent("IncrementEffectCount", Schema.Null);
export const ResetEffectCount = defineIntent("ResetEffectCount", Schema.Null);
export const spikeIntents = [IncrementEffectCount, ResetEffectCount] as const;

export const initialSpikeState = (): SpikeState => ({
  effectCount: 0,
  lastAction: "Effect runtime mounted",
});

const mappingRows = nativeSdkComponentAdoption.slice(0, 9).map((entry) =>
  Stack({ key: `mapping-${entry.effectNative}`, direction: "row", gap: "2", align: "center" }, [
    Badge({
      key: `mapping-lane-${entry.effectNative}`,
      label: entry.lane,
      tone: entry.lane === "direct" ? "success" : "info",
      variant: "soft",
      size: "sm",
    }),
    Text({
      key: `mapping-name-${entry.effectNative}`,
      content: `${entry.effectNative} → ${entry.nativeSdk}`,
      variant: "caption",
      color: "textPrimary",
      style: { flex: 1 },
    }),
  ]),
);

export const spikeView = (state: SpikeState): View =>
  Stack({ key: "effect-native-spike", direction: "column", gap: "6", padding: "6" }, [
    Stack({ key: "heading", direction: "column", gap: "1" }, [
      Badge({
        key: "renderer-badge",
        label: "REAL EFFECT NATIVE VIEWPROGRAM",
        tone: "success",
        variant: "soft",
        size: "sm",
      }),
      Text({
        key: "title",
        content: "Effect Native renderer surface",
        variant: "heading",
        color: "textPrimary",
        weight: "bold",
      }),
      Text({
        key: "subtitle",
        content:
          "This pane is the shared typed catalog running through @effect-native/render-dom inside a Native SDK child WebView.",
        variant: "body",
        color: "textMuted",
      }),
    ]),
    Card({ key: "effect-loop", padding: "6", radius: "lg" }, [
      Stack({ key: "effect-loop-content", direction: "column", gap: "4" }, [
        Text({
          key: "effect-count-label",
          content: "Typed intent loop",
          variant: "label",
          color: "textMuted",
          weight: "semibold",
        }),
        Text({
          key: "effect-count",
          content: String(state.effectCount),
          variant: "heading",
          color: "textPrimary",
          weight: "bold",
        }),
        Text({
          key: "effect-last-action",
          content: state.lastAction,
          variant: "caption",
          color: "textMuted",
        }),
        Stack({ key: "effect-actions", direction: "row", gap: "2" }, [
          Button({
            key: "effect-increment",
            label: "Increment in Effect",
            tone: "accent",
            variant: "solid",
            size: "md",
            onPress: IntentRef("IncrementEffectCount"),
          }),
          Button({
            key: "effect-reset",
            label: "Reset",
            tone: "secondary",
            variant: "outline",
            size: "md",
            onPress: IntentRef("ResetEffectCount"),
          }),
        ]),
      ]),
    ]),
    Divider({ key: "adoption-divider" }),
    Stack({ key: "adoption", direction: "column", gap: "2" }, [
      Text({
        key: "adoption-title",
        content: "Candidate Native SDK lowerings",
        variant: "label",
        color: "textPrimary",
        weight: "semibold",
      }),
      Text({
        key: "adoption-summary",
        content: `${adoptionCounts.direct} direct · ${adoptionCounts.composite} composite · ${adoptionCounts["host-only"]} host-only`,
        variant: "caption",
        color: "textMuted",
      }),
      ...mappingRows,
    ]),
  ]);

export const makeSpikeRuntime = Effect.gen(function* () {
  const state = yield* SubscriptionRef.make(initialSpikeState());
  const registry = yield* makeIntentRegistry(spikeIntents, {
    IncrementEffectCount: () =>
      SubscriptionRef.update(state, (current) => ({
        effectCount: current.effectCount + 1,
        lastAction: "IncrementEffectCount decoded and handled",
      })),
    ResetEffectCount: () =>
      SubscriptionRef.set(state, {
        effectCount: 0,
        lastAction: "ResetEffectCount decoded and handled",
      }),
  });
  return {
    state,
    registry,
    program: makeViewProgramFromState(state, spikeView),
  };
});

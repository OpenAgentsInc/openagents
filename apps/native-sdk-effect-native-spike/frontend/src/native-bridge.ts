import { Schema } from "@effect-native/core/effect";

import type { SpikeState } from "./program.ts";
import { fixtureSessions } from "./program.ts";
import { assertNativeProductionCommandBindings } from "./production-command-parity.ts";

export const bridgePayloadLimit = 8 * 1024;

const NativeIntentSchema = Schema.Struct({
  protocol: Schema.Literal(1),
  sequence: Schema.Number,
  intent: Schema.Union([
    Schema.Struct({ _tag: Schema.Literal("NewChatRequested"), commandId: Schema.Literal("chat.new") }),
    Schema.Struct({ _tag: Schema.Literal("WorkspaceSelected"), workspace: Schema.Literal("chat"), commandId: Schema.Literal("chat.open") }),
    Schema.Struct({ _tag: Schema.Literal("WorkspaceSelected"), workspace: Schema.Literal("home"), commandId: Schema.Literal("workspace.home") }),
    Schema.Struct({ _tag: Schema.Literal("WorkspaceSelected"), workspace: Schema.Literal("settings"), commandId: Schema.Literal("settings.open") }),
    Schema.Struct({ _tag: Schema.Literal("SessionSelected"), sessionRef: Schema.String, commandId: Schema.Null }),
  ]),
});

export type NativeIntentEnvelope = typeof NativeIntentSchema.Type;
const decodeNativeIntentSchema = Schema.decodeUnknownSync(NativeIntentSchema);
export const decodeNativeIntent = (candidate: unknown): NativeIntentEnvelope => {
  assertNativeProductionCommandBindings();
  return decodeNativeIntentSchema(candidate, { onExcessProperty: "error" });
};

export interface NativeProjection {
  readonly protocol: 1;
  readonly revision: number;
  readonly workspace: SpikeState["workspace"];
  readonly selectedSessionRef: string | null;
  readonly messageCount: number;
  readonly pending: boolean;
  readonly status: string;
}

export const projectNativeState = (state: SpikeState): NativeProjection => ({
  protocol: 1,
  revision: state.revision,
  workspace: state.workspace,
  selectedSessionRef: state.selectedSessionRef,
  messageCount: state.messages.length,
  pending: state.pending,
  status: state.pending ? "Codex is working" : "Effect state synchronized",
});

type ZeroBridge = {
  readonly invoke: (command: string, payload: unknown) => Promise<unknown>;
};

const zeroBridge = (): ZeroBridge | undefined =>
  (globalThis as typeof globalThis & { zero?: ZeroBridge }).zero;

export const publishNativeProjection = async (
  state: SpikeState,
  acknowledgedNativeSequence = 0,
): Promise<NativeIntentEnvelope | null> => {
  const bridge = zeroBridge();
  if (bridge === undefined) return null;
  const request = { ...projectNativeState(state), acknowledgedNativeSequence };
  if (new TextEncoder().encode(JSON.stringify(request)).length > bridgePayloadLimit) return null;
  const response = await bridge.invoke("openagents.spike.projection.v1", request);
  if (response === null || typeof response !== "object" || !("intent" in response)) return null;
  const intent = (response as Readonly<{ intent?: unknown }>).intent;
  if (intent === null || intent === undefined) return null;
  try {
    return decodeNativeIntent(intent);
  } catch {
    return null;
  }
};

export const startNativeBridgeSync = (
  readState: () => Promise<SpikeState>,
  handler: (intent: NativeIntentEnvelope) => void,
): (() => void) => {
  let acknowledgedSequence = 0;
  let inFlight = false;
  const tick = (): void => {
    if (inFlight) return;
    inFlight = true;
    void readState()
      .then((state) => publishNativeProjection(state, acknowledgedSequence))
      .then((envelope) => {
        if (envelope === null || !Number.isSafeInteger(envelope.sequence) || envelope.sequence <= acknowledgedSequence) return;
        if (envelope.intent._tag === "SessionSelected") {
          const sessionRef = (envelope.intent as Readonly<{ sessionRef: string }>).sessionRef;
          if (!fixtureSessions.some((session) => session.ref === sessionRef)) return;
        }
        acknowledgedSequence = envelope.sequence;
        handler(envelope);
      })
      .catch(() => undefined)
      .finally(() => { inFlight = false; });
  };
  tick();
  const timer = globalThis.setInterval(tick, 120);
  return () => globalThis.clearInterval(timer);
};

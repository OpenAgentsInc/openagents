import {
  AcpRuntimeProjector,
  bindAcpSession,
  createBoundedAcpNativeEvidenceStore,
  createAcpRuntimeNativeEnvelope,
  type AcpProjectionEvent,
  type AcpNativeEvidenceStore,
} from "@openagentsinc/agent-client-runtime-bridge";
import { decodeStableAcpMethodPayload } from "@openagentsinc/agent-client-protocol/stable";
import type { NeutralChatTurnEvent } from "./types.ts";

export type AcpSessionUpdate = Record<string, unknown> & {
  readonly sessionUpdate?: string;
  readonly content?: { readonly type?: string; readonly text?: string };
};

/**
 * Temporary Grok presentation adapter. Native admission, canonical identity,
 * ordering, merge, and tool state are owned by the shared ACP runtime bridge.
 */
export function createGrokAcpEventProjector(input: {
  readonly threadId: string;
  readonly turnId: string;
  readonly grokSessionId?: string;
  readonly messageId?: string;
  readonly connectionRef?: string;
  readonly processGeneration?: number;
  readonly nativeEvidenceStore?: AcpNativeEvidenceStore;
  readonly onCanonicalEvent?: (event: AcpProjectionEvent) => void;
}): {
  readonly onUpdate: (
    update: AcpSessionUpdate,
    nativeSessionId?: string,
    nativeMeta?: Readonly<Record<string, unknown>>,
  ) => Promise<readonly NeutralChatTurnEvent[]>;
  readonly finish: (promptResult?: unknown) => Promise<readonly NeutralChatTurnEvent[]>;
  readonly text: () => string;
} {
  const connectionRef = input.connectionRef ?? `grok.connection.${input.threadId}`;
  const processGeneration = input.processGeneration ?? 1;
  const grokSessionId = input.grokSessionId ?? input.threadId;
  const binding = bindAcpSession({
    profile: "grok",
    processGeneration,
    connectionRef,
    peerSessionId: grokSessionId,
    canonicalThreadSeed: input.threadId,
  });
  const native =
    input.nativeEvidenceStore ??
    createBoundedAcpNativeEvidenceStore({ maxEntries: 1_024, maxBytes: 8 * 1_048_576 });
  const projector = new AcpRuntimeProjector({
    binding,
    turnSeed: input.turnId,
    store: native,
  });
  const messageId = input.messageId ?? `msg_${input.turnId}`;
  let receiveSequence = 0;
  let started = false;
  let full = "";

  const present = (events: readonly AcpProjectionEvent[]): readonly NeutralChatTurnEvent[] => {
    const output: NeutralChatTurnEvent[] = [];
    for (const event of events) {
      if (event.kind === "text.delta") {
        if (!started) {
          started = true;
          output.push({
            type: "message_start",
            turnId: input.turnId,
            message: { id: messageId, role: "assistant", content: "" },
          });
        }
        full += event.text;
        output.push({ type: "message_delta", turnId: input.turnId, messageId, delta: event.text });
      } else if (
        event.kind === "tool.call" ||
        event.kind === "tool.result" ||
        event.kind === "tool.error"
      ) {
        output.push({
          type: "tool_event",
          turnId: input.turnId,
          event: {
            kind: event.kind,
            name: event.toolName,
            ...(event.kind === "tool.error" ? { detail: event.messageSafe } : {}),
          },
        });
      }
    }
    return output;
  };

  return {
    text: () => full,
    async onUpdate(update, nativeSessionId = grokSessionId, nativeMeta) {
      const sequence = receiveSequence++;
      const discriminant =
        typeof update.sessionUpdate === "string" ? update.sessionUpdate : "unknown_session_update";
      const decoded = decodeStableAcpMethodPayload({
        direction: "agent-to-client",
        method: "session/update",
        phase: "params",
        payload: { sessionId: nativeSessionId, update },
      });
      const validated =
        decoded._tag === "Decoded" ? (decoded.value as { update: AcpSessionUpdate }) : { update };
      const admitted = createAcpRuntimeNativeEnvelope({
        profile: "grok",
        protocolVersion: 1,
        connectionRef,
        processGeneration,
        method: "session/update",
        updateId: String(sequence),
        sessionId: nativeSessionId,
        observedAt: new Date().toISOString(),
        discriminant,
        validatedPayload: validated.update,
        ...(nativeMeta === undefined ? {} : { nativeMeta }),
        validationStatus: decoded._tag === "Decoded" ? "validated" : "decode-failure",
      });
      if ("kind" in admitted) return [];
      const result = await projector.apply({ envelope: admitted, sequence });
      for (const event of result.events) input.onCanonicalEvent?.(event);
      return present(result.events);
    },
    async finish(promptResult = { stopReason: "end_turn" }) {
      const raw =
        promptResult !== null && typeof promptResult === "object"
          ? (promptResult as Record<string, unknown>)
          : { stopReason: promptResult };
      const stopReason = typeof raw.stopReason === "string" ? raw.stopReason : "unknown";
      const decoded = decodeStableAcpMethodPayload({
        direction: "client-to-agent",
        method: "session/prompt",
        phase: "result",
        payload: raw,
      });
      const admitted = createAcpRuntimeNativeEnvelope({
        profile: "grok",
        protocolVersion: 1,
        connectionRef,
        processGeneration,
        method: "session/prompt",
        requestId: input.turnId,
        updateId: `prompt-result-${input.turnId}`,
        sessionId: grokSessionId,
        observedAt: new Date().toISOString(),
        discriminant: `prompt_response/${stopReason}`,
        validatedPayload: decoded._tag === "Decoded" ? decoded.value : raw,
        validationStatus: decoded._tag === "Decoded" ? "validated" : "decode-failure",
      });
      if ("kind" in admitted) return [];
      const canonical = await projector.settle(admitted, stopReason);
      for (const event of canonical) input.onCanonicalEvent?.(event);
      const events = [...present(canonical)];
      if (!started)
        events.unshift({
          type: "message_start",
          turnId: input.turnId,
          message: { id: messageId, role: "assistant", content: "" },
        });
      events.push({ type: "message_done", turnId: input.turnId, messageId });
      return events;
    },
  };
}

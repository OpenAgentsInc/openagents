import { createHash } from "node:crypto";
import type { KhalaRuntimeEvent } from "@openagentsinc/agent-runtime-schema";
import type { AcpNativeEvidenceStore, AcpRuntimeNativeEnvelope } from "./native-envelope.ts";

type RecordValue = Record<string, unknown>;
const object = (value: unknown): RecordValue =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as RecordValue)
    : {};
const textOf = (value: unknown): string | undefined => {
  const content = object(value);
  return content.type === "text" && typeof content.text === "string" ? content.text : undefined;
};
const ref = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`;

export type AcpCanonicalStateEvent = Readonly<{
  kind:
    | "user-message"
    | "plan-snapshot"
    | "available-commands"
    | "mode-snapshot"
    | "config-snapshot"
    | "session-info"
    | "usage-snapshot"
    | "degraded";
  threadId: string;
  turnId: string;
  stateRef: string;
  safeSummary: string;
  nativeRef: string;
  snapshot: Readonly<Record<string, unknown>>;
}>;

export type AcpProjectionEvent = KhalaRuntimeEvent | AcpCanonicalStateEvent;
export type AcpProjectionDisposition = Readonly<{
  outcome: "applied" | "duplicate" | "quarantined";
  reason?:
    | "old-generation"
    | "out-of-order"
    | "tool-state-regression"
    | "late-after-turn"
    | "invalid-envelope";
  events: readonly AcpProjectionEvent[];
}>;

export type AcpSessionBinding = Readonly<{
  profile: string;
  processGeneration: number;
  connectionRef: string;
  peerSessionId: string;
  threadId: string;
}>;

export const bindAcpSession = (
  input: Omit<AcpSessionBinding, "threadId"> & { readonly canonicalThreadSeed: string },
): AcpSessionBinding => ({
  ...input,
  threadId: ref(
    "thread",
    `${input.canonicalThreadSeed}:${input.profile}:${input.processGeneration}:${input.connectionRef}:${input.peerSessionId}`,
  ),
});

type ToolState = {
  rank: number;
  status?: string | null;
  name: string;
  started: boolean;
  kind?: string | null;
  contentRef?: string;
  locationsRef?: string;
  emittedTerminal: boolean;
};

export class AcpRuntimeProjector {
  readonly #seen = new Set<string>();
  readonly #lastSequence = new Map<string, number>();
  readonly #tools = new Map<string, ToolState>();
  readonly #completedChannels = new Set<string>();
  readonly #settlementIds = new Set<string>();
  readonly #store: AcpNativeEvidenceStore;
  readonly #binding: AcpSessionBinding;
  readonly #turnId: string;
  #eventSequence = 0;
  #settled = false;

  constructor(input: {
    readonly binding: AcpSessionBinding;
    readonly turnSeed: string;
    readonly store: AcpNativeEvidenceStore;
  }) {
    this.#binding = input.binding;
    this.#turnId = ref("turn", `${input.binding.threadId}:${input.turnSeed}`);
    this.#store = input.store;
  }

  get threadId(): string {
    return this.#binding.threadId;
  }
  get turnId(): string {
    return this.#turnId;
  }

  async begin(observedAt: string): Promise<KhalaRuntimeEvent> {
    return this.#event("turn.started", observedAt, {});
  }

  async apply(input: {
    readonly envelope: AcpRuntimeNativeEnvelope;
    readonly sequence: number;
  }): Promise<AcpProjectionDisposition> {
    const native = input.envelope;
    const identity = `${native.processGeneration}:${native.sessionId}:${native.updateId}`;
    if (this.#seen.has(identity)) return { outcome: "duplicate", events: [] };
    const stored = await this.#store.put(native);
    const quarantine = (
      reason: NonNullable<AcpProjectionDisposition["reason"]>,
      summary: string,
    ): AcpProjectionDisposition => {
      this.#seen.add(identity);
      return {
        outcome: "quarantined",
        reason,
        events: [this.#state("degraded", stored.rawEventRef, summary)],
      };
    };
    if (
      native.sessionId !== this.#binding.peerSessionId ||
      native.peer.profile !== this.#binding.profile ||
      native.peer.connectionRef !== this.#binding.connectionRef
    )
      return quarantine("invalid-envelope", "quarantined ACP envelope binding mismatch");
    if (native.processGeneration !== this.#binding.processGeneration)
      return quarantine("old-generation", "quarantined stale ACP process generation");
    if (this.#settled)
      return quarantine("late-after-turn", "quarantined ACP update after turn settlement");
    const orderKey = `${native.processGeneration}:${native.sessionId}`;
    const last = this.#lastSequence.get(orderKey) ?? -1;
    if (!Number.isSafeInteger(input.sequence) || input.sequence <= last)
      return quarantine("out-of-order", "quarantined out-of-order ACP update");
    const rawEventRef = stored.rawEventRef;
    const update = object(native.validatedPayload);
    const discriminator =
      typeof update.sessionUpdate === "string" ? update.sessionUpdate : native.discriminant;
    if (native.validationStatus === "decode-failure") {
      this.#seen.add(identity);
      this.#lastSequence.set(orderKey, input.sequence);
      return {
        outcome: "quarantined",
        reason: "invalid-envelope",
        events: [
          this.#event("raw.sidecar_ref", native.observedAt, {
            rawEventRef,
            rawEventKind:
              native.peer.profile === "grok"
                ? "grok_acp_event"
                : native.peer.profile === "cursor"
                  ? "cursor_acp_event"
                  : "agent_client_protocol_event",
          }),
          this.#state("degraded", rawEventRef, "ACP payload failed pinned schema validation"),
        ],
      };
    }
    if (discriminator === "tool_call" || discriminator === "tool_call_update") {
      const toolCallId =
        typeof update.toolCallId === "string"
          ? ref("tool", `${native.processGeneration}:${native.sessionId}:${update.toolCallId}`)
          : ref("tool", identity);
      const previous = this.#tools.get(toolCallId);
      if (discriminator === "tool_call" && previous?.started === true)
        return quarantine("tool-state-regression", "quarantined duplicate ACP tool start");
      const status = typeof update.status === "string" ? update.status : previous?.status;
      const rank =
        status === "pending"
          ? 0
          : status === "in_progress"
            ? 1
            : status === "completed" || status === "failed"
              ? 2
              : (previous?.rank ?? 0);
      if (previous && (rank < previous.rank || (previous.rank === 2 && status !== previous.status)))
        return quarantine("tool-state-regression", "quarantined ACP tool state regression");
    }
    this.#seen.add(identity);
    this.#lastSequence.set(orderKey, input.sequence);
    const sidecar = this.#event("raw.sidecar_ref", native.observedAt, {
      rawEventRef,
      rawEventKind:
        native.peer.profile === "grok"
          ? "grok_acp_event"
          : native.peer.profile === "cursor"
            ? "cursor_acp_event"
            : "agent_client_protocol_event",
    });
    const projected = this.#project(discriminator, update, native, rawEventRef);
    return { outcome: "applied", events: [sidecar, ...projected] };
  }

  complete(stopReason: string, observedAt: string): readonly KhalaRuntimeEvent[] {
    if (this.#settled) return [];
    this.#settled = true;
    const events: KhalaRuntimeEvent[] = [];
    for (const channel of ["text", "reasoning"] as const) {
      const key = `${channel}:${this.#turnId}`;
      if (!this.#completedChannels.has(key)) continue;
      events.push(
        this.#event(
          channel === "text" ? "text.completed" : "reasoning.completed",
          observedAt,
          channel === "text"
            ? { messageId: ref("message", key) }
            : { messageId: ref("message", key) },
        ),
      );
    }
    for (const [toolCallId, tool] of this.#tools) {
      if (!tool.started || tool.emittedTerminal) continue;
      tool.emittedTerminal = true;
      events.push(
        this.#event("tool.error", observedAt, {
          toolCallId,
          toolName: tool.name,
          errorRef: ref("tool_interrupted", `${toolCallId}:${stopReason}`),
          messageSafe:
            stopReason === "cancelled"
              ? "ACP tool interrupted by cancellation"
              : "ACP tool ended without a terminal provider update",
          authority: {
            authorityRef: ref("authority", toolCallId),
            policyRef: "policy.acp_bridge",
            decisionRef: "decision.provider_reported_not_authority",
            toolRef: ref("toolref", tool.name),
            status: "denied",
            allowed: false,
            blockerRefs: ["blocker.provider_event_not_authority"],
          },
          providerExecuted: true,
        }),
      );
    }
    const finishReason =
      stopReason === "end_turn"
        ? "stop"
        : stopReason === "max_tokens"
          ? "length"
          : stopReason === "cancelled"
            ? "cancelled"
            : stopReason === "refusal"
              ? "content-filter"
              : "unknown";
    events.push(
      this.#event("turn.finished", observedAt, {
        finishReason,
        providerMetadata: { metadataRefs: [ref("native_stop", stopReason)] },
      }),
    );
    return events;
  }

  async settle(
    envelope: AcpRuntimeNativeEnvelope,
    stopReason: string,
  ): Promise<readonly AcpProjectionEvent[]> {
    const settlementId = `${envelope.processGeneration}:${envelope.requestId ?? envelope.updateId}`;
    if (this.#settlementIds.has(settlementId)) return [];
    if (
      envelope.method !== "session/prompt" ||
      envelope.sessionId !== this.#binding.peerSessionId ||
      envelope.processGeneration !== this.#binding.processGeneration ||
      envelope.peer.connectionRef !== this.#binding.connectionRef
    ) {
      const stored = await this.#store.put(envelope);
      this.#settlementIds.add(settlementId);
      return [
        this.#state(
          "degraded",
          stored.rawEventRef,
          "quarantined ACP prompt settlement binding mismatch",
        ),
      ];
    }
    const stored = await this.#store.put(envelope);
    this.#settlementIds.add(settlementId);
    if (envelope.validationStatus === "decode-failure") {
      return [
        this.#event("raw.sidecar_ref", envelope.observedAt, {
          rawEventRef: stored.rawEventRef,
          rawEventKind:
            envelope.peer.profile === "grok"
              ? "grok_acp_event"
              : envelope.peer.profile === "cursor"
                ? "cursor_acp_event"
                : "agent_client_protocol_event",
        }),
        this.#state(
          "degraded",
          stored.rawEventRef,
          "ACP prompt result failed pinned schema validation",
        ),
      ];
    }
    if (this.#settled)
      return [this.#state("degraded", stored.rawEventRef, "late ACP prompt settlement ignored")];
    const sidecar = this.#event("raw.sidecar_ref", envelope.observedAt, {
      rawEventRef: stored.rawEventRef,
      rawEventKind:
        envelope.peer.profile === "grok"
          ? "grok_acp_event"
          : envelope.peer.profile === "cursor"
            ? "cursor_acp_event"
            : "agent_client_protocol_event",
    });
    return [sidecar, ...this.complete(stopReason, envelope.observedAt)];
  }

  #project(
    discriminator: string,
    update: RecordValue,
    native: AcpRuntimeNativeEnvelope,
    nativeRef: string,
  ): AcpProjectionEvent[] {
    const at = native.observedAt;
    if (discriminator === "agent_message_chunk" || discriminator === "agent_thought_chunk") {
      const channel = discriminator === "agent_message_chunk" ? "text" : "reasoning";
      const text = textOf(update.content);
      if (text === undefined)
        return [this.#state("degraded", nativeRef, `unsupported ${discriminator} content`)];
      const key = `${channel}:${this.#turnId}`;
      this.#completedChannels.add(key);
      return [
        this.#event(channel === "text" ? "text.delta" : "reasoning.delta", at, {
          messageId: ref("message", key),
          chunkId: ref("chunk", native.updateId),
          text,
        }),
      ];
    }
    if (discriminator === "user_message_chunk")
      return [
        this.#state(
          "user-message",
          nativeRef,
          textOf(update.content) === undefined
            ? "user attachment retained privately"
            : "user message chunk",
          {
            contentRef: nativeRef,
            contentType:
              typeof object(update.content).type === "string"
                ? object(update.content).type
                : "unknown",
          },
        ),
      ];
    if (discriminator === "tool_call" || discriminator === "tool_call_update")
      return this.#projectTool(discriminator, update, native, nativeRef);
    const stateKinds: Record<string, [AcpCanonicalStateEvent["kind"], string]> = {
      plan: ["plan-snapshot", "plan replacement snapshot"],
      available_commands_update: ["available-commands", "available command metadata replacement"],
      current_mode_update: ["mode-snapshot", "current mode snapshot"],
      config_option_update: ["config-snapshot", "configuration replacement snapshot"],
      session_info_update: ["session-info", "session information patch"],
      usage_update: ["usage-snapshot", "cumulative context and cost snapshot"],
      "cursor/update_todos": ["plan-snapshot", "Cursor todo replacement snapshot"],
    };
    const state = stateKinds[discriminator];
    if (state === undefined)
      return [
        this.#state("degraded", nativeRef, `unmapped ACP update ${discriminator.slice(0, 80)}`),
      ];
    const snapshot = (() => {
      if (discriminator === "plan" || discriminator === "cursor/update_todos") {
        const values = Array.isArray(update.entries)
          ? update.entries
          : Array.isArray(update.todos)
            ? update.todos
            : [];
        return {
          entries: values.slice(0, 256).map((entry, index) => {
            const value = object(entry);
            const contentIdentity = String(value.content ?? value.description ?? index);
            const status = ["pending", "in_progress", "completed"].includes(String(value.status))
              ? String(value.status)
              : "pending";
            const priority = ["high", "medium", "low"].includes(String(value.priority))
              ? String(value.priority)
              : undefined;
            return {
              entryRef: ref(
                "plan_entry",
                `${native.processGeneration}:${index}:${contentIdentity}`,
              ),
              contentRef: ref("content", contentIdentity),
              status,
              ...(priority === undefined ? {} : { priority }),
            };
          }),
        };
      }
      if (discriminator === "available_commands_update") {
        const values = Array.isArray(update.availableCommands) ? update.availableCommands : [];
        return {
          commands: values.slice(0, 256).map((entry, index) => {
            const value = object(entry);
            return {
              commandRef: ref(
                "command",
                `${native.processGeneration}:${String(value.name ?? index)}`,
              ),
              ...(typeof value.description === "string"
                ? { descriptionRef: ref("description", value.description) }
                : {}),
            };
          }),
        };
      }
      if (discriminator === "current_mode_update")
        return {
          currentModeRef: ref("mode", String(update.currentModeId ?? "cleared")),
          cleared: update.currentModeId === null,
        };
      if (discriminator === "config_option_update") {
        const values = Array.isArray(update.configOptions) ? update.configOptions : [];
        return {
          options: values.slice(0, 256).map((entry, index) => {
            const value = object(entry);
            return {
              optionRef: ref("config", `${native.processGeneration}:${String(value.id ?? index)}`),
              selectedValueRef: ref(
                "config_value",
                JSON.stringify(value.value ?? value.currentValue ?? null),
              ),
            };
          }),
        };
      }
      if (discriminator === "session_info_update")
        return {
          titleRef:
            update.title === null
              ? null
              : typeof update.title === "string"
                ? ref("title", update.title)
                : undefined,
          updatedAt: typeof update.updatedAt === "string" ? update.updatedAt : undefined,
        };
      if (discriminator === "usage_update")
        return {
          used: typeof update.used === "number" ? update.used : undefined,
          size: typeof update.size === "number" ? update.size : undefined,
          costRef: Object.prototype.hasOwnProperty.call(update, "cost")
            ? ref("cost", JSON.stringify(update.cost))
            : undefined,
        };
      return { nativeRef };
    })();
    return [this.#state(state[0], nativeRef, state[1], snapshot)];
  }

  #projectTool(
    discriminator: string,
    update: RecordValue,
    native: AcpRuntimeNativeEnvelope,
    nativeRef: string,
  ): KhalaRuntimeEvent[] {
    const nativeId = typeof update.toolCallId === "string" ? update.toolCallId : native.updateId;
    const toolCallId = ref("tool", `${native.processGeneration}:${native.sessionId}:${nativeId}`);
    const previous = this.#tools.get(toolCallId);
    const titlePresent = Object.prototype.hasOwnProperty.call(update, "title");
    const name = titlePresent
      ? typeof update.title === "string"
        ? update.title.slice(0, 256)
        : "ACP tool"
      : (previous?.name ?? "ACP tool");
    const statusPresent = Object.prototype.hasOwnProperty.call(update, "status");
    const status = statusPresent
      ? typeof update.status === "string"
        ? update.status
        : null
      : previous?.status;
    const rank =
      status === "pending"
        ? 0
        : status === "in_progress"
          ? 1
          : status === "completed" || status === "failed"
            ? 2
            : (previous?.rank ?? 0);
    const authority = {
      authorityRef: ref("authority", toolCallId),
      policyRef: "policy.acp_bridge",
      decisionRef: "decision.provider_reported_not_authority",
      toolRef: ref("toolref", name),
      status: "denied" as const,
      allowed: false,
      blockerRefs: ["blocker.provider_event_not_authority"],
    };
    const next: ToolState = {
      rank,
      ...(status === undefined ? {} : { status }),
      name,
      started: discriminator === "tool_call" || previous?.started === true,
      ...(Object.prototype.hasOwnProperty.call(update, "kind")
        ? { kind: typeof update.kind === "string" ? update.kind : null }
        : previous?.kind === undefined
          ? {}
          : { kind: previous.kind }),
      ...(Object.prototype.hasOwnProperty.call(update, "content")
        ? update.content === null
          ? {}
          : { contentRef: nativeRef }
        : previous?.contentRef === undefined
          ? {}
          : { contentRef: previous.contentRef }),
      ...(Object.prototype.hasOwnProperty.call(update, "locations")
        ? update.locations === null
          ? {}
          : { locationsRef: nativeRef }
        : previous?.locationsRef === undefined
          ? {}
          : { locationsRef: previous.locationsRef }),
      emittedTerminal: previous?.emittedTerminal ?? false,
    };
    this.#tools.set(toolCallId, next);
    if (discriminator === "tool_call") {
      const call = this.#event("tool.call", native.observedAt, {
        toolCallId,
        toolName: name,
        inputRef: nativeRef,
        authority,
      });
      if ((status === "completed" || status === "failed") && !next.emittedTerminal) {
        next.emittedTerminal = true;
        return [
          call,
          status === "completed"
            ? this.#event("tool.result", native.observedAt, {
                toolCallId,
                toolName: name,
                resultRef: nativeRef,
                authority,
                providerExecuted: true,
              })
            : this.#event("tool.error", native.observedAt, {
                toolCallId,
                toolName: name,
                errorRef: nativeRef,
                messageSafe: "ACP tool reported failure",
                authority,
                providerExecuted: true,
              }),
        ];
      }
      return [call];
    }
    if (!next.started)
      return [
        this.#event("provider.metadata", native.observedAt, {
          providerMetadata: { metadataRefs: [nativeRef, "degraded.tool_update_before_start"] },
        }),
      ];
    if ((status === "completed" || status === "failed") && !next.emittedTerminal) {
      next.emittedTerminal = true;
      return [
        status === "completed"
          ? this.#event("tool.result", native.observedAt, {
              toolCallId,
              toolName: name,
              resultRef: nativeRef,
              authority,
              providerExecuted: true,
            })
          : this.#event("tool.error", native.observedAt, {
              toolCallId,
              toolName: name,
              errorRef: nativeRef,
              messageSafe: "ACP tool reported failure",
              authority,
              providerExecuted: true,
            }),
      ];
    }
    return [
      this.#event("provider.metadata", native.observedAt, {
        providerMetadata: { metadataRefs: [nativeRef] },
      }),
    ];
  }

  #state(
    kind: AcpCanonicalStateEvent["kind"],
    nativeRef: string,
    safeSummary: string,
    snapshot: Readonly<Record<string, unknown>> = {},
  ): AcpCanonicalStateEvent {
    return {
      kind,
      threadId: this.threadId,
      turnId: this.turnId,
      stateRef: ref("state", `${kind}:${nativeRef}`),
      safeSummary,
      nativeRef,
      snapshot,
    };
  }

  #event(
    kind: KhalaRuntimeEvent["kind"],
    observedAt: string,
    fields: RecordValue,
  ): KhalaRuntimeEvent {
    this.#eventSequence += 1;
    return {
      schema: "openagents.khala_runtime_event.v1",
      eventId: ref("event", `${this.#turnId}:${this.#eventSequence}`),
      turnId: this.#turnId,
      threadId: this.threadId,
      sequence: this.#eventSequence,
      observedAt,
      source: {
        lane: "agent_client_protocol",
        adapterKind:
          this.#binding.profile === "grok"
            ? "grok_cli"
            : this.#binding.profile === "cursor"
              ? "cursor_cli"
              : "agent_client_protocol",
        surface: "server",
        providerRef: ref("provider", this.#binding.profile),
        adapterSessionRef: ref(
          "session",
          `${this.#binding.processGeneration}:${this.#binding.peerSessionId}`,
        ),
      },
      visibility: "private",
      redactionClass: "private_ref",
      causalityRefs: [],
      kind,
      ...fields,
    } as KhalaRuntimeEvent;
  }
}

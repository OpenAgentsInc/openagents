import { Effect, Schema as S } from "effect";
import { type ResolvedProbeBackendProfile, type ResolveProbeBackendProfileOptions } from "../backend-profile";
import { resolveAppleFmBackendProfile, type ProbeBackendRegistryError } from "../registry";
import {
  AppleFmChatCompletionResponse,
  AppleFmStreamSnapshotEvent,
  AppleFmHealthResponse,
  type AppleFmChatCompletionResponse,
  type AppleFmChatMessage,
  type AppleFmHealthResponse,
  type AppleFmUnavailableReason,
  type AppleFmUsageMeasurement,
} from "./contract";
import {
  AppleFmBackendFailureReceipt,
  makeAppleFmTranscriptReceipt,
  makeAppleFmAvailabilityReceipt,
  makeAppleFmFailureReceipt,
  type AppleFmBackendAvailabilityReceipt,
  type AppleFmBackendTranscriptReceipt,
} from "./receipts";
import {
  startAppleFmToolCallbackServer,
  type AppleFmToolCallbackServer,
  type AppleFmToolCallbackSession,
} from "./tools";

export const AppleFmHealthStatus = S.Literals(["ready", "unavailable", "unsupported", "malformed", "unreachable"]);
export type AppleFmHealthStatus = typeof AppleFmHealthStatus.Type;

export interface AppleFmClientOptions extends ResolveProbeBackendProfileOptions {
  readonly fetch?: typeof fetch;
  readonly now?: Date;
}

export interface AppleFmReadiness {
  readonly profile: ResolvedProbeBackendProfile;
  readonly status: AppleFmHealthStatus;
  readonly ready: boolean;
  readonly health?: AppleFmHealthResponse;
  readonly unavailableReason?: AppleFmUnavailableReason;
  readonly message?: string;
  readonly receipt: AppleFmBackendAvailabilityReceipt;
}

export interface AppleFmClient {
  readonly profile: ResolvedProbeBackendProfile;
  readonly health: () => Effect.Effect<AppleFmReadiness, never>;
  readonly requireReady: () => Effect.Effect<AppleFmReadiness, AppleFmBackendError>;
  readonly completePlainText: (
    messages: ReadonlyArray<AppleFmChatMessage>,
  ) => Effect.Effect<AppleFmPlainTextCompletion, AppleFmBackendError>;
  readonly streamPlainTextSnapshots: (
    messages: ReadonlyArray<AppleFmChatMessage>,
  ) => Effect.Effect<AppleFmSnapshotStreamResult, AppleFmBackendError>;
  readonly streamSessionWithTools: (
    input: AppleFmToolStreamInput,
  ) => Effect.Effect<AppleFmToolStreamResult, AppleFmBackendError>;
  readonly smoke: (prompt: string) => Effect.Effect<AppleFmPlainTextCompletion, AppleFmBackendError>;
}

export interface AppleFmPlainTextCompletion {
  readonly profile: ResolvedProbeBackendProfile;
  readonly text: string;
  readonly response: AppleFmChatCompletionResponse;
  readonly usage: AppleFmUsageMeasurement;
  readonly receipt: AppleFmBackendTranscriptReceipt;
}

export const AppleFmRuntimeStreamEvent = S.Struct({
  kind: S.Literals([
    "assistant_stream_started",
    "assistant_snapshot",
    "assistant_stream_finished",
    "assistant_final_commit",
    "assistant_stream_failed",
  ]),
  sequence: S.optional(S.Number),
  content: S.optional(S.String),
  timeToFirstSnapshotMs: S.optional(S.Number),
  observedAt: S.String,
  receipt: S.optional(S.Unknown),
});
export type AppleFmRuntimeStreamEvent = typeof AppleFmRuntimeStreamEvent.Type;

export interface AppleFmSnapshotStreamResult {
  readonly profile: ResolvedProbeBackendProfile;
  readonly snapshots: ReadonlyArray<typeof AppleFmStreamSnapshotEvent.Type>;
  readonly completion: AppleFmPlainTextCompletion;
  readonly events: ReadonlyArray<AppleFmRuntimeStreamEvent>;
}

export interface AppleFmToolStreamInput {
  readonly prompt: string;
  readonly toolSession: AppleFmToolCallbackSession;
  readonly instructions?: string;
}

export interface AppleFmToolStreamResult {
  readonly profile: ResolvedProbeBackendProfile;
  readonly bridgeSessionId: string;
  readonly callbackServer: {
    readonly callbackUrl: "[redacted]";
  };
  readonly events: ReadonlyArray<AppleFmRuntimeStreamEvent>;
  readonly completion: AppleFmPlainTextCompletion;
  readonly toolTranscript: AppleFmToolCallbackSession["transcript"];
}

export class AppleFmBackendError extends S.TaggedErrorClass<AppleFmBackendError>()("AppleFmBackendError", {
  reason: S.String,
  failureClass: S.String,
  receipt: S.optional(AppleFmBackendFailureReceipt),
}) {}

export function makeAppleFmClient(
  options: AppleFmClientOptions = {},
): Effect.Effect<AppleFmClient, ProbeBackendRegistryError> {
  return Effect.gen(function* () {
    const profile = yield* resolveAppleFmBackendProfile(options);
    const fetchImpl = options.fetch ?? fetch;
    const now = () => (options.now ?? new Date()).toISOString();

    const client: AppleFmClient = {
      profile,
      health: () => checkAppleFmHealth(profile, fetchImpl, now()),
      requireReady: () =>
        checkAppleFmHealth(profile, fetchImpl, now()).pipe(
          Effect.flatMap((readiness) =>
            readiness.ready
              ? Effect.succeed(readiness)
              : Effect.fail(
                  new AppleFmBackendError({
                    reason: readiness.message ?? `Apple FM backend is ${readiness.status}`,
                    failureClass: readiness.unavailableReason ?? readiness.status,
                    receipt: makeAppleFmFailureReceipt({
                      profileId: profile.id,
                      model: profile.model,
                      baseUrl: profile.baseUrl,
                      failureClass: readiness.unavailableReason ?? readiness.status,
                      message: readiness.message ?? `Apple FM backend is ${readiness.status}`,
                      observedAt: now(),
                    }),
                  }),
                ),
          ),
        ),
      completePlainText: (messages) => completeAppleFmPlainText(profile, messages, fetchImpl, now()),
      streamPlainTextSnapshots: (messages) => streamAppleFmPlainTextSnapshots(profile, messages, fetchImpl, now()),
      streamSessionWithTools: (input) => streamAppleFmSessionWithTools(profile, input, fetchImpl, now()),
      smoke: (prompt) =>
        client.requireReady().pipe(
          Effect.flatMap(() =>
            completeAppleFmPlainText(
              profile,
              [
                {
                  role: "user",
                  content: prompt,
                },
              ],
              fetchImpl,
              now(),
            ),
          ),
        ),
    };

    return client;
  });
}

export function streamAppleFmSessionWithTools(
  profile: ResolvedProbeBackendProfile,
  input: AppleFmToolStreamInput,
  fetchImpl: typeof fetch = fetch,
  observedAt = new Date().toISOString(),
): Effect.Effect<AppleFmToolStreamResult, AppleFmBackendError> {
  return Effect.acquireUseRelease(
    Effect.sync(() => startAppleFmToolCallbackServer(input.toolSession)),
    (callbackServer) => streamAppleFmSessionWithCallbackServer(profile, input, callbackServer, fetchImpl, observedAt),
    (callbackServer) => Effect.sync(() => callbackServer.stop()),
  );
}

export function streamAppleFmPlainTextSnapshots(
  profile: ResolvedProbeBackendProfile,
  messages: ReadonlyArray<AppleFmChatMessage>,
  fetchImpl: typeof fetch = fetch,
  observedAt = new Date().toISOString(),
): Effect.Effect<AppleFmSnapshotStreamResult, AppleFmBackendError> {
  return Effect.gen(function* () {
    const startedAt = Date.now();
    const endpoint = new URL("/v1/chat/completions", withTrailingSlash(profile.baseUrl));
    const started: AppleFmRuntimeStreamEvent = {
      kind: "assistant_stream_started",
      observedAt,
    };
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: profile.model,
            messages,
            stream: true,
            streamMode: "snapshot",
          }),
        }),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM snapshot stream request failed: ${String(error)}`,
          failureClass: "bridge_unreachable",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "bridge_unreachable",
            message: `Apple FM snapshot stream request failed: ${String(error)}`,
            observedAt,
          }),
        }),
    });

    const rawText = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM snapshot stream response could not be read: ${String(error)}`,
          failureClass: "malformed_response",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "malformed_response",
            message: `Apple FM snapshot stream response could not be read: ${String(error)}`,
            observedAt,
          }),
        }),
    });

    if (!response.ok) {
      const errorMessage = bridgeErrorMessage(parseMaybeJson(rawText)) ?? `Apple FM snapshot stream returned HTTP ${response.status}`;
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: errorMessage,
          failureClass: `stream_http_${response.status}`,
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: `stream_http_${response.status}`,
            message: errorMessage,
            observedAt,
          }),
        }),
      );
    }

    const snapshots = yield* decodeSnapshotStream(rawText, observedAt).pipe(
      Effect.mapError(
        (error) =>
          new AppleFmBackendError({
            reason: error,
            failureClass: "malformed_response",
            receipt: makeAppleFmFailureReceipt({
              profileId: profile.id,
              model: profile.model,
              baseUrl: profile.baseUrl,
              failureClass: "malformed_response",
              message: error,
              observedAt,
            }),
          }),
      ),
    );
    const finalSnapshot = snapshots.at(-1);

    if (finalSnapshot === undefined) {
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: "Apple FM snapshot stream did not include any assistant snapshots",
          failureClass: "empty_stream",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "empty_stream",
            message: "Apple FM snapshot stream did not include any assistant snapshots",
            observedAt,
          }),
        }),
      );
    }

    const usage: AppleFmUsageMeasurement = { truth: "unknown" };
    const receipt = makeAppleFmTranscriptReceipt({
      profileId: profile.id,
      model: profile.model,
      usage,
      observedAt,
    });
    const completion: AppleFmPlainTextCompletion = {
      profile,
      text: finalSnapshot.content,
      response: {
        model: profile.model,
        choices: [
          {
            index: 0,
            message: {
              role: "assistant",
              content: finalSnapshot.content,
            },
            finishReason: finalSnapshot.finishReason ?? "stop",
          },
        ],
        usage,
      },
      usage,
      receipt,
    };
    const firstSnapshotAt = Date.now();
    const snapshotEvents: AppleFmRuntimeStreamEvent[] = snapshots.map((snapshot) => ({
      kind: "assistant_snapshot",
      sequence: snapshot.sequence,
      content: snapshot.content,
      observedAt: snapshot.observedAt,
      ...(snapshot.sequence === snapshots[0]?.sequence
        ? { timeToFirstSnapshotMs: Math.max(0, firstSnapshotAt - startedAt) }
        : {}),
    }));
    const finished: AppleFmRuntimeStreamEvent = {
      kind: "assistant_stream_finished",
      observedAt,
    };
    const committed: AppleFmRuntimeStreamEvent = {
      kind: "assistant_final_commit",
      content: finalSnapshot.content,
      observedAt,
      receipt,
    };

    return {
      profile,
      snapshots,
      completion,
      events: [started, ...snapshotEvents, finished, committed],
    };
  });
}

export function completeAppleFmPlainText(
  profile: ResolvedProbeBackendProfile,
  messages: ReadonlyArray<AppleFmChatMessage>,
  fetchImpl: typeof fetch = fetch,
  observedAt = new Date().toISOString(),
): Effect.Effect<AppleFmPlainTextCompletion, AppleFmBackendError> {
  return Effect.gen(function* () {
    const endpoint = new URL("/v1/chat/completions", withTrailingSlash(profile.baseUrl));
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: profile.model,
            messages,
          }),
        }),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM completion request failed: ${String(error)}`,
          failureClass: "bridge_unreachable",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "bridge_unreachable",
            message: `Apple FM completion request failed: ${String(error)}`,
            observedAt,
          }),
        }),
    });

    const raw = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM completion response was not JSON: ${String(error)}`,
          failureClass: "malformed_response",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "malformed_response",
            message: `Apple FM completion response was not JSON: ${String(error)}`,
            observedAt,
          }),
        }),
    });

    if (!response.ok) {
      const errorMessage = bridgeErrorMessage(raw) ?? `Apple FM completion returned HTTP ${response.status}`;
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: errorMessage,
          failureClass: `completion_http_${response.status}`,
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: `completion_http_${response.status}`,
            message: errorMessage,
            observedAt,
          }),
        }),
      );
    }

    const normalized = normalizeChatCompletion(raw, profile.model);
    const decoded = yield* S.decodeUnknownEffect(AppleFmChatCompletionResponse)(normalized).pipe(
      Effect.mapError(
        (error) =>
          new AppleFmBackendError({
            reason: `Apple FM completion response was malformed: ${String(error)}`,
            failureClass: "malformed_response",
            receipt: makeAppleFmFailureReceipt({
              profileId: profile.id,
              model: profile.model,
              baseUrl: profile.baseUrl,
              failureClass: "malformed_response",
              message: `Apple FM completion response was malformed: ${String(error)}`,
              observedAt,
            }),
          }),
      ),
    );

    const choice = decoded.choices[0];

    if (choice === undefined || choice.message.content.length === 0) {
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: "Apple FM completion response did not include assistant text",
          failureClass: "empty_completion",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "empty_completion",
            message: "Apple FM completion response did not include assistant text",
            observedAt,
          }),
        }),
      );
    }

    const usage = decoded.usage ?? { truth: "unknown" as const };

    return {
      profile,
      text: choice.message.content,
      response: decoded,
      usage,
      receipt: makeAppleFmTranscriptReceipt({
        profileId: profile.id,
        model: decoded.model ?? profile.model,
        usage,
        observedAt,
      }),
    };
  });
}

function streamAppleFmSessionWithCallbackServer(
  profile: ResolvedProbeBackendProfile,
  input: AppleFmToolStreamInput,
  callbackServer: AppleFmToolCallbackServer,
  fetchImpl: typeof fetch,
  observedAt: string,
): Effect.Effect<AppleFmToolStreamResult, AppleFmBackendError> {
  return Effect.gen(function* () {
    const sessionResponse = yield* requestJson(fetchImpl, new URL("/v1/sessions", withTrailingSlash(profile.baseUrl)), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        instructions: input.instructions,
        model: {
          id: profile.model,
          use_case: "general",
          guardrails: "default",
        },
        tools: input.toolSession.projectedTools.map((tool) => ({
          name: tool.name,
          description: tool.description,
          arguments_schema: tool.inputSchema,
        })),
        tool_callback: {
          url: callbackServer.callbackUrl,
          session_token: input.toolSession.token,
        },
      }),
    }, {
      profile,
      operation: "session_create",
      observedAt,
    });
    const bridgeSessionId = yield* readBridgeSessionId(sessionResponse, profile, observedAt);
    const streamResponse = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(new URL(`/v1/sessions/${encodeURIComponent(bridgeSessionId)}/responses/stream`, withTrailingSlash(profile.baseUrl)), {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            prompt: input.prompt,
          }),
        }),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM tool stream request failed: ${String(error)}`,
          failureClass: "tool_stream_unreachable",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "tool_stream_unreachable",
            message: `Apple FM tool stream request failed: ${String(error)}`,
            observedAt,
          }),
        }),
    });
    const streamText = yield* Effect.tryPromise({
      try: () => streamResponse.text(),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM tool stream response could not be read: ${String(error)}`,
          failureClass: "tool_stream_malformed",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "tool_stream_malformed",
            message: `Apple FM tool stream response could not be read: ${String(error)}`,
            observedAt,
          }),
        }),
    });

    if (!streamResponse.ok) {
      const errorMessage = bridgeErrorMessage(parseMaybeJson(streamText)) ?? `Apple FM tool stream returned HTTP ${streamResponse.status}`;
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: errorMessage,
          failureClass: `tool_stream_http_${streamResponse.status}`,
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: `tool_stream_http_${streamResponse.status}`,
            message: errorMessage,
            observedAt,
          }),
        }),
      );
    }

    const sseEvents = parseSseFrames(streamText);
    const snapshots = sseEvents
      .filter((event) => event.event === "snapshot")
      .map((event, index) => normalizeBridgeTextStreamEvent(event.data, index, observedAt));
    const completed = sseEvents.find((event) => event.event === "completed");
    const error = sseEvents.find((event) => event.event === "error");

    if (error !== undefined) {
      const errorMessage = bridgeErrorMessage(error.data) ?? "Apple FM tool stream emitted an error event";
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: errorMessage,
          failureClass: "tool_stream_error_event",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "tool_stream_error_event",
            message: errorMessage,
            observedAt,
          }),
        }),
      );
    }

    const finalEvent = completed?.data ?? snapshots.at(-1);
    const finalOutput = readStringField(finalEvent, "output") ?? readStringField(finalEvent, "content") ?? "";

    if (finalOutput.length === 0) {
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: "Apple FM tool stream did not include final assistant output",
          failureClass: "tool_stream_empty",
          receipt: makeAppleFmFailureReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            failureClass: "tool_stream_empty",
            message: "Apple FM tool stream did not include final assistant output",
            observedAt,
          }),
        }),
      );
    }

    const usage = normalizeUsage(readObjectField(finalEvent, "usage"));
    const receipt = makeAppleFmTranscriptReceipt({
      profileId: profile.id,
      model: readStringField(finalEvent, "model") ?? profile.model,
      usage,
      observedAt,
    });
    const events: AppleFmRuntimeStreamEvent[] = [
      { kind: "assistant_stream_started", observedAt },
      ...snapshots.map((snapshot, index) => ({
        kind: "assistant_snapshot" as const,
        sequence: index,
        content: readStringField(snapshot, "output") ?? "",
        observedAt,
      })),
      { kind: "assistant_stream_finished", observedAt },
      { kind: "assistant_final_commit", content: finalOutput, receipt, observedAt },
    ];

    return {
      profile,
      bridgeSessionId,
      callbackServer: {
        callbackUrl: "[redacted]",
      },
      events,
      completion: {
        profile,
        text: finalOutput,
        response: {
          model: readStringField(finalEvent, "model") ?? profile.model,
          choices: [
            {
              index: 0,
              message: {
                role: "assistant",
                content: finalOutput,
              },
              finishReason: "stop",
            },
          ],
          usage,
        },
        usage,
        receipt,
      },
      toolTranscript: input.toolSession.transcript,
    };
  });
}

export function checkAppleFmHealth(
  profile: ResolvedProbeBackendProfile,
  fetchImpl: typeof fetch = fetch,
  observedAt = new Date().toISOString(),
): Effect.Effect<AppleFmReadiness, never> {
  return Effect.gen(function* () {
    const endpoint = new URL(profile.readinessPath, withTrailingSlash(profile.baseUrl));
    const response = yield* Effect.tryPromise({
      try: () => fetchImpl(endpoint, { method: "GET" }),
      catch: (error) =>
        unavailableReadiness(profile, {
          status: "unreachable",
          unavailableReason: "bridge_unreachable",
          message: `Apple FM bridge is unreachable: ${String(error)}`,
          observedAt,
        }),
    });

    if (isReadiness(response)) {
      return response;
    }

    if (!response.ok) {
      return unavailableReadiness(profile, {
        status: "unavailable",
        unavailableReason: "not_ready",
        message: `Apple FM bridge health returned HTTP ${response.status}`,
        observedAt,
      });
    }

    const raw = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) =>
        unavailableReadiness(profile, {
          status: "malformed",
          unavailableReason: "malformed_response",
          message: `Apple FM bridge health response was not JSON: ${String(error)}`,
          observedAt,
        }),
    });

    if (isReadiness(raw)) {
      return raw;
    }

    const decoded = yield* S.decodeUnknownEffect(AppleFmHealthResponse)(normalizeHealthResponse(raw, profile.model)).pipe(
      Effect.mapError((error) =>
        unavailableReadiness(profile, {
          status: "malformed",
          unavailableReason: "malformed_response",
          message: `Apple FM bridge health response was malformed: ${String(error)}`,
          observedAt,
        }),
      ),
    );

    if (isReadiness(decoded)) {
      return decoded;
    }

    const model = decoded.modelId ?? decoded.model ?? profile.model;
    const ready = decoded.ready === true;
    const unavailableReason = decoded.unavailableReason;
    const status = ready ? "ready" : healthStatusFromReason(unavailableReason);

    return {
      profile,
      status,
      ready,
      health: decoded,
      unavailableReason,
      message: decoded.message,
      receipt: makeAppleFmAvailabilityReceipt({
        profileId: profile.id,
        model,
        baseUrl: profile.baseUrl,
        ready,
        unavailableReason,
        message: decoded.message,
        observedAt,
      }),
    };
  }).pipe(Effect.catch((readiness: AppleFmReadiness) => Effect.succeed(readiness)));
}

function unavailableReadiness(
  profile: ResolvedProbeBackendProfile,
  input: {
    readonly status: Exclude<AppleFmHealthStatus, "ready">;
    readonly unavailableReason: AppleFmUnavailableReason;
    readonly message: string;
    readonly observedAt: string;
  },
): AppleFmReadiness {
  return {
    profile,
    status: input.status,
    ready: false,
    unavailableReason: input.unavailableReason,
    message: input.message,
    receipt: makeAppleFmAvailabilityReceipt({
      profileId: profile.id,
      model: profile.model,
      baseUrl: profile.baseUrl,
      ready: false,
      unavailableReason: input.unavailableReason,
      message: input.message,
      observedAt: input.observedAt,
    }),
  };
}

function healthStatusFromReason(reason: AppleFmUnavailableReason | undefined): AppleFmHealthStatus {
  if (reason === "unsupported_hardware" || reason === "apple_intelligence_disabled") {
    return "unsupported";
  }

  if (reason === "malformed_response") {
    return "malformed";
  }

  if (reason === "bridge_unreachable") {
    return "unreachable";
  }

  return "unavailable";
}

function isReadiness(value: unknown): value is AppleFmReadiness {
  return typeof value === "object" && value !== null && "profile" in value && "receipt" in value && "status" in value;
}

function withTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}

function normalizeChatCompletion(value: unknown, fallbackModel: string): unknown {
  if (typeof value !== "object" || value === null) {
    return value;
  }

  const input = value as Record<string, unknown>;
  const choices = Array.isArray(input.choices) ? input.choices.map(normalizeChoice) : [];

  return {
    id: typeof input.id === "string" ? input.id : undefined,
    model: typeof input.model === "string" ? input.model : fallbackModel,
    choices,
    usage: normalizeUsage(input.usage),
  };
}

function normalizeChoice(value: unknown): unknown {
  if (typeof value !== "object" || value === null) {
    return value;
  }

  const input = value as Record<string, unknown>;
  const message = typeof input.message === "object" && input.message !== null ? input.message as Record<string, unknown> : {};

  return {
    index: typeof input.index === "number" ? input.index : undefined,
    message: {
      role: normalizeRole(message.role),
      content: typeof message.content === "string" ? message.content : "",
      name: typeof message.name === "string" ? message.name : undefined,
      toolCallId: typeof message.toolCallId === "string" ? message.toolCallId : undefined,
    },
    finishReason: normalizeFinishReason(input.finishReason ?? input.finish_reason),
  };
}

function normalizeUsage(value: unknown): AppleFmUsageMeasurement {
  if (typeof value !== "object" || value === null) {
    return {
      truth: "unknown",
    };
  }

  const input = value as Record<string, unknown>;
  const promptTokens = numberField(input.promptTokens) ?? numberField(input.prompt_tokens) ?? measurementValue(input.prompt_tokens_detail);
  const completionTokens =
    numberField(input.completionTokens) ?? numberField(input.completion_tokens) ?? measurementValue(input.completion_tokens_detail);
  const totalTokens = numberField(input.totalTokens) ?? numberField(input.total_tokens) ?? measurementValue(input.total_tokens_detail);
  const hasTokenCounts = promptTokens !== undefined || completionTokens !== undefined || totalTokens !== undefined;
  const detailTruth =
    measurementTruth(input.total_tokens_detail) ?? measurementTruth(input.prompt_tokens_detail) ?? measurementTruth(input.completion_tokens_detail);
  const truth = input.truth === "exact" || input.truth === "estimated" || input.truth === "unknown"
    ? input.truth
    : detailTruth !== undefined
      ? detailTruth
    : hasTokenCounts
      ? "estimated"
      : "unknown";

  return {
    truth,
    promptTokens,
    completionTokens,
    totalTokens,
  };
}

function numberField(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function measurementValue(value: unknown): number | undefined {
  return typeof value === "object" && value !== null ? numberField((value as Record<string, unknown>).value) : undefined;
}

function measurementTruth(value: unknown): AppleFmUsageMeasurement["truth"] | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const truth = (value as Record<string, unknown>).truth;
  return truth === "exact" || truth === "estimated" ? truth : undefined;
}

function normalizeRole(value: unknown): AppleFmChatMessage["role"] {
  return value === "system" || value === "user" || value === "assistant" || value === "tool" ? value : "assistant";
}

function normalizeFinishReason(value: unknown): AppleFmChatCompletionResponse["choices"][number]["finishReason"] {
  if (
    value === "stop" ||
    value === "length" ||
    value === "tool_calls" ||
    value === "content_filter" ||
    value === "error" ||
    value === "unknown"
  ) {
    return value;
  }

  return "unknown";
}

function bridgeErrorMessage(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const input = value as Record<string, unknown>;
  const error = input.error;

  if (typeof error === "string") {
    return error;
  }

  if (typeof error === "object" && error !== null) {
    const errorObject = error as Record<string, unknown>;

    if (typeof errorObject.message === "string") {
      return errorObject.message;
    }
  }

  if (typeof input.message === "string") {
    return input.message;
  }

  return undefined;
}

function decodeSnapshotStream(
  rawText: string,
  observedAt: string,
): Effect.Effect<ReadonlyArray<typeof AppleFmStreamSnapshotEvent.Type>, string> {
  return Effect.gen(function* () {
    const rawEvents = parseSnapshotLines(rawText);
    const snapshots = rawEvents.map((event, index) => normalizeSnapshotEvent(event, index, observedAt));

    return yield* S.decodeUnknownEffect(S.Array(AppleFmStreamSnapshotEvent))(snapshots).pipe(
      Effect.mapError((error) => `Apple FM snapshot stream was malformed: ${String(error)}`),
    );
  });
}

function parseSnapshotLines(rawText: string): unknown[] {
  const trimmed = rawText.trim();

  if (trimmed.length === 0) {
    return [];
  }

  const parsed = parseMaybeJson(trimmed);

  if (Array.isArray(parsed)) {
    return parsed;
  }

  if (parsed !== undefined) {
    return [parsed];
  }

  return trimmed
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .filter((line) => line !== "data: [DONE]" && line !== "[DONE]")
    .map((line) => line.startsWith("data: ") ? line.slice("data: ".length) : line)
    .map((line) => JSON.parse(line) as unknown);
}

function normalizeSnapshotEvent(value: unknown, index: number, observedAt: string): unknown {
  if (typeof value !== "object" || value === null) {
    return value;
  }

  const input = value as Record<string, unknown>;
  const content = input.content ?? input.snapshot ?? input.text;

  return {
    kind: "apple_fm_assistant_snapshot",
    sequence: typeof input.sequence === "number" ? input.sequence : index,
    content: typeof content === "string" ? content : "",
    observedAt: typeof input.observedAt === "string" ? input.observedAt : observedAt,
    finishReason: normalizeFinishReason(input.finishReason ?? input.finish_reason),
  };
}

function parseMaybeJson(value: string): unknown | undefined {
  try {
    return JSON.parse(value) as unknown;
  } catch {
    return undefined;
  }
}

function normalizeHealthResponse(value: unknown, fallbackModel: string): unknown {
  if (typeof value !== "object" || value === null) {
    return value;
  }

  const input = value as Record<string, unknown>;
  const ready = booleanField(input.ready) ?? booleanField(input.modelAvailable) ?? booleanField(input.model_available) ?? input.status === "ok";

  return {
    ready,
    model: readStringField(input, "model"),
    modelId: readStringField(input, "modelId") ?? readStringField(input, "model_id") ?? fallbackModel,
    unavailableReason: normalizeUnavailableReason(input.unavailableReason ?? input.unavailable_reason),
    message: readStringField(input, "message") ?? readStringField(input, "availabilityMessage") ?? readStringField(input, "availability_message"),
    platform: readStringField(input, "platform"),
    version: readStringField(input, "version"),
  };
}

function normalizeUnavailableReason(value: unknown): AppleFmUnavailableReason | undefined {
  if (value === "apple_intelligence_not_enabled" || value === "appleIntelligenceNotEnabled") {
    return "apple_intelligence_disabled";
  }

  if (value === "device_not_eligible" || value === "deviceNotEligible") {
    return "unsupported_hardware";
  }

  if (value === "model_not_ready" || value === "modelNotReady") {
    return "model_unavailable";
  }

  return value === "bridge_unreachable" ||
    value === "apple_intelligence_disabled" ||
    value === "unsupported_hardware" ||
    value === "model_unavailable" ||
    value === "permission_denied" ||
    value === "malformed_response" ||
    value === "not_ready" ||
    value === "unknown"
    ? value
    : undefined;
}

function booleanField(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function readBridgeSessionId(
  value: unknown,
  profile: ResolvedProbeBackendProfile,
  observedAt: string,
): Effect.Effect<string, AppleFmBackendError> {
  const session = readObjectField(value, "session");
  const id = readStringField(session, "id");

  if (id === undefined) {
    return Effect.fail(
      new AppleFmBackendError({
        reason: "Apple FM session create response did not include session.id",
        failureClass: "session_create_malformed",
        receipt: makeAppleFmFailureReceipt({
          profileId: profile.id,
          model: profile.model,
          baseUrl: profile.baseUrl,
          failureClass: "session_create_malformed",
          message: "Apple FM session create response did not include session.id",
          observedAt,
        }),
      }),
    );
  }

  return Effect.succeed(id);
}

function requestJson(
  fetchImpl: typeof fetch,
  endpoint: URL,
  init: RequestInit,
  context: {
    readonly profile: ResolvedProbeBackendProfile;
    readonly operation: string;
    readonly observedAt: string;
  },
): Effect.Effect<unknown, AppleFmBackendError> {
  return Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () => fetchImpl(endpoint, init),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM ${context.operation} request failed: ${String(error)}`,
          failureClass: `${context.operation}_unreachable`,
          receipt: makeAppleFmFailureReceipt({
            profileId: context.profile.id,
            model: context.profile.model,
            baseUrl: context.profile.baseUrl,
            failureClass: `${context.operation}_unreachable`,
            message: `Apple FM ${context.operation} request failed: ${String(error)}`,
            observedAt: context.observedAt,
          }),
        }),
    });
    const raw = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) =>
        new AppleFmBackendError({
          reason: `Apple FM ${context.operation} response was not JSON: ${String(error)}`,
          failureClass: `${context.operation}_malformed`,
          receipt: makeAppleFmFailureReceipt({
            profileId: context.profile.id,
            model: context.profile.model,
            baseUrl: context.profile.baseUrl,
            failureClass: `${context.operation}_malformed`,
            message: `Apple FM ${context.operation} response was not JSON: ${String(error)}`,
            observedAt: context.observedAt,
          }),
        }),
    });

    if (!response.ok) {
      const errorMessage = bridgeErrorMessage(raw) ?? `Apple FM ${context.operation} returned HTTP ${response.status}`;
      return yield* Effect.fail(
        new AppleFmBackendError({
          reason: errorMessage,
          failureClass: `${context.operation}_http_${response.status}`,
          receipt: makeAppleFmFailureReceipt({
            profileId: context.profile.id,
            model: context.profile.model,
            baseUrl: context.profile.baseUrl,
            failureClass: `${context.operation}_http_${response.status}`,
            message: errorMessage,
            observedAt: context.observedAt,
          }),
        }),
      );
    }

    return raw;
  });
}

function parseSseFrames(value: string): ReadonlyArray<{ readonly event: string; readonly data: unknown }> {
  const frames: Array<{ readonly event: string; readonly data: unknown }> = [];
  let event = "message";
  const dataLines: string[] = [];

  for (const line of value.split(/\r?\n/)) {
    if (line.length === 0) {
      if (dataLines.length > 0) {
        const data = dataLines.join("\n");
        frames.push({ event, data: parseMaybeJson(data) ?? data });
      }
      event = "message";
      dataLines.length = 0;
      continue;
    }

    if (line.startsWith("event:")) {
      event = line.slice("event:".length).trim();
      continue;
    }

    if (line.startsWith("data:")) {
      dataLines.push(line.slice("data:".length).trimStart());
    }
  }

  if (dataLines.length > 0) {
    const data = dataLines.join("\n");
    frames.push({ event, data: parseMaybeJson(data) ?? data });
  }

  return frames;
}

function normalizeBridgeTextStreamEvent(value: unknown, sequence: number, observedAt: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null) {
    return {
      sequence,
      output: "",
      observedAt,
    };
  }

  return {
    sequence,
    ...value as Record<string, unknown>,
    observedAt,
  };
}

function readStringField(value: unknown, key: string): string | undefined {
  return typeof value === "object" && value !== null && typeof (value as Record<string, unknown>)[key] === "string"
    ? (value as Record<string, string>)[key]
    : undefined;
}

function readObjectField(value: unknown, key: string): Record<string, unknown> | undefined {
  const field = typeof value === "object" && value !== null ? (value as Record<string, unknown>)[key] : undefined;
  return typeof field === "object" && field !== null ? field as Record<string, unknown> : undefined;
}

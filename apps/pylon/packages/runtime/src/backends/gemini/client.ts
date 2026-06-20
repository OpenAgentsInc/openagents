import { Effect, Schema as S } from "effect";
import { type ResolveProbeBackendProfileOptions, type ResolvedProbeBackendProfile } from "../backend-profile.js";
import { resolveGeminiBackendProfile, type ProbeBackendRegistryError } from "../registry.js";
import { makeGeminiAuthHeaders, resolveGeminiApiKey, type ResolvedGeminiApiKey } from "./auth.js";
import {
  finishGeminiSseParseState,
  geminiEndpointPath,
  lowerProbeLlmRequestToGeminiBody,
  makeGeminiSseParseState,
  parseGeminiSsePayload,
} from "./protocol.js";
import {
  ProbeLlmEvents,
  makeProbeLlmMessage,
  makeProbeLlmRequest,
  makeProbeLlmToolResult,
  type ProbeLlmEvent,
  type ProbeLlmRequest,
} from "../../llm/index.js";
import { dispatchProbeLlmTool } from "../../llm/tool-runtime.js";
import { type ProbeLlmTools } from "../../llm/tool.js";
import {
  GeminiBackendFailureReceipt,
  makeGeminiFailureReceipt,
  makeGeminiToolCallReceipt,
  makeGeminiTranscriptReceipt,
  type GeminiBackendToolCallReceipt,
  type GeminiBackendTranscriptReceipt,
} from "./receipts.js";

export interface GeminiClientOptions extends ResolveProbeBackendProfileOptions {
  readonly apiKey?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly fetch?: typeof fetch;
  readonly now?: Date;
}

export interface GeminiClient {
  readonly profile: ResolvedProbeBackendProfile;
  readonly apiKey: {
    readonly source: ResolvedGeminiApiKey["source"];
    readonly redacted: true;
  };
  readonly complete: (input: GeminiCompleteInput) => Effect.Effect<GeminiCompleteResult, GeminiClientError>;
}

export interface GeminiCompleteInput {
  readonly request: ProbeLlmRequest;
  readonly tools?: ProbeLlmTools;
  readonly maxModelRoundTrips?: number;
  readonly onEvent?: (event: ProbeLlmEvent) => void;
}

export interface GeminiCompleteResult {
  readonly profile: ResolvedProbeBackendProfile;
  readonly events: ReadonlyArray<ProbeLlmEvent>;
  readonly text: string;
  readonly finalRequest: ProbeLlmRequest;
  readonly roundTrips: number;
  readonly receipt: GeminiBackendTranscriptReceipt;
  readonly toolReceipts: ReadonlyArray<GeminiBackendToolCallReceipt>;
}

export class GeminiClientError extends S.TaggedErrorClass<GeminiClientError>()("GeminiClientError", {
  reason: S.String,
  failureClass: S.String,
  statusCode: S.optional(S.Number),
  receipt: S.optional(GeminiBackendFailureReceipt),
}) {}

export function makeGeminiClient(
  options: GeminiClientOptions = {},
): Effect.Effect<GeminiClient, ProbeBackendRegistryError | GeminiClientError> {
  return Effect.gen(function* () {
    const profile = yield* resolveGeminiBackendProfile(options);
    const apiKey = yield* resolveGeminiApiKey({ apiKey: options.apiKey, env: options.env, profileId: profile.id }).pipe(
      Effect.mapError(
        (error) =>
          new GeminiClientError({
            reason: error.reason,
            failureClass: "missing_credential",
          }),
      ),
    );
    const fetchImpl = options.fetch ?? fetch;
    const now = () => (options.now ?? new Date()).toISOString();

    return {
      profile,
      apiKey: {
        source: apiKey.source,
        redacted: true as const,
      },
      complete: (input) => completeGemini({ profile, apiKey, fetchImpl, input, now }),
    };
  });
}

function completeGemini(input: {
  readonly profile: ResolvedProbeBackendProfile;
  readonly apiKey: ResolvedGeminiApiKey;
  readonly fetchImpl: typeof fetch;
  readonly input: GeminiCompleteInput;
  readonly now: () => string;
}): Effect.Effect<GeminiCompleteResult, GeminiClientError> {
  return Effect.gen(function* () {
    const maxModelRoundTrips = input.input.maxModelRoundTrips ?? Infinity;
    let request = input.input.request;
    let events: ProbeLlmEvent[] = [];
    let toolReceipts: GeminiBackendToolCallReceipt[] = [];
    let roundTrips = 0;

    while (roundTrips < maxModelRoundTrips) {
      roundTrips += 1;
      const modelEvents = yield* callGemini(input.profile, input.apiKey, input.fetchImpl, request, input.input.onEvent);
      events = [...events, ...modelEvents];
      const toolCalls = modelEvents.filter(ProbeLlmEvents.isToolCall);

      if (toolCalls.length === 0) {
        return {
          profile: input.profile,
          events,
          text: collectText(events),
          finalRequest: request,
          roundTrips,
          receipt: makeGeminiTranscriptReceipt({
            profileId: input.profile.id,
            model: request.model.model,
            roundTrips,
            usage: [...events].reverse().find((event) => event.type === "finish")?.usage,
            observedAt: input.now(),
          }),
          toolReceipts,
        };
      }

      const toolResultParts = [];

      for (const call of toolCalls) {
        const dispatched = yield* dispatchProbeLlmTool(input.input.tools ?? {}, call);
        events = [...events, ...dispatched.events];
        emitGeminiEvents(dispatched.events, input.input.onEvent);
        toolReceipts = [
          ...toolReceipts,
          makeGeminiToolCallReceipt({
            profileId: input.profile.id,
            model: request.model.model,
            toolCallId: call.id,
            toolName: call.name,
            status: dispatched.result.type === "error" ? "error" : "success",
            observedAt: input.now(),
          }),
        ];
        toolResultParts.push(makeProbeLlmToolResult({ id: call.id, name: call.name, result: dispatched.result }));
      }

      request = makeProbeLlmRequest({
        ...request,
        toolChoice: request.toolChoice?.type === "tool" ? { type: "auto" as const } : request.toolChoice,
        messages: [
          ...request.messages,
          makeProbeLlmMessage(
            "assistant",
            toolCalls.map((call) => ({
              type: "tool-call" as const,
              id: call.id,
              name: call.name,
              input: call.input,
              providerMetadata: call.providerMetadata,
            })),
          ),
          makeProbeLlmMessage("tool", toolResultParts),
        ],
      });
    }

    return yield* Effect.fail(
      new GeminiClientError({
        reason: "Gemini tool-call round-trip limit reached",
        failureClass: "round_trip_limit",
        receipt: makeGeminiFailureReceipt({
          profileId: input.profile.id,
          model: request.model.model,
          baseUrl: input.profile.baseUrl,
          failureClass: "round_trip_limit",
          message: "Gemini tool-call round-trip limit reached",
          observedAt: input.now(),
        }),
      }),
    );
  });
}

function callGemini(
  profile: ResolvedProbeBackendProfile,
  apiKey: ResolvedGeminiApiKey,
  fetchImpl: typeof fetch,
  request: ProbeLlmRequest,
  onEvent?: (event: ProbeLlmEvent) => void,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, GeminiClientError> {
  return Effect.gen(function* () {
    const endpoint = new URL(`${withoutTrailingSlash(profile.baseUrl)}${geminiEndpointPath(request.model.model)}`);
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...makeGeminiAuthHeaders(apiKey),
          },
          body: JSON.stringify(lowerProbeLlmRequestToGeminiBody(request)),
        }),
      catch: (error) =>
        new GeminiClientError({
          reason: `Gemini request failed: ${String(error)}`,
          failureClass: "request_failed",
          receipt: makeGeminiFailureReceipt({
            profileId: profile.id,
            model: request.model.model,
            baseUrl: profile.baseUrl,
            failureClass: "request_failed",
            message: `Gemini request failed: ${String(error)}`,
          }),
        }),
    });

    if (!response.ok) {
      const rawText = yield* readGeminiResponseText(response, profile, request);

      return yield* Effect.fail(
        new GeminiClientError({
          reason: `Gemini returned HTTP ${response.status}${rawText.length === 0 ? "" : `: ${rawText.slice(0, 500)}`}`,
          failureClass: `http_${response.status}`,
          statusCode: response.status,
          receipt: makeGeminiFailureReceipt({
            profileId: profile.id,
            model: request.model.model,
            baseUrl: profile.baseUrl,
            failureClass: `http_${response.status}`,
            message: `Gemini returned HTTP ${response.status}`,
          }),
        }),
      );
    }

    const events = yield* parseGeminiResponseStream(response, profile, request, onEvent);

    return events;
  });
}

function parseGeminiResponseStream(
  response: Response,
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
  onEvent?: (event: ProbeLlmEvent) => void,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, GeminiClientError> {
  return Effect.gen(function* () {
    if (response.body === null) {
      return yield* Effect.fail(
        new GeminiClientError({
          reason: "Gemini response body was empty",
          failureClass: "malformed_response",
          receipt: makeGeminiFailureReceipt({
            profileId: profile.id,
            model: request.model.model,
            baseUrl: profile.baseUrl,
            failureClass: "malformed_response",
            message: "Gemini response body was empty",
          }),
        }),
      );
    }

    const events: ProbeLlmEvent[] = [ProbeLlmEvents.stepStart(0)];
    emitGeminiEvents(events, onEvent);
    const decoder = new TextDecoder();
    const reader = response.body.getReader();
    let state = makeGeminiSseParseState();
    let buffer = "";

    try {
      for (;;) {
        const chunk = yield* Effect.tryPromise({
          try: () => reader.read(),
          catch: (error) => makeMalformedGeminiResponseError(profile, request, `Gemini response could not be read: ${String(error)}`),
        });

        if (chunk.done) {
          break;
        }

        buffer += decoder.decode(chunk.value, { stream: true });
        const parsed = yield* drainGeminiSseBuffer(buffer, state).pipe(
          Effect.mapError((error) => makeMalformedGeminiResponseError(profile, request, error.reason)),
        );
        buffer = parsed.remaining;
        state = parsed.state;
        events.push(...parsed.events);
        emitGeminiEvents(parsed.events, onEvent);
      }

      buffer += decoder.decode();
      const parsed = yield* drainGeminiSseBuffer(buffer, state, true).pipe(
        Effect.mapError((error) => makeMalformedGeminiResponseError(profile, request, error.reason)),
      );
      state = parsed.state;
      events.push(...parsed.events);
      emitGeminiEvents(parsed.events, onEvent);
      const finishEvents = finishGeminiSseParseState(state);
      events.push(...finishEvents);
      emitGeminiEvents(finishEvents, onEvent);

      return events;
    } finally {
      reader.releaseLock();
    }
  });
}

function drainGeminiSseBuffer(
  input: string,
  state: ReturnType<typeof makeGeminiSseParseState>,
  flush = false,
) {
  return Effect.gen(function* () {
    let buffer = input;
    let nextState = state;
    const events: ProbeLlmEvent[] = [];

    for (;;) {
      const boundary = findSseEventBoundary(buffer);

      if (boundary === undefined) {
        break;
      }

      const rawEvent = buffer.slice(0, boundary.length);
      buffer = buffer.slice(boundary.nextIndex);
      const payload = readSsePayload(rawEvent);

      if (payload === undefined) {
        continue;
      }

      const parsed = yield* parseGeminiSsePayload(payload, nextState);
      nextState = parsed.state;
      events.push(...parsed.events);
    }

    if (flush && buffer.trim().length > 0) {
      const payload = readSsePayload(buffer);

      if (payload !== undefined) {
        const parsed = yield* parseGeminiSsePayload(payload, nextState);
        nextState = parsed.state;
        events.push(...parsed.events);
        buffer = "";
      }
    }

    return {
      events,
      remaining: buffer,
      state: nextState,
    };
  });
}

function findSseEventBoundary(input: string): { readonly length: number; readonly nextIndex: number } | undefined {
  const lf = input.indexOf("\n\n");
  const crlf = input.indexOf("\r\n\r\n");

  if (lf === -1 && crlf === -1) {
    return undefined;
  }

  if (crlf !== -1 && (lf === -1 || crlf < lf)) {
    return { length: crlf, nextIndex: crlf + 4 };
  }

  return { length: lf, nextIndex: lf + 2 };
}

function readSsePayload(rawEvent: string): string | undefined {
  const lines = rawEvent.split(/\r?\n/);
  const payload = lines
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trimStart())
    .filter((line) => line !== "[DONE]")
    .join("\n");

  return payload.length === 0 ? undefined : payload;
}

function readGeminiResponseText(
  response: Response,
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
): Effect.Effect<string, GeminiClientError> {
  return Effect.tryPromise({
    try: () => response.text(),
    catch: (error) =>
      makeMalformedGeminiResponseError(profile, request, `Gemini response could not be read: ${String(error)}`),
  });
}

function makeMalformedGeminiResponseError(
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
  message: string,
): GeminiClientError {
  return new GeminiClientError({
    reason: message,
    failureClass: "malformed_response",
    receipt: makeGeminiFailureReceipt({
      profileId: profile.id,
      model: request.model.model,
      baseUrl: profile.baseUrl,
      failureClass: "malformed_response",
      message,
    }),
  });
}

function emitGeminiEvents(events: ReadonlyArray<ProbeLlmEvent>, onEvent: GeminiCompleteInput["onEvent"]): void {
  if (onEvent === undefined) {
    return;
  }

  for (const event of events) {
    onEvent(event);
  }
}

function collectText(events: ReadonlyArray<ProbeLlmEvent>): string {
  return events.flatMap((event) => (event.type === "text-delta" ? [event.text] : [])).join("");
}

function withoutTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

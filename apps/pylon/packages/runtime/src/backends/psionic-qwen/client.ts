import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";
import { type ResolvedProbeBackendProfile, type ResolveProbeBackendProfileOptions } from "../backend-profile.js";
import { resolvePsionicQwenBackendProfile, type ProbeBackendRegistryError } from "../registry.js";
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
  PSIONIC_QWEN_SUPPORTED_ENDPOINT_REFS,
  PsionicQwenHealthResponse,
  PsionicQwenModelListResponse,
} from "./contract.js";
import {
  admitPsionicQwenModelRows,
  descriptorsFromPsionicModelList,
  modelRefFromModelId,
  selectPsionicQwenModel,
  type PsionicQwenModelAdmission,
  type PsionicQwenModelSelection,
} from "./model-admission.js";
import {
  finishPsionicOpenAiStreamState,
  lowerProbeLlmRequestToPsionicOpenAiBody,
  makePsionicOpenAiStreamState,
  parsePsionicOpenAiChatCompletion,
  parsePsionicOpenAiSsePayload,
  type PsionicOpenAiProtocolError,
} from "./protocol.js";
import {
  makePsionicQwenAvailabilityReceipt,
  makePsionicQwenFailureReceipt,
  makePsionicQwenToolCallReceipt,
  makePsionicQwenTranscriptReceipt,
  type PsionicQwenAvailabilityReceipt,
  PsionicQwenFailureReceipt,
  type PsionicQwenToolCallReceipt,
  type PsionicQwenTranscriptReceipt,
} from "./receipts.js";

export const PsionicQwenHealthStatus = S.Literals(["ready", "configured", "malformed", "unreachable"]);
export type PsionicQwenHealthStatus = typeof PsionicQwenHealthStatus.Type;

export interface PsionicQwenClientOptions extends ResolveProbeBackendProfileOptions {
  readonly fetch?: typeof fetch;
  readonly now?: Date;
}

export interface PsionicQwenReadiness {
  readonly profile: ResolvedProbeBackendProfile;
  readonly status: PsionicQwenHealthStatus;
  readonly ready: boolean;
  readonly health?: PsionicQwenHealthResponse;
  readonly modelIds: ReadonlyArray<string>;
  readonly modelRefs: ReadonlyArray<string>;
  readonly observedModelRefs: ReadonlyArray<string>;
  readonly modelAdmission?: PsionicQwenModelAdmission;
  readonly codingAgentSelection?: PsionicQwenModelSelection;
  readonly supportedEndpointRefs: ReadonlyArray<string>;
  readonly blockerRefs: ReadonlyArray<string>;
  readonly message?: string;
  readonly receipt: PsionicQwenAvailabilityReceipt;
}

export interface PsionicQwenClient {
  readonly profile: ResolvedProbeBackendProfile;
  readonly doctor: () => Effect.Effect<PsionicQwenReadiness, never>;
  readonly complete: (input: PsionicQwenCompleteInput) => Effect.Effect<PsionicQwenCompleteResult, PsionicQwenClientError>;
}

export interface PsionicQwenCompleteInput {
  readonly request: ProbeLlmRequest;
  readonly tools?: ProbeLlmTools;
  readonly maxModelRoundTrips?: number;
  readonly onEvent?: (event: ProbeLlmEvent) => void;
  readonly stream?: boolean;
}

export interface PsionicQwenCompleteResult {
  readonly profile: ResolvedProbeBackendProfile;
  readonly events: ReadonlyArray<ProbeLlmEvent>;
  readonly text: string;
  readonly finalRequest: ProbeLlmRequest;
  readonly roundTrips: number;
  readonly receipt: PsionicQwenTranscriptReceipt;
  readonly toolReceipts: ReadonlyArray<PsionicQwenToolCallReceipt>;
}

export class PsionicQwenClientError extends S.TaggedErrorClass<PsionicQwenClientError>()("PsionicQwenClientError", {
  reason: S.String,
  failureClass: S.String,
  statusCode: S.optional(S.Number),
  receipt: S.optional(PsionicQwenFailureReceipt),
}) {}

export function makePsionicQwenClient(
  options: PsionicQwenClientOptions = {},
): Effect.Effect<PsionicQwenClient, ProbeBackendRegistryError> {
  return Effect.gen(function* () {
    const profile = yield* resolvePsionicQwenBackendProfile(options);
    const fetchImpl = options.fetch ?? fetch;
    const now = () => (options.now ?? new Date()).toISOString();

    return {
      profile,
      doctor: () => checkPsionicQwenHealth(profile, fetchImpl, now()),
      complete: (input) => completePsionicQwen({ profile, fetchImpl, input, now }),
    };
  });
}

function completePsionicQwen(input: {
  readonly profile: ResolvedProbeBackendProfile;
  readonly fetchImpl: typeof fetch;
  readonly input: PsionicQwenCompleteInput;
  readonly now: () => string;
}): Effect.Effect<PsionicQwenCompleteResult, PsionicQwenClientError> {
  return Effect.gen(function* () {
    const maxModelRoundTrips = input.input.maxModelRoundTrips ?? Infinity;
    let request = input.input.request;
    let events: ProbeLlmEvent[] = [];
    let toolReceipts: PsionicQwenToolCallReceipt[] = [];
    let roundTrips = 0;

    while (roundTrips < maxModelRoundTrips) {
      roundTrips += 1;
      const modelEvents = yield* callPsionicQwen(input.profile, input.fetchImpl, request, input.input);
      events = [...events, ...modelEvents];
      const toolCalls = modelEvents.filter(ProbeLlmEvents.isToolCall);

      if (toolCalls.length === 0) {
        return {
          profile: input.profile,
          events,
          text: collectText(events),
          finalRequest: request,
          roundTrips,
          receipt: makePsionicQwenTranscriptReceipt({
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
        emitPsionicEvents(dispatched.events, input.input.onEvent);
        toolReceipts = [
          ...toolReceipts,
          makePsionicQwenToolCallReceipt({
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
      new PsionicQwenClientError({
        reason: "Psionic Qwen tool-call round-trip limit reached",
        failureClass: "round_trip_limit",
        receipt: makePsionicQwenFailureReceipt({
          profileId: input.profile.id,
          model: request.model.model,
          baseUrl: input.profile.baseUrl,
          failureClass: "round_trip_limit",
          message: "Psionic Qwen tool-call round-trip limit reached",
          observedAt: input.now(),
        }),
      }),
    );
  });
}

function callPsionicQwen(
  profile: ResolvedProbeBackendProfile,
  fetchImpl: typeof fetch,
  request: ProbeLlmRequest,
  input: PsionicQwenCompleteInput,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, PsionicQwenClientError> {
  return Effect.gen(function* () {
    const stream = input.stream === true || request.providerOptions?.psionic?.stream === true;
    const endpoint = new URL("/v1/chat/completions", withTrailingSlash(profile.baseUrl));
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(lowerProbeLlmRequestToPsionicOpenAiBody(request, { stream })),
        }),
      catch: (error) =>
        new PsionicQwenClientError({
          reason: `Psionic Qwen request failed: ${String(error)}`,
          failureClass: "request_failed",
          receipt: makePsionicQwenFailureReceipt({
            profileId: profile.id,
            model: request.model.model,
            baseUrl: profile.baseUrl,
            failureClass: "request_failed",
            message: `Psionic Qwen request failed: ${String(error)}`,
          }),
        }),
    });

    if (!response.ok) {
      const rawText = yield* readPsionicResponseText(response, profile, request);

      return yield* Effect.fail(
        new PsionicQwenClientError({
          reason: `Psionic Qwen returned HTTP ${response.status}${rawText.length === 0 ? "" : `: ${rawText.slice(0, 500)}`}`,
          failureClass: `http_${response.status}`,
          statusCode: response.status,
          receipt: makePsionicQwenFailureReceipt({
            profileId: profile.id,
            model: request.model.model,
            baseUrl: profile.baseUrl,
            failureClass: `http_${response.status}`,
            message: `Psionic Qwen returned HTTP ${response.status}`,
          }),
        }),
      );
    }

    const events = stream
      ? yield* parsePsionicResponseStream(response, profile, request, input.onEvent)
      : yield* parsePsionicJsonResponse(response, profile, request, input.onEvent);

    return events;
  });
}

function parsePsionicJsonResponse(
  response: Response,
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
  onEvent?: (event: ProbeLlmEvent) => void,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, PsionicQwenClientError> {
  return Effect.gen(function* () {
    const raw = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) => makeMalformedPsionicResponseError(profile, request, `Psionic Qwen response was not JSON: ${String(error)}`),
    });
    const events = yield* parsePsionicOpenAiChatCompletion(raw).pipe(
      Effect.mapError((error) => makePsionicProtocolClientError(profile, request, error)),
    );
    emitPsionicEvents(events, onEvent);

    return events;
  });
}

function parsePsionicResponseStream(
  response: Response,
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
  onEvent?: (event: ProbeLlmEvent) => void,
): Effect.Effect<ReadonlyArray<ProbeLlmEvent>, PsionicQwenClientError> {
  return Effect.gen(function* () {
    if (response.body === null) {
      return yield* Effect.fail(makeMalformedPsionicResponseError(profile, request, "Psionic Qwen response body was empty"));
    }

    const events: ProbeLlmEvent[] = [ProbeLlmEvents.stepStart(0)];
    emitPsionicEvents(events, onEvent);
    const decoder = new TextDecoder();
    const reader = response.body.getReader();
    let state = makePsionicOpenAiStreamState();
    let buffer = "";

    try {
      for (;;) {
        const chunk = yield* Effect.tryPromise({
          try: () => reader.read(),
          catch: (error) => makeMalformedPsionicResponseError(profile, request, `Psionic Qwen response could not be read: ${String(error)}`),
        });

        if (chunk.done) {
          break;
        }

        buffer += decoder.decode(chunk.value, { stream: true });
        const parsed = yield* drainPsionicSseBuffer(buffer, state).pipe(
          Effect.mapError((error) => makePsionicProtocolClientError(profile, request, error)),
        );
        buffer = parsed.remaining;
        state = parsed.state;
        events.push(...parsed.events);
        emitPsionicEvents(parsed.events, onEvent);
      }

      buffer += decoder.decode();
      const parsed = yield* drainPsionicSseBuffer(buffer, state, true).pipe(
        Effect.mapError((error) => makePsionicProtocolClientError(profile, request, error)),
      );
      state = parsed.state;
      events.push(...parsed.events);
      emitPsionicEvents(parsed.events, onEvent);
      const finishEvents = yield* finishPsionicOpenAiStreamState(state).pipe(
        Effect.mapError((error) => makePsionicProtocolClientError(profile, request, error)),
      );
      events.push(...finishEvents);
      emitPsionicEvents(finishEvents, onEvent);

      return events;
    } finally {
      reader.releaseLock();
    }
  });
}

export function checkPsionicQwenHealth(
  profile: ResolvedProbeBackendProfile,
  fetchImpl: typeof fetch = fetch,
  observedAt = new Date().toISOString(),
): Effect.Effect<PsionicQwenReadiness, never> {
  return Effect.gen(function* () {
    const healthEndpoint = new URL(profile.readinessPath, withTrailingSlash(profile.baseUrl));
    const healthResponse = yield* Effect.tryPromise({
      try: () => fetchImpl(healthEndpoint, { method: "GET" }),
      catch: (error) =>
        psionicReadiness(profile, {
          status: "unreachable",
          blockerRefs: ["blocker.psionic_qwen35.health_unreachable"],
          message: `Psionic health endpoint is unreachable: ${String(error)}`,
          observedAt,
        }),
    });

    if (isReadiness(healthResponse)) {
      return healthResponse;
    }

    if (!healthResponse.ok) {
      return psionicReadiness(profile, {
        status: "unreachable",
        blockerRefs: ["blocker.psionic_qwen35.health_unreachable"],
        message: `Psionic health endpoint returned HTTP ${healthResponse.status}`,
        observedAt,
      });
    }

    const healthRaw = yield* Effect.tryPromise({
      try: () => healthResponse.json(),
      catch: (error) =>
        psionicReadiness(profile, {
          status: "malformed",
          blockerRefs: ["blocker.psionic_qwen35.health_unreachable"],
          message: `Psionic health response was not JSON: ${String(error)}`,
          observedAt,
        }),
    });

    if (isReadiness(healthRaw)) {
      return healthRaw;
    }

    const health = yield* S.decodeUnknownEffect(PsionicQwenHealthResponse)(normalizeHealthResponse(healthRaw)).pipe(
      Effect.mapError((error) =>
        psionicReadiness(profile, {
          status: "malformed",
          blockerRefs: ["blocker.psionic_qwen35.health_unreachable"],
          message: `Psionic health response was malformed: ${String(error)}`,
          observedAt,
        }),
      ),
    );

    if (isReadiness(health)) {
      return health;
    }

    const modelsResponse = yield* Effect.tryPromise({
      try: () => fetchImpl(new URL("/v1/models", withTrailingSlash(profile.baseUrl)), { method: "GET" }),
      catch: (error) =>
        psionicReadiness(profile, {
          status: "unreachable",
          health,
          blockerRefs: ["blocker.psionic_qwen35.qwen35_model_missing"],
          supportedEndpointRefs: endpointRefsFromHealth(health),
          message: `Psionic model endpoint is unreachable: ${String(error)}`,
          observedAt,
        }),
    });

    if (isReadiness(modelsResponse)) {
      return modelsResponse;
    }

    if (!modelsResponse.ok) {
      return psionicReadiness(profile, {
        status: "configured",
        health,
        blockerRefs: ["blocker.psionic_qwen35.qwen35_model_missing"],
        supportedEndpointRefs: endpointRefsFromHealth(health),
        message: `Psionic model endpoint returned HTTP ${modelsResponse.status}`,
        observedAt,
      });
    }

    const modelsRaw = yield* Effect.tryPromise({
      try: () => modelsResponse.json(),
      catch: (error) =>
        psionicReadiness(profile, {
          status: "malformed",
          health,
          blockerRefs: ["blocker.psionic_qwen35.qwen35_model_missing"],
          supportedEndpointRefs: endpointRefsFromHealth(health),
          message: `Psionic model response was not JSON: ${String(error)}`,
          observedAt,
        }),
    });

    if (isReadiness(modelsRaw)) {
      return modelsRaw;
    }

    const models = yield* S.decodeUnknownEffect(PsionicQwenModelListResponse)(normalizeModelListResponse(modelsRaw)).pipe(
      Effect.mapError((error) =>
        psionicReadiness(profile, {
          status: "malformed",
          health,
          blockerRefs: ["blocker.psionic_qwen35.qwen35_model_missing"],
          supportedEndpointRefs: endpointRefsFromHealth(health),
          message: `Psionic model response was malformed: ${String(error)}`,
          observedAt,
        }),
      ),
    );

    if (isReadiness(models)) {
      return models;
    }

    const descriptors = descriptorsFromPsionicModelList(models);
    const modelIds = uniqueStrings([...modelIdsFromHealth(health), ...descriptors.map((descriptor) => descriptor.id)])
      .map(projectModelId);
    const modelAdmission = admitPsionicQwenModelRows(descriptors);
    const modelRefs = modelAdmission.admittedModelRefs;
    const codingAgentSelection = selectPsionicQwenModel(modelAdmission, "coding_agent");
    const supportedEndpointRefs = endpointRefsFromHealth(health);
    const blockerRefs = psionicBlockers(health, modelRefs, modelAdmission.blockerRefs);

    return psionicReadiness(profile, {
      status: blockerRefs.length === 0 ? "ready" : "configured",
      health,
      modelIds,
      modelRefs,
      observedModelRefs: modelAdmission.observedModelRefs,
      modelAdmission,
      codingAgentSelection,
      supportedEndpointRefs,
      blockerRefs,
      message: blockerRefs.length === 0 ? health.message : blockerRefs.join(", "),
      observedAt,
    });
  }).pipe(Effect.catch((readiness: PsionicQwenReadiness) => Effect.succeed(readiness)));
}

function psionicReadiness(
  profile: ResolvedProbeBackendProfile,
  input: {
    readonly status: PsionicQwenHealthStatus;
    readonly health?: PsionicQwenHealthResponse;
    readonly modelIds?: ReadonlyArray<string>;
    readonly modelRefs?: ReadonlyArray<string>;
    readonly observedModelRefs?: ReadonlyArray<string>;
    readonly modelAdmission?: PsionicQwenModelAdmission;
    readonly codingAgentSelection?: PsionicQwenModelSelection;
    readonly supportedEndpointRefs?: ReadonlyArray<string>;
    readonly blockerRefs?: ReadonlyArray<string>;
    readonly message?: string;
    readonly observedAt: string;
  },
): PsionicQwenReadiness {
  const modelIds = input.modelIds ?? [];
  const modelRefs = input.modelRefs ?? [];
  const observedModelRefs = input.observedModelRefs ?? [];
  const supportedEndpointRefs = input.supportedEndpointRefs ?? [];
  const blockerRefs = input.blockerRefs ?? [];
  const ready = input.status === "ready" && blockerRefs.length === 0;

  return {
    profile,
    status: input.status,
    ready,
    health: input.health,
    modelIds,
    modelRefs,
    observedModelRefs,
    modelAdmission: input.modelAdmission,
    codingAgentSelection: input.codingAgentSelection,
    supportedEndpointRefs,
    blockerRefs,
    message: input.message,
    receipt: makePsionicQwenAvailabilityReceipt({
      profileId: profile.id,
      model: profile.model,
      baseUrl: profile.baseUrl,
      ready,
      status: input.status,
      modelRefs,
      supportedEndpointRefs,
      blockerRefs,
      message: input.message,
      observedAt: input.observedAt,
    }),
  };
}

function psionicBlockers(
  health: PsionicQwenHealthResponse,
  modelRefs: ReadonlyArray<string>,
  modelAdmissionBlockers: ReadonlyArray<string> = [],
): ReadonlyArray<string> {
  const blockers: string[] = [];
  const engine = health.execution_engine ?? health.executionEngine ?? health.backend;

  if (engine !== undefined && engine.toLowerCase() !== "psionic") {
    blockers.push("blocker.psionic_qwen35.execution_engine_not_psionic");
  }

  blockers.push(...modelAdmissionBlockers);

  if (modelRefs.length === 0) {
    blockers.push("blocker.psionic_qwen35.qwen35_model_missing");
  }

  return [...new Set(blockers)];
}

function endpointRefsFromHealth(health: PsionicQwenHealthResponse): ReadonlyArray<string> {
  const endpoints = uniqueStrings([...(health.supported_endpoints ?? []), ...(health.supportedEndpoints ?? [])])
    .map((value) => value.toLowerCase());
  const refs = new Set<string>([PSIONIC_QWEN_SUPPORTED_ENDPOINT_REFS.health, PSIONIC_QWEN_SUPPORTED_ENDPOINT_REFS.models]);

  if (endpoints.some((endpoint) => endpoint.includes("chat/completions") || endpoint.includes("chat_completions"))) {
    refs.add(PSIONIC_QWEN_SUPPORTED_ENDPOINT_REFS.chatCompletions);
  }

  if (endpoints.some((endpoint) => endpoint.includes("responses"))) {
    refs.add(PSIONIC_QWEN_SUPPORTED_ENDPOINT_REFS.responses);
  }

  return [...refs];
}

function modelIdsFromHealth(health: PsionicQwenHealthResponse): ReadonlyArray<string> {
  return uniqueStrings([
    health.default_model,
    health.defaultModel,
    health.model,
    ...(health.models ?? []),
  ].filter(isString));
}

function projectModelId(modelId: string): string {
  if (modelId.includes("/") || modelId.includes("\\") || modelId.includes("~")) {
    return `model_id.redacted.${createHash("sha256").update(modelId).digest("hex").slice(0, 12)}`;
  }

  return modelId;
}

function normalizeHealthResponse(value: unknown): unknown {
  if (typeof value !== "object" || value === null) {
    return value;
  }

  return value;
}

function normalizeModelListResponse(value: unknown): unknown {
  if (typeof value !== "object" || value === null) {
    return value;
  }

  const input = value as Record<string, unknown>;
  const data = Array.isArray(input.data) ? input.data : [];

  return {
    object: typeof input.object === "string" ? input.object : undefined,
    data,
  };
}

function uniqueStrings(values: ReadonlyArray<string>): ReadonlyArray<string> {
  return [...new Set(values.filter((value) => value.trim().length > 0))];
}

function isString(value: unknown): value is string {
  return typeof value === "string";
}

function isReadiness(value: unknown): value is PsionicQwenReadiness {
  return typeof value === "object" && value !== null && "profile" in value && "receipt" in value && "status" in value;
}

function drainPsionicSseBuffer(
  input: string,
  state: ReturnType<typeof makePsionicOpenAiStreamState>,
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

      const parsed = yield* parsePsionicOpenAiSsePayload(payload, nextState);
      nextState = parsed.state;
      events.push(...parsed.events);
    }

    if (flush && buffer.trim().length > 0) {
      const payload = readSsePayload(buffer);

      if (payload !== undefined) {
        const parsed = yield* parsePsionicOpenAiSsePayload(payload, nextState);
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

function readPsionicResponseText(
  response: Response,
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
): Effect.Effect<string, PsionicQwenClientError> {
  return Effect.tryPromise({
    try: () => response.text(),
    catch: (error) =>
      makeMalformedPsionicResponseError(profile, request, `Psionic Qwen response could not be read: ${String(error)}`),
  });
}

function makePsionicProtocolClientError(
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
  error: PsionicOpenAiProtocolError,
): PsionicQwenClientError {
  return new PsionicQwenClientError({
    reason: error.reason,
    failureClass: error.failureClass,
    receipt: makePsionicQwenFailureReceipt({
      profileId: profile.id,
      model: request.model.model,
      baseUrl: profile.baseUrl,
      failureClass: error.failureClass,
      message: error.reason,
    }),
  });
}

function makeMalformedPsionicResponseError(
  profile: ResolvedProbeBackendProfile,
  request: ProbeLlmRequest,
  message: string,
): PsionicQwenClientError {
  return new PsionicQwenClientError({
    reason: message,
    failureClass: "malformed_response",
    receipt: makePsionicQwenFailureReceipt({
      profileId: profile.id,
      model: request.model.model,
      baseUrl: profile.baseUrl,
      failureClass: "malformed_response",
      message,
    }),
  });
}

function emitPsionicEvents(events: ReadonlyArray<ProbeLlmEvent>, onEvent: PsionicQwenCompleteInput["onEvent"]): void {
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

function withTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}

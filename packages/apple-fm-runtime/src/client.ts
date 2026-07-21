import { Effect, Schema as S } from "effect";

import { APPLE_FM_DEFAULT_BASE_URL, APPLE_FM_DEFAULT_MODEL_ID, APPLE_FM_LOCAL_PROFILE_ID } from "./identity.js";
import {
  AppleFmChatCompletionResponse,
  AppleFmHealthResponse,
  type AppleFmChatMessage,
  type AppleFmUnavailableReason,
  type AppleFmUsageMeasurement,
  type AppleFmUsageTruth,
} from "./wire.js";

/**
 * `@openagentsinc/apple-fm-runtime` portable loopback client (AFS-02).
 *
 * A self-contained `fetch`-based client for the Swift `foundation-bridge`
 * loopback HTTP contract. It replaces the Desktop dependency on the nested
 * Pylon runtime's `makeAppleFmClient`: it carries only the health probe and one
 * bounded read-only completion, with NO Pylon receipts, workspace tools,
 * Blueprint tools, fleet data, or registry. It is portable (uses only global
 * `fetch`, Effect, and the wire schemas), so it lives in the root export and
 * both Desktop and Pylon can consume it through thin adapters.
 *
 * Every response is decoded and normalized at this boundary. Neither function
 * throws: a transport, HTTP, or shape failure maps to a bounded, public-safe
 * result. No helper path, loopback URL, token, or raw transport detail crosses
 * out of this module.
 */

/** Health-derived readiness status (mirrors the frozen consumer contract). */
export type AppleFmReadinessStatus = "ready" | "unavailable" | "unsupported" | "malformed" | "unreachable";

/** A live readiness probe, health-derived, bounded and public-safe. */
export interface AppleFmProbe {
  readonly status: AppleFmReadinessStatus;
  readonly ready: boolean;
  readonly model?: string;
  readonly profileId?: string;
  readonly usageTruth?: AppleFmUsageTruth;
  readonly unavailableReason?: string;
}

/** One bounded read-only completion outcome. */
export interface AppleFmCompletionTurn {
  readonly outcome: "completed" | "failed";
  readonly text?: string;
  readonly usageTruth: AppleFmUsageTruth;
  readonly promptTokens?: number;
  readonly completionTokens?: number;
  readonly totalTokens?: number;
  readonly failureClass?: string;
}

export interface AppleFmBridgeClientOptions {
  readonly baseUrl?: string;
  readonly model?: string;
  readonly profileId?: string;
  readonly fetch?: typeof fetch;
}

const MAX_TEXT = 8192 as const;

const withTrailingSlash = (value: string): string => (value.endsWith("/") ? value : `${value}/`);

const boundedToken = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9_.]+/g, "_").replace(/^_+|_+$/g, "").slice(0, MAX_TEXT) || "unknown";

const decodeHealth = S.decodeUnknownExit(AppleFmHealthResponse);
const decodeCompletion = S.decodeUnknownExit(AppleFmChatCompletionResponse);

const asRecord = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>) : undefined;

const stringField = (value: unknown, key: string): string | undefined => {
  const record = asRecord(value);
  const field = record?.[key];
  return typeof field === "string" ? field : undefined;
};

const numberField = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;

const booleanField = (value: unknown): boolean | undefined =>
  typeof value === "boolean" ? value : undefined;

const normalizeUnavailableReason = (value: unknown): AppleFmUnavailableReason | undefined => {
  if (value === "apple_intelligence_not_enabled" || value === "appleIntelligenceNotEnabled") {
    return "apple_intelligence_disabled";
  }
  if (value === "device_not_eligible" || value === "deviceNotEligible") return "unsupported_hardware";
  if (value === "model_not_ready" || value === "modelNotReady") return "model_unavailable";
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
};

const statusFromReason = (reason: AppleFmUnavailableReason | undefined): AppleFmReadinessStatus => {
  if (reason === "unsupported_hardware" || reason === "apple_intelligence_disabled") return "unsupported";
  if (reason === "malformed_response") return "malformed";
  if (reason === "bridge_unreachable") return "unreachable";
  return "unavailable";
};

const normalizeHealth = (value: unknown, fallbackModel: string): unknown => {
  const record = asRecord(value);
  if (record === undefined) return value;
  const ready =
    booleanField(record.ready) ??
    booleanField(record.modelAvailable) ??
    booleanField(record.model_available) ??
    record.status === "ok";
  return {
    ready,
    model: stringField(record, "model"),
    modelId: stringField(record, "modelId") ?? stringField(record, "model_id") ?? fallbackModel,
    unavailableReason: normalizeUnavailableReason(record.unavailableReason ?? record.unavailable_reason),
    message: stringField(record, "message"),
    platform: stringField(record, "platform"),
    version: stringField(record, "version"),
  };
};

const normalizeUsage = (value: unknown): AppleFmUsageMeasurement => {
  const record = asRecord(value);
  if (record === undefined) return { truth: "unknown" };
  const promptTokens = numberField(record.promptTokens) ?? numberField(record.prompt_tokens);
  const completionTokens = numberField(record.completionTokens) ?? numberField(record.completion_tokens);
  const totalTokens = numberField(record.totalTokens) ?? numberField(record.total_tokens);
  const hasTokenCounts =
    promptTokens !== undefined || completionTokens !== undefined || totalTokens !== undefined;
  const declaredTruth = record.truth;
  const truth: AppleFmUsageTruth =
    declaredTruth === "exact" || declaredTruth === "estimated" || declaredTruth === "unknown"
      ? declaredTruth
      : hasTokenCounts
        ? "estimated"
        : "unknown";
  const usage: { truth: AppleFmUsageTruth; promptTokens?: number; completionTokens?: number; totalTokens?: number } = {
    truth,
  };
  if (promptTokens !== undefined) usage.promptTokens = promptTokens;
  if (completionTokens !== undefined) usage.completionTokens = completionTokens;
  if (totalTokens !== undefined) usage.totalTokens = totalTokens;
  return usage;
};

const normalizeCompletion = (value: unknown, fallbackModel: string): unknown => {
  const record = asRecord(value);
  if (record === undefined) return value;
  const rawChoices = Array.isArray(record.choices) ? record.choices : [];
  const choices = rawChoices.map((choice) => {
    const choiceRecord = asRecord(choice) ?? {};
    const message = asRecord(choiceRecord.message) ?? {};
    const role = message.role;
    return {
      index: numberField(choiceRecord.index),
      message: {
        role: role === "system" || role === "user" || role === "assistant" || role === "tool" ? role : "assistant",
        content: typeof message.content === "string" ? message.content : "",
      },
      finishReason: normalizeFinishReason(choiceRecord.finishReason ?? choiceRecord.finish_reason),
    };
  });
  return {
    id: stringField(record, "id"),
    model: stringField(record, "model") ?? fallbackModel,
    choices,
    usage: normalizeUsage(record.usage),
  };
};

const normalizeFinishReason = (value: unknown): string => {
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
};

const unreachableProbe: AppleFmProbe = {
  status: "unreachable",
  ready: false,
  unavailableReason: "bridge_unreachable",
};

/**
 * Probe live readiness through the loopback health endpoint. Never throws: a
 * transport, HTTP, or shape failure maps to a bounded probe.
 */
export const appleFmProbe = (
  baseUrl: string = APPLE_FM_DEFAULT_BASE_URL,
  fetchImpl: typeof fetch = fetch,
  profileId: string = APPLE_FM_LOCAL_PROFILE_ID,
): Promise<AppleFmProbe> => {
  const effect = Effect.gen(function* () {
    const endpoint = new URL("/health", withTrailingSlash(baseUrl));
    const response = yield* Effect.tryPromise({
      try: () => fetchImpl(endpoint, { method: "GET" }),
      catch: () => "unreachable" as const,
    });
    if (!response.ok) {
      return {
        status: "unavailable" as const,
        ready: false,
        profileId,
        unavailableReason: "not_ready",
      } satisfies AppleFmProbe;
    }
    const raw = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: () => "malformed" as const,
    });
    const decoded = decodeHealth(normalizeHealth(raw, APPLE_FM_DEFAULT_MODEL_ID));
    if (decoded._tag === "Failure") {
      return {
        status: "malformed" as const,
        ready: false,
        profileId,
        unavailableReason: "malformed_response",
      } satisfies AppleFmProbe;
    }
    const health = decoded.value;
    const ready = health.ready === true;
    const reason = health.unavailableReason;
    const status: AppleFmReadinessStatus = ready ? "ready" : statusFromReason(reason);
    const model = health.modelId ?? health.model;
    const probe: AppleFmProbe = {
      status,
      ready,
      profileId,
      usageTruth: ready ? "estimated" : "unknown",
    };
    return {
      ...probe,
      ...(model !== undefined ? { model } : {}),
      ...(reason !== undefined ? { unavailableReason: reason } : {}),
    } satisfies AppleFmProbe;
  });
  return Effect.runPromise(
    effect.pipe(
      Effect.catch((tag: "unreachable" | "malformed") =>
        Effect.succeed(
          tag === "malformed"
            ? ({ status: "malformed", ready: false, profileId, unavailableReason: "malformed_response" } satisfies AppleFmProbe)
            : unreachableProbe,
        ),
      ),
    ),
  );
};

/**
 * Run one bounded read-only completion through the loopback completion
 * endpoint. Never throws: a transport, HTTP, empty, or malformed result maps to
 * a bounded failed turn with a public-safe failure class.
 */
export const appleFmComplete = (
  baseUrl: string = APPLE_FM_DEFAULT_BASE_URL,
  prompt: string,
  fetchImpl: typeof fetch = fetch,
  model: string = APPLE_FM_DEFAULT_MODEL_ID,
  /**
   * When set to a non-empty candidate set, the bridge runs GUIDED generation
   * (constrained sampling): the model must pick exactly one candidate from this
   * set and the bridge returns a well-formed route-recommendation JSON. Omit for
   * a normal free-text completion. (Owner directive 2026-07-20: on-device router.)
   */
  routeCandidates?: ReadonlyArray<string>,
): Promise<AppleFmCompletionTurn> => {
  const effect = Effect.gen(function* () {
    const endpoint = new URL("/v1/chat/completions", withTrailingSlash(baseUrl));
    const messages: ReadonlyArray<AppleFmChatMessage> = [{ role: "user", content: prompt }];
    const route =
      routeCandidates !== undefined && routeCandidates.length > 0
        ? { route: { candidates: routeCandidates } }
        : {};
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ model, messages, ...route }),
        }),
      catch: () => "bridge_unreachable" as const,
    });
    const raw = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: () => "malformed_response" as const,
    });
    if (!response.ok) return yield* Effect.fail("completion_http_error" as const);
    const decoded = decodeCompletion(normalizeCompletion(raw, model));
    if (decoded._tag === "Failure") return yield* Effect.fail("malformed_response" as const);
    const choice = decoded.value.choices[0];
    if (choice === undefined || choice.message.content.length === 0) {
      return yield* Effect.fail("empty_completion" as const);
    }
    const usage = decoded.value.usage ?? { truth: "unknown" as const };
    const turn: AppleFmCompletionTurn = {
      outcome: "completed",
      text: choice.message.content.slice(0, MAX_TEXT),
      usageTruth: usage.truth,
    };
    return {
      ...turn,
      ...(usage.promptTokens !== undefined ? { promptTokens: usage.promptTokens } : {}),
      ...(usage.completionTokens !== undefined ? { completionTokens: usage.completionTokens } : {}),
      ...(usage.totalTokens !== undefined ? { totalTokens: usage.totalTokens } : {}),
    } satisfies AppleFmCompletionTurn;
  });
  return Effect.runPromise(
    effect.pipe(
      Effect.catch((failureClass: "bridge_unreachable" | "malformed_response" | "completion_http_error" | "empty_completion") =>
        Effect.succeed({
          outcome: "failed" as const,
          usageTruth: "unknown" as const,
          failureClass: boundedToken(failureClass),
        } satisfies AppleFmCompletionTurn),
      ),
    ),
  );
};

/** A small client object over the two bounded operations. */
export interface AppleFmBridgeClient {
  readonly baseUrl: string;
  readonly model: string;
  readonly profileId: string;
  readonly health: () => Promise<AppleFmProbe>;
  readonly completePlainText: (prompt: string) => Promise<AppleFmCompletionTurn>;
}

export const makeAppleFmBridgeClient = (options: AppleFmBridgeClientOptions = {}): AppleFmBridgeClient => {
  const baseUrl = options.baseUrl ?? APPLE_FM_DEFAULT_BASE_URL;
  const model = options.model ?? APPLE_FM_DEFAULT_MODEL_ID;
  const profileId = options.profileId ?? APPLE_FM_LOCAL_PROFILE_ID;
  const fetchImpl = options.fetch ?? fetch;
  return {
    baseUrl,
    model,
    profileId,
    health: () => appleFmProbe(baseUrl, fetchImpl, profileId),
    completePlainText: (prompt) => appleFmComplete(baseUrl, prompt, fetchImpl, model),
  };
};

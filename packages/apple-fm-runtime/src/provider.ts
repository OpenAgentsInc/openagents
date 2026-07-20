import { Effect, Layer, Schema as S, Stream } from "effect";

import {
  APPLE_FM_DATA_DESTINATION,
  APPLE_FM_DEFAULT_MODEL_ID,
  APPLE_FM_TURN_CANDIDATE,
} from "./identity.js";
import type { AppleFmCompletionTurn, AppleFmProbe } from "./client.js";
import { decodeAppleFmRouteOutput, type AppleFmRouteDecodeResult } from "./recommendation.js";

import {
  CANDIDATE_SCHEMA_LITERAL,
  CandidateRef,
  MAX_TURN_CONTEXT_CHARS,
  MAX_TURN_OUTPUT_CHARS,
  PROVIDER_SCHEMA_LITERAL,
  ProviderTurnRef,
  TurnProviderRef,
  turnIntentTaskClass,
  type AnswerCandidate,
  type InferenceProviderDescriptor,
  type ProviderReadiness,
  type TurnCandidate,
  type TurnIntent,
  type TurnProviderCandidate,
  type TurnRefusalReason,
} from "@openagentsinc/agent-runtime-schema";
import {
  ProviderRegistry,
  ProviderStartError,
  ProviderStreamEvent,
  type ProviderRegistryInterface,
  type ProviderRun,
  type ProviderStartInput,
} from "@openagentsinc/agent-turn-runtime";

/**
 * `@openagentsinc/apple-fm-runtime` inference provider adapter (AFS-02).
 *
 * Apple FM becomes a local advisory inference provider under the shared turn
 * kernel: this adapter implements the AFS-01 `ProviderRegistry`
 * (InferenceProviderRegistry) port. It declares Apple FM's supported intent and
 * candidate kinds, its on-device data destination, and its NO-external-action
 * capability; it maps helper readiness into provider readiness with no renderer
 * input; and it converts one bounded text completion into an `AnswerCandidate`.
 *
 * `agent-turn-runtime` must NOT import this adapter; the dependency is the other
 * way (Apple FM implements the port). The turn kernel drives it through the
 * shared `ProviderRegistry` tag.
 */

/** Readiness snapshot the host derives from main-owned helper state. */
export interface AppleFmReadinessSnapshot {
  readonly ready: boolean;
  readonly unavailableReason?: string;
}

export interface AppleFmProviderConfig {
  /** The concrete selectable Apple FM lane instance. */
  readonly providerRef: string;
  readonly model?: string;
  /** Current helper readiness, derived from main-owned state (never the renderer). */
  readiness: () => AppleFmReadinessSnapshot | Promise<AppleFmReadinessSnapshot>;
  /** Run one bounded read-only completion. */
  complete: (prompt: string) => Promise<AppleFmCompletionTurn>;
  readonly now?: () => number;
  /** Deterministic turn/candidate id suffix source (tests inject a counter). */
  readonly nextId?: () => string;
  readonly maxOutputChars?: number;
}

const decodeProviderRef = S.decodeUnknownSync(TurnProviderRef);
const decodeProviderTurnRef = S.decodeUnknownSync(ProviderTurnRef);
const decodeCandidateRef = S.decodeUnknownSync(CandidateRef);

const readinessProviderMap = (reason: string | undefined): ProviderReadiness => {
  const known = [
    "bridge_unreachable",
    "apple_intelligence_disabled",
    "unsupported_hardware",
    "model_unavailable",
    "permission_denied",
    "malformed_response",
    "not_ready",
    "account_missing",
    "account_unhealthy",
    "unknown",
  ] as const;
  const mapped = (known as ReadonlyArray<string>).includes(reason ?? "") ? (reason as (typeof known)[number]) : "not_ready";
  return { state: "unavailable", reason: mapped };
};

const startErrorReason = (reason: string | undefined): ProviderStartError["reason"] => {
  if (reason === "not_ready") return "not_ready";
  if (reason === "helper_missing") return "helper_missing";
  return "unavailable";
};

/** Text-bearing intents Apple FM can answer. */
const intentPrompt = (intent: TurnIntent): string | null => {
  switch (intent._tag) {
    case "Ask":
      return intent.text;
    case "ProposeEdit":
      return intent.instruction;
    case "RecommendRoute":
      return intent.objective;
    default:
      return null;
  }
};

/** Build the current Apple FM descriptor, mapping helper readiness → provider readiness. */
export const makeAppleFmDescriptor = (input: {
  readonly providerRef: string;
  readonly model?: string;
  readonly readiness: AppleFmReadinessSnapshot;
}): InferenceProviderDescriptor => {
  const readiness: ProviderReadiness = input.readiness.ready
    ? { state: "ready" }
    : readinessProviderMap(input.readiness.unavailableReason);
  return {
    schema: PROVIDER_SCHEMA_LITERAL,
    providerRef: decodeProviderRef(input.providerRef),
    candidate: APPLE_FM_TURN_CANDIDATE,
    model: input.model ?? APPLE_FM_DEFAULT_MODEL_ID,
    placement: "owner_local",
    supportedIntents: ["Ask"],
    supportedCandidateKinds: ["answer"],
    dataDestination: APPLE_FM_DATA_DESTINATION,
    // Local inference reports estimated usage from character counts.
    usageTruth: "estimated",
    costClass: "local_resource_only",
    maxContextChars: MAX_TURN_CONTEXT_CHARS,
    maxOutputChars: MAX_TURN_OUTPUT_CHARS,
    supportsStreaming: false,
    supportsCancellation: true,
    supportsExternalTools: false,
    supportsExternalActions: false,
    readiness,
  };
};

/** Structured action-claim guard reused for the answer path (never dispatch an action-claim). */
const answerClaimsAction = (raw: string): boolean => {
  const trimmed = raw.trim();
  if (!trimmed.startsWith("{")) return false;
  const decoded = decodeAppleFmRouteOutput({ raw, admittedCandidates: [] });
  return decoded._tag === "Reject" && decoded.reason === "action_claim_rejected";
};

const answerRefusal = (raw: string, maxOutputChars: number): TurnRefusalReason | null => {
  const trimmed = raw.trim();
  if (trimmed.length === 0) return "empty_output";
  if (raw.length > maxOutputChars) return "oversized_output";
  if (answerClaimsAction(raw)) return "action_claim_rejected";
  return null;
};

/**
 * Convert a completion turn into a terminal provider-stream event: a validated
 * `AnswerCandidate`, or a typed refusal. Records model/usage/latency provenance
 * without exposing any helper secret.
 */
const completionToEvent = (input: {
  readonly turn: AppleFmCompletionTurn;
  readonly intent: TurnIntent;
  readonly providerRef: string;
  readonly model: string;
  readonly candidateRefSeed: string;
  readonly latencyMs: number;
  readonly maxOutputChars: number;
}): ProviderStreamEvent => {
  if (input.turn.outcome === "failed") {
    return ProviderStreamEvent.Failed({ detail: input.turn.failureClass ?? "apple_fm_failed" });
  }
  const text = input.turn.text ?? "";
  const refusal = answerRefusal(text, input.maxOutputChars);
  if (refusal !== null) return ProviderStreamEvent.Refused({ reason: refusal });
  const candidate: AnswerCandidate = {
    schema: CANDIDATE_SCHEMA_LITERAL,
    kind: "answer",
    candidateRef: decodeCandidateRef(`candidate.apple_fm.${input.candidateRefSeed}`),
    provenance: {
      providerRef: decodeProviderRef(input.providerRef),
      candidate: APPLE_FM_TURN_CANDIDATE,
      model: input.model,
      taskClass: turnIntentTaskClass[input.intent._tag],
      usageTruth: input.turn.usageTruth,
      dataDestination: APPLE_FM_DATA_DESTINATION,
      latencyMs: Math.max(0, input.latencyMs),
      stale: false,
    },
    text: text.slice(0, input.maxOutputChars),
  };
  const turnCandidate: TurnCandidate = candidate;
  return ProviderStreamEvent.Completed({ candidate: turnCandidate });
};

/** Create the Apple FM `ProviderRegistry` interface for one owner-local lane. */
export const makeAppleFmProviderRegistry = (config: AppleFmProviderConfig): ProviderRegistryInterface => {
  const model = config.model ?? APPLE_FM_DEFAULT_MODEL_ID;
  const now = config.now ?? (() => Date.now());
  const maxOutputChars = config.maxOutputChars ?? MAX_TURN_OUTPUT_CHARS;
  let counter = 0;
  const nextId = config.nextId ?? (() => `${(counter += 1)}`);

  const describe = Effect.gen(function* () {
    const readiness = yield* Effect.promise(async () => config.readiness());
    return [makeAppleFmDescriptor({ providerRef: config.providerRef, model, readiness })];
  });

  const start = (input: ProviderStartInput): Effect.Effect<ProviderRun, ProviderStartError> =>
    Effect.gen(function* () {
      // Only this admitted lane may start; the renderer never supplies readiness.
      const readiness = yield* Effect.promise(async () => config.readiness());
      if (!readiness.ready) {
        return yield* Effect.fail(new ProviderStartError({ reason: startErrorReason(readiness.unavailableReason) }));
      }
      const prompt = intentPrompt(input.intent);
      if (prompt === null || prompt.trim().length === 0) {
        return yield* Effect.fail(new ProviderStartError({ reason: "unavailable" }));
      }
      const seed = nextId();
      const providerTurnRef = decodeProviderTurnRef(`providerturn.apple_fm.${seed}`);
      const startedAt = now();
      const events = Stream.concat(
        Stream.make(ProviderStreamEvent.Progress()),
        Stream.unwrap(
          Effect.promise(() => config.complete(prompt)).pipe(
            Effect.map((turn) =>
              Stream.make(
                completionToEvent({
                  turn,
                  intent: input.intent,
                  providerRef: config.providerRef,
                  model,
                  candidateRefSeed: seed,
                  latencyMs: now() - startedAt,
                  maxOutputChars,
                }),
              ),
            ),
          ),
        ),
      );
      const run: ProviderRun = { providerTurnRef, events };
      return run;
    });

  return { describe, start };
};

/** A ready-to-provide Layer for the shared `ProviderRegistry` tag. */
export const appleFmProviderRegistryLayer = (config: AppleFmProviderConfig): Layer.Layer<ProviderRegistry> =>
  Layer.succeed(ProviderRegistry, ProviderRegistry.of(makeAppleFmProviderRegistry(config)));

/**
 * Decode a completion turn's text into a route recommendation, a safe advisory
 * answer, or a typed rejection (the Phase-1 delegation-flow helper). FAIL-CLOSED:
 * only a `Recommendation` result may drive a dispatch decision.
 */
export const decodeAppleFmRecommendationFromTurn = (input: {
  readonly turn: AppleFmCompletionTurn;
  readonly admittedCandidates: ReadonlyArray<TurnProviderCandidate>;
  readonly maxOutputChars?: number;
}): AppleFmRouteDecodeResult => {
  if (input.turn.outcome === "failed" || input.turn.text === undefined) {
    return { _tag: "Reject", reason: "empty_output" };
  }
  return decodeAppleFmRouteOutput({
    raw: input.turn.text,
    admittedCandidates: input.admittedCandidates,
    ...(input.maxOutputChars !== undefined ? { maxOutputChars: input.maxOutputChars } : {}),
  });
};

export type { AppleFmProbe };

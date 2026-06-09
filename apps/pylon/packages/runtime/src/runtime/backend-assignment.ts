import { Effect, Schema as S } from "effect";
import { AppleFmBackendError, makeAppleFmClient, type AppleFmPlainTextCompletion } from "../backends/apple-fm/client";
import { APPLE_FM_BACKEND_KIND } from "../backends/apple-fm/contract";
import { GeminiClientError, makeGeminiClient, type GeminiCompleteResult } from "../backends/gemini/client";
import { GEMINI_BACKEND_KIND } from "../backends/gemini/contract";
import {
  assignmentSelectsGeminiBackend,
  requireAppleFmAssignmentBackend,
  requireGeminiAssignmentBackend,
  selectedAssignmentBackendProfileId,
  type ProbeRunAssignment,
} from "../contracts/assignment";
import { type AppleFmBackendAvailabilityReceipt, type AppleFmBackendFailureReceipt } from "../backends/apple-fm/receipts";
import {
  authorizeRunnerForAssignment,
  type ProbeRunnerAssignmentProof,
  type ProbeRunnerAuthorizationError,
  type ProbeRunnerIdentity,
} from "../runner/identity";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { type ProbeBackendRegistryError } from "../backends/registry";
import { makeProbeLlmRequest } from "../llm";
import {
  bestEffortRecordProbeTokenUsageEvent,
  makeAppleFmProbeTokenUsageEvent,
  makeGeminiProbeTokenUsageEvent,
  makeProbeAssignmentTokenUsageSourceRefs,
  makeProbeTokenUsageTelemetryClientFromEnv,
  probeTokenUsageActorFromEnv,
  probeTokenUsagePrivacyFromEnv,
} from "../fleet/token-usage";

export const ProbeBackendRunEvent = S.Struct({
  kind: S.Literals(["probe_backend_run_started", "probe_backend_run_finished", "probe_backend_run_failed"]),
  assignmentId: S.String,
  runnerSessionId: S.String,
  backendKind: S.Literals([APPLE_FM_BACKEND_KIND, GEMINI_BACKEND_KIND]),
  profileId: S.String,
  model: S.String,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
  receipt: S.optional(S.Unknown),
});
export type ProbeBackendRunEvent = typeof ProbeBackendRunEvent.Type;

export interface ProbeBackendAssignmentRunInput {
  readonly runner: ProbeRunnerIdentity;
  readonly proof: ProbeRunnerAssignmentProof;
  readonly assignment: ProbeRunAssignment;
  readonly trustedBackendBaseUrl?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly fetch?: typeof fetch;
  readonly now?: Date;
}

export interface ProbeBackendAssignmentRunResult {
  readonly assignmentId: string;
  readonly runnerSessionId: string;
  readonly backendKind: typeof APPLE_FM_BACKEND_KIND | typeof GEMINI_BACKEND_KIND;
  readonly profileId: string;
  readonly authRequired: false;
  readonly completion: AppleFmPlainTextCompletion | GeminiCompleteResult;
  readonly events: ReadonlyArray<ProbeBackendRunEvent>;
}

export type ProbeBackendAssignmentRunError =
  | ProbeRunnerAuthorizationError
  | ProbePublicProjectionUnsafe
  | ProbeBackendRegistryError
  | ProbeBackendAssignmentError;

export class ProbeBackendAssignmentError extends S.TaggedErrorClass<ProbeBackendAssignmentError>()(
  "ProbeBackendAssignmentError",
  {
    reason: S.String,
    receipt: S.optional(S.Unknown),
    events: S.Array(ProbeBackendRunEvent),
  },
) {}

export function runProbeBackendAssignment(
  input: ProbeBackendAssignmentRunInput,
): Effect.Effect<ProbeBackendAssignmentRunResult, ProbeBackendAssignmentRunError> {
  return assignmentSelectsGeminiBackend(input.assignment) ? runGeminiBackendAssignment(input) : runAppleFmBackendAssignment(input);
}

function runAppleFmBackendAssignment(
  input: ProbeBackendAssignmentRunInput,
): Effect.Effect<ProbeBackendAssignmentRunResult, ProbeBackendAssignmentRunError> {
  return Effect.gen(function* () {
    yield* authorizeRunnerForAssignment(input.runner, input.proof, input.assignment, input.now);
    const backend = yield* requireAppleFmAssignmentBackend(input.assignment).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBackendAssignmentError({
            reason: error.reason,
            events: [],
          }),
      ),
    );
    const client = yield* makeAppleFmClient({
      profileId: backend.profile,
      explicitBaseUrl: input.trustedBackendBaseUrl,
      env: input.env,
      fetch: input.fetch,
      now: input.now,
    });
    const observedAt = (input.now ?? new Date()).toISOString();
    const started = backendEvent({
      kind: "probe_backend_run_started",
      assignment: input.assignment,
      backendKind: APPLE_FM_BACKEND_KIND,
      profileId: client.profile.id,
      model: client.profile.model,
      observedAt,
    });
    const readiness = yield* client.health();

    if (!readiness.ready) {
      const failed = backendEvent({
        kind: "probe_backend_run_failed",
        assignment: input.assignment,
        backendKind: APPLE_FM_BACKEND_KIND,
        profileId: client.profile.id,
        model: client.profile.model,
        observedAt,
        receipt: readiness.receipt,
      });

      return yield* Effect.fail(
        new ProbeBackendAssignmentError({
          reason: readiness.message ?? `Apple FM backend is ${readiness.status}`,
          receipt: readiness.receipt,
          events: [started, failed],
        }),
      );
    }

    const completion = yield* client.completePlainText([{ role: "user", content: input.assignment.goal }]).pipe(
      Effect.mapError((error: AppleFmBackendError) => {
        const failed = backendEvent({
          kind: "probe_backend_run_failed",
          assignment: input.assignment,
          backendKind: APPLE_FM_BACKEND_KIND,
          profileId: client.profile.id,
          model: client.profile.model,
          observedAt,
          receipt: error.receipt,
        });

        return new ProbeBackendAssignmentError({
          reason: error.reason,
          receipt: error.receipt,
          events: [started, failed],
        });
      }),
    );
    const finished = backendEvent({
      kind: "probe_backend_run_finished",
      assignment: input.assignment,
      backendKind: APPLE_FM_BACKEND_KIND,
      profileId: client.profile.id,
      model: completion.response.model ?? client.profile.model,
      observedAt,
      receipt: completion.receipt,
    });
    yield* recordAssignmentAppleFmTokenUsage(input, completion);

    return {
      assignmentId: input.assignment.assignmentId,
      runnerSessionId: input.assignment.runnerSessionId,
      backendKind: APPLE_FM_BACKEND_KIND,
      profileId: client.profile.id,
      authRequired: false,
      completion,
      events: [started, finished],
    };
  });
}

function runGeminiBackendAssignment(
  input: ProbeBackendAssignmentRunInput,
): Effect.Effect<ProbeBackendAssignmentRunResult, ProbeBackendAssignmentRunError> {
  return Effect.gen(function* () {
    yield* authorizeRunnerForAssignment(input.runner, input.proof, input.assignment, input.now);
    const backend = yield* requireGeminiAssignmentBackend(input.assignment).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBackendAssignmentError({
            reason: error.reason,
            events: [],
          }),
      ),
    );
    const client = yield* makeGeminiClient({
      profileId: selectedAssignmentBackendProfileId(backend),
      explicitBaseUrl: input.trustedBackendBaseUrl,
      env: input.env,
      fetch: input.fetch,
      now: input.now,
    }).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBackendAssignmentError({
            reason: "reason" in error ? error.reason : String(error),
            receipt: error instanceof GeminiClientError ? error.receipt : undefined,
            events: [],
          }),
      ),
    );
    const observedAt = (input.now ?? new Date()).toISOString();
    const started = backendEvent({
      kind: "probe_backend_run_started",
      assignment: input.assignment,
      backendKind: GEMINI_BACKEND_KIND,
      profileId: client.profile.id,
      model: client.profile.model,
      observedAt,
    });

    const completion = yield* client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "google", model: client.profile.model },
        prompt: input.assignment.goal,
        generation: { maxTokens: 1024, temperature: 0 },
      }),
    }).pipe(
      Effect.mapError((error) => {
        const failed = backendEvent({
          kind: "probe_backend_run_failed",
          assignment: input.assignment,
          backendKind: GEMINI_BACKEND_KIND,
          profileId: client.profile.id,
          model: client.profile.model,
          observedAt,
          receipt: error.receipt,
        });

        return new ProbeBackendAssignmentError({
          reason: error.reason,
          receipt: error.receipt,
          events: [started, failed],
        });
      }),
    );
    const finished = backendEvent({
      kind: "probe_backend_run_finished",
      assignment: input.assignment,
      backendKind: GEMINI_BACKEND_KIND,
      profileId: client.profile.id,
      model: client.profile.model,
      observedAt,
      receipt: completion.receipt,
    });
    yield* recordAssignmentGeminiTokenUsage(input, completion);

    return {
      assignmentId: input.assignment.assignmentId,
      runnerSessionId: input.assignment.runnerSessionId,
      backendKind: GEMINI_BACKEND_KIND,
      profileId: client.profile.id,
      authRequired: false,
      completion,
      events: [started, finished],
    };
  });
}

function recordAssignmentAppleFmTokenUsage(
  input: ProbeBackendAssignmentRunInput,
  completion: AppleFmPlainTextCompletion,
): Effect.Effect<void, never> {
  const env = input.env ?? {};
  const client = makeProbeTokenUsageTelemetryClientFromEnv({
    env,
    fetch: input.fetch,
    managedAssignment: true,
  });

  return bestEffortRecordProbeTokenUsageEvent(
    client,
    makeAppleFmProbeTokenUsageEvent({
      actor: probeTokenUsageActorFromEnv(env),
      agentSurface: "managed_assignment",
      model: completion.response.model ?? completion.profile.model,
      observedAt: completion.receipt.observedAt,
      privacy: probeTokenUsagePrivacyFromEnv(env),
      profile: completion.profile,
      sourceRefs: makeProbeAssignmentTokenUsageSourceRefs(input.assignment),
      usage: completion.usage,
    }),
  );
}

function recordAssignmentGeminiTokenUsage(
  input: ProbeBackendAssignmentRunInput,
  completion: GeminiCompleteResult,
): Effect.Effect<void, never> {
  const env = input.env ?? {};
  const client = makeProbeTokenUsageTelemetryClientFromEnv({
    env,
    fetch: input.fetch,
    managedAssignment: true,
  });

  return bestEffortRecordProbeTokenUsageEvent(
    client,
    makeGeminiProbeTokenUsageEvent({
      actor: probeTokenUsageActorFromEnv(env),
      agentSurface: "managed_assignment",
      privacy: probeTokenUsagePrivacyFromEnv(env),
      result: completion,
      sourceRefs: makeProbeAssignmentTokenUsageSourceRefs(input.assignment),
    }),
  );
}

function backendEvent(input: {
  readonly kind: ProbeBackendRunEvent["kind"];
  readonly assignment: ProbeRunAssignment;
  readonly backendKind: typeof APPLE_FM_BACKEND_KIND | typeof GEMINI_BACKEND_KIND;
  readonly profileId: string;
  readonly model: string;
  readonly observedAt: string;
  readonly receipt?:
    | AppleFmBackendAvailabilityReceipt
    | AppleFmBackendFailureReceipt
    | AppleFmPlainTextCompletion["receipt"]
    | GeminiCompleteResult["receipt"];
}): ProbeBackendRunEvent {
  return {
    kind: input.kind,
    assignmentId: input.assignment.assignmentId,
    runnerSessionId: input.assignment.runnerSessionId,
    backendKind: input.backendKind,
    profileId: input.profileId,
    model: input.model,
    observedAt: input.observedAt,
    contentRedacted: true,
    receipt: input.receipt,
  };
}

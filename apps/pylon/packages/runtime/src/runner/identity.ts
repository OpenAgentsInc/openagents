import { Effect, Schema as S } from "effect";
import { PROBE_APPLE_FM_BACKEND_CAPABILITY } from "../backends/apple-fm/contract.js";
import { PROBE_GEMINI_BACKEND_CAPABILITY } from "../backends/gemini/contract.js";
import { PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY } from "../backends/psionic-qwen/contract.js";
import {
  assignmentRequiresProviderGrant,
  assignmentSelectsAppleFmBackend,
  assignmentSelectsGeminiBackend,
  assignmentSelectsPsionicQwenBackend,
  validateProbeAssignmentBlueprintScope,
  validateProbeAssignmentProjection,
  type ProbeRunAssignment,
} from "../contracts/assignment.js";
import { validateProbePublicProjection, type ProbePublicProjectionUnsafe } from "../contracts/provider-account.js";
import {
  materializeProbeAuthGrant,
  type ProbeAuthMaterializationError,
  type ProbeBrokeredAuthSecret,
  type ProbeMaterializedAuth,
} from "../auth/materializer.js";
import { type OmegaGrantResolver, type ProbeAuthGrantError } from "../omega/grant-client.js";

export const ProbeRunnerKind = S.Literals(["local", "shc", "pylon", "sandbox"]);
export type ProbeRunnerKind = typeof ProbeRunnerKind.Type;
export { PROBE_APPLE_FM_BACKEND_CAPABILITY } from "../backends/apple-fm/contract.js";
export { PROBE_GEMINI_BACKEND_CAPABILITY } from "../backends/gemini/contract.js";
export { PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY } from "../backends/psionic-qwen/contract.js";

export const ProbeRunnerIdentity = S.Struct({
  runnerId: S.String,
  kind: ProbeRunnerKind,
  linkedSubject: S.String,
  linkedAt: S.String,
  expiresAt: S.optional(S.String),
  capabilities: S.Array(S.String),
});
export type ProbeRunnerIdentity = typeof ProbeRunnerIdentity.Type;

export const ProbeRunnerAssignmentProof = S.Struct({
  runnerId: S.String,
  assignmentId: S.String,
  runnerSessionId: S.String,
  issuedAt: S.String,
  nonce: S.String,
  proofKind: S.Literals(["pylon_signed_link", "shc_broker", "sandbox_control", "test"]),
  signatureRef: S.optional(S.String),
});
export type ProbeRunnerAssignmentProof = typeof ProbeRunnerAssignmentProof.Type;

export class ProbeRunnerAuthorizationError extends S.TaggedErrorClass<ProbeRunnerAuthorizationError>()(
  "ProbeRunnerAuthorizationError",
  {
    reason: S.String,
  },
) {}

export interface ProbeSecretBroker {
  readonly resolveSecret: (
    grantProviderSecretRef: string,
    runner: ProbeRunnerIdentity,
  ) => Effect.Effect<ProbeBrokeredAuthSecret, ProbeRunnerAuthorizationError>;
}

export interface AuthorizedProbeAuthRunInput {
  readonly runner: ProbeRunnerIdentity;
  readonly proof: ProbeRunnerAssignmentProof;
  readonly assignment: ProbeRunAssignment;
  readonly grantResolver: OmegaGrantResolver;
  readonly secretBroker: ProbeSecretBroker;
  readonly runHome: string;
  readonly now?: Date;
}

export type AuthorizedProbeAuthRunError =
  | ProbeRunnerAuthorizationError
  | ProbePublicProjectionUnsafe
  | ProbeAuthGrantError
  | ProbeAuthMaterializationError;

export function authorizeRunnerForAssignment(
  runner: ProbeRunnerIdentity,
  proof: ProbeRunnerAssignmentProof,
  assignment: ProbeRunAssignment,
  now: Date = new Date(),
): Effect.Effect<void, ProbeRunnerAuthorizationError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbePublicProjection(runner, "runner");
    yield* validateProbePublicProjection(proof, "proof");
    yield* validateProbeAssignmentProjection(assignment);
    yield* validateProbeAssignmentBlueprintScope(assignment).pipe(
      Effect.mapError(
        (error) =>
          new ProbeRunnerAuthorizationError({
            reason: `assignment Blueprint scope is invalid: ${error.reason}`,
          }),
      ),
    );

    const missingCapabilities = requiredRunnerCapabilitiesForAssignment(assignment).filter(
      (capability) => !runner.capabilities.includes(capability),
    );

    if (missingCapabilities.length > 0) {
      return yield* Effect.fail(
        new ProbeRunnerAuthorizationError({
          reason: `runner is missing required capabilities: ${missingCapabilities.join(", ")}`,
        }),
      );
    }

    if (runner.runnerId !== proof.runnerId) {
      return yield* Effect.fail(
        new ProbeRunnerAuthorizationError({
          reason: "runner proof does not match linked runner identity",
        }),
      );
    }

    if (proof.assignmentId !== assignment.assignmentId) {
      return yield* Effect.fail(new ProbeRunnerAuthorizationError({ reason: "runner proof assignment mismatch" }));
    }

    if (proof.runnerSessionId !== assignment.runnerSessionId) {
      return yield* Effect.fail(
        new ProbeRunnerAuthorizationError({
          reason: "runner proof session mismatch",
        }),
      );
    }

    if (runner.expiresAt !== undefined) {
      const expiresAtMs = Date.parse(runner.expiresAt);

      if (Number.isNaN(expiresAtMs) || expiresAtMs <= now.getTime()) {
        return yield* Effect.fail(new ProbeRunnerAuthorizationError({ reason: "runner link is expired" }));
      }
    }
  });
}

export function requiredRunnerCapabilitiesForAssignment(assignment: ProbeRunAssignment): ReadonlyArray<string> {
  const capabilities = ["probe.run"];

  if (assignmentRequiresProviderGrant(assignment)) {
    capabilities.push("omega.grant.resolve");
  }

  if (assignmentSelectsAppleFmBackend(assignment)) {
    capabilities.push(PROBE_APPLE_FM_BACKEND_CAPABILITY);
  }

  if (assignmentSelectsGeminiBackend(assignment)) {
    capabilities.push(PROBE_GEMINI_BACKEND_CAPABILITY);
  }

  if (assignmentSelectsPsionicQwenBackend(assignment)) {
    capabilities.push(PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY);
  }

  return capabilities;
}

export function prepareAuthorizedProbeAuthRun(
  input: AuthorizedProbeAuthRunInput,
): Effect.Effect<ProbeMaterializedAuth, AuthorizedProbeAuthRunError> {
  return Effect.gen(function* () {
    yield* authorizeRunnerForAssignment(input.runner, input.proof, input.assignment, input.now);
    const grant = yield* input.grantResolver.resolveGrant(input.assignment);
    const secret = yield* input.secretBroker.resolveSecret(grant.providerSecretRef, input.runner);

    return yield* materializeProbeAuthGrant({
      grant,
      secret,
      runHome: input.runHome,
      now: input.now,
    });
  });
}

export function makeStaticProbeSecretBroker(secret: ProbeBrokeredAuthSecret): ProbeSecretBroker {
  return {
    resolveSecret: (grantProviderSecretRef) =>
      grantProviderSecretRef === secret.providerSecretRef
        ? Effect.succeed(secret)
        : Effect.fail(new ProbeRunnerAuthorizationError({ reason: "broker secret ref mismatch" })),
  };
}

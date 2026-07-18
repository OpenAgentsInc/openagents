import { Context, DateTime, Effect, Layer, Schema as S } from "effect";

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Timestamp = S.DateTimeUtcFromString;

export const AUTHORITY_DECISION_RECEIPT_SCHEMA =
  "openagents.authority_decision_receipt.v1" as const;

export const AuthorityGrantSchema = S.Struct({
  grantRef: Ref,
  roles: S.Array(Ref),
  actions: S.Array(Ref),
  resources: S.Array(Ref),
  programs: S.Array(Ref),
  conditionRefs: S.Array(Ref),
});
export interface AuthorityGrant extends S.Schema.Type<typeof AuthorityGrantSchema> {}

export const AuthorityRuntimeProfileSchema = S.Struct({
  profileRef: Ref,
  revision: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  lifecycle: S.Literals(["admitted", "suspended", "revoked", "superseded"]),
  authorityMayAmplify: S.Literal(false),
  explicitDenyWins: S.Literal(true),
  grants: S.Array(AuthorityGrantSchema),
  reservedActions: S.Array(Ref),
});
export interface AuthorityRuntimeProfile extends S.Schema.Type<
  typeof AuthorityRuntimeProfileSchema
> {}

export const AuthorityConditionResultSchema = S.Struct({
  conditionRef: Ref,
  passed: S.Boolean,
  evidenceRefs: S.Array(Ref),
});
export interface AuthorityConditionResult extends S.Schema.Type<
  typeof AuthorityConditionResultSchema
> {}

export const AuthorityActionRequestSchema = S.Struct({
  requestRef: Ref,
  actorRef: Ref,
  actorRole: Ref,
  action: Ref,
  resource: Ref,
  programRef: Ref,
  triggerRef: Ref,
  conditionResults: S.Array(AuthorityConditionResultSchema),
  startedAt: Timestamp,
});
export interface AuthorityActionRequest extends S.Schema.Type<
  typeof AuthorityActionRequestSchema
> {}

const DecisionFields = {
  request: AuthorityActionRequestSchema,
  profileRef: Ref,
  profileRevision: S.Int.check(S.isGreaterThanOrEqualTo(1)),
};

export const AuthorityDecisionSchema = S.TaggedUnion({
  Allowed: {
    ...DecisionFields,
    grantRef: Ref,
  },
  Denied: {
    ...DecisionFields,
    reason: S.Literals([
      "profile_inactive",
      "reserved_action",
      "grant_not_found",
      "condition_missing",
      "condition_failed",
      "profile_invalid",
    ]),
  },
});
export type AuthorityDecision = typeof AuthorityDecisionSchema.Type;

export const AuthorityDecisionReceiptSchema = S.Struct({
  schema: S.Literal(AUTHORITY_DECISION_RECEIPT_SCHEMA),
  receiptRef: Ref,
  profileRef: Ref,
  profileRevision: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  programRef: Ref,
  grantRef: S.NullOr(Ref),
  actorRef: Ref,
  actorRole: Ref,
  action: Ref,
  targetRef: Ref,
  triggerRef: Ref,
  conditionResults: S.Array(AuthorityConditionResultSchema),
  startedAt: Timestamp,
  settledAt: Timestamp,
  outcome: S.Literals(["succeeded", "refused"]),
  evidenceRefs: S.Array(Ref),
});
export interface AuthorityDecisionReceipt extends S.Schema.Type<
  typeof AuthorityDecisionReceiptSchema
> {}

export class AuthorityProfileInvalid extends S.TaggedErrorClass<AuthorityProfileInvalid>()(
  "AuthorityProfileInvalid",
  { cause: S.Defect() },
) {}

export type AuthorityServiceShape = Readonly<{
  resolve: (request: unknown) => Effect.Effect<AuthorityDecision, AuthorityProfileInvalid>;
  receipt: (
    input: Readonly<{
      decision: AuthorityDecision;
      receiptRef: string;
      settledAt: Date;
      evidenceRefs: ReadonlyArray<string>;
    }>,
  ) => Effect.Effect<AuthorityDecisionReceipt, AuthorityProfileInvalid>;
}>;

export class AuthorityService extends Context.Service<AuthorityService, AuthorityServiceShape>()(
  "@openagentsinc/authority/AuthorityService",
) {}

const matchesGrant = (grant: AuthorityGrant, request: AuthorityActionRequest): boolean =>
  grant.roles.includes(request.actorRole) &&
  grant.actions.includes(request.action) &&
  grant.resources.includes(request.resource) &&
  grant.programs.includes(request.programRef);

const resolveDecision = (
  profile: AuthorityRuntimeProfile,
  request: AuthorityActionRequest,
): AuthorityDecision => {
  const base = {
    profileRef: profile.profileRef,
    profileRevision: profile.revision,
    request,
  };
  if (profile.lifecycle !== "admitted") {
    return AuthorityDecisionSchema.cases.Denied.make({
      ...base,
      reason: "profile_inactive",
    });
  }
  if (profile.reservedActions.includes(request.action)) {
    return AuthorityDecisionSchema.cases.Denied.make({
      ...base,
      reason: "reserved_action",
    });
  }
  const grant = profile.grants.find((candidate) => matchesGrant(candidate, request));
  if (grant === undefined) {
    return AuthorityDecisionSchema.cases.Denied.make({
      ...base,
      reason: "grant_not_found",
    });
  }
  const byRef = new Map(request.conditionResults.map((result) => [result.conditionRef, result]));
  const missing = grant.conditionRefs.some((ref) => !byRef.has(ref));
  if (missing) {
    return AuthorityDecisionSchema.cases.Denied.make({
      ...base,
      reason: "condition_missing",
    });
  }
  const failed = grant.conditionRefs.some((ref) => byRef.get(ref)?.passed !== true);
  if (failed) {
    return AuthorityDecisionSchema.cases.Denied.make({
      ...base,
      reason: "condition_failed",
    });
  }
  return AuthorityDecisionSchema.cases.Allowed.make({
    ...base,
    grantRef: grant.grantRef,
  });
};

export const makeAuthorityServiceLayer = (
  rawProfile: unknown,
): Layer.Layer<AuthorityService, AuthorityProfileInvalid> =>
  Layer.effect(
    AuthorityService,
    Effect.gen(function* () {
      const profile = yield* S.decodeUnknownEffect(AuthorityRuntimeProfileSchema)(rawProfile, {
        onExcessProperty: "error",
      }).pipe(Effect.mapError((cause) => new AuthorityProfileInvalid({ cause })));

      const resolve = Effect.fn("AuthorityService.resolve")(function* (rawRequest: unknown) {
        const request = yield* S.decodeUnknownEffect(AuthorityActionRequestSchema)(rawRequest, {
          onExcessProperty: "error",
        }).pipe(Effect.mapError((cause) => new AuthorityProfileInvalid({ cause })));
        return resolveDecision(profile, request);
      });

      const receipt = Effect.fn("AuthorityService.receipt")(function* (
        input: Readonly<{
          decision: AuthorityDecision;
          receiptRef: string;
          settledAt: Date;
          evidenceRefs: ReadonlyArray<string>;
        }>,
      ) {
        const allowed = input.decision._tag === "Allowed";
        return yield* S.decodeUnknownEffect(AuthorityDecisionReceiptSchema)({
          schema: AUTHORITY_DECISION_RECEIPT_SCHEMA,
          receiptRef: input.receiptRef,
          profileRef: input.decision.profileRef,
          profileRevision: input.decision.profileRevision,
          programRef: input.decision.request.programRef,
          grantRef: allowed ? input.decision.grantRef : null,
          actorRef: input.decision.request.actorRef,
          actorRole: input.decision.request.actorRole,
          action: input.decision.request.action,
          targetRef: input.decision.request.resource,
          triggerRef: input.decision.request.triggerRef,
          conditionResults: input.decision.request.conditionResults,
          startedAt: DateTime.formatIso(input.decision.request.startedAt),
          settledAt: input.settledAt.toISOString(),
          outcome: allowed ? "succeeded" : "refused",
          evidenceRefs: input.evidenceRefs,
        }).pipe(Effect.mapError((cause) => new AuthorityProfileInvalid({ cause })));
      });

      return AuthorityService.of({ resolve, receipt });
    }),
  );

export const resolveAuthorityDecision = (
  profile: unknown,
  request: unknown,
): Effect.Effect<AuthorityDecision, AuthorityProfileInvalid> =>
  Effect.gen(function* () {
    const authority = yield* AuthorityService;
    return yield* authority.resolve(request);
  }).pipe(Effect.provide(makeAuthorityServiceLayer(profile)));

import { createHash } from "node:crypto";
import { Context, Effect, Layer, Option, Schema as S } from "effect";

import {
  decodeSignedWorkroomLedger,
  DelegationGrantRefSchema,
  EvidenceRefSchema,
  IsoTimestampSchema,
  NostrPublicKeySchema,
  PrincipalRefSchema,
  PrivacyClassSchema,
  SafeIntegerSchema,
  type SignedWorkroomActivity,
  SignedWorkroomActivityKindSchema,
  type SignedWorkroomDeliveryRequest,
  type SignedWorkroomDeliveryResult,
  type SignedWorkroomEnqueueRequest,
  type SignedWorkroomEnqueueResult,
  type SignedWorkroomLedger,
  type SignedWorkroomReadRequest,
  type SignedWorkroomReadResult,
  SignedWorkroomLedgerSchema,
  WorkroomAudienceSchema,
  WorkroomRefSchema,
  WorkRefSchema,
} from "./generated.ts";
import { verifySignedWorkroomNostrActivity } from "./signed-workroom-nostr.ts";

export const SIGNED_WORKROOM_WRITE_CAPABILITY = "capability:workroom-activity:enqueue" as const;
export const SIGNED_WORKROOM_DELIVERY_CAPABILITY = "capability:workroom-activity:deliver" as const;
export const SIGNED_WORKROOM_PUBLISH_CAPABILITY = "capability:workroom-activity:publish" as const;

export class SignedWorkroomError extends S.TaggedErrorClass<SignedWorkroomError>()(
  "SignedWorkroomError",
  {
    reason: S.Literals([
      "forbidden",
      "revision_conflict",
      "duplicate_event",
      "idempotency_conflict",
      "causal_gap",
      "invalid_audience",
      "invalid_supersession",
      "invalid_revocation",
      "invalid_event_id",
      "invalid_signature",
      "invalid_projection_profile",
      "signer_actor_mismatch",
      "actor_grant_required",
      "invalid_actor_grant",
      "stale_actor_grant",
      "outbox_not_found",
      "invalid_delivery",
      "invalid_state",
      "storage_unavailable",
    ]),
    detail: S.String,
  },
) {}

export const SIGNED_WORKROOM_ACTOR_GRANT_PURPOSE =
  "purpose:signed-workroom:project-activity" as const;

export const SignedWorkroomActorGrantSchema = S.Struct({
  grantRef: DelegationGrantRefSchema,
  issuerPrincipalRef: PrincipalRefSchema,
  actorRef: PrincipalRefSchema,
  signerPubkey: NostrPublicKeySchema,
  purpose: S.Literal(SIGNED_WORKROOM_ACTOR_GRANT_PURPOSE),
  workroomRef: WorkroomRefSchema,
  workRef: S.NullOr(WorkRefSchema),
  activityKinds: S.Array(SignedWorkroomActivityKindSchema).check(
    S.isMinLength(1),
    S.isMaxLength(14),
  ),
  audiences: S.Array(WorkroomAudienceSchema).check(S.isMinLength(1), S.isMaxLength(6)),
  privacyClasses: S.Array(PrivacyClassSchema).check(S.isMinLength(1), S.isMaxLength(6)),
  generation: SafeIntegerSchema,
  validFrom: IsoTimestampSchema,
  expiresAt: IsoTimestampSchema,
  state: S.Literals(["active", "revoked", "superseded"]),
  evidenceRefs: S.Array(EvidenceRefSchema).check(S.isMinLength(1), S.isMaxLength(64)),
}).annotate({ identifier: "SignedWorkroomActorGrant" });
export interface SignedWorkroomActorGrant extends S.Schema.Type<
  typeof SignedWorkroomActorGrantSchema
> {}

export interface SignedWorkroomActorGrantResolverShape {
  readonly resolve: (
    grantRef: string,
  ) => Effect.Effect<SignedWorkroomActorGrant | null, SignedWorkroomError>;
}

export class SignedWorkroomActorGrantResolver extends Context.Service<
  SignedWorkroomActorGrantResolver,
  SignedWorkroomActorGrantResolverShape
>()("SignedWorkroomAuthority.ActorGrantResolver") {}

export const makeSignedWorkroomActorGrantResolverLayer = (
  grants: ReadonlyArray<unknown>,
): Layer.Layer<SignedWorkroomActorGrantResolver, SignedWorkroomError> =>
  Layer.effect(
    SignedWorkroomActorGrantResolver,
    Effect.gen(function* () {
      const decoded = yield* Effect.forEach(grants, (grant) =>
        S.decodeUnknownEffect(SignedWorkroomActorGrantSchema)(grant, {
          onExcessProperty: "error",
        }).pipe(
          Effect.mapError(
            () =>
              new SignedWorkroomError({
                reason: "invalid_actor_grant",
                detail: "actor grant authority returned an invalid fact",
              }),
          ),
        ),
      );
      const byRef = new Map(decoded.map((grant) => [grant.grantRef, grant]));
      if (byRef.size !== decoded.length) {
        return yield* new SignedWorkroomError({
          reason: "invalid_actor_grant",
          detail: "actor grant authority returned duplicate grant refs",
        });
      }
      return SignedWorkroomActorGrantResolver.of({
        resolve: (grantRef) => Effect.succeed(byRef.get(grantRef) ?? null),
      });
    }),
  );

export const SignedWorkroomStateSchema = S.Struct({
  ledger: SignedWorkroomLedgerSchema,
  idempotency: S.Record(S.String, S.Struct({ digest: S.String, result: S.Unknown })),
}).annotate({ identifier: "SignedWorkroomState" });
export interface SignedWorkroomState extends S.Schema.Type<typeof SignedWorkroomStateSchema> {}

export interface SignedWorkroomStateStoreShape {
  readonly load: Effect.Effect<SignedWorkroomState | null, SignedWorkroomError>;
  readonly save: (
    expectedRevision: number,
    state: SignedWorkroomState,
  ) => Effect.Effect<void, SignedWorkroomError>;
}

export class SignedWorkroomStateStore extends Context.Service<
  SignedWorkroomStateStore,
  SignedWorkroomStateStoreShape
>()("SignedWorkroomAuthority.StateStore") {}

const digest = (value: unknown): string =>
  createHash("sha256").update(JSON.stringify(value)).digest("hex");

export const emptySignedWorkroomLedger = (observedAt: string): SignedWorkroomLedger =>
  decodeSignedWorkroomLedger({
    contractVersion: "openagents.all_work_boundary.v1",
    revision: 0,
    eventCursor: "cursor:signed-workroom:0",
    activities: [],
    outbox: [],
    completeness: { state: "complete", cursor: "cursor:signed-workroom:0", gapRefs: [] },
    freshness: { state: "fresh", observedAt },
  });

export const emptySignedWorkroomState = (observedAt: string): SignedWorkroomState => ({
  ledger: emptySignedWorkroomLedger(observedAt),
  idempotency: {},
});

const validateAudience = (
  activity: SignedWorkroomActivity,
): Effect.Effect<void, SignedWorkroomError> =>
  activity.audience === activity.privacyClass
    ? Effect.void
    : Effect.fail(
        new SignedWorkroomError({
          reason: "invalid_audience",
          detail: "audience and privacy class must match the closed disclosure profile",
        }),
      );

const validateCausality = (
  ledger: SignedWorkroomLedger,
  activity: SignedWorkroomActivity,
): Effect.Effect<void, SignedWorkroomError> => {
  const byRef = new Map(ledger.activities.map((value) => [value.eventRef, value]));
  const missing = activity.causalParentRefs.find((value) => !byRef.has(value));
  if (missing !== undefined) {
    return Effect.fail(
      new SignedWorkroomError({ reason: "causal_gap", detail: `missing parent ${missing}` }),
    );
  }
  if (activity.supersedesEventRef !== null) {
    const prior = byRef.get(activity.supersedesEventRef);
    if (
      prior === undefined ||
      prior.workroomRef !== activity.workroomRef ||
      prior.actorRef !== activity.actorRef ||
      prior.generation >= activity.generation
    ) {
      return Effect.fail(
        new SignedWorkroomError({
          reason: "invalid_supersession",
          detail: "supersession must advance the same actor and Workroom generation",
        }),
      );
    }
  }
  if (activity.revokesEventRef !== null) {
    const prior = byRef.get(activity.revokesEventRef);
    if (
      activity.kind !== "revocation" ||
      prior === undefined ||
      prior.generation >= activity.generation
    ) {
      return Effect.fail(
        new SignedWorkroomError({
          reason: "invalid_revocation",
          detail: "revocation must name an earlier known generation",
        }),
      );
    }
  }
  return Effect.void;
};

export const validateSignedWorkroomProjection = (
  activity: SignedWorkroomActivity,
): Effect.Effect<void, SignedWorkroomError> => {
  const verification = verifySignedWorkroomNostrActivity(activity);
  switch (verification) {
    case "valid":
      return Effect.void;
    case "event_id_mismatch":
      return Effect.fail(
        new SignedWorkroomError({
          reason: "invalid_event_id",
          detail: "the NIP-01 event ID does not bind the Workroom projection bytes",
        }),
      );
    case "signature_invalid":
      return Effect.fail(
        new SignedWorkroomError({
          reason: "invalid_signature",
          detail: "the Workroom projection Schnorr signature is invalid",
        }),
      );
  }
};

const isSupportedGrantedActor = (actorRef: string): boolean =>
  ["principal:agent:", "principal:device:", "principal:organization:"].some(
    (prefix) => actorRef.startsWith(prefix) && actorRef.length > prefix.length,
  );

const validateActorBindingShape = (
  activity: SignedWorkroomActivity,
): Effect.Effect<"direct" | "granted", SignedWorkroomError> => {
  const directActorRef = `principal:nostr:${activity.signerPubkey}`;
  const actorGrantRef = activity.actorGrantRef ?? null;
  const actorGrantGeneration = activity.actorGrantGeneration ?? null;
  if (activity.actorRef === directActorRef) {
    return actorGrantRef === null && actorGrantGeneration === null
      ? Effect.succeed("direct" as const)
      : Effect.fail(
          new SignedWorkroomError({
            reason: "invalid_actor_grant",
            detail: "direct Nostr actors cannot attach an actor grant",
          }),
        );
  }
  if (
    !isSupportedGrantedActor(activity.actorRef) ||
    activity.projectionProfile !== "openagents.signed-workroom.v2" ||
    actorGrantRef === null ||
    actorGrantGeneration === null
  ) {
    return Effect.fail(
      new SignedWorkroomError({
        reason: "signer_actor_mismatch",
        detail: "non-direct actors require a bound agent, device, or organization grant",
      }),
    );
  }
  return Effect.succeed("granted" as const);
};

export const validateSignedWorkroomAdmission = (
  activity: SignedWorkroomActivity,
  admittedAt: string,
  options?: Readonly<{ allowLegacyDirectReplay?: boolean }>,
): Effect.Effect<void, SignedWorkroomError> =>
  Effect.gen(function* () {
    yield* validateSignedWorkroomProjection(activity);
    const binding = yield* validateActorBindingShape(activity);
    if (
      activity.projectionProfile !== "openagents.signed-workroom.v2" &&
      !(binding === "direct" && options?.allowLegacyDirectReplay === true)
    ) {
      return yield* new SignedWorkroomError({
        reason: "invalid_projection_profile",
        detail: "new signed Workroom admission requires the current projection profile",
      });
    }
    if (binding === "direct") return;
    const resolver = yield* Effect.serviceOption(SignedWorkroomActorGrantResolver);
    if (Option.isNone(resolver)) {
      return yield* new SignedWorkroomError({
        reason: "actor_grant_required",
        detail: "authoritative actor grant resolver is unavailable",
      });
    }
    const grant = yield* resolver.value.resolve(activity.actorGrantRef!);
    if (grant === null || grant.state !== "active") {
      return yield* new SignedWorkroomError({
        reason: "stale_actor_grant",
        detail: "actor grant is absent, revoked, or superseded",
      });
    }
    const admittedTime = Date.parse(admittedAt);
    const occurredTime = Date.parse(activity.occurredAt);
    const validFrom = Date.parse(grant.validFrom);
    const expiresAt = Date.parse(grant.expiresAt);
    const scopeMatches =
      grant.grantRef === activity.actorGrantRef &&
      grant.generation === activity.actorGrantGeneration &&
      grant.actorRef === activity.actorRef &&
      grant.signerPubkey === activity.signerPubkey &&
      grant.workroomRef === activity.workroomRef &&
      grant.workRef === activity.workRef &&
      grant.activityKinds.includes(activity.kind) &&
      grant.audiences.includes(activity.audience) &&
      grant.privacyClasses.includes(activity.privacyClass) &&
      grant.evidenceRefs.every((evidenceRef) => activity.evidenceRefs.includes(evidenceRef));
    if (!scopeMatches) {
      return yield* new SignedWorkroomError({
        reason: "invalid_actor_grant",
        detail: "actor grant does not match the signed activity scope",
      });
    }
    if (
      !Number.isFinite(admittedTime) ||
      admittedTime < validFrom ||
      admittedTime >= expiresAt ||
      occurredTime < validFrom ||
      occurredTime >= expiresAt
    ) {
      return yield* new SignedWorkroomError({
        reason: "stale_actor_grant",
        detail: "actor grant is not valid at occurrence and admission time",
      });
    }
  });

export const validateSignedWorkroomState = (
  state: SignedWorkroomState,
): Effect.Effect<void, SignedWorkroomError> =>
  Effect.gen(function* () {
    const activitiesByRef = new Map(
      state.ledger.activities.map((activity) => [activity.eventRef, activity]),
    );
    if (
      activitiesByRef.size !== state.ledger.activities.length ||
      state.ledger.revision < state.ledger.activities.length ||
      state.ledger.activities.some(
        (activity, index, activities) =>
          activity.revision > state.ledger.revision ||
          (index > 0 && activity.revision <= activities[index - 1]!.revision),
      )
    ) {
      return yield* new SignedWorkroomError({
        reason: "invalid_state",
        detail: "signed Workroom revision is behind its unique canonical activities",
      });
    }
    for (const activity of state.ledger.activities) {
      yield* validateSignedWorkroomProjection(activity);
      yield* validateActorBindingShape(activity);
    }
    for (const record of state.ledger.outbox) {
      yield* validateSignedWorkroomProjection(record.activity);
      yield* validateActorBindingShape(record.activity);
      const canonical = activitiesByRef.get(record.activity.eventRef);
      if (canonical === undefined || digest(canonical) !== digest(record.activity)) {
        return yield* new SignedWorkroomError({
          reason: "invalid_state",
          detail: "signed Workroom outbox does not match canonical activity",
        });
      }
      const deliveryAttempts = record.deliveryAttempts;
      const latestAttemptAt = deliveryAttempts
        .map((attempt) => attempt.attemptedAt)
        .sort()
        .at(-1);
      const acceptedAttemptRelayUrls = [
        ...new Set(
          deliveryAttempts
            .filter((attempt) => attempt.outcome === "accepted")
            .map((attempt) => attempt.relayUrl),
        ),
      ].sort();
      if (
        new Set(record.relayUrls).size !== record.relayUrls.length ||
        new Set(record.acceptedRelayUrls).size !== record.acceptedRelayUrls.length ||
        record.acceptedRelayUrls.some((relayUrl) => !record.relayUrls.includes(relayUrl)) ||
        deliveryAttempts.some((attempt) => !record.relayUrls.includes(attempt.relayUrl)) ||
        acceptedAttemptRelayUrls.length !== record.acceptedRelayUrls.length ||
        acceptedAttemptRelayUrls.some(
          (relayUrl, index) => relayUrl !== record.acceptedRelayUrls[index],
        ) ||
        record.attemptCount !== deliveryAttempts.length ||
        (latestAttemptAt ?? null) !== record.lastAttemptAt ||
        (record.state === "accepted" &&
          record.relayUrls.some((relayUrl) => !record.acceptedRelayUrls.includes(relayUrl))) ||
        (record.attemptCount === 0) !== (record.lastAttemptAt === null)
      ) {
        return yield* new SignedWorkroomError({
          reason: "invalid_state",
          detail: `signed Workroom outbox delivery facts do not reconcile for ${record.activity.eventRef}`,
        });
      }
    }
  });

export const enqueueSignedWorkroomActivity = (
  request: SignedWorkroomEnqueueRequest,
  persistedAt: string,
): Effect.Effect<SignedWorkroomEnqueueResult, SignedWorkroomError, SignedWorkroomStateStore> =>
  Effect.gen(function* () {
    const store = yield* SignedWorkroomStateStore;
    const state = (yield* store.load) ?? emptySignedWorkroomState(persistedAt);
    yield* validateSignedWorkroomState(state);
    const requestDigest = digest(request);
    const replay = state.idempotency[request.idempotencyKey];
    if (replay !== undefined) {
      if (replay.digest !== requestDigest) {
        return yield* new SignedWorkroomError({
          reason: "idempotency_conflict",
          detail: "idempotency key was reused with different signed bytes",
        });
      }
      return replay.result as SignedWorkroomEnqueueResult;
    }
    if (
      request.capabilityRef !== SIGNED_WORKROOM_WRITE_CAPABILITY ||
      request.effectivePrincipalRef !== request.activity.actorRef
    ) {
      return yield* new SignedWorkroomError({
        reason: "forbidden",
        detail: "effective principal or capability does not admit this projection",
      });
    }
    if (request.expectedRevision !== state.ledger.revision) {
      return yield* new SignedWorkroomError({
        reason: "revision_conflict",
        detail: `expected ${request.expectedRevision}, found ${state.ledger.revision}`,
      });
    }
    if (request.activity.revision !== state.ledger.revision + 1) {
      return yield* new SignedWorkroomError({
        reason: "revision_conflict",
        detail: `signed activity revision ${request.activity.revision} does not advance ledger ${state.ledger.revision}`,
      });
    }
    if (state.ledger.activities.some((value) => value.eventRef === request.activity.eventRef)) {
      return yield* new SignedWorkroomError({
        reason: "duplicate_event",
        detail: `event ${request.activity.eventRef} already exists`,
      });
    }
    yield* validateSignedWorkroomAdmission(request.activity, persistedAt);
    yield* validateAudience(request.activity);
    yield* validateCausality(state.ledger, request.activity);
    const revision = state.ledger.revision + 1;
    const eventCursor = `cursor:signed-workroom:${revision}`;
    const ledger = decodeSignedWorkroomLedger({
      ...state.ledger,
      revision,
      eventCursor,
      activities: [...state.ledger.activities, request.activity],
      outbox: [
        ...state.ledger.outbox,
        {
          activity: request.activity,
          canonicalPersistedAt: persistedAt,
          state: "pending",
          relayUrls: [...new Set(request.relayUrls)].sort(),
          acceptedRelayUrls: [],
          deliveryAttempts: [],
          attemptCount: 0,
          lastAttemptAt: null,
          lastError: null,
        },
      ],
      completeness: { state: "complete", cursor: eventCursor, gapRefs: [] },
      freshness: { state: "fresh", observedAt: persistedAt },
    });
    const result: SignedWorkroomEnqueueResult = {
      ledger,
      receipt: {
        idempotencyKey: request.idempotencyKey,
        previousRevision: state.ledger.revision,
        revision,
        eventCursor,
        eventRef: request.activity.eventRef,
        persistedBeforePublish: true,
        relayAcceptanceIsAuthority: false,
        admittedEffect: false,
      },
    };
    yield* store.save(state.ledger.revision, {
      ledger,
      idempotency: {
        ...state.idempotency,
        [request.idempotencyKey]: { digest: requestDigest, result },
      },
    });
    return result;
  });

export const deliverSignedWorkroomActivity = (
  request: SignedWorkroomDeliveryRequest,
): Effect.Effect<SignedWorkroomDeliveryResult, SignedWorkroomError, SignedWorkroomStateStore> =>
  Effect.gen(function* () {
    const store = yield* SignedWorkroomStateStore;
    const state = yield* store.load;
    if (state === null) {
      return yield* new SignedWorkroomError({
        reason: "outbox_not_found",
        detail: request.eventRef,
      });
    }
    yield* validateSignedWorkroomState(state);
    const requestDigest = digest(request);
    const replay = state.idempotency[request.idempotencyKey];
    if (replay !== undefined) {
      if (replay.digest !== requestDigest) {
        return yield* new SignedWorkroomError({
          reason: "idempotency_conflict",
          detail: "idempotency key was reused with different delivery facts",
        });
      }
      return replay.result as SignedWorkroomDeliveryResult;
    }
    if (request.capabilityRef !== SIGNED_WORKROOM_DELIVERY_CAPABILITY) {
      return yield* new SignedWorkroomError({
        reason: "forbidden",
        detail: "effective principal lacks the signed Workroom delivery capability",
      });
    }
    if (request.expectedRevision !== state.ledger.revision) {
      return yield* new SignedWorkroomError({
        reason: "revision_conflict",
        detail: `expected ${request.expectedRevision}, found ${state.ledger.revision}`,
      });
    }
    const recordIndex = state.ledger.outbox.findIndex(
      (record) => record.activity.eventRef === request.eventRef,
    );
    const record = state.ledger.outbox[recordIndex];
    if (record === undefined) {
      return yield* new SignedWorkroomError({
        reason: "outbox_not_found",
        detail: request.eventRef,
      });
    }
    if (request.effectivePrincipalRef !== record.activity.actorRef) {
      return yield* new SignedWorkroomError({
        reason: "forbidden",
        detail: "only the admitted activity actor can record delivery in this profile",
      });
    }
    if (["accepted", "superseded", "revoked"].includes(record.state)) {
      return yield* new SignedWorkroomError({
        reason: "invalid_delivery",
        detail: `outbox record is terminal in ${record.state}`,
      });
    }
    const attemptedRelayUrls = request.attempts.map((attempt) => attempt.relayUrl);
    if (
      new Set(attemptedRelayUrls).size !== attemptedRelayUrls.length ||
      attemptedRelayUrls.some((relayUrl) => !record.relayUrls.includes(relayUrl)) ||
      record.deliveryAttempts.length + request.attempts.length > 10_000
    ) {
      return yield* new SignedWorkroomError({
        reason: "invalid_delivery",
        detail: "delivery attempts must be unique configured relay targets",
      });
    }

    const acceptedRelayUrls = [
      ...new Set([
        ...record.acceptedRelayUrls,
        ...request.attempts
          .filter((attempt) => attempt.outcome === "accepted")
          .map((attempt) => attempt.relayUrl),
      ]),
    ].sort();
    const allAccepted = record.relayUrls.every((relayUrl) => acceptedRelayUrls.includes(relayUrl));
    const failedAttempts = request.attempts.filter((attempt) => attempt.outcome !== "accepted");
    const outboxState = allAccepted
      ? ("accepted" as const)
      : failedAttempts.length > 0
        ? ("failed" as const)
        : ("publishing" as const);
    const lastAttemptAt = request.attempts
      .map((attempt) => attempt.attemptedAt)
      .sort()
      .at(-1)!;
    const lastFailure = failedAttempts.at(-1);
    const updatedRecord = {
      ...record,
      state: outboxState,
      acceptedRelayUrls,
      deliveryAttempts: [...record.deliveryAttempts, ...request.attempts],
      attemptCount: record.attemptCount + request.attempts.length,
      lastAttemptAt,
      lastError:
        lastFailure === undefined
          ? null
          : (lastFailure.detail ?? `${lastFailure.outcome}: ${lastFailure.relayUrl}`),
    };
    const revision = state.ledger.revision + 1;
    const eventCursor = `cursor:signed-workroom:${revision}`;
    const ledger = decodeSignedWorkroomLedger({
      ...state.ledger,
      revision,
      eventCursor,
      outbox: state.ledger.outbox.map((candidate, index) =>
        index === recordIndex ? updatedRecord : candidate,
      ),
      freshness: { state: "fresh", observedAt: lastAttemptAt },
    });
    const result: SignedWorkroomDeliveryResult = {
      ledger,
      receipt: {
        idempotencyKey: request.idempotencyKey,
        previousRevision: state.ledger.revision,
        revision,
        eventCursor,
        eventRef: request.eventRef,
        outboxState,
        relayAcceptanceIsAuthority: false,
        admittedEffect: false,
      },
    };
    yield* store.save(state.ledger.revision, {
      ledger,
      idempotency: {
        ...state.idempotency,
        [request.idempotencyKey]: { digest: requestDigest, result },
      },
    });
    return result;
  });

export const readSignedWorkroomActivity = (
  state: SignedWorkroomState,
  request: SignedWorkroomReadRequest,
): SignedWorkroomReadResult => {
  const activityRefs = new Set(
    state.ledger.activities
      .filter(
        (activity) =>
          (request.workroomRef == null || activity.workroomRef === request.workroomRef) &&
          (request.workRef == null || activity.workRef === request.workRef),
      )
      .map((activity) => activity.eventRef),
  );
  return {
    ledger: decodeSignedWorkroomLedger({
      ...state.ledger,
      activities: state.ledger.activities.filter((activity) => activityRefs.has(activity.eventRef)),
      outbox: state.ledger.outbox.filter((record) => activityRefs.has(record.activity.eventRef)),
    }),
  };
};

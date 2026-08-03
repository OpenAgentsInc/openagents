import { createHash } from "node:crypto";
import { Context, Effect, Schema as S } from "effect";

import {
  decodeSignedWorkroomLedger,
  type SignedWorkroomActivity,
  type SignedWorkroomEnqueueRequest,
  type SignedWorkroomEnqueueResult,
  type SignedWorkroomLedger,
  type SignedWorkroomReadRequest,
  type SignedWorkroomReadResult,
  SignedWorkroomLedgerSchema,
} from "./generated.ts";

export const SIGNED_WORKROOM_WRITE_CAPABILITY = "capability:workroom-activity:enqueue" as const;

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
      "storage_unavailable",
    ]),
    detail: S.String,
  },
) {}

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

export const enqueueSignedWorkroomActivity = (
  request: SignedWorkroomEnqueueRequest,
  persistedAt: string,
): Effect.Effect<SignedWorkroomEnqueueResult, SignedWorkroomError, SignedWorkroomStateStore> =>
  Effect.gen(function* () {
    const store = yield* SignedWorkroomStateStore;
    const state = (yield* store.load) ?? emptySignedWorkroomState(persistedAt);
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
    if (state.ledger.activities.some((value) => value.eventRef === request.activity.eventRef)) {
      return yield* new SignedWorkroomError({
        reason: "duplicate_event",
        detail: `event ${request.activity.eventRef} already exists`,
      });
    }
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

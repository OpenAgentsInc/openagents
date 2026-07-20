import { Context, Effect, Layer, Result, Schema as S } from "effect";

import {
  IDE_REVIEW_PROJECTION_SCHEMA_LITERAL,
  IdeProjectionRef,
  IdeProjectionTimestamp,
  IdeReviewAvailability,
  IdeReviewItem,
  IdeReviewProjection,
  IdeReviewSource,
  MAX_IDE_REVIEW_ITEMS,
  hasForbiddenIdeProjectionMaterial,
} from "./ide-review-projection.js";

export const MAX_IDE_REVIEW_INPUT_FACTS = 1_000 as const;
export const MAX_IDE_REVIEW_EXPIRY_SECONDS = 300 as const;
export const IDE_REVIEW_LIVE_AGE_MILLIS = 5_000 as const;
export const IDE_REVIEW_CACHED_AGE_MILLIS = 30_000 as const;
export const IDE_REVIEW_FUTURE_SKEW_MILLIS = 5_000 as const;

const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
const PositiveExpirySeconds = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(MAX_IDE_REVIEW_EXPIRY_SECONDS),
);

export const IdeReviewAccessScope = S.TaggedUnion({
  Owner: {
    authenticated: S.Boolean,
    ownerRef: IdeProjectionRef,
    actorRef: IdeProjectionRef,
  },
  NamedAudience: {
    authenticated: S.Boolean,
    ownerRef: IdeProjectionRef,
    actorRef: IdeProjectionRef,
    audienceScopeRef: IdeProjectionRef,
    audienceActorRefs: S.Array(IdeProjectionRef).check(S.isMaxLength(64)),
  },
}).annotate({ identifier: "IdeReviewAccessScope" });
export type IdeReviewAccessScope = typeof IdeReviewAccessScope.Type;

export const IdeReviewProjectorRequest = S.Struct({
  projectionRef: IdeProjectionRef,
  access: IdeReviewAccessScope,
  source: IdeReviewSource,
  availability: IdeReviewAvailability,
  facts: S.Array(S.Unknown).check(S.isMaxLength(MAX_IDE_REVIEW_INPUT_FACTS)),
  upstreamOmittedCount: NonNegativeInt,
  nextCursorRef: S.optionalKey(IdeProjectionRef),
  sourceSequence: NonNegativeInt,
  lastContiguousSequence: S.optionalKey(NonNegativeInt),
  observedAt: IdeProjectionTimestamp,
  asOf: IdeProjectionTimestamp,
  expiresInSeconds: PositiveExpirySeconds,
}).annotate({ identifier: "IdeReviewProjectorRequest" });
export interface IdeReviewProjectorRequest extends S.Schema.Type<
  typeof IdeReviewProjectorRequest
> {}

export const IdeReviewProjectorFailureReason = S.Literals([
  "invalid_request",
  "forbidden_material",
  "unauthenticated",
  "audience_denied",
  "invalid_fact",
  "replayed_sequence",
  "future_observation",
  "invalid_projection",
]);
export type IdeReviewProjectorFailureReason = typeof IdeReviewProjectorFailureReason.Type;

export class IdeReviewProjectorFailure extends S.TaggedErrorClass<IdeReviewProjectorFailure>()(
  "IdeReviewProjectorFailure",
  {
    operation: S.Literal("IdeReviewProjector.project"),
    reason: IdeReviewProjectorFailureReason,
    itemIndex: S.optionalKey(NonNegativeInt),
  },
) {}

const failure = (
  reason: IdeReviewProjectorFailureReason,
  itemIndex?: number,
): IdeReviewProjectorFailure =>
  new IdeReviewProjectorFailure({
    operation: "IdeReviewProjector.project",
    reason,
    ...(itemIndex === undefined ? {} : { itemIndex }),
  });

const decodeRequest = S.decodeUnknownResult(IdeReviewProjectorRequest);
const decodeFact = S.decodeUnknownResult(IdeReviewItem);
const decodeProjection = S.decodeUnknownResult(IdeReviewProjection);

const strictOptions = { onExcessProperty: "error" as const };

const authorizedAudience = (
  access: IdeReviewAccessScope,
):
  | {
      readonly audience: "owner_authenticated" | "named_audience_authenticated";
      readonly audienceScopeRef: IdeProjectionRef;
    }
  | undefined => {
  if (!access.authenticated) {
    return undefined;
  }
  if (IdeReviewAccessScope.guards.Owner(access)) {
    return access.actorRef === access.ownerRef
      ? { audience: "owner_authenticated", audienceScopeRef: access.ownerRef }
      : undefined;
  }
  return access.audienceActorRefs.includes(access.actorRef)
    ? {
        audience: "named_audience_authenticated",
        audienceScopeRef: access.audienceScopeRef,
      }
    : undefined;
};

/** Compile internal facts without acquiring time, network, storage, or client authority. */
export const compileIdeReviewProjection = (
  input: unknown,
): Result.Result<IdeReviewProjection, IdeReviewProjectorFailure> => {
  if (hasForbiddenIdeProjectionMaterial(input)) {
    return Result.fail(failure("forbidden_material"));
  }

  const requestResult = decodeRequest(input, strictOptions);
  if (Result.isFailure(requestResult)) {
    return Result.fail(failure("invalid_request"));
  }
  const request = requestResult.success;

  if (!request.access.authenticated) {
    return Result.fail(failure("unauthenticated"));
  }
  const authorized = authorizedAudience(request.access);
  if (authorized === undefined) {
    return Result.fail(failure("audience_denied"));
  }

  const asOfMillis = Date.parse(request.asOf);
  const observedAtMillis = Date.parse(request.observedAt);
  if (!Number.isFinite(asOfMillis) || !Number.isFinite(observedAtMillis)) {
    return Result.fail(failure("invalid_request"));
  }
  const ageMillis = asOfMillis - observedAtMillis;
  if (ageMillis < -IDE_REVIEW_FUTURE_SKEW_MILLIS) {
    return Result.fail(failure("future_observation"));
  }

  const decodedFacts: Array<IdeReviewItem> = [];
  for (const [index, fact] of request.facts.entries()) {
    const factResult = decodeFact(fact, strictOptions);
    if (Result.isFailure(factResult)) {
      return Result.fail(failure("invalid_fact", index));
    }
    decodedFacts.push(factResult.success);
  }

  let gapAfterSequence: number | undefined;
  if (request.lastContiguousSequence !== undefined) {
    if (request.sourceSequence <= request.lastContiguousSequence) {
      return Result.fail(failure("replayed_sequence"));
    }
    if (request.sourceSequence > request.lastContiguousSequence + 1) {
      gapAfterSequence = request.lastContiguousSequence;
    }
  }

  const locallyOmittedCount = Math.max(0, decodedFacts.length - MAX_IDE_REVIEW_ITEMS);
  const omittedCount = request.upstreamOmittedCount + locallyOmittedCount;
  const truncated = omittedCount > 0;
  if (truncated !== (request.nextCursorRef !== undefined)) {
    return Result.fail(failure("invalid_request"));
  }

  const freshness =
    gapAfterSequence !== undefined
      ? {
          state: "gap" as const,
          observedAt: request.observedAt,
          sourceSequence: request.sourceSequence,
          gapAfterSequence,
        }
      : {
          state:
            ageMillis <= IDE_REVIEW_LIVE_AGE_MILLIS
              ? ("live" as const)
              : ageMillis <= IDE_REVIEW_CACHED_AGE_MILLIS
                ? ("cached" as const)
                : ("stale" as const),
          observedAt: request.observedAt,
          sourceSequence: request.sourceSequence,
        };

  const projectionResult = decodeProjection(
    {
      schema: IDE_REVIEW_PROJECTION_SCHEMA_LITERAL,
      projectionRef: request.projectionRef,
      audience: authorized.audience,
      audienceScopeRef: authorized.audienceScopeRef,
      source: request.source,
      freshness,
      availability: request.availability,
      items: decodedFacts.slice(0, MAX_IDE_REVIEW_ITEMS),
      omittedCount,
      truncated,
      ...(request.nextCursorRef === undefined ? {} : { nextCursorRef: request.nextCursorRef }),
      generatedAt: request.asOf,
      expiresAt: new Date(asOfMillis + request.expiresInSeconds * 1_000).toISOString(),
    },
    strictOptions,
  );
  return Result.isFailure(projectionResult)
    ? Result.fail(failure("invalid_projection"))
    : Result.succeed(projectionResult.success);
};

export interface IdeReviewProjectorInterface {
  readonly project: (
    input: unknown,
  ) => Effect.Effect<IdeReviewProjection, IdeReviewProjectorFailure>;
}

export class IdeReviewProjector extends Context.Service<
  IdeReviewProjector,
  IdeReviewProjectorInterface
>()("ide-runtime.IdeReviewProjector") {}

const project = Effect.fn("IdeReviewProjector.project")((input: unknown) =>
  Effect.fromResult(compileIdeReviewProjection(input)),
);

export const IdeReviewProjectorLayer = Layer.effect(
  IdeReviewProjector,
  Effect.succeed(IdeReviewProjector.of({ project })),
);

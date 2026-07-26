import { Context, Effect, Layer, Schema as S } from "effect";

export const FORGE_COLLABORATION_SCHEMA = "openagents.forge.collaboration_read.v1" as const;

const NonEmpty = S.String.check(S.isMinLength(1));
const IsoTime = NonEmpty;

export const ForgeCollaborationRequest = S.Struct({
  owner: NonEmpty,
  repo: NonEmpty,
  view: S.Literals(["change", "work", "attention"]),
  changeRef: S.optionalKey(NonEmpty),
  workRef: S.optionalKey(NonEmpty),
});
export interface ForgeCollaborationRequest extends S.Schema.Type<typeof ForgeCollaborationRequest> {}

const ForgeSource = S.Struct({
  eventId: NonEmpty,
  kind: S.Number,
  author: NonEmpty,
  observedAt: IsoTime,
  freshness: S.Literals(["fresh", "stale", "unknown"]),
});

const ForgeSourceBackedText = S.Struct({ value: NonEmpty, sources: S.NonEmptyArray(ForgeSource) });

const ForgeNamedState = S.Struct({
  label: NonEmpty,
  state: S.Literals(["open", "draft", "ready", "blocked", "applied", "closed", "passed", "failed", "stale", "unknown"]),
  detail: S.String,
  source: ForgeSource,
});

const ForgeComment = S.Struct({
  commentRef: NonEmpty,
  body: S.String,
  author: NonEmpty,
  createdAt: IsoTime,
  source: S.Struct({ ...ForgeSource.fields, kind: S.Literal(1111) }),
});

const ForgeCheck = S.Struct({
  checkRef: NonEmpty,
  name: NonEmpty,
  state: S.Literals(["passed", "failed", "stale", "running", "unknown"]),
  completedAt: S.optionalKey(IsoTime),
  receiptRef: S.optionalKey(NonEmpty),
  source: ForgeSource,
});

const ForgeReceipt = S.Struct({
  receiptRef: NonEmpty,
  kind: S.Literals(["verification", "merge", "promotion", "mirror"]),
  summary: NonEmpty,
  createdAt: IsoTime,
  source: ForgeSource,
});

const ForgeChange = S.Struct({
  changeRef: NonEmpty,
  title: NonEmpty,
  proposalDialect: S.Literals(["standard_1617", "pointer_pr", "pointer_pr_legacy"]),
  proposalResolution: S.Literals(["resolved", "unresolved", "disagreement"]),
  base: ForgeSourceBackedText,
  head: ForgeSourceBackedText,
  state: ForgeNamedState,
  reviews: S.Array(ForgeNamedState),
  comments: S.Array(ForgeComment),
  checks: S.Array(ForgeCheck),
  receipts: S.Array(ForgeReceipt),
  merge: S.NullOr(
    S.Struct({
      outcome: S.Literals(["merged", "blocked", "pending"]),
      signedReceipt: S.optionalKey(ForgeReceipt),
      source: ForgeSource,
    }),
  ),
});

const ForgeWork = S.Struct({
  workRef: NonEmpty,
  title: NonEmpty,
  objective: ForgeSourceBackedText,
  actor: ForgeSourceBackedText,
  state: ForgeNamedState,
  blockers: S.Array(ForgeSourceBackedText),
  targetChangeRef: S.NullOr(NonEmpty),
});

const ForgeAttentionItem = S.Struct({
  attentionRef: NonEmpty,
  kind: S.Literals([
    "review_request",
    "check_failed",
    "check_stale",
    "work_blocked",
    "decision_required",
    "disagreement",
  ]),
  title: NonEmpty,
  detail: NonEmpty,
  target: NonEmpty,
  actorRequired: S.optionalKey(NonEmpty),
  source: ForgeSource,
});

export const ForgeCollaborationProjection = S.Struct({
  schema: S.Literal(FORGE_COLLABORATION_SCHEMA),
  servedAt: IsoTime,
  repository: S.Struct({ owner: NonEmpty, name: NonEmpty, repositoryRef: NonEmpty }),
  change: S.NullOr(ForgeChange),
  work: S.NullOr(ForgeWork),
  attention: S.Array(ForgeAttentionItem),
});
export interface ForgeCollaborationProjection extends S.Schema.Type<
  typeof ForgeCollaborationProjection
> {}

export const ForgeCollaborationFailure = S.TaggedUnion({
  not_found: { detail: NonEmpty },
  authentication_required: { detail: NonEmpty },
  unavailable: { detail: NonEmpty, retryable: S.Boolean },
  malformed_response: { detail: NonEmpty },
});
export type ForgeCollaborationFailure = S.Schema.Type<typeof ForgeCollaborationFailure>;

export const ForgeCollaborationResult = S.TaggedUnion({
  loaded: { projection: ForgeCollaborationProjection },
  failed: { failure: ForgeCollaborationFailure },
});
export type ForgeCollaborationResult = S.Schema.Type<typeof ForgeCollaborationResult>;

export class ForgeCollaborationTransportError extends S.TaggedErrorClass<ForgeCollaborationTransportError>()(
  "ForgeCollaborationTransportError",
  { operation: S.String, cause: S.Defect() },
) {}

export interface ForgeCollaborationReaderInterface {
  readonly read: (
    request: ForgeCollaborationRequest,
    authorizationCookie: string | undefined,
  ) => Effect.Effect<ForgeCollaborationResult, ForgeCollaborationTransportError>;
}

export class ForgeCollaborationReader extends Context.Service<
  ForgeCollaborationReader,
  ForgeCollaborationReaderInterface
>()("@openagents.com/ForgeCollaborationReader") {}

export const makeForgeCollaborationReaderTestLayer = (
  read: ForgeCollaborationReaderInterface["read"],
) => Layer.succeed(ForgeCollaborationReader, ForgeCollaborationReader.of({ read }));

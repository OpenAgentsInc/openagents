import { createHash } from "node:crypto";

import { Context, Effect, Layer, Schema as S } from "effect";

import { OrganizationRefSchema, WorkRefSchema } from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

export const FORENSIC_PRIOR_WORK_STATE_SCHEMA =
  "openagents.all_work_forensic_prior_work_state.v1" as const;
export const FORENSIC_OCCURRENCE_IDENTITY_SCHEMA =
  "openagents.forensic_occurrence_identity.v1" as const;
export const FORENSIC_ROOT_CAUSE_IDENTITY_SCHEMA =
  "openagents.forensic_root_cause_identity.v1" as const;
export const FORENSIC_PRIOR_WORK_QUERY_RECEIPT_SCHEMA =
  "openagents.forensic_prior_work_query_receipt.v1" as const;
export const FORENSIC_OCCURRENCE_ALGORITHM = "forensic-occurrence-sha256-v1" as const;
export const FORENSIC_ROOT_CAUSE_ALGORITHM = "forensic-root-cause-semantic-v1" as const;

const RefSchema = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z][A-Za-z0-9._:/-]*$/u),
);
const DigestSchema = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/u));
const RevisionSchema = S.String.check(S.isPattern(/^[a-f0-9]{40}$/u));
const TimestampSchema = S.String.check(S.isMinLength(10), S.isMaxLength(64));
const BoundedTextSchema = S.String.check(S.isMinLength(1), S.isMaxLength(4_096));
const BoundedRefsSchema = S.Array(RefSchema).check(S.isMaxLength(512));

export const ForensicWorkDispositionSchema = S.Literals([
  "confirmed",
  "dismissed",
  "rejected",
  "inconclusive",
  "expired",
  "superseded",
  "corrected",
  "duplicate",
  "retained",
]);
export type ForensicWorkDisposition = typeof ForensicWorkDispositionSchema.Type;

export const ForensicOccurrenceIdentitySchema = S.Struct({
  schema: S.Literal(FORENSIC_OCCURRENCE_IDENTITY_SCHEMA),
  algorithmVersion: S.Literal(FORENSIC_OCCURRENCE_ALGORITHM),
  occurrenceRef: RefSchema,
  repositoryRef: RefSchema,
  revision: RevisionSchema,
  path: S.String.check(S.isMinLength(1), S.isMaxLength(4_096)),
  symbol: S.NullOr(S.String.check(S.isMinLength(1), S.isMaxLength(512))),
  startLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  endLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  sourceWindowDigest: DigestSchema,
});
export interface ForensicOccurrenceIdentity extends S.Schema.Type<
  typeof ForensicOccurrenceIdentitySchema
> {}

export const ForensicRootCauseIdentitySchema = S.Struct({
  schema: S.Literal(FORENSIC_ROOT_CAUSE_IDENTITY_SCHEMA),
  algorithmVersion: S.Literal(FORENSIC_ROOT_CAUSE_ALGORITHM),
  rootCauseRef: RefSchema,
  mechanismClass: RefSchema,
  causalMechanism: BoundedTextSchema,
  affectedBehavior: BoundedTextSchema,
  securityBoundary: BoundedTextSchema,
  normalizedMechanismDigest: DigestSchema,
});
export interface ForensicRootCauseIdentity extends S.Schema.Type<
  typeof ForensicRootCauseIdentitySchema
> {}

export const ForensicWorkAudienceSchema = S.Struct({
  visibility: S.Literals(["public", "organization", "private"]),
  organizationRef: S.NullOr(OrganizationRefSchema),
  principalRef: S.NullOr(RefSchema),
}).pipe(
  S.check(
    S.makeFilter(
      (audience) =>
        (audience.visibility === "public" &&
          audience.organizationRef === null &&
          audience.principalRef === null) ||
        (audience.visibility === "organization" &&
          audience.organizationRef !== null &&
          audience.principalRef === null) ||
        (audience.visibility === "private" &&
          audience.organizationRef === null &&
          audience.principalRef !== null),
      { message: "forensic Work audience fields must match visibility" },
    ),
  ),
);
export interface ForensicWorkAudience extends S.Schema.Type<typeof ForensicWorkAudienceSchema> {}

export const ForensicDispositionEventSchema = S.Struct({
  eventRef: RefSchema,
  workRef: WorkRefSchema,
  disposition: ForensicWorkDispositionSchema,
  reason: BoundedTextSchema,
  actorRef: RefSchema,
  occurredAt: TimestampSchema,
  idempotencyRef: RefSchema,
});
export interface ForensicDispositionEvent extends S.Schema.Type<
  typeof ForensicDispositionEventSchema
> {}

export const ForensicWorkRelationEventSchema = S.Struct({
  relationRef: RefSchema,
  fromWorkRef: WorkRefSchema,
  kind: S.Literals(["duplicate", "related", "supersedes", "split_from", "merged_into"]),
  targetWorkRef: WorkRefSchema,
  confidence: S.Literals(["exact", "probable", "possible"]),
  reason: BoundedTextSchema,
  actorRef: RefSchema,
  occurredAt: TimestampSchema,
  idempotencyRef: RefSchema,
});
export interface ForensicWorkRelationEvent extends S.Schema.Type<
  typeof ForensicWorkRelationEventSchema
> {}

export const ForensicPriorWorkRecordSchema = S.Struct({
  recordRef: RefSchema,
  rootCause: ForensicRootCauseIdentitySchema,
  primaryWorkRef: WorkRefSchema,
  workRefs: S.Array(WorkRefSchema).check(S.isMinLength(1), S.isMaxLength(512)),
  occurrences: S.Array(ForensicOccurrenceIdentitySchema).check(
    S.isMinLength(1),
    S.isMaxLength(4_096),
  ),
  audience: ForensicWorkAudienceSchema,
  causalChainSummary: BoundedTextSchema,
  promptRefs: BoundedRefsSchema,
  sourceRefs: BoundedRefsSchema,
  evidenceRefs: BoundedRefsSchema,
  dispositions: S.Array(ForensicDispositionEventSchema).check(
    S.isMinLength(1),
    S.isMaxLength(4_096),
  ),
  relations: S.Array(ForensicWorkRelationEventSchema).check(S.isMaxLength(4_096)),
  firstIdentifiedAt: TimestampSchema,
  updatedAt: TimestampSchema,
});
export interface ForensicPriorWorkRecord extends S.Schema.Type<
  typeof ForensicPriorWorkRecordSchema
> {}

export const ForensicPriorWorkQuerySchema = S.Struct({
  queryRef: RefSchema,
  principalRef: RefSchema,
  organizationRefs: S.Array(OrganizationRefSchema).check(S.isMaxLength(128)),
  includePublic: S.Boolean,
  mode: S.Literals(["exact", "semantic"]),
  exactRef: S.NullOr(RefSchema),
  text: S.NullOr(S.String.check(S.isMinLength(2), S.isMaxLength(2_048))),
  dispositionFilter: S.Array(ForensicWorkDispositionSchema).check(S.isMinLength(1)),
  cursor: S.NullOr(S.String.check(S.isMaxLength(512))),
  limit: S.Int.check(S.isGreaterThanOrEqualTo(1), S.isLessThanOrEqualTo(100)),
});
export interface ForensicPriorWorkQuery extends S.Schema.Type<
  typeof ForensicPriorWorkQuerySchema
> {}

export const ForensicPriorWorkMatchSchema = S.Struct({
  record: ForensicPriorWorkRecordSchema,
  matchedWorkRefs: S.Array(WorkRefSchema),
  matchedOccurrenceRefs: BoundedRefsSchema,
  scoreBasisPoints: S.Int.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(10_000)),
});
export interface ForensicPriorWorkMatch extends S.Schema.Type<
  typeof ForensicPriorWorkMatchSchema
> {}

export const ForensicPriorWorkQueryReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_PRIOR_WORK_QUERY_RECEIPT_SCHEMA),
  receiptRef: RefSchema,
  queryRef: RefSchema,
  stateRevision: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  queryDigest: DigestSchema,
  resultDigest: DigestSchema,
  authorizedPopulationComplete: S.Boolean,
  lossRefs: BoundedRefsSchema,
  searchedAuthorizedCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  returnedCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  observedAt: TimestampSchema,
});
export interface ForensicPriorWorkQueryReceipt extends S.Schema.Type<
  typeof ForensicPriorWorkQueryReceiptSchema
> {}

export const ForensicPriorWorkQueryResultSchema = S.Struct({
  matches: S.Array(ForensicPriorWorkMatchSchema).check(S.isMaxLength(100)),
  nextCursor: S.NullOr(S.String),
  receipt: ForensicPriorWorkQueryReceiptSchema,
});
export interface ForensicPriorWorkQueryResult extends S.Schema.Type<
  typeof ForensicPriorWorkQueryResultSchema
> {}

export const ForensicPriorWorkStateSchema = S.Struct({
  schema: S.Literal(FORENSIC_PRIOR_WORK_STATE_SCHEMA),
  revision: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  records: S.Array(ForensicPriorWorkRecordSchema).check(S.isMaxLength(100_000)),
  queryReceipts: S.Array(ForensicPriorWorkQueryReceiptSchema).check(S.isMaxLength(10_000)),
  commandDigests: S.Record(RefSchema, DigestSchema),
});
export interface ForensicPriorWorkState extends S.Schema.Type<
  typeof ForensicPriorWorkStateSchema
> {}

export const ForensicPriorWorkSubmissionSchema = S.Struct({
  workRef: WorkRefSchema,
  repositoryRef: RefSchema,
  revision: RevisionSchema,
  path: S.String.check(S.isMinLength(1), S.isMaxLength(4_096)),
  symbol: S.NullOr(S.String.check(S.isMinLength(1), S.isMaxLength(512))),
  startLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  endLine: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  sourceWindowDigest: DigestSchema,
  mechanismClass: RefSchema,
  causalMechanism: BoundedTextSchema,
  affectedBehavior: BoundedTextSchema,
  securityBoundary: BoundedTextSchema,
  causalChainSummary: BoundedTextSchema,
  promptRefs: BoundedRefsSchema,
  sourceRefs: BoundedRefsSchema,
  evidenceRefs: BoundedRefsSchema,
  audience: ForensicWorkAudienceSchema,
  disposition: ForensicWorkDispositionSchema,
  actorRef: RefSchema,
  submittedAt: TimestampSchema,
  idempotencyRef: RefSchema,
});
export interface ForensicPriorWorkSubmission extends S.Schema.Type<
  typeof ForensicPriorWorkSubmissionSchema
> {}

export const ForensicRelationCommandSchema = S.Struct({
  fromWorkRef: WorkRefSchema,
  targetWorkRef: WorkRefSchema,
  kind: S.Literals(["duplicate", "related", "supersedes", "split_from", "merged_into"]),
  confidence: S.Literals(["exact", "probable", "possible"]),
  reason: BoundedTextSchema,
  actorRef: RefSchema,
  occurredAt: TimestampSchema,
  idempotencyRef: RefSchema,
});
export interface ForensicRelationCommand extends S.Schema.Type<
  typeof ForensicRelationCommandSchema
> {}

export const ForensicDispositionCommandSchema = S.Struct({
  workRef: WorkRefSchema,
  disposition: ForensicWorkDispositionSchema,
  reason: BoundedTextSchema,
  actorRef: RefSchema,
  occurredAt: TimestampSchema,
  idempotencyRef: RefSchema,
});
export interface ForensicDispositionCommand extends S.Schema.Type<
  typeof ForensicDispositionCommandSchema
> {}

const digest = (value: unknown): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex")}`;
const normalize = (value: string): string =>
  value.normalize("NFKC").trim().toLowerCase().replace(/\s+/gu, " ");
const audienceKey = (audience: ForensicWorkAudience): string =>
  `${audience.visibility}:${audience.organizationRef ?? audience.principalRef ?? "public"}`;
const sortedUnique = <A extends string>(values: ReadonlyArray<A>): Array<A> =>
  [...new Set(values)].sort();
const decode = <Schema extends S.ConstraintDecoder<unknown, never>>(
  schema: Schema,
  input: unknown,
): Schema["Type"] => S.decodeUnknownSync(schema)(input, { onExcessProperty: "error" });

export const deriveForensicOccurrenceIdentity = (
  input: Pick<
    ForensicPriorWorkSubmission,
    | "repositoryRef"
    | "revision"
    | "path"
    | "symbol"
    | "startLine"
    | "endLine"
    | "sourceWindowDigest"
  >,
): ForensicOccurrenceIdentity => {
  if (input.endLine < input.startLine) throw new Error("occurrence line range is reversed");
  const identity = {
    algorithmVersion: FORENSIC_OCCURRENCE_ALGORITHM,
    repositoryRef: input.repositoryRef,
    revision: input.revision,
    path: input.path.normalize("NFKC"),
    symbol: input.symbol?.normalize("NFKC") ?? null,
    startLine: input.startLine,
    endLine: input.endLine,
    sourceWindowDigest: input.sourceWindowDigest,
  };
  return decode(ForensicOccurrenceIdentitySchema, {
    schema: FORENSIC_OCCURRENCE_IDENTITY_SCHEMA,
    ...identity,
    occurrenceRef: `occurrence:${digest(identity).slice(7, 39)}`,
  });
};

export const deriveForensicRootCauseIdentity = (
  input: Pick<
    ForensicPriorWorkSubmission,
    "mechanismClass" | "causalMechanism" | "affectedBehavior" | "securityBoundary"
  >,
): ForensicRootCauseIdentity => {
  const normalized = {
    algorithmVersion: FORENSIC_ROOT_CAUSE_ALGORITHM,
    mechanismClass: input.mechanismClass,
    causalMechanism: normalize(input.causalMechanism),
    affectedBehavior: normalize(input.affectedBehavior),
    securityBoundary: normalize(input.securityBoundary),
  };
  const normalizedMechanismDigest = digest(normalized);
  return decode(ForensicRootCauseIdentitySchema, {
    schema: FORENSIC_ROOT_CAUSE_IDENTITY_SCHEMA,
    ...normalized,
    normalizedMechanismDigest,
    rootCauseRef: `root-cause:${normalizedMechanismDigest.slice(7, 39)}`,
  });
};

const authorized = (record: ForensicPriorWorkRecord, query: ForensicPriorWorkQuery): boolean =>
  (record.audience.visibility === "public" && query.includePublic) ||
  (record.audience.visibility === "organization" &&
    record.audience.organizationRef !== null &&
    query.organizationRefs.includes(record.audience.organizationRef)) ||
  (record.audience.visibility === "private" && record.audience.principalRef === query.principalRef);

const tokens = (value: string): Set<string> =>
  new Set(
    normalize(value)
      .split(/[^a-z0-9_.:-]+/u)
      .filter((token) => token.length > 1),
  );
const semanticScore = (record: ForensicPriorWorkRecord, text: string): number => {
  const queryTokens = tokens(text);
  const recordTokens = tokens(
    [
      record.rootCause.mechanismClass,
      record.rootCause.causalMechanism,
      record.rootCause.affectedBehavior,
      record.rootCause.securityBoundary,
      record.causalChainSummary,
    ].join(" "),
  );
  if (queryTokens.size === 0) return 0;
  const overlap = [...queryTokens].filter((token) => recordTokens.has(token)).length;
  return Math.floor((overlap * 10_000) / queryTokens.size);
};

export class ForensicPriorWorkAuthorityError extends S.TaggedErrorClass<ForensicPriorWorkAuthorityError>()(
  "ForensicPriorWorkAuthority.Error",
  {
    reason: S.Literals([
      "invalid_request",
      "invalid_state",
      "storage_unavailable",
      "revision_conflict",
      "idempotency_conflict",
      "work_not_found",
      "forbidden",
      "cursor_stale",
    ]),
    detail: S.String,
  },
) {}

export interface ForensicPriorWorkStateStoreShape {
  readonly load: Effect.Effect<ForensicPriorWorkState | null, ForensicPriorWorkAuthorityError>;
  readonly save: (
    expectedRevision: number,
    state: ForensicPriorWorkState,
  ) => Effect.Effect<void, ForensicPriorWorkAuthorityError>;
}
export class ForensicPriorWorkStateStore extends Context.Service<
  ForensicPriorWorkStateStore,
  ForensicPriorWorkStateStoreShape
>()("ForensicPriorWorkStateStore.Service") {}

export interface ForensicPriorWorkAuthorityShape {
  readonly query: (
    input: unknown,
  ) => Effect.Effect<ForensicPriorWorkQueryResult, ForensicPriorWorkAuthorityError>;
  readonly submit: (
    input: unknown,
  ) => Effect.Effect<ForensicPriorWorkRecord, ForensicPriorWorkAuthorityError>;
  readonly relate: (
    input: unknown,
  ) => Effect.Effect<ForensicPriorWorkRecord, ForensicPriorWorkAuthorityError>;
  readonly dispose: (
    input: unknown,
  ) => Effect.Effect<ForensicPriorWorkRecord, ForensicPriorWorkAuthorityError>;
}
export class ForensicPriorWorkAuthority extends Context.Service<
  ForensicPriorWorkAuthority,
  ForensicPriorWorkAuthorityShape
>()("ForensicPriorWorkAuthority.Service") {}

const invalid = (reason: ForensicPriorWorkAuthorityError["reason"], detail: string) =>
  new ForensicPriorWorkAuthorityError({ reason, detail });
const decodeEffect = <Schema extends S.ConstraintDecoder<unknown, never>>(
  schema: Schema,
  input: unknown,
): Effect.Effect<Schema["Type"], ForensicPriorWorkAuthorityError> =>
  S.decodeUnknownEffect(schema)(input, { onExcessProperty: "error" }).pipe(
    Effect.mapError(() => invalid("invalid_request", "decode")),
  );

export const emptyForensicPriorWorkState = (): ForensicPriorWorkState =>
  decode(ForensicPriorWorkStateSchema, {
    schema: FORENSIC_PRIOR_WORK_STATE_SCHEMA,
    revision: 0,
    records: [],
    queryReceipts: [],
    commandDigests: {},
  });

const appendCommandDigest = (
  state: ForensicPriorWorkState,
  idempotencyRef: string,
  commandDigest: `sha256:${string}`,
): Record<string, string> => {
  const prior = state.commandDigests[idempotencyRef];
  if (prior !== undefined && prior !== commandDigest) {
    throw invalid("idempotency_conflict", idempotencyRef);
  }
  return { ...state.commandDigests, [idempotencyRef]: commandDigest };
};

export const ForensicPriorWorkAuthorityLive = Layer.effect(
  ForensicPriorWorkAuthority,
  Effect.gen(function* () {
    const store = yield* ForensicPriorWorkStateStore;
    const load = store.load.pipe(
      Effect.flatMap((state) =>
        state === null
          ? Effect.fail(invalid("invalid_state", "store is empty"))
          : Effect.succeed(state),
      ),
    );
    return ForensicPriorWorkAuthority.of({
      query: Effect.fn("ForensicPriorWorkAuthority.query")(function* (input: unknown) {
        const query = yield* decodeEffect(ForensicPriorWorkQuerySchema, input);
        const state = yield* load;
        const authorizedRecords = state.records.filter((record) => authorized(record, query));
        const dispositions = new Set(query.dispositionFilter);
        const candidates = authorizedRecords
          .filter((record) =>
            record.dispositions.some((event) => dispositions.has(event.disposition)),
          )
          .map((record) => {
            const exact = query.exactRef;
            const matchedWorkRefs =
              exact === null ? [] : record.workRefs.filter((ref) => ref === exact);
            const matchedOccurrenceRefs =
              exact === null
                ? []
                : record.occurrences
                    .filter((occurrence) => occurrence.occurrenceRef === exact)
                    .map((occurrence) => occurrence.occurrenceRef);
            const exactMatch =
              exact !== null &&
              (record.recordRef === exact ||
                record.rootCause.rootCauseRef === exact ||
                matchedWorkRefs.length > 0 ||
                matchedOccurrenceRefs.length > 0);
            const scoreBasisPoints =
              query.mode === "exact"
                ? exactMatch
                  ? 10_000
                  : 0
                : semanticScore(record, query.text ?? "");
            return { record, matchedWorkRefs, matchedOccurrenceRefs, scoreBasisPoints };
          })
          .filter((match) => match.scoreBasisPoints > 0)
          .sort(
            (left, right) =>
              right.scoreBasisPoints - left.scoreBasisPoints ||
              left.record.recordRef.localeCompare(right.record.recordRef),
          );
        const queryDigest = digest({ ...query, cursor: null });
        let offset = 0;
        if (query.cursor !== null) {
          const match = /^cursor:forensic:([a-f0-9]{16}):(\d+):(\d+)$/u.exec(query.cursor);
          if (
            match === null ||
            match[1] !== queryDigest.slice(7, 23) ||
            Number(match[2]) !== state.revision
          ) {
            return yield* invalid("cursor_stale", "query cursor does not match this revision");
          }
          offset = Number(match[3]);
        }
        const matches = candidates.slice(offset, offset + query.limit);
        const nextOffset = offset + matches.length;
        const nextCursor =
          nextOffset < candidates.length
            ? `cursor:forensic:${queryDigest.slice(7, 23)}:${state.revision}:${nextOffset}`
            : null;
        const resultDigest = digest(matches);
        const receipt = decode(ForensicPriorWorkQueryReceiptSchema, {
          schema: FORENSIC_PRIOR_WORK_QUERY_RECEIPT_SCHEMA,
          receiptRef: `receipt:forensic-query:${digest({ queryDigest, resultDigest, revision: state.revision }).slice(7, 31)}`,
          queryRef: query.queryRef,
          stateRevision: state.revision,
          queryDigest,
          resultDigest,
          authorizedPopulationComplete: true,
          lossRefs: [],
          searchedAuthorizedCount: authorizedRecords.length,
          returnedCount: matches.length,
          observedAt: new Date().toISOString(),
        });
        return decode(ForensicPriorWorkQueryResultSchema, { matches, nextCursor, receipt });
      }),
      submit: (() => {
        const once = Effect.fn("ForensicPriorWorkAuthority.submit")(function* (input: unknown) {
          const command = yield* decodeEffect(ForensicPriorWorkSubmissionSchema, input);
          const state = yield* load;
          const commandDigest = digest(command);
          const priorDigest = state.commandDigests[command.idempotencyRef];
          if (priorDigest !== undefined) {
            if (priorDigest !== commandDigest)
              return yield* invalid("idempotency_conflict", command.idempotencyRef);
            const replay = state.records.find((record) =>
              record.workRefs.includes(command.workRef),
            );
            if (replay === undefined)
              return yield* invalid("invalid_state", "idempotent result missing");
            return replay;
          }
          const occurrence = deriveForensicOccurrenceIdentity(command);
          const rootCause = deriveForensicRootCauseIdentity(command);
          const existingIndex = state.records.findIndex(
            (record) =>
              record.rootCause.rootCauseRef === rootCause.rootCauseRef &&
              audienceKey(record.audience) === audienceKey(command.audience),
          );
          const disposition = decode(ForensicDispositionEventSchema, {
            eventRef: `event:forensic-disposition:${commandDigest.slice(7, 31)}`,
            workRef: command.workRef,
            disposition: existingIndex >= 0 ? "duplicate" : command.disposition,
            reason:
              existingIndex >= 0
                ? "Same versioned causal mechanism within this audience"
                : "Initial submission",
            actorRef: command.actorRef,
            occurredAt: command.submittedAt,
            idempotencyRef: command.idempotencyRef,
          });
          const records = [...state.records];
          let record: ForensicPriorWorkRecord;
          if (existingIndex < 0) {
            record = decode(ForensicPriorWorkRecordSchema, {
              recordRef: `forensic-record:${digest({ rootCause: rootCause.rootCauseRef, audience: audienceKey(command.audience) }).slice(7, 39)}`,
              rootCause,
              primaryWorkRef: command.workRef,
              workRefs: [command.workRef],
              occurrences: [occurrence],
              audience: command.audience,
              causalChainSummary: command.causalChainSummary,
              promptRefs: sortedUnique(command.promptRefs),
              sourceRefs: sortedUnique(command.sourceRefs),
              evidenceRefs: sortedUnique(command.evidenceRefs),
              dispositions: [disposition],
              relations: [],
              firstIdentifiedAt: command.submittedAt,
              updatedAt: command.submittedAt,
            });
            records.push(record);
          } else {
            const existing = records[existingIndex]!;
            const relation = decode(ForensicWorkRelationEventSchema, {
              relationRef: `relation:forensic-duplicate:${commandDigest.slice(7, 31)}`,
              fromWorkRef: command.workRef,
              kind: "duplicate",
              targetWorkRef: existing.primaryWorkRef,
              confidence: "exact",
              reason: "Versioned root-cause identity matched within the authorized audience",
              actorRef: command.actorRef,
              occurredAt: command.submittedAt,
              idempotencyRef: command.idempotencyRef,
            });
            record = decode(ForensicPriorWorkRecordSchema, {
              ...existing,
              workRefs: sortedUnique([...existing.workRefs, command.workRef]),
              occurrences: [...existing.occurrences, occurrence]
                .filter(
                  (value, index, all) =>
                    all.findIndex(
                      (candidate) => candidate.occurrenceRef === value.occurrenceRef,
                    ) === index,
                )
                .sort((left, right) => left.occurrenceRef.localeCompare(right.occurrenceRef)),
              promptRefs: sortedUnique([...existing.promptRefs, ...command.promptRefs]),
              sourceRefs: sortedUnique([...existing.sourceRefs, ...command.sourceRefs]),
              evidenceRefs: sortedUnique([...existing.evidenceRefs, ...command.evidenceRefs]),
              dispositions: [...existing.dispositions, disposition],
              relations: [...existing.relations, relation],
              firstIdentifiedAt:
                existing.firstIdentifiedAt.localeCompare(command.submittedAt) <= 0
                  ? existing.firstIdentifiedAt
                  : command.submittedAt,
              updatedAt: command.submittedAt,
            });
            records[existingIndex] = record;
          }
          const next = decode(ForensicPriorWorkStateSchema, {
            ...state,
            revision: state.revision + 1,
            records,
            commandDigests: appendCommandDigest(state, command.idempotencyRef, commandDigest),
          });
          yield* store.save(state.revision, next);
          return record;
        });
        const run = (
          input: unknown,
          attemptsRemaining: number,
        ): Effect.Effect<ForensicPriorWorkRecord, ForensicPriorWorkAuthorityError> =>
          once(input).pipe(
            Effect.catch((error) =>
              error.reason === "revision_conflict" && attemptsRemaining > 0
                ? run(input, attemptsRemaining - 1)
                : Effect.fail(error),
            ),
          );
        return (input: unknown) => run(input, 4);
      })(),
      relate: Effect.fn("ForensicPriorWorkAuthority.relate")(function* (input: unknown) {
        const command = yield* decodeEffect(ForensicRelationCommandSchema, input);
        const state = yield* load;
        const commandDigest = digest(command);
        const existingIndex = state.records.findIndex((record) =>
          record.workRefs.includes(command.fromWorkRef),
        );
        if (existingIndex < 0) return yield* invalid("work_not_found", command.fromWorkRef);
        const record = state.records[existingIndex]!;
        if (
          !record.workRefs.includes(command.targetWorkRef) &&
          !state.records.some((candidate) => candidate.workRefs.includes(command.targetWorkRef))
        ) {
          return yield* invalid("work_not_found", command.targetWorkRef);
        }
        const prior = state.commandDigests[command.idempotencyRef];
        if (prior !== undefined) {
          if (prior !== commandDigest)
            return yield* invalid("idempotency_conflict", command.idempotencyRef);
          return record;
        }
        const relation = decode(ForensicWorkRelationEventSchema, {
          relationRef: `relation:forensic:${commandDigest.slice(7, 31)}`,
          ...command,
        });
        const updated = decode(ForensicPriorWorkRecordSchema, {
          ...record,
          relations: [...record.relations, relation],
          updatedAt: command.occurredAt,
        });
        const records = [...state.records];
        records[existingIndex] = updated;
        const next = decode(ForensicPriorWorkStateSchema, {
          ...state,
          revision: state.revision + 1,
          records,
          commandDigests: appendCommandDigest(state, command.idempotencyRef, commandDigest),
        });
        yield* store.save(state.revision, next);
        return updated;
      }),
      dispose: Effect.fn("ForensicPriorWorkAuthority.dispose")(function* (input: unknown) {
        const command = yield* decodeEffect(ForensicDispositionCommandSchema, input);
        const state = yield* load;
        const commandDigest = digest(command);
        const existingIndex = state.records.findIndex((record) =>
          record.workRefs.includes(command.workRef),
        );
        if (existingIndex < 0) return yield* invalid("work_not_found", command.workRef);
        const record = state.records[existingIndex]!;
        const prior = state.commandDigests[command.idempotencyRef];
        if (prior !== undefined) {
          if (prior !== commandDigest) {
            return yield* invalid("idempotency_conflict", command.idempotencyRef);
          }
          return record;
        }
        const event = decode(ForensicDispositionEventSchema, {
          eventRef: `event:forensic-disposition:${commandDigest.slice(7, 31)}`,
          ...command,
        });
        const updated = decode(ForensicPriorWorkRecordSchema, {
          ...record,
          dispositions: [...record.dispositions, event],
          updatedAt: command.occurredAt,
        });
        const records = [...state.records];
        records[existingIndex] = updated;
        const next = decode(ForensicPriorWorkStateSchema, {
          ...state,
          revision: state.revision + 1,
          records,
          commandDigests: appendCommandDigest(state, command.idempotencyRef, commandDigest),
        });
        yield* store.save(state.revision, next);
        return updated;
      }),
    });
  }),
);

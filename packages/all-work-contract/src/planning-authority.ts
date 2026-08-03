import { createHash } from "node:crypto";
import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import {
  decodePlanningGraph,
  decodeWorkSnapshot,
  type PlanningGraph,
  PlanningGraphSchema,
  type PlanningResource,
  type PlanningTextRecord,
  PlanningTextRecordSchema,
  type WorkPlanningLink,
  WorkPlanningLinkSchema,
  type WorkRelation,
  WorkRelationSchema,
  type WorkSnapshot,
} from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

export const PLANNING_AUTHORITY_STATE_SCHEMA = "openagents.all_work_planning_authority_state.v1";

const CommandRefSchema = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(160),
  S.isPattern(/^[A-Za-z][A-Za-z0-9._:/-]{0,159}$/u),
).pipe(S.brand("PlanningCommandRef"));
export type PlanningCommandRef = typeof CommandRefSchema.Type;

const IdempotencyKeySchema = S.String.check(S.isMinLength(8), S.isMaxLength(256)).pipe(
  S.brand("PlanningIdempotencyKey"),
);

export const PlanningCommandSchema = S.Union(
  [
    S.Struct({
      command: S.Literal("create_work"),
      commandRef: CommandRefSchema,
      idempotencyKey: IdempotencyKeySchema,
      expectedRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
      workRef: S.String,
      identifier: S.String,
      title: S.String,
      description: S.optionalKey(S.String),
      projectRef: S.optionalKey(S.NullOr(S.String)),
      projectMilestoneRef: S.optionalKey(S.NullOr(S.String)),
      cycleRef: S.optionalKey(S.NullOr(S.String)),
      workflowStateRef: S.optionalKey(S.NullOr(S.String)),
      priority: S.Literals(["none", "urgent", "high", "normal", "low"]),
      ownerRef: S.String,
      occurredAt: S.String,
    }),
    S.Struct({
      command: S.Literal("update_work"),
      commandRef: CommandRefSchema,
      idempotencyKey: IdempotencyKeySchema,
      expectedRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
      workRef: S.String,
      title: S.optionalKey(S.String),
      description: S.optionalKey(S.NullOr(S.String)),
      state: S.optionalKey(
        S.Literals([
          "triage",
          "planned",
          "active",
          "waiting",
          "blocked",
          "failed",
          "completed",
          "canceled",
          "archived",
        ]),
      ),
      priority: S.optionalKey(S.Literals(["none", "urgent", "high", "normal", "low"])),
      occurredAt: S.String,
    }),
    S.Struct({
      command: S.Literal("add_relation"),
      commandRef: CommandRefSchema,
      idempotencyKey: IdempotencyKeySchema,
      expectedRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
      workRef: S.String,
      targetWorkRef: S.String,
      kind: S.Literals([
        "parent",
        "child",
        "blocks",
        "blocked_by",
        "related",
        "duplicate",
        "supersedes",
      ]),
      occurredAt: S.String,
    }),
    S.Struct({
      command: S.Literal("add_comment"),
      commandRef: CommandRefSchema,
      idempotencyKey: IdempotencyKeySchema,
      expectedRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
      workRef: S.String,
      recordRef: S.String,
      body: S.String,
      authorRef: S.String,
      occurredAt: S.String,
    }),
    S.Struct({
      command: S.Literal("set_planning"),
      commandRef: CommandRefSchema,
      idempotencyKey: IdempotencyKeySchema,
      expectedRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
      workRef: S.String,
      projectRef: S.NullOr(S.String),
      projectMilestoneRef: S.NullOr(S.String),
      cycleRef: S.NullOr(S.String),
      workflowStateRef: S.NullOr(S.String),
      releasePlanningRecordRef: S.NullOr(S.String),
      occurredAt: S.String,
    }),
  ],
  { mode: "oneOf" },
);
export type PlanningCommand = typeof PlanningCommandSchema.Type;

export const PlanningCommandReceiptSchema = S.Struct({
  commandRef: CommandRefSchema,
  idempotencyKey: IdempotencyKeySchema,
  commandDigest: S.String.check(
    S.isMinLength(64),
    S.isMaxLength(64),
    S.isPattern(/^[a-f0-9]{64}$/u),
  ),
  workRef: S.String,
  previousRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  revision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  eventCursor: S.String,
  acceptedAt: S.String,
  githubWriteCount: S.Literal(0),
});
export type PlanningCommandReceipt = typeof PlanningCommandReceiptSchema.Type;

export const PlanningAuthorityStateSchema = S.Struct({
  schema: S.Literal(PLANNING_AUTHORITY_STATE_SCHEMA),
  graph: PlanningGraphSchema,
  receipts: S.Array(PlanningCommandReceiptSchema),
});
export type PlanningAuthorityState = typeof PlanningAuthorityStateSchema.Type;

export class PlanningAuthorityError extends S.TaggedErrorClass<PlanningAuthorityError>()(
  "AllWorkPlanningAuthority.Error",
  {
    reason: S.Literals([
      "invalid_state",
      "storage_unavailable",
      "revision_conflict",
      "idempotency_conflict",
      "work_exists",
      "work_not_found",
      "target_not_found",
      "source_read_only",
      "invalid_command",
    ]),
    detail: S.String,
  },
) {}

export interface PlanningStateStoreShape {
  readonly load: Effect.Effect<PlanningAuthorityState | null, PlanningAuthorityError>;
  readonly save: (
    expectedRevision: number,
    state: PlanningAuthorityState,
  ) => Effect.Effect<void, PlanningAuthorityError>;
}

export class PlanningStateStore extends Context.Service<
  PlanningStateStore,
  PlanningStateStoreShape
>()("AllWorkPlanningAuthority.StateStore") {}

const stateDecode = (input: unknown) =>
  S.decodeUnknownEffect(PlanningAuthorityStateSchema)(input, { onExcessProperty: "error" });
const commandDecode = (input: unknown) =>
  S.decodeUnknownEffect(PlanningCommandSchema)(input, { onExcessProperty: "error" });
const receiptDecode = (input: unknown) =>
  S.decodeUnknownEffect(PlanningCommandReceiptSchema)(input, { onExcessProperty: "error" });

export const emptyPlanningGraph = (generatedAt: string): PlanningGraph =>
  decodePlanningGraph({
    contractVersion: "openagents.all_work_boundary.v1",
    graphRef: "planning-graph:all-work",
    revision: 0,
    eventCursor: "cursor:planning:0",
    reconciliationDigest: createHash("sha256").update("empty").digest("hex"),
    generatedAt,
    resources: [],
    work: [],
    planningLinks: [],
    labelLinks: [],
    textRecords: [],
    releaseScopeLinks: [],
    sourceCoordinates: [],
    projectionIssues: [],
    completeness: { state: "complete", cursor: "cursor:planning:0", gapRefs: [] },
    freshness: { state: "fresh", observedAt: generatedAt },
  });

export const emptyPlanningAuthorityState = (generatedAt: string): PlanningAuthorityState =>
  S.decodeUnknownSync(PlanningAuthorityStateSchema)({
    schema: PLANNING_AUTHORITY_STATE_SCHEMA,
    graph: emptyPlanningGraph(generatedAt),
    receipts: [],
  });

const digest = (value: unknown): string =>
  createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex");

const relationEquals = (left: WorkRelation, right: WorkRelation): boolean =>
  left.kind === right.kind && left.targetWorkRef === right.targetWorkRef;

const replaceWork = (
  work: ReadonlyArray<WorkSnapshot>,
  workRef: string,
  replacement: WorkSnapshot,
): ReadonlyArray<WorkSnapshot> =>
  work.map((snapshot) => (snapshot.summary.workRef === workRef ? replacement : snapshot));

const buildNativeSnapshot = (
  command: Extract<PlanningCommand, { readonly command: "create_work" }>,
): WorkSnapshot => {
  const candidate = {
    summary: {
      contractVersion: "openagents.all_work_boundary.v1",
      workRef: command.workRef,
      title: command.title,
      ...(command.description === undefined ? {} : { description: command.description }),
      domain: "development",
      workClass: "task",
      state: "triage",
      priority: command.priority,
      ownerRef: command.ownerRef,
      assignee: null,
      agentDelegate: null,
      portfolio: null,
      sourceAuthority: {
        kind: "effect_service",
        sourceRef: `all-work-native:${command.workRef}`,
        adapterVersion: "all-work-planning-authority-v1",
        writable: true,
      },
      revision: 1,
      updatedAt: command.occurredAt,
      freshness: { state: "fresh", observedAt: command.occurredAt },
      completeness: { state: "complete", gapRefs: [] },
      redaction: {
        privacyClass: "private",
        redactedFieldCount: 0,
        policyRef: "policy:all-work-native-private-v1",
      },
    },
    issue: {
      workRef: command.workRef,
      identifier: command.identifier,
      state: "triage",
      revision: 1,
    },
    relations: [],
    threadRefs: [],
    sessionRefs: [],
    agentSessionRefs: [],
    agentActivityRefs: [],
    runRefs: [],
    intentRefs: [],
    eventRefs: [],
    receiptRefs: [],
    evidenceRefs: [],
    verificationRefs: [],
    ownerDispositionRefs: [],
  };
  return decodeWorkSnapshot(candidate);
};

const requireWork = (graph: PlanningGraph, workRef: string): WorkSnapshot => {
  const snapshot = graph.work.find((candidate) => candidate.summary.workRef === workRef);
  if (snapshot === undefined) {
    throw new PlanningAuthorityError({ reason: "work_not_found", detail: workRef });
  }
  return snapshot;
};

const assertNativeWritable = (snapshot: WorkSnapshot): void => {
  if (
    snapshot.summary.sourceAuthority.kind !== "effect_service" ||
    snapshot.summary.sourceAuthority.writable !== true
  ) {
    throw new PlanningAuthorityError({
      reason: "source_read_only",
      detail: snapshot.summary.workRef,
    });
  }
};

const updateNativeSnapshot = (
  snapshot: WorkSnapshot,
  command: Extract<PlanningCommand, { readonly command: "update_work" }>,
): WorkSnapshot => {
  assertNativeWritable(snapshot);
  const nextRevision = snapshot.summary.revision + 1;
  const state = command.state ?? snapshot.summary.state;
  const description =
    command.description === undefined
      ? snapshot.summary.description
      : (command.description ?? undefined);
  return decodeWorkSnapshot({
    ...snapshot,
    summary: {
      ...snapshot.summary,
      title: command.title ?? snapshot.summary.title,
      ...(description === undefined ? { description: undefined } : { description }),
      state,
      priority: command.priority ?? snapshot.summary.priority,
      revision: nextRevision,
      updatedAt: command.occurredAt,
    },
    issue:
      snapshot.issue === undefined || snapshot.issue === null
        ? snapshot.issue
        : { ...snapshot.issue, state, revision: nextRevision },
  });
};

const planningLinkFromCreate = (
  command: Extract<PlanningCommand, { readonly command: "create_work" }>,
): WorkPlanningLink =>
  S.decodeUnknownSync(WorkPlanningLinkSchema)({
    workRef: command.workRef,
    projectRef: command.projectRef ?? null,
    projectMilestoneRef: command.projectMilestoneRef ?? null,
    cycleRef: command.cycleRef ?? null,
    workflowStateRef: command.workflowStateRef ?? null,
    releasePlanningRecordRef: null,
  });

const advanceGraph = (
  graph: PlanningGraph,
  occurredAt: string,
  patch: Partial<
    Pick<PlanningGraph, "work" | "planningLinks" | "textRecords" | "releaseScopeLinks">
  >,
): PlanningGraph => {
  const revision = graph.revision + 1;
  return decodePlanningGraph({
    ...graph,
    ...patch,
    revision,
    eventCursor: `cursor:planning:${revision}`,
    generatedAt: occurredAt,
    reconciliationDigest: digest({
      resources: graph.resources,
      work: patch.work ?? graph.work,
      planningLinks: patch.planningLinks ?? graph.planningLinks,
      textRecords: patch.textRecords ?? graph.textRecords,
      releaseScopeLinks: patch.releaseScopeLinks ?? graph.releaseScopeLinks,
      sourceCoordinates: graph.sourceCoordinates,
      projectionIssues: graph.projectionIssues,
    }),
    completeness: {
      ...graph.completeness,
      cursor: `cursor:planning:${revision}`,
    },
    freshness: { state: "fresh", observedAt: occurredAt },
  });
};

const applyCommand = (graph: PlanningGraph, command: PlanningCommand): PlanningGraph => {
  if (command.command === "create_work") {
    if (graph.work.some((snapshot) => snapshot.summary.workRef === command.workRef)) {
      throw new PlanningAuthorityError({ reason: "work_exists", detail: command.workRef });
    }
    return advanceGraph(graph, command.occurredAt, {
      work: [...graph.work, buildNativeSnapshot(command)],
      planningLinks: [...graph.planningLinks, planningLinkFromCreate(command)],
    });
  }
  const snapshot = requireWork(graph, command.workRef);
  if (command.command === "update_work") {
    return advanceGraph(graph, command.occurredAt, {
      work: replaceWork(graph.work, command.workRef, updateNativeSnapshot(snapshot, command)),
    });
  }
  if (command.command === "add_relation") {
    assertNativeWritable(snapshot);
    requireWork(graph, command.targetWorkRef);
    const relation = S.decodeUnknownSync(WorkRelationSchema)({
      kind: command.kind,
      targetWorkRef: command.targetWorkRef,
    });
    const relations = snapshot.relations.some((candidate) => relationEquals(candidate, relation))
      ? snapshot.relations
      : [...snapshot.relations, relation];
    const next = decodeWorkSnapshot({ ...snapshot, relations });
    return advanceGraph(graph, command.occurredAt, {
      work: replaceWork(graph.work, command.workRef, next),
    });
  }
  if (command.command === "add_comment") {
    assertNativeWritable(snapshot);
    if (graph.textRecords.some((record) => record.recordRef === command.recordRef)) {
      throw new PlanningAuthorityError({ reason: "work_exists", detail: command.recordRef });
    }
    const record: PlanningTextRecord = S.decodeUnknownSync(PlanningTextRecordSchema)({
      recordRef: command.recordRef,
      kind: "comment",
      workRef: command.workRef,
      resourceRef: null,
      body: command.body,
      authorRef: command.authorRef,
      createdAt: command.occurredAt,
      sourceRef: null,
    });
    return advanceGraph(graph, command.occurredAt, {
      textRecords: [...graph.textRecords, record],
    });
  }
  assertNativeWritable(snapshot);
  const nextLink = S.decodeUnknownSync(WorkPlanningLinkSchema)({
    workRef: command.workRef,
    projectRef: command.projectRef,
    projectMilestoneRef: command.projectMilestoneRef,
    cycleRef: command.cycleRef,
    workflowStateRef: command.workflowStateRef,
    releasePlanningRecordRef: command.releasePlanningRecordRef,
  });
  return advanceGraph(graph, command.occurredAt, {
    planningLinks: [
      ...graph.planningLinks.filter((link) => link.workRef !== command.workRef),
      nextLink,
    ],
  });
};

export interface PlanningAuthorityShape {
  readonly readGraph: Effect.Effect<PlanningGraph, PlanningAuthorityError>;
  readonly execute: (
    input: unknown,
  ) => Effect.Effect<PlanningCommandReceipt, PlanningAuthorityError>;
  readonly replaceFromReconciliation: (
    graph: PlanningGraph,
    expectedRevision: number,
  ) => Effect.Effect<void, PlanningAuthorityError>;
}

export class PlanningAuthority extends Context.Service<PlanningAuthority, PlanningAuthorityShape>()(
  "AllWorkPlanningAuthority.Service",
) {}

export const PlanningAuthorityLive = Layer.effect(
  PlanningAuthority,
  Effect.gen(function* () {
    const store = yield* PlanningStateStore;
    const load = store.load.pipe(
      Effect.flatMap((state) =>
        state === null
          ? Effect.fail(
              new PlanningAuthorityError({ reason: "invalid_state", detail: "store is empty" }),
            )
          : Effect.succeed(state),
      ),
    );
    return PlanningAuthority.of({
      readGraph: load.pipe(Effect.map((state) => state.graph)),
      execute: (input) =>
        Effect.gen(function* () {
          const command = yield* commandDecode(input).pipe(
            Effect.mapError(
              () => new PlanningAuthorityError({ reason: "invalid_command", detail: "decode" }),
            ),
          );
          const state = yield* load;
          const commandDigest = digest(command);
          const previousReceipt = state.receipts.find(
            (receipt) => receipt.idempotencyKey === command.idempotencyKey,
          );
          if (previousReceipt !== undefined) {
            if (previousReceipt.commandDigest !== commandDigest) {
              return yield* new PlanningAuthorityError({
                reason: "idempotency_conflict",
                detail: command.idempotencyKey,
              });
            }
            return previousReceipt;
          }
          if (state.graph.revision !== command.expectedRevision) {
            return yield* new PlanningAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${command.expectedRevision}, found ${state.graph.revision}`,
            });
          }
          const graph = yield* Effect.try({
            try: () => applyCommand(state.graph, command),
            catch: (error) =>
              error instanceof PlanningAuthorityError
                ? error
                : new PlanningAuthorityError({ reason: "invalid_command", detail: "transition" }),
          });
          const receipt = yield* receiptDecode({
            commandRef: command.commandRef,
            idempotencyKey: command.idempotencyKey,
            commandDigest,
            workRef: command.workRef,
            previousRevision: state.graph.revision,
            revision: graph.revision,
            eventCursor: graph.eventCursor,
            acceptedAt: command.occurredAt,
            githubWriteCount: 0,
          }).pipe(
            Effect.mapError(
              () => new PlanningAuthorityError({ reason: "invalid_state", detail: "receipt" }),
            ),
          );
          const next = yield* stateDecode({
            schema: PLANNING_AUTHORITY_STATE_SCHEMA,
            graph,
            receipts: [...state.receipts, receipt],
          }).pipe(
            Effect.mapError(
              () => new PlanningAuthorityError({ reason: "invalid_state", detail: "transition" }),
            ),
          );
          yield* store.save(state.graph.revision, next);
          return receipt;
        }),
      replaceFromReconciliation: (graph, expectedRevision) =>
        Effect.gen(function* () {
          const state = yield* load;
          if (state.graph.revision !== expectedRevision) {
            return yield* new PlanningAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${state.graph.revision}`,
            });
          }
          const next = yield* stateDecode({ ...state, graph }).pipe(
            Effect.mapError(
              () => new PlanningAuthorityError({ reason: "invalid_state", detail: "reconcile" }),
            ),
          );
          yield* store.save(expectedRevision, next);
        }),
    });
  }),
);

export const inMemoryPlanningStateStoreLayer = (
  initial: PlanningAuthorityState,
): Layer.Layer<PlanningStateStore> =>
  Layer.effect(
    PlanningStateStore,
    Effect.gen(function* () {
      const state = yield* Ref.make(initial);
      return PlanningStateStore.of({
        load: Ref.get(state),
        save: (expectedRevision, next) =>
          Ref.modify(state, (current) =>
            current.graph.revision !== expectedRevision
              ? [
                  Effect.fail(
                    new PlanningAuthorityError({
                      reason: "revision_conflict",
                      detail: `expected ${expectedRevision}, found ${current.graph.revision}`,
                    }),
                  ),
                  current,
                ]
              : [Effect.void, next],
          ).pipe(Effect.flatten),
      });
    }),
  );

export const planningResourcesByKind = (
  graph: PlanningGraph,
  kind: PlanningResource["kind"],
): ReadonlyArray<PlanningResource> => graph.resources.filter((resource) => resource.kind === kind);

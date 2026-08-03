import { createHash } from "node:crypto";
import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import {
  AgentActivityRefSchema,
  AgentSessionRefSchema,
  CapabilityRefSchema,
  decodeWorkCommandExecuteRequest,
  decodeWorkCommandExecuteResult,
  type DelegationGrant,
  DelegationGrantSchema,
  DelegationGrantRefSchema,
  DiffRefSchema,
  EffectRefSchema,
  HostRefSchema,
  OrganizationRefSchema,
  PrincipalRefSchema,
  RunRefSchema,
  SafeIntegerSchema,
  SessionRefSchema,
  SourceRefSchema,
  ThreadRefSchema,
  type WorkCommand,
  type WorkCommandExecuteRequest,
  WorkCommandExecuteRequestSchema,
  type WorkCommandExecuteResult,
  WorkCommandExecuteResultSchema,
  WorkReviewRefSchema,
  type WorkSnapshot,
  WorkSnapshotSchema,
} from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

export const WORK_COMMAND_AUTHORITY_STATE_SCHEMA = "openagents.all_work_command_authority_state.v1";

const SessionStateSchema = S.Literals(["active", "paused", "stopped", "revoked"]);

const WorkCommandSessionSchema = S.Struct({
  sessionRef: SessionRefSchema,
  runRef: RunRefSchema,
  threadRef: ThreadRefSchema,
  agentSessionRef: AgentSessionRefSchema,
  grantRef: DelegationGrantRefSchema,
  generation: SafeIntegerSchema,
  hostRef: HostRefSchema,
  state: SessionStateSchema,
});
export type WorkCommandSession = typeof WorkCommandSessionSchema.Type;

const WorkCommandActivityFactSchema = S.Struct({
  activityRef: AgentActivityRefSchema,
  sessionRef: SessionRefSchema,
  runRef: RunRefSchema,
  generation: SafeIntegerSchema,
  kind: S.String,
  summary: S.String,
  providerEventRef: S.NullOr(SourceRefSchema),
  lossRefs: S.Array(SourceRefSchema),
  effectRef: S.NullOr(EffectRefSchema),
});
export type WorkCommandActivityFact = typeof WorkCommandActivityFactSchema.Type;

export const WorkCommandAuthorityStateSchema = S.Struct({
  schema: S.Literal(WORK_COMMAND_AUTHORITY_STATE_SCHEMA),
  organizationRef: OrganizationRefSchema,
  authorizedPrincipalRefs: S.Array(PrincipalRefSchema),
  capabilityRefs: S.Array(CapabilityRefSchema),
  snapshot: WorkSnapshotSchema,
  activeGrant: S.NullOr(DelegationGrantSchema),
  lastGrantGeneration: SafeIntegerSchema,
  sessions: S.Array(WorkCommandSessionSchema),
  activities: S.Array(WorkCommandActivityFactSchema),
  diffRefs: S.Array(DiffRefSchema),
  reviewRefs: S.Array(WorkReviewRefSchema),
  requests: S.Array(WorkCommandExecuteRequestSchema),
  results: S.Array(WorkCommandExecuteResultSchema),
});
export type WorkCommandAuthorityState = typeof WorkCommandAuthorityStateSchema.Type;

export class WorkCommandAuthorityError extends S.TaggedErrorClass<WorkCommandAuthorityError>()(
  "AllWorkCommandAuthority.Error",
  {
    reason: S.Literals([
      "invalid_state",
      "storage_unavailable",
      "invalid_command",
      "work_not_found",
      "organization_mismatch",
      "principal_forbidden",
      "capability_forbidden",
      "revision_conflict",
      "idempotency_conflict",
      "delegation_conflict",
      "invalid_delegation",
      "stale_generation",
      "session_not_found",
      "session_conflict",
      "owner_disposition_forbidden",
    ]),
    detail: S.String,
  },
) {}

export interface WorkCommandStateStoreShape {
  readonly load: Effect.Effect<WorkCommandAuthorityState | null, WorkCommandAuthorityError>;
  readonly save: (
    expectedRevision: number,
    state: WorkCommandAuthorityState,
  ) => Effect.Effect<void, WorkCommandAuthorityError>;
}

export class WorkCommandStateStore extends Context.Service<
  WorkCommandStateStore,
  WorkCommandStateStoreShape
>()("AllWorkCommandAuthority.StateStore") {}

const stateDecode = (input: unknown) =>
  S.decodeUnknownEffect(WorkCommandAuthorityStateSchema)(input, { onExcessProperty: "error" });

const resultDecode = (input: unknown) =>
  S.decodeUnknownEffect(WorkCommandExecuteResultSchema)(input, { onExcessProperty: "error" });

const digest = (value: unknown): string =>
  createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex");

const appendUnique = <A>(values: ReadonlyArray<A>, value: A): ReadonlyArray<A> =>
  values.includes(value) ? values : [...values, value];

const requireActiveGrant = (
  state: WorkCommandAuthorityState,
  grantRef: string,
  generation: number,
): DelegationGrant => {
  const grant = state.activeGrant;
  if (grant === null || grant.grantRef !== grantRef || grant.generation !== generation) {
    throw new WorkCommandAuthorityError({
      reason: "stale_generation",
      detail: `expected active grant ${grantRef} generation ${generation}`,
    });
  }
  return grant;
};

const requireSession = (
  state: WorkCommandAuthorityState,
  sessionRef: string,
  generation: number,
): WorkCommandSession => {
  const session = state.sessions.find((candidate) => candidate.sessionRef === sessionRef);
  if (session === undefined) {
    throw new WorkCommandAuthorityError({ reason: "session_not_found", detail: sessionRef });
  }
  if (session.generation !== generation || session.state === "revoked") {
    throw new WorkCommandAuthorityError({
      reason: "stale_generation",
      detail: `${sessionRef} generation ${generation}`,
    });
  }
  return session;
};

const validateGrant = (
  state: WorkCommandAuthorityState,
  request: WorkCommandExecuteRequest,
  grant: DelegationGrant,
): void => {
  if (state.activeGrant !== null) {
    throw new WorkCommandAuthorityError({
      reason: "delegation_conflict",
      detail: state.activeGrant.grantRef,
    });
  }
  if (grant.issuedBy !== request.effectivePrincipalRef) {
    throw new WorkCommandAuthorityError({
      reason: "principal_forbidden",
      detail: "delegation issuer is not the effective principal",
    });
  }
  if (grant.generation <= state.lastGrantGeneration) {
    throw new WorkCommandAuthorityError({
      reason: "stale_generation",
      detail: `generation ${grant.generation} is not newer than ${state.lastGrantGeneration}`,
    });
  }
  if (grant.budgetLimit <= 0 || Date.parse(grant.expiresAt) <= Date.parse(request.occurredAt)) {
    throw new WorkCommandAuthorityError({
      reason: "invalid_delegation",
      detail: "delegation requires a positive budget and future expiry",
    });
  }
  if ((grant.claimRef === null) !== (grant.leaseRef === null)) {
    throw new WorkCommandAuthorityError({
      reason: "invalid_delegation",
      detail: "repository claim and lease references must be supplied together",
    });
  }
};

interface Transition {
  readonly snapshot: WorkSnapshot;
  readonly authorizedPrincipalRefs: ReadonlyArray<string>;
  readonly activeGrant: DelegationGrant | null;
  readonly lastGrantGeneration: number;
  readonly sessions: ReadonlyArray<WorkCommandSession>;
  readonly activities: ReadonlyArray<WorkCommandActivityFact>;
  readonly diffRefs: ReadonlyArray<string>;
  readonly reviewRefs: ReadonlyArray<string>;
  readonly effectRef: string | null;
}

const transition = (
  state: WorkCommandAuthorityState,
  request: WorkCommandExecuteRequest,
): Transition => {
  const command: WorkCommand = request.command;
  let snapshot: WorkSnapshot = state.snapshot;
  let authorizedPrincipalRefs = state.authorizedPrincipalRefs;
  let activeGrant = state.activeGrant;
  let lastGrantGeneration = state.lastGrantGeneration;
  let sessions = state.sessions;
  let activities = state.activities;
  let diffRefs = state.diffRefs;
  let reviewRefs = state.reviewRefs;
  let effectRef: string | null = null;

  switch (command.command) {
    case "assign":
      snapshot = { ...snapshot, summary: { ...snapshot.summary, assignee: command.assignee } };
      authorizedPrincipalRefs = appendUnique(
        authorizedPrincipalRefs,
        command.assignee.principalRef,
      );
      break;
    case "unassign": {
      const previousAssigneeRef = snapshot.summary.assignee?.principalRef;
      snapshot = { ...snapshot, summary: { ...snapshot.summary, assignee: null } };
      if (previousAssigneeRef !== undefined && previousAssigneeRef !== snapshot.summary.ownerRef) {
        authorizedPrincipalRefs = authorizedPrincipalRefs.filter(
          (principalRef) => principalRef !== previousAssigneeRef,
        );
      }
      break;
    }
    case "delegate":
      validateGrant(state, request, command.grant);
      activeGrant = command.grant;
      lastGrantGeneration = command.grant.generation;
      snapshot = {
        ...snapshot,
        summary: {
          ...snapshot.summary,
          agentDelegate: {
            agentRef: command.grant.agentRef,
            delegationGrantRef: command.grant.grantRef,
            generation: command.grant.generation,
          },
        },
      };
      break;
    case "revoke":
      requireActiveGrant(state, command.grantRef, command.expectedGeneration);
      activeGrant = null;
      sessions = sessions.map((session) =>
        session.generation === command.expectedGeneration
          ? { ...session, state: "revoked" as const }
          : session,
      );
      snapshot = { ...snapshot, summary: { ...snapshot.summary, agentDelegate: null } };
      break;
    case "start_agent_session": {
      const grant = requireActiveGrant(state, command.grantRef, command.expectedGeneration);
      if (grant.hostRef !== command.hostRef) {
        throw new WorkCommandAuthorityError({
          reason: "invalid_delegation",
          detail: `host ${command.hostRef} is outside grant ${grant.grantRef}`,
        });
      }
      if (
        sessions.some(
          (session) =>
            session.sessionRef === command.sessionRef ||
            session.runRef === command.runRef ||
            session.agentSessionRef === command.agentSessionRef,
        )
      ) {
        throw new WorkCommandAuthorityError({
          reason: "session_conflict",
          detail: command.sessionRef,
        });
      }
      sessions = [
        ...sessions,
        {
          sessionRef: command.sessionRef,
          runRef: command.runRef,
          threadRef: command.threadRef,
          agentSessionRef: command.agentSessionRef,
          grantRef: command.grantRef,
          generation: command.expectedGeneration,
          hostRef: command.hostRef,
          state: "active",
        },
      ];
      snapshot = {
        ...snapshot,
        threadRefs: appendUnique(snapshot.threadRefs, command.threadRef),
        sessionRefs: appendUnique(snapshot.sessionRefs, command.sessionRef),
        agentSessionRefs: appendUnique(snapshot.agentSessionRefs, command.agentSessionRef),
        runRefs: appendUnique(snapshot.runRefs, command.runRef),
      };
      break;
    }
    case "record_activity": {
      const session = requireSession(state, command.sessionRef, command.expectedGeneration);
      requireActiveGrant(state, session.grantRef, command.expectedGeneration);
      if (session.runRef !== command.runRef) {
        throw new WorkCommandAuthorityError({
          reason: "session_not_found",
          detail: `${command.sessionRef}/${command.runRef}`,
        });
      }
      if (activities.some((activity) => activity.activityRef === command.activityRef)) {
        throw new WorkCommandAuthorityError({
          reason: "session_conflict",
          detail: command.activityRef,
        });
      }
      activities = [
        ...activities,
        {
          activityRef: command.activityRef,
          sessionRef: command.sessionRef,
          runRef: command.runRef,
          generation: command.expectedGeneration,
          kind: command.kind,
          summary: command.summary,
          providerEventRef: command.providerEventRef,
          lossRefs: command.lossRefs,
          effectRef: command.effectRef,
        },
      ];
      effectRef = command.effectRef;
      snapshot = {
        ...snapshot,
        agentActivityRefs: appendUnique(snapshot.agentActivityRefs, command.activityRef),
      };
      break;
    }
    case "control_session": {
      const session = requireSession(state, command.sessionRef, command.expectedGeneration);
      requireActiveGrant(state, session.grantRef, command.expectedGeneration);
      sessions = sessions.map((candidate) =>
        candidate.sessionRef !== command.sessionRef
          ? candidate
          : {
              ...candidate,
              state:
                command.control === "pause"
                  ? ("paused" as const)
                  : command.control === "stop"
                    ? ("stopped" as const)
                    : ("active" as const),
            },
      );
      break;
    }
    case "attach_diff":
      diffRefs = appendUnique(diffRefs, command.diffRef);
      break;
    case "attach_review":
      reviewRefs = appendUnique(reviewRefs, command.reviewRef);
      break;
    case "attach_evidence":
      snapshot = {
        ...snapshot,
        evidenceRefs: appendUnique(snapshot.evidenceRefs, command.evidenceRef),
      };
      break;
    case "attach_verification":
      snapshot = {
        ...snapshot,
        verificationRefs: appendUnique(snapshot.verificationRefs, command.verificationRef),
      };
      break;
    case "owner_disposition": {
      const assigneeRef = snapshot.summary.assignee?.principalRef;
      if (
        request.effectivePrincipalRef !== snapshot.summary.ownerRef &&
        request.effectivePrincipalRef !== assigneeRef
      ) {
        throw new WorkCommandAuthorityError({
          reason: "owner_disposition_forbidden",
          detail: request.effectivePrincipalRef,
        });
      }
      const missingVerification = command.verificationRefs.find(
        (verificationRef) => !snapshot.verificationRefs.includes(verificationRef),
      );
      if (missingVerification !== undefined) {
        throw new WorkCommandAuthorityError({
          reason: "invalid_command",
          detail: `unknown verification ${missingVerification}`,
        });
      }
      snapshot = {
        ...snapshot,
        ownerDispositionRefs: appendUnique(snapshot.ownerDispositionRefs, command.dispositionRef),
      };
      break;
    }
  }

  return {
    snapshot,
    authorizedPrincipalRefs,
    activeGrant,
    lastGrantGeneration,
    sessions,
    activities,
    diffRefs,
    reviewRefs,
    effectRef,
  };
};

export const emptyWorkCommandAuthorityState = (input: {
  readonly snapshot: WorkSnapshot;
  readonly organizationRef: string;
  readonly authorizedPrincipalRefs: ReadonlyArray<string>;
  readonly capabilityRefs?: ReadonlyArray<string>;
}): WorkCommandAuthorityState =>
  S.decodeUnknownSync(WorkCommandAuthorityStateSchema)({
    schema: WORK_COMMAND_AUTHORITY_STATE_SCHEMA,
    organizationRef: input.organizationRef,
    authorizedPrincipalRefs: input.authorizedPrincipalRefs,
    capabilityRefs: input.capabilityRefs ?? ["capability:work-command:execute"],
    snapshot: input.snapshot,
    activeGrant: null,
    lastGrantGeneration: 0,
    sessions: [],
    activities: [],
    diffRefs: [],
    reviewRefs: [],
    requests: [],
    results: [],
  });

export interface WorkCommandAuthorityShape {
  readonly read: Effect.Effect<WorkCommandAuthorityState, WorkCommandAuthorityError>;
  readonly execute: (
    input: unknown,
  ) => Effect.Effect<WorkCommandExecuteResult, WorkCommandAuthorityError>;
}

export class WorkCommandAuthority extends Context.Service<
  WorkCommandAuthority,
  WorkCommandAuthorityShape
>()("AllWorkCommandAuthority.Service") {}

export const WorkCommandAuthorityLive = Layer.effect(
  WorkCommandAuthority,
  Effect.gen(function* () {
    const store = yield* WorkCommandStateStore;
    const load = store.load.pipe(
      Effect.flatMap((state) =>
        state === null
          ? Effect.fail(
              new WorkCommandAuthorityError({ reason: "invalid_state", detail: "store is empty" }),
            )
          : Effect.succeed(state),
      ),
    );
    return WorkCommandAuthority.of({
      read: load,
      execute: (input) =>
        Effect.gen(function* () {
          const request = yield* Effect.try({
            try: () => decodeWorkCommandExecuteRequest(input),
            catch: () =>
              new WorkCommandAuthorityError({ reason: "invalid_command", detail: "decode" }),
          });
          const state = yield* load;
          const commandDigest = digest(request);
          const previous = state.results.find(
            (result) => result.receipt.idempotencyKey === request.idempotencyKey,
          );
          if (previous !== undefined) {
            if (previous.receipt.commandDigest !== commandDigest) {
              return yield* new WorkCommandAuthorityError({
                reason: "idempotency_conflict",
                detail: request.idempotencyKey,
              });
            }
            return previous;
          }
          if (state.snapshot.summary.workRef !== request.workRef) {
            return yield* new WorkCommandAuthorityError({
              reason: "work_not_found",
              detail: request.workRef,
            });
          }
          if (state.organizationRef !== request.organizationRef) {
            return yield* new WorkCommandAuthorityError({
              reason: "organization_mismatch",
              detail: request.organizationRef,
            });
          }
          if (!state.authorizedPrincipalRefs.includes(request.effectivePrincipalRef)) {
            return yield* new WorkCommandAuthorityError({
              reason: "principal_forbidden",
              detail: request.effectivePrincipalRef,
            });
          }
          if (!state.capabilityRefs.includes(request.capabilityRef)) {
            return yield* new WorkCommandAuthorityError({
              reason: "capability_forbidden",
              detail: request.capabilityRef,
            });
          }
          if (state.snapshot.summary.revision !== request.expectedRevision) {
            return yield* new WorkCommandAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${request.expectedRevision}, found ${state.snapshot.summary.revision}`,
            });
          }
          const applied = yield* Effect.try({
            try: () => transition(state, request),
            catch: (error) =>
              error instanceof WorkCommandAuthorityError
                ? error
                : new WorkCommandAuthorityError({
                    reason: "invalid_command",
                    detail: "transition",
                  }),
          });
          const previousRevision = state.snapshot.summary.revision;
          const revision = previousRevision + 1;
          const eventRef = `event:work-command:${revision}`;
          const receiptRef = `receipt:work-command:${revision}`;
          const snapshot = {
            ...applied.snapshot,
            summary: {
              ...applied.snapshot.summary,
              revision,
              updatedAt: request.occurredAt,
              freshness: { state: "fresh" as const, observedAt: request.occurredAt },
            },
            issue:
              applied.snapshot.issue === undefined || applied.snapshot.issue === null
                ? applied.snapshot.issue
                : { ...applied.snapshot.issue, revision },
            intentRefs: appendUnique(applied.snapshot.intentRefs, request.intentRef),
            eventRefs: appendUnique(applied.snapshot.eventRefs, eventRef),
            receiptRefs: appendUnique(applied.snapshot.receiptRefs, receiptRef),
          };
          const result = yield* resultDecode({
            snapshot,
            receipt: {
              intentRef: request.intentRef,
              idempotencyKey: request.idempotencyKey,
              commandDigest,
              workRef: request.workRef,
              previousRevision,
              revision,
              eventCursor: `cursor:work-command:${revision}`,
              effectivePrincipalRef: request.effectivePrincipalRef,
              organizationRef: request.organizationRef,
              acceptedAt: request.occurredAt,
              outcome: {
                admitted: true,
                eventRef,
                effectRef: applied.effectRef,
                refusalReason: null,
              },
              githubWriteCount: 0,
            },
          }).pipe(
            Effect.mapError(
              () => new WorkCommandAuthorityError({ reason: "invalid_state", detail: "result" }),
            ),
          );
          const next = yield* stateDecode({
            ...state,
            snapshot: result.snapshot,
            authorizedPrincipalRefs: applied.authorizedPrincipalRefs,
            activeGrant: applied.activeGrant,
            lastGrantGeneration: applied.lastGrantGeneration,
            sessions: applied.sessions,
            activities: applied.activities,
            diffRefs: applied.diffRefs,
            reviewRefs: applied.reviewRefs,
            requests: [...state.requests, request],
            results: [...state.results, result],
          }).pipe(
            Effect.mapError(
              () =>
                new WorkCommandAuthorityError({ reason: "invalid_state", detail: "transition" }),
            ),
          );
          yield* store.save(previousRevision, next);
          return decodeWorkCommandExecuteResult(result);
        }),
    });
  }),
);

export const inMemoryWorkCommandStateStoreLayer = (
  initial: WorkCommandAuthorityState,
): Layer.Layer<WorkCommandStateStore> =>
  Layer.effect(
    WorkCommandStateStore,
    Effect.gen(function* () {
      const state = yield* Ref.make(initial);
      return WorkCommandStateStore.of({
        load: Ref.get(state),
        save: (expectedRevision, next) =>
          Ref.modify(state, (current) =>
            current.snapshot.summary.revision !== expectedRevision
              ? [
                  Effect.fail(
                    new WorkCommandAuthorityError({
                      reason: "revision_conflict",
                      detail: `expected ${expectedRevision}, found ${current.snapshot.summary.revision}`,
                    }),
                  ),
                  current,
                ]
              : [Effect.void, next],
          ).pipe(Effect.flatten),
      });
    }),
  );

import {
  decodeWorkCommandExecuteRequest,
  emptyWorkCommandAuthorityState,
  fileWorkCommandStateStoreLayer,
  initializeFileWorkCommandState,
  WorkCommandAuthority,
  WorkCommandAuthorityError,
  WorkCommandAuthorityLive,
  WorkCommandStateStore,
} from "@openagentsinc/all-work-contract";
import { Effect, Layer } from "effect";

import {
  bootstrapAllWorkPlanningAuthority,
  readAllWorkPlanningGraph,
} from "./all-work-planning-authority.ts";

const authorityLayer = (dataRoot: string, workRef: string) =>
  WorkCommandAuthorityLive.pipe(Layer.provide(fileWorkCommandStateStoreLayer(dataRoot, workRef)));

const OPENAGENTS_ORGANIZATION_REF = "organization:openagents";

export const executeAllWorkCommand = (dataRoot: string, input: unknown) => {
  const request = decodeWorkCommandExecuteRequest(input);
  return Effect.gen(function* () {
    yield* bootstrapAllWorkPlanningAuthority(dataRoot);
    const graph = yield* readAllWorkPlanningGraph(dataRoot);
    const snapshot = graph.work.find((candidate) => candidate.summary.workRef === request.workRef);
    if (snapshot === undefined) {
      return yield* new WorkCommandAuthorityError({
        reason: "work_not_found",
        detail: request.workRef,
      });
    }
    const authorizedPrincipalRefs = [
      snapshot.summary.ownerRef,
      ...(snapshot.summary.assignee === null ? [] : [snapshot.summary.assignee.principalRef]),
    ];
    yield* initializeFileWorkCommandState(
      dataRoot,
      request.workRef,
      emptyWorkCommandAuthorityState({
        snapshot,
        organizationRef: OPENAGENTS_ORGANIZATION_REF,
        authorizedPrincipalRefs,
      }),
    );
    const authority = yield* WorkCommandAuthority;
    return yield* authority.execute(request);
  }).pipe(Effect.provide(authorityLayer(dataRoot, request.workRef)));
};

export const readAllWorkCommandSnapshot = (dataRoot: string, workRef: string) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkPlanningAuthority(dataRoot);
    const graph = yield* readAllWorkPlanningGraph(dataRoot);
    const planningSnapshot = graph.work.find((candidate) => candidate.summary.workRef === workRef);
    if (planningSnapshot === undefined) {
      return yield* new WorkCommandAuthorityError({ reason: "work_not_found", detail: workRef });
    }
    const store = yield* WorkCommandStateStore;
    const state = yield* store.load;
    if (state !== null && state.snapshot.summary.workRef !== workRef) {
      return yield* new WorkCommandAuthorityError({
        reason: "invalid_state",
        detail: "command state contains the wrong Work identity",
      });
    }
    return state?.snapshot ?? planningSnapshot;
  }).pipe(Effect.provide(fileWorkCommandStateStoreLayer(dataRoot, workRef)));

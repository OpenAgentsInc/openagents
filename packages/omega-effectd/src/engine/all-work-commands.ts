import {
  decodeWorkCommandExecuteRequest,
  emptyWorkCommandAuthorityState,
  fileWorkCommandStateStoreLayer,
  initializeFileWorkCommandState,
  WorkCommandAuthority,
  WorkCommandAuthorityError,
  WorkCommandAuthorityLive,
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

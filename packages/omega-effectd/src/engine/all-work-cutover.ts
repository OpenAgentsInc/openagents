import {
  emptyWorkCutoverAuthorityState,
  fileWorkCutoverStateStoreLayer,
  initializeFileWorkCutoverState,
  WorkCutoverAuthority,
  WorkCutoverAuthorityLive,
} from "@openagentsinc/all-work-contract";
import { Effect, Layer } from "effect";

import {
  bootstrapAllWorkPlanningAuthority,
  readAllWorkPlanningGraph,
} from "./all-work-planning-authority.ts";

const OPENAGENTS_ORGANIZATION_REF = "organization:openagents";
const OMEGA_LOCAL_OWNER_REF = "principal:omega:local-owner";

const authorityLayer = (dataRoot: string) =>
  WorkCutoverAuthorityLive.pipe(Layer.provide(fileWorkCutoverStateStoreLayer(dataRoot)));

export const bootstrapAllWorkCutoverAuthority = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkCutoverAuthority",
)(function* (dataRoot: string) {
  yield* bootstrapAllWorkPlanningAuthority(dataRoot);
  const graph = yield* readAllWorkPlanningGraph(dataRoot);
  yield* initializeFileWorkCutoverState(
    dataRoot,
    emptyWorkCutoverAuthorityState({
      organizationRef: OPENAGENTS_ORGANIZATION_REF,
      authorizedPrincipalRefs: [OMEGA_LOCAL_OWNER_REF],
      sourceDigest: graph.reconciliationDigest,
      sourceCursor: graph.eventCursor,
    }),
  );
});

export const readAllWorkCutover = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkCutoverAuthority(dataRoot);
    const authority = yield* WorkCutoverAuthority;
    return yield* authority.read(input);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

export const executeAllWorkCutover = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkCutoverAuthority(dataRoot);
    const authority = yield* WorkCutoverAuthority;
    return yield* authority.execute(input);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

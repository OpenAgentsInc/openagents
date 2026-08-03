import {
  decodeSignedWorkroomDeliveryRequest,
  decodeSignedWorkroomEnqueueRequest,
  decodeSignedWorkroomReadRequest,
  deliverSignedWorkroomActivity,
  emptySignedWorkroomState,
  enqueueSignedWorkroomActivity,
  fileSignedWorkroomStateStoreLayer,
  initializeFileSignedWorkroomState,
  readSignedWorkroomActivity,
  SignedWorkroomStateStore,
} from "@openagentsinc/all-work-contract";
import { Effect } from "effect";

const withStore = <A, E>(dataRoot: string, effect: Effect.Effect<A, E, SignedWorkroomStateStore>) =>
  effect.pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const bootstrapAllWorkSignedWorkroom = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkSignedWorkroom",
)(function* (dataRoot: string) {
  yield* initializeFileSignedWorkroomState(
    dataRoot,
    emptySignedWorkroomState("2026-08-03T10:00:00Z"),
  );
  return yield* withStore(
    dataRoot,
    Effect.gen(function* () {
      const store = yield* SignedWorkroomStateStore;
      const state = yield* store.load;
      if (state === null) return yield* Effect.fail(new Error("signed Workroom state missing"));
      return state.ledger;
    }),
  );
});

export const readAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const store = yield* SignedWorkroomStateStore;
    const state = yield* store.load;
    if (state === null) return yield* Effect.fail(new Error("signed Workroom state missing"));
    return readSignedWorkroomActivity(state, decodeSignedWorkroomReadRequest(input));
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const enqueueAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const request = decodeSignedWorkroomEnqueueRequest(input);
    return yield* enqueueSignedWorkroomActivity(request, request.activity.occurredAt);
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const deliverAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const request = decodeSignedWorkroomDeliveryRequest(input);
    return yield* deliverSignedWorkroomActivity(request);
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

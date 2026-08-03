import {
  decodeSignedWorkroomDeliveryRequest,
  decodeSignedWorkroomCommitRequest,
  decodeSignedWorkroomEnqueueRequest,
  decodeSignedWorkroomPrepareRequest,
  decodeSignedWorkroomReadRequest,
  decodeSignedWorkroomPublishRequest,
  deliverSignedWorkroomActivity,
  commitSignedWorkroomActivity,
  emptySignedWorkroomState,
  enqueueSignedWorkroomActivity,
  fileSignedWorkroomStateStoreLayer,
  initializeFileSignedWorkroomState,
  makeSignedWorkroomRelayPublisherLayer,
  publishSignedWorkroomOutbox,
  prepareSignedWorkroomActivity,
  readSignedWorkroomActivity,
  SignedWorkroomStateStore,
} from "@openagentsinc/all-work-contract";
import { Effect } from "effect";

const withStore = <A, E>(dataRoot: string, effect: Effect.Effect<A, E, SignedWorkroomStateStore>) =>
  effect.pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const configuredSignedWorkroomRelayUrls = (
  value = process.env.OPENAGENTS_OMEGA_SIGNED_WORKROOM_RELAYS ?? "",
): ReadonlyArray<string> =>
  value
    .split(",")
    .map((relayUrl) => relayUrl.trim())
    .filter((relayUrl) => relayUrl.length > 0);

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
    return yield* enqueueSignedWorkroomActivity(
      { ...request, relayUrls: configuredSignedWorkroomRelayUrls() },
      request.activity.occurredAt,
    );
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const prepareAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const request = decodeSignedWorkroomPrepareRequest(input);
    return yield* prepareSignedWorkroomActivity(
      request,
      new Date().toISOString(),
      configuredSignedWorkroomRelayUrls(),
    );
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const commitAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const request = decodeSignedWorkroomCommitRequest(input);
    return yield* commitSignedWorkroomActivity(
      request,
      new Date().toISOString(),
      configuredSignedWorkroomRelayUrls(),
    );
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const deliverAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const request = decodeSignedWorkroomDeliveryRequest(input);
    return yield* deliverSignedWorkroomActivity(request);
  }).pipe(Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)));

export const publishAllWorkSignedWorkroom = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkSignedWorkroom(dataRoot);
    const request = decodeSignedWorkroomPublishRequest(input);
    return yield* publishSignedWorkroomOutbox(request);
  }).pipe(
    Effect.provide(fileSignedWorkroomStateStoreLayer(dataRoot)),
    Effect.provide(makeSignedWorkroomRelayPublisherLayer()),
  );

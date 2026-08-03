import {
  emptyForensicPriorWorkState,
  fileForensicPriorWorkStateStoreLayer,
  ForensicPriorWorkAuthority,
  ForensicPriorWorkAuthorityLive,
  initializeFileForensicPriorWorkState,
} from "@openagentsinc/all-work-contract";
import { Effect, Layer } from "effect";

const authorityLayer = (dataRoot: string) =>
  ForensicPriorWorkAuthorityLive.pipe(
    Layer.provide(fileForensicPriorWorkStateStoreLayer(dataRoot)),
  );

export const bootstrapAllWorkForensicPriorWork = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkForensicPriorWork",
)(function* (dataRoot: string) {
  yield* initializeFileForensicPriorWorkState(dataRoot, emptyForensicPriorWorkState());
});

const withAuthority = <A>(
  dataRoot: string,
  operation: (authority: ForensicPriorWorkAuthority["Service"]) => Effect.Effect<A, unknown>,
) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkForensicPriorWork(dataRoot);
    const authority = yield* ForensicPriorWorkAuthority;
    return yield* operation(authority);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

export const queryAllWorkForensicPriorWork = (dataRoot: string, input: unknown) =>
  withAuthority(dataRoot, (authority) => authority.query(input));

export const submitAllWorkForensicPriorWork = (dataRoot: string, input: unknown) =>
  withAuthority(dataRoot, (authority) => authority.submit(input));

export const relateAllWorkForensicPriorWork = (dataRoot: string, input: unknown) =>
  withAuthority(dataRoot, (authority) => authority.relate(input));

export const disposeAllWorkForensicPriorWork = (dataRoot: string, input: unknown) =>
  withAuthority(dataRoot, (authority) => authority.dispose(input));

import {
  emptyStrictBugCandidateAuthorityState,
  fileStrictBugCandidateStateStoreLayer,
  initializeFileStrictBugCandidateState,
  StrictBugCandidateAuthority,
  StrictBugCandidateAuthorityLive,
} from "@openagentsinc/all-work-contract";
import { Effect, Layer } from "effect";

const authorityLayer = (dataRoot: string) =>
  StrictBugCandidateAuthorityLive.pipe(
    Layer.provide(fileStrictBugCandidateStateStoreLayer(dataRoot)),
  );

export const bootstrapAllWorkStrictBugCandidates = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkStrictBugCandidates",
)(function* (dataRoot: string) {
  yield* initializeFileStrictBugCandidateState(
    dataRoot,
    emptyStrictBugCandidateAuthorityState(
      "2026-08-03T12:00:00Z",
      ["principal:github:webhook"],
      ["principal:omega:local-owner"],
    ),
  );
});

export const readAllWorkStrictBugCandidates = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkStrictBugCandidates(dataRoot);
    const authority = yield* StrictBugCandidateAuthority;
    return yield* authority.read(input);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

export const executeAllWorkStrictBugCandidate = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkStrictBugCandidates(dataRoot);
    const authority = yield* StrictBugCandidateAuthority;
    return yield* authority.execute(input);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

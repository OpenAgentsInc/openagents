import bootstrapInput from "@openagentsinc/all-work-contract/bootstrap/v0.2.0" with { type: "json" };
import {
  decodeRepositoryClaimExecuteRequest,
  emptyRepositoryClaimAuthorityState,
  fileRepositoryClaimStateStoreLayer,
  initializeFileRepositoryClaimState,
  importHistoricalRepositoryClaimComments,
  githubWorkRef,
  RepositoryClaimAuthority,
  RepositoryClaimAuthorityLive,
  RepositoryClaimStateStore,
} from "@openagentsinc/all-work-contract";
import { Effect, Layer } from "effect";

import { readAllWorkPlanningGraph } from "./all-work-planning-authority.ts";

const withStore = <A, E>(
  dataRoot: string,
  effect: Effect.Effect<A, E, RepositoryClaimStateStore>,
) => effect.pipe(Effect.provide(fileRepositoryClaimStateStoreLayer(dataRoot)));

const authorityLayer = (dataRoot: string) =>
  RepositoryClaimAuthorityLive.pipe(Layer.provide(fileRepositoryClaimStateStoreLayer(dataRoot)));

export const bootstrapAllWorkRepositoryClaims = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkRepositoryClaims",
)(function* (dataRoot: string) {
  yield* initializeFileRepositoryClaimState(
    dataRoot,
    emptyRepositoryClaimAuthorityState("2026-08-03T05:00:00Z"),
  );
  return yield* withStore(
    dataRoot,
    Effect.gen(function* () {
      const store = yield* RepositoryClaimStateStore;
      const state = yield* store.load;
      if (state === null) return yield* Effect.fail(new Error("claim state is not initialized"));
      const comments = bootstrapInput.pages.flatMap((page) =>
        page.issues.flatMap((issue) =>
          issue.comments.map((comment) => ({
            sourceRef: `evidence:github-comment:${comment.id}`,
            workRef: githubWorkRef(issue.repository, issue.number),
            repositoryRef: `repository:${issue.repository.split("/").at(-1)?.toLowerCase() ?? "unknown"}`,
            authorRef: comment.authorRef,
            observedAt: comment.createdAt,
            body: comment.body,
          })),
        ),
      );
      const complete = bootstrapInput.pages.every(
        (page) => page.complete && page.nextCursor === null,
      );
      const ledger = importHistoricalRepositoryClaimComments(
        state.ledger,
        comments,
        complete,
        bootstrapInput.fetchedAt,
      );
      if (
        ledger.revision !== state.ledger.revision ||
        ledger.completeness.state !== state.ledger.completeness.state
      ) {
        yield* store.save(state.ledger.revision, { ...state, ledger });
      }
      return ledger;
    }),
  );
});

export const readAllWorkRepositoryClaims = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkRepositoryClaims(dataRoot);
    const authority = yield* RepositoryClaimAuthority;
    return yield* authority.read(input);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

export const executeAllWorkRepositoryClaim = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkRepositoryClaims(dataRoot);
    const request = decodeRepositoryClaimExecuteRequest(input);
    if (request.command.command === "create_packet") {
      const graph = yield* readAllWorkPlanningGraph(dataRoot);
      if (!graph.work.some((work) => work.summary.workRef === request.command.workRef)) {
        return yield* Effect.fail(new Error("Work Packet references unknown canonical Work"));
      }
    }
    const authority = yield* RepositoryClaimAuthority;
    return yield* authority.execute(request);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));

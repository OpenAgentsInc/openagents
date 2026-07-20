import { Effect, Layer, Ref, Schema as S, Stream } from "effect";

import {
  CANDIDATE_SCHEMA_LITERAL,
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  InferenceProviderDescriptor,
  OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
  OwnerBoundCandidateSet,
  PROVIDER_SCHEMA_LITERAL,
  ProviderTurnRef,
  ROUTE_DECISION_SCHEMA_LITERAL,
  RouteDecision,
  TurnCandidate,
  WorkContextEnvelope,
  type ReleasedArtifact,
  type TurnRefusalReason,
  type TurnRequestRef,
} from "@openagentsinc/agent-runtime-schema";

import {
  ActionBroker,
  ArtifactResolver,
  ContextSource,
  ContextSourceError,
  ProviderRegistry,
  ProviderStartError,
  ProviderStreamEvent,
  ThreadRepository,
  TurnJournal,
  TurnJournalError,
  TurnPolicy,
  type ProviderRun,
  type ThreadTurnMessage,
} from "./ports.js";
import type { TurnStateRecord } from "./turn-state.js";
import { TurnService, layer as turnServiceLayer } from "./turn-service.js";

/**
 * AFS-01 deterministic test fixtures.
 *
 * These layers are the deterministic in-memory implementations of every kernel
 * port. They let a test compose the real `TurnService` and drive a fixture
 * provider through complete, fail, refuse, and cancel outcomes without any
 * platform API, provider SDK, store driver, or timed sleep.
 */

const AT = "2026-07-20T08:00:00.000Z" as const;

const decodeContext = S.decodeUnknownSync(WorkContextEnvelope);
const decodeCandidateSet = S.decodeUnknownSync(OwnerBoundCandidateSet);
const decodeDecision = S.decodeUnknownSync(RouteDecision);
const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor);
const decodeCandidate = S.decodeUnknownSync(TurnCandidate);

const fixtureProviderTurnRef = S.decodeUnknownSync(ProviderTurnRef)("providerturn.fixture.1");

export const fixtureCandidateSet: OwnerBoundCandidateSet = decodeCandidateSet({
  schema: OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
  ordered: ["provider.codex.1"],
  policyArtifactRef: "artifact.policy.1",
});

export const fixtureAnswerCandidate: TurnCandidate = decodeCandidate({
  schema: CANDIDATE_SCHEMA_LITERAL,
  kind: "answer",
  candidateRef: "candidate.fixture.1",
  provenance: {
    providerRef: "provider.codex.1",
    candidate: "codex",
    model: "codex",
    taskClass: "local_answer",
    usageTruth: "exact",
    dataDestination: "remote_provider",
    stale: false,
  },
  text: "fixture answer",
});

const fixtureDescriptor = decodeDescriptor({
  schema: PROVIDER_SCHEMA_LITERAL,
  providerRef: "provider.codex.1",
  candidate: "codex",
  model: "codex",
  placement: "owner_local",
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  dataDestination: "remote_provider",
  usageTruth: "exact",
  costClass: "metered_provider_tokens",
  maxContextChars: 4000,
  maxOutputChars: 8192,
  supportsStreaming: true,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  readiness: { state: "ready" },
});

const admittedDecision = decodeDecision({
  schema: ROUTE_DECISION_SCHEMA_LITERAL,
  outcome: "admitted",
  routeDecisionRef: "route.fixture.1",
  requestRef: "request.fixture.1",
  selected: "provider.codex.1",
  effective: "provider.codex.1",
  admittedCandidateSet: ["provider.codex.1"],
  policyArtifactRef: "artifact.policy.1",
  contextManifestRef: "context.fixture.1",
  disclosure: {
    dataDestination: "remote_provider",
    costClass: "metered_provider_tokens",
    localOnly: false,
    providerRef: "provider.codex.1",
  },
  decisionReason: "admitted_first_candidate",
  dispositions: [],
  decidedAt: AT,
});

const closedDecision = decodeDecision({
  schema: ROUTE_DECISION_SCHEMA_LITERAL,
  outcome: "closed",
  routeDecisionRef: "route.closed.1",
  requestRef: "request.fixture.1",
  policyArtifactRef: "artifact.policy.1",
  contextManifestRef: "context.fixture.1",
  decisionReason: "no_candidate_fail_closed",
  dispositions: [],
  decidedAt: AT,
});

/** Context source that always produces a small valid manifest. */
export const contextSourceFixtureLayer = (options: { readonly fail?: boolean } = {}) =>
  Layer.succeed(
    ContextSource,
    ContextSource.of({
      manifest: (input) =>
        options.fail === true
          ? Effect.fail(new ContextSourceError({ reason: "not_ready" }))
          : Effect.succeed(
              decodeContext({
                schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
                manifestRef: "context.fixture.1",
                threadRef: input.threadRef,
                generation: { state: "known", value: 0 },
                createdAt: AT,
                items: [],
                totalByteLength: 0,
                byteLimit: 0,
                truncated: false,
                redacted: false,
              }),
            ),
    }),
  );

/** Policy that admits the codex lane, or fails closed. */
export const turnPolicyFixtureLayer = (options: { readonly closed?: boolean } = {}) =>
  Layer.succeed(
    TurnPolicy,
    TurnPolicy.of({
      decide: () => Effect.succeed(options.closed === true ? closedDecision : admittedDecision),
    }),
  );

export type ProviderFixtureOutcome = "completes" | "fails" | "refuses" | "hangs" | "start_unavailable";

const outcomeStream = (outcome: ProviderFixtureOutcome): Stream.Stream<ProviderStreamEvent> => {
  switch (outcome) {
    case "completes": {
      const events: ReadonlyArray<ProviderStreamEvent> = [
        ProviderStreamEvent.Progress(),
        ProviderStreamEvent.Completed({ candidate: fixtureAnswerCandidate }),
      ];
      return Stream.fromIterable(events);
    }
    case "fails": {
      const events: ReadonlyArray<ProviderStreamEvent> = [
        ProviderStreamEvent.Failed({ detail: "fixture failure" }),
      ];
      return Stream.fromIterable(events);
    }
    case "refuses": {
      const reason: TurnRefusalReason = "empty_output";
      const events: ReadonlyArray<ProviderStreamEvent> = [ProviderStreamEvent.Refused({ reason })];
      return Stream.fromIterable(events);
    }
    case "hangs":
      return Stream.never;
    case "start_unavailable":
      return Stream.empty;
  }
};

/** Provider registry that runs one scripted outcome. */
export const providerRegistryFixtureLayer = (outcome: ProviderFixtureOutcome) =>
  Layer.succeed(
    ProviderRegistry,
    ProviderRegistry.of({
      describe: Effect.succeed([fixtureDescriptor]),
      start: () =>
        outcome === "start_unavailable"
          ? Effect.fail(new ProviderStartError({ reason: "unavailable" }))
          : Effect.succeed<ProviderRun>({
              providerTurnRef: fixtureProviderTurnRef,
              events: outcomeStream(outcome),
            }),
    }),
  );

/** In-memory journal used by the runtime's own kernel tests. */
export const turnJournalMemoryLayer = Layer.effect(
  TurnJournal,
  Effect.gen(function* () {
    const records = yield* Ref.make(new Map<string, TurnStateRecord>());
    return TurnJournal.of({
      record: (state) =>
        Ref.update(records, (map) => new Map(map).set(state.requestRef, state)).pipe(
          Effect.catch(() => Effect.fail(new TurnJournalError({ reason: "storage_unavailable" }))),
        ),
      load: (requestRef: TurnRequestRef) => Ref.get(records).pipe(Effect.map((map) => map.get(requestRef) ?? null)),
      list: Ref.get(records).pipe(Effect.map((map) => [...map.values()])),
    });
  }),
);

/** In-memory thread repository that records appended messages. */
export const threadRepositoryMemoryLayer = Layer.effect(
  ThreadRepository,
  Effect.gen(function* () {
    const messages = yield* Ref.make<ReadonlyArray<ThreadTurnMessage>>([]);
    return ThreadRepository.of({
      exists: () => Effect.succeed(true),
      appendUser: (_thread, message) => Ref.update(messages, (all) => [...all, message]),
      appendAssistant: (_thread, message) => Ref.update(messages, (all) => [...all, message]),
    });
  }),
);

/** Artifact resolver that resolves nothing (no released artifact in fixtures). */
export const artifactResolverFixtureLayer = Layer.succeed(
  ArtifactResolver,
  ArtifactResolver.of({ resolve: () => Effect.succeed<ReleasedArtifact | null>(null) }),
);

/** Action broker that records advisory deliveries so a test can assert none replay on reload. */
export const actionBrokerRecordingLayer = Layer.effect(
  ActionBroker,
  Effect.gen(function* () {
    const delivered = yield* Ref.make(0);
    return ActionBroker.of({ deliver: () => Ref.update(delivered, (count) => count + 1) });
  }),
);

/** Compose the real TurnService over deterministic fixtures. */
export const testTurnServiceLayer = (
  options: {
    readonly outcome?: ProviderFixtureOutcome;
    readonly closedPolicy?: boolean;
    readonly failContext?: boolean;
  } = {},
): Layer.Layer<TurnService> =>
  turnServiceLayer.pipe(
    Layer.provide(contextSourceFixtureLayer({ fail: options.failContext ?? false })),
    Layer.provide(turnPolicyFixtureLayer({ closed: options.closedPolicy ?? false })),
    Layer.provide(providerRegistryFixtureLayer(options.outcome ?? "completes")),
    Layer.provide(turnJournalMemoryLayer),
    Layer.provide(threadRepositoryMemoryLayer),
    Layer.provide(artifactResolverFixtureLayer),
    Layer.provide(actionBrokerRecordingLayer),
  );

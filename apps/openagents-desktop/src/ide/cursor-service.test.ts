import { Effect, Layer, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";
import { ideAgentFixtureProposal } from "./agent-code-fixture.ts";
import {
  ideCursorFixtureCandidate,
  ideCursorFixtureCapabilities,
  ideCursorFixtureDecision,
  ideCursorFixtureDigest,
  ideCursorFixtureDisclosure,
  ideCursorFixtureIdentity,
  ideCursorFixtureRequest,
  ideCursorFixtureInput,
} from "./cursor-fixture.ts";
import { IdeCursorProvider } from "./cursor-provider.ts";
import {
  IdeCursorAuthorityFailure,
  IdeCursorDocumentAuthority,
  IdeCursorInvalidInput,
  IdeCursorProposalAuthority,
  IdeCursorService,
  IdeCursorStale,
  makeIdeCursorServiceLayer,
  type IdeCursorDocumentAuthorityShape,
  type IdeCursorProposalAuthorityShape,
} from "./cursor-service.ts";
import {
  IdeCursorCandidateRefSchema,
  IdeCursorCandidateSchema,
  type IdeCursorCapabilities,
  type IdeCursorProviderInput,
  type IdeCursorStreamEvent,
} from "./cursor-contract.ts";

type EventFactory = (input: IdeCursorProviderInput) => ReadonlyArray<unknown>;

const events = (input: IdeCursorProviderInput): ReadonlyArray<IdeCursorStreamEvent> => {
  const candidate = ideCursorFixtureCandidate(input.request);
  return [
    {
      _tag: "Identity",
      requestRef: input.request.requestRef,
      attemptRef: input.request.attemptRef,
      identity: input.request.identity,
    },
    { _tag: "Candidate", candidate },
    {
      _tag: "Finished",
      requestRef: input.request.requestRef,
      attemptRef: input.request.attemptRef,
      disclosure: ideCursorFixtureDisclosure(),
    },
  ];
};

const testLayer = (
  factory: EventFactory = events,
  options: Readonly<{
    capabilities?: IdeCursorCapabilities;
    validate?: IdeCursorDocumentAuthorityShape["validate"];
    submitProposal?: IdeCursorProposalAuthorityShape["submit"];
  }> = {},
) => {
  const core = Layer.merge(
    Layer.succeed(IdeCursorProvider, {
      capabilities: options.capabilities ?? ideCursorFixtureCapabilities(),
      generate: (input) => Stream.fromIterable(factory(input)),
    }),
    Layer.succeed(IdeCursorDocumentAuthority, {
      validate: options.validate ?? (() => Effect.void),
      accept: () =>
        Effect.succeed({
          previousContentDigest: ideCursorFixtureDigest("a"),
          resultContentDigest: ideCursorFixtureDigest("d"),
        }),
      undo: () =>
        Effect.succeed({
          previousContentDigest: ideCursorFixtureDigest("d"),
          resultContentDigest: ideCursorFixtureDigest("a"),
        }),
    }),
  );
  const dependencies = options.submitProposal === undefined
    ? core
    : Layer.merge(core, Layer.succeed(IdeCursorProposalAuthority, {
        submit: options.submitProposal,
      }));
  return makeIdeCursorServiceLayer({
    now: () => "2026-07-19T12:00:10.000Z",
  }).pipe(
    Layer.provide(dependencies),
  );
};

const run = <A, E>(
  effect: Effect.Effect<A, E, IdeCursorService>,
  factory?: EventFactory,
  options?: Parameters<typeof testLayer>[1],
) => Effect.runPromise(effect.pipe(Effect.provide(testLayer(factory, options))));

const settle = Effect.sleep("10 millis");

describe("IdeCursorService", () => {
  test("admits identity before candidate and retains the provider disclosure", async () => {
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
    );
    expect(snapshot.candidates).toHaveLength(1);
    expect(snapshot.candidates[0]?.identity.admitted.provider.value).toBe("provider.fixture");
    expect(snapshot.finalDisclosure).toEqual(ideCursorFixtureDisclosure());
    expect(snapshot.failure).toBeNull();
    expect(snapshot.state).toBe("complete");
  });

  test("drops a candidate emitted before the attempt identity", async () => {
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      (input) => {
        const candidate = ideCursorFixtureCandidate(input.request);
        return [
          { _tag: "Candidate", candidate },
          {
            _tag: "Identity",
            requestRef: input.request.requestRef,
            attemptRef: input.request.attemptRef,
            identity: input.request.identity,
          },
        ];
      },
    );
    expect(snapshot.candidates).toHaveLength(0);
    expect(snapshot.failure?.reason).toBe("invalid_output");
    expect(snapshot.state).toBe("failed");
  });

  test("requires the next exact sequence and interrupts the previous attempt", async () => {
    const result = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        const request = ideCursorFixtureRequest("three", 3);
        return yield* service.start(ideCursorFixtureInput(request)).pipe(Effect.flip);
      }),
      () => [],
    );
    expect(result).toBeInstanceOf(IdeCursorStale);
    if (result instanceof IdeCursorStale) expect(result.reason).toBe("sequence");
  });

  test("records accept and undo as authority-backed receipts", async () => {
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        const input = ideCursorFixtureInput();
        const candidate = ideCursorFixtureCandidate(input.request);
        yield* service.start(input);
        yield* settle;
        yield* service.decide(ideCursorFixtureDecision(candidate, "accept"));
        return yield* service.decide(ideCursorFixtureDecision(candidate, "undo"));
      }),
    );
    expect(snapshot.decisions.map((decision) => decision._tag)).toEqual(["Accept", "Undo"]);
    expect(snapshot.receipts.map((receipt) => receipt.applied)).toEqual([true, true]);
    expect(snapshot.receipts[1]?.resultContentDigest).toBe(ideCursorFixtureDigest("a"));
  });

  test("submits multi-file candidates to IDE-08 without bypassing proposal review", async () => {
    const request = ideCursorFixtureRequest("proposal", 1, {
      intent: { _tag: "Edit", instruction: "Change the exact attached file." },
    });
    const proposal = ideAgentFixtureProposal();
    const completion = ideCursorFixtureCandidate(request);
    const { replace: _replace, text: _text, ...common } = completion;
    const candidate = IdeCursorCandidateSchema.cases.Proposal.make({
      ...common,
      _tag: "Proposal",
      proposalRef: proposal.proposalRef,
      proposal,
    });
    let submitted: string | null = null;
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput(request));
        yield* settle;
        return yield* service.decide(ideCursorFixtureDecision(candidate, "accept"));
      }),
      input => [
        {
          _tag: "Identity",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          identity: input.request.identity,
        },
        { _tag: "Candidate", candidate },
        {
          _tag: "Finished",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          disclosure: ideCursorFixtureDisclosure(),
        },
      ],
      {
        submitProposal: value => {
          submitted = value.proposalRef;
          return Effect.void;
        },
      },
    );
    expect(submitted).toBe(proposal.proposalRef);
    expect(snapshot.receipts.at(-1)).toMatchObject({
      proposalRef: proposal.proposalRef,
      proposalSubmitted: true,
      applied: false,
    });
  });

  test("rejects a provider or model that was never admitted", async () => {
    const request = ideCursorFixtureRequest("mismatch", 1, {
      identity: ideCursorFixtureIdentity({ provider: "provider.untrusted" }),
    });
    const failure = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        return yield* service.start(ideCursorFixtureInput(request)).pipe(Effect.flip);
      }),
    );
    expect(failure).toBeInstanceOf(IdeCursorStale);
    if (failure instanceof IdeCursorStale) expect(failure.reason).toBe("identity");
  });

  test("stops the service and rejects every later request", async () => {
    const result = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        const stopped = yield* service.stop("test teardown");
        const failure = yield* service.start(ideCursorFixtureInput()).pipe(Effect.flip);
        return { stopped, failure };
      }),
      () => [],
    );
    expect(result.stopped.state).toBe("stopped");
    expect(result.failure).toBeInstanceOf(IdeCursorStale);
  });

  test("retains the provider terminal failure instead of converting it to idle", async () => {
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      (input) => [
        {
          _tag: "Identity",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          identity: input.request.identity,
        },
        {
          _tag: "Failed",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          reason: "budget",
          detail: "fixture budget exhausted",
        },
      ],
    );
    expect(snapshot.state).toBe("failed");
    expect(snapshot.failure?.reason).toBe("budget");
    expect(snapshot.failure?.detail).toBe("fixture budget exhausted");
    expect(snapshot.finalDisclosure).toBeNull();
  });

  test("fails closed for stale candidates and mismatched intent kinds", async () => {
    const stale = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      (input) => [
        {
          _tag: "Identity",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          identity: input.request.identity,
        },
        {
          _tag: "Candidate",
          candidate: ideCursorFixtureCandidate(input.request, {
            staleness: { _tag: "Stale", reason: "selection" },
          }),
        },
      ],
    );
    expect(stale.failure?.reason).toBe("stale");
    expect(stale.candidates).toHaveLength(0);

    const wrongKind = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      (input) => {
        const completion = ideCursorFixtureCandidate(input.request);
        return [
          {
            _tag: "Identity",
            requestRef: input.request.requestRef,
            attemptRef: input.request.attemptRef,
            identity: input.request.identity,
          },
          {
            _tag: "Candidate",
            candidate: { ...completion, _tag: "Answer", markdown: "not a completion" },
          },
        ];
      },
    );
    expect(wrongKind.failure?.reason).toBe("invalid_output");
    expect(wrongKind.candidates).toHaveLength(0);
  });

  test("retains only the newest 32 immutable candidates", async () => {
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      (input) => [
        {
          _tag: "Identity",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          identity: input.request.identity,
        },
        ...Array.from({ length: 40 }, (_, index) => ({
          _tag: "Candidate",
          candidate: ideCursorFixtureCandidate(input.request, {
            candidateRef: IdeCursorCandidateRefSchema.make(
              `ide.cursor-candidate.fixture.bound.${index}`,
            ),
          }),
        })),
        {
          _tag: "Finished",
          requestRef: input.request.requestRef,
          attemptRef: input.request.attemptRef,
          disclosure: ideCursorFixtureDisclosure(),
        },
      ],
    );
    expect(snapshot.candidates).toHaveLength(32);
    expect(snapshot.candidates[0]?.candidateRef).toBe("ide.cursor-candidate.fixture.bound.8");
    expect(snapshot.candidates[31]?.candidateRef).toBe("ide.cursor-candidate.fixture.bound.39");
  });

  test("rejects reuse of an immutable candidate ref for different content", async () => {
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      (input) => {
        const candidate = ideCursorFixtureCandidate(input.request);
        return [
          {
            _tag: "Identity",
            requestRef: input.request.requestRef,
            attemptRef: input.request.attemptRef,
            identity: input.request.identity,
          },
          { _tag: "Candidate", candidate },
          { _tag: "Candidate", candidate: { ...candidate, text: "different bytes" } },
        ];
      },
    );
    expect(snapshot.candidates).toHaveLength(1);
    expect(snapshot.failure?.reason).toBe("invalid_output");
    expect(snapshot.state).toBe("failed");
  });

  test("does not swallow candidate-time authority validation", async () => {
    let validations = 0;
    const snapshot = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        yield* service.start(ideCursorFixtureInput());
        yield* settle;
        return yield* service.snapshot;
      }),
      events,
      {
        validate: () => {
          validations += 1;
          return validations === 1
            ? Effect.void
            : Effect.fail(
                new IdeCursorAuthorityFailure({
                  operation: "fixture.validate",
                  reason: "stale",
                  detail: "the Monaco document advanced",
                }),
              );
        },
      },
    );
    expect(validations).toBe(2);
    expect(snapshot.candidates).toHaveLength(0);
    expect(snapshot.failure?.reason).toBe("stale");
    expect(snapshot.failure?.detail).toBe("the Monaco document advanced");
  });

  test("enforces capability intent and the anchored content digest before dispatch", async () => {
    const unsupported = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        return yield* service.start(ideCursorFixtureInput()).pipe(Effect.flip);
      }),
      () => [],
      {
        capabilities: { ...ideCursorFixtureCapabilities(), intents: ["ask"] },
      },
    );
    expect(unsupported).toBeInstanceOf(IdeCursorInvalidInput);

    const request = ideCursorFixtureRequest("digest", 1, {
      anchor: { ...ideCursorFixtureRequest().anchor, contentDigest: ideCursorFixtureDigest("f") },
    });
    const stale = await run(
      Effect.gen(function* () {
        const service = yield* IdeCursorService;
        return yield* service.start(ideCursorFixtureInput(request)).pipe(Effect.flip);
      }),
      () => [],
    );
    expect(stale).toBeInstanceOf(IdeCursorStale);
    if (stale instanceof IdeCursorStale) expect(stale.reason).toBe("anchor");
  });
});

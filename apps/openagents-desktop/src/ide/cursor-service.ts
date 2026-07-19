import { sha256 } from "@noble/hashes/sha256";

import { Context, Effect, Fiber, Layer, Option, PubSub, Ref, Schema, Semaphore, Stream } from "effect";
import {
  IdeCursorCapabilitiesSchema,
  IdeCursorCandidateSchema,
  IdeCursorDecisionReceiptSchema,
  IdeCursorDecisionSchema,
  IdeCursorFailureSchema,
  IdeCursorProviderInputSchema,
  IdeCursorSnapshotSchema,
  IdeCursorStreamEventSchema,
  emptyIdeCursorSnapshot,
  type IdeCursorAnchor,
  type IdeCursorCapabilities,
  type IdeCursorCandidate,
  type IdeCursorDecision,
  type IdeCursorProviderInput,
  type IdeCursorSnapshot,
  type IdeCursorStreamEvent,
  type IdeCursorFailure,
} from "./cursor-contract.ts";
import { IdeCursorProvider, IdeCursorProviderFailure } from "./cursor-provider.ts";
import { IdeTimestampSchema } from "./project-contract.ts";

const message = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000));

export class IdeCursorInvalidInput extends Schema.TaggedErrorClass<IdeCursorInvalidInput>()(
  "IdeCursor.InvalidInput",
  { operation: Schema.String, detail: message },
) {}

export class IdeCursorStale extends Schema.TaggedErrorClass<IdeCursorStale>()("IdeCursor.Stale", {
  operation: Schema.String,
  reason: Schema.Literals(["sequence", "anchor", "identity", "candidate", "stopped"]),
  detail: message,
}) {}

export class IdeCursorAuthorityFailure extends Schema.TaggedErrorClass<IdeCursorAuthorityFailure>()(
  "IdeCursor.AuthorityFailure",
  {
    operation: Schema.String,
    reason: Schema.Literals(["stale", "unavailable", "conflict"]),
    detail: message,
  },
) {}

export type IdeCursorServiceError =
  | IdeCursorInvalidInput
  | IdeCursorStale
  | IdeCursorAuthorityFailure;

export const IdeCursorAppliedResultSchema = Schema.Struct({
  previousContentDigest: Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u)),
  resultContentDigest: Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u)),
}).annotate({ identifier: "IdeCursorAppliedResult" });
export type IdeCursorAppliedResult = typeof IdeCursorAppliedResultSchema.Type;

export interface IdeCursorDocumentAuthorityShape {
  readonly validate: (anchor: IdeCursorAnchor) => Effect.Effect<void, IdeCursorAuthorityFailure>;
  readonly accept: (
    candidate: IdeCursorCandidate,
    granularity: "word" | "line" | "all",
  ) => Effect.Effect<IdeCursorAppliedResult, IdeCursorAuthorityFailure>;
  readonly undo: (
    candidate: IdeCursorCandidate,
  ) => Effect.Effect<IdeCursorAppliedResult, IdeCursorAuthorityFailure>;
}

export class IdeCursorDocumentAuthority extends Context.Service<
  IdeCursorDocumentAuthority,
  IdeCursorDocumentAuthorityShape
>()("@openagentsinc/openagents/IdeCursorDocumentAuthority") {}

export interface IdeCursorProposalAuthorityShape {
  readonly submit: (
    candidate: Extract<IdeCursorCandidate, { readonly _tag: "Proposal" }>,
  ) => Effect.Effect<void, IdeCursorAuthorityFailure>;
}

export class IdeCursorProposalAuthority extends Context.Service<
  IdeCursorProposalAuthority,
  IdeCursorProposalAuthorityShape
>()("@openagentsinc/openagents/IdeCursorProposalAuthority") {}

export interface IdeCursorServiceShape {
  readonly snapshot: Effect.Effect<IdeCursorSnapshot>;
  readonly changes: Stream.Stream<IdeCursorSnapshot>;
  readonly start: (
    input: IdeCursorProviderInput,
  ) => Effect.Effect<IdeCursorSnapshot, IdeCursorServiceError>;
  readonly decide: (
    decision: IdeCursorDecision,
  ) => Effect.Effect<IdeCursorSnapshot, IdeCursorServiceError>;
  readonly stop: (reason: string) => Effect.Effect<IdeCursorSnapshot>;
}

export class IdeCursorService extends Context.Service<IdeCursorService, IdeCursorServiceShape>()(
  "@openagentsinc/openagents/IdeCursorService",
) {}

const sameEvidence = (
  left: IdeCursorCandidate["identity"]["effective"]["provider"]["evidence"],
  right: IdeCursorCandidate["identity"]["effective"]["provider"]["evidence"],
): boolean => {
  if (left._tag !== right._tag) return false;
  switch (left._tag) {
    case "Observed":
    case "ProviderDeclared":
      return (
        (right._tag === "Observed" || right._tag === "ProviderDeclared") &&
        left.evidenceRef === right.evidenceRef &&
        left.observedAt === right.observedAt
      );
    case "RequestedOnly":
    case "NotAvailable":
      return (
        (right._tag === "RequestedOnly" || right._tag === "NotAvailable") &&
        left.reason === right.reason
      );
  }
};

const sameIdentityValue = (
  left: IdeCursorCandidate["identity"]["effective"]["provider"],
  right: IdeCursorCandidate["identity"]["effective"]["provider"],
): boolean => left.value === right.value && sameEvidence(left.evidence, right.evidence);

const sameExecutionIdentity = (
  left: IdeCursorCandidate["identity"]["effective"],
  right: IdeCursorCandidate["identity"]["effective"],
): boolean =>
  sameIdentityValue(left.harness, right.harness) &&
  sameIdentityValue(left.provider, right.provider) &&
  sameIdentityValue(left.model, right.model) &&
  sameIdentityValue(left.account, right.account) &&
  left.placementRef === right.placementRef &&
  left.placementGeneration === right.placementGeneration &&
  left.indexPosture === right.indexPosture &&
  left.networkPosture === right.networkPosture;

const sameExecutionCoordinates = (
  left: IdeCursorCandidate["identity"]["effective"],
  right: IdeCursorCandidate["identity"]["effective"],
): boolean =>
  left.harness.value === right.harness.value &&
  left.provider.value === right.provider.value &&
  left.model.value === right.model.value &&
  left.account.value === right.account.value &&
  left.placementRef === right.placementRef &&
  left.placementGeneration === right.placementGeneration &&
  left.indexPosture === right.indexPosture &&
  left.networkPosture === right.networkPosture;

const sameSubstitution = (
  left: IdeCursorCandidate["identity"]["substitution"],
  right: IdeCursorCandidate["identity"]["substitution"],
): boolean => {
  if (left._tag !== right._tag) return false;
  switch (left._tag) {
    case "None":
      return true;
    case "BeforeContent":
      return (
        right._tag === "BeforeContent" &&
        left.from === right.from &&
        left.to === right.to &&
        left.reason === right.reason
      );
    case "NewAttempt":
      return (
        right._tag === "NewAttempt" &&
        left.fromAttemptRef === right.fromAttemptRef &&
        left.reason === right.reason
      );
  }
};

const sameIdentity = (
  left: IdeCursorCandidate["identity"],
  right: IdeCursorCandidate["identity"],
): boolean =>
  sameExecutionIdentity(left.requested, right.requested) &&
  sameExecutionIdentity(left.admitted, right.admitted) &&
  sameExecutionIdentity(left.effective, right.effective) &&
  sameSubstitution(left.substitution, right.substitution);

const sameAnchor = (left: IdeCursorAnchor, right: IdeCursorAnchor): boolean =>
  left.projectRef === right.projectRef &&
  left.rootRef === right.rootRef &&
  left.worktreeRef === right.worktreeRef &&
  left.attachmentRef === right.attachmentRef &&
  left.attachmentGeneration === right.attachmentGeneration &&
  left.sessionRef === right.sessionRef &&
  left.sourceDocumentRef === right.sourceDocumentRef &&
  left.sourceDocumentGeneration === right.sourceDocumentGeneration &&
  left.fileRef === right.fileRef &&
  left.documentRef === right.documentRef &&
  left.documentGeneration === right.documentGeneration &&
  left.documentSequence === right.documentSequence &&
  left.modelVersion === right.modelVersion &&
  left.selectionVersion === right.selectionVersion &&
  left.pathRef === right.pathRef &&
  left.selection.start.line === right.selection.start.line &&
  left.selection.start.column === right.selection.start.column &&
  left.selection.end.line === right.selection.end.line &&
  left.selection.end.column === right.selection.end.column &&
  left.contentDigest === right.contentDigest;

const sameCandidate = (left: IdeCursorCandidate, right: IdeCursorCandidate): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

const contentDigest = (value: string): string =>
  `sha256:${Array.from(sha256(new TextEncoder().encode(value)), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("")}`;

const requiredCapability = (
  intent: IdeCursorProviderInput["request"]["intent"],
): IdeCursorCapabilities["intents"][number] => {
  switch (intent._tag) {
    case "Complete":
      return "complete";
    case "NextEdit":
      return "next_edit";
    case "Ask":
      return "ask";
    case "Edit":
    case "Generate":
      return "change";
  }
};

const eligibleCandidate = (
  intent: IdeCursorProviderInput["request"]["intent"],
  candidate: IdeCursorCandidate,
): boolean => {
  switch (intent._tag) {
    case "Complete":
      return candidate._tag === "Completion";
    case "NextEdit":
      return candidate._tag === "NextEdit";
    case "Ask":
      return candidate._tag === "Answer";
    case "Edit":
    case "Generate":
      return candidate._tag === "Proposal";
  }
};

const hasEffectiveEvidence = (identity: IdeCursorCandidate["identity"]): boolean =>
  [
    identity.effective.harness,
    identity.effective.provider,
    identity.effective.model,
    identity.effective.account,
  ].every(
    (value) => value.evidence._tag === "Observed" || value.evidence._tag === "ProviderDeclared",
  );

const validIdentityProgress = (
  identity: IdeCursorCandidate["identity"],
  attemptRef: string,
): boolean => {
  if (
    !sameExecutionCoordinates(identity.admitted, identity.effective) ||
    !hasEffectiveEvidence(identity)
  )
    return false;
  if (
    identity.effective.networkPosture === "offline" &&
    identity.effective.indexPosture === "remote"
  )
    return false;
  switch (identity.substitution._tag) {
    case "None":
      return sameExecutionCoordinates(identity.requested, identity.admitted);
    case "BeforeContent":
      return (
        identity.substitution.from === identity.requested.model.value &&
        identity.substitution.to === identity.admitted.model.value &&
        identity.substitution.from !== identity.substitution.to &&
        identity.requested.harness.value === identity.admitted.harness.value &&
        identity.requested.provider.value === identity.admitted.provider.value &&
        identity.requested.account.value === identity.admitted.account.value &&
        identity.requested.placementRef === identity.admitted.placementRef &&
        identity.requested.placementGeneration === identity.admitted.placementGeneration &&
        identity.requested.indexPosture === identity.admitted.indexPosture &&
        identity.requested.networkPosture === identity.admitted.networkPosture
      );
    case "NewAttempt":
      return identity.substitution.fromAttemptRef !== attemptRef;
  }
};

const publish = (bus: PubSub.PubSub<IdeCursorSnapshot>, snapshot: IdeCursorSnapshot) =>
  PubSub.publish(bus, snapshot).pipe(Effect.asVoid);

const decodeInput = <S extends Schema.ConstraintDecoder<unknown, never>>(
  operation: string,
  schema: S,
  value: unknown,
): Effect.Effect<S["Type"], IdeCursorInvalidInput> =>
  Schema.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(
      (error) => new IdeCursorInvalidInput({ operation, detail: String(error).slice(0, 2_000) }),
    ),
  );

export const makeIdeCursorServiceLayer = (input?: Readonly<{
  now?: () => string
  initialSequence?: number
}>) =>
  Layer.effect(
    IdeCursorService,
    Effect.gen(function* () {
      const provider = yield* IdeCursorProvider;
      const authority = yield* IdeCursorDocumentAuthority;
      const proposalAuthority = yield* Effect.serviceOption(IdeCursorProposalAuthority);
      const state = yield* Ref.make<IdeCursorSnapshot>(IdeCursorSnapshotSchema.make({
        ...emptyIdeCursorSnapshot(),
        latestSequence: input?.initialSequence ?? 0,
      }));
      const active = yield* Ref.make<Fiber.Fiber<void, never> | null>(null);
      const identities = yield* Ref.make(
        new Map<string, IdeCursorStreamEvent & { readonly _tag: "Identity" }>(),
      );
      const bus = yield* PubSub.unbounded<IdeCursorSnapshot>();
      const lock = yield* Semaphore.make(1);
      const now = () => IdeTimestampSchema.make(input?.now?.() ?? new Date().toISOString());

      const update = (f: (current: IdeCursorSnapshot) => IdeCursorSnapshot) =>
        Effect.gen(function* () {
          const next = yield* Ref.updateAndGet(state, (current) =>
            IdeCursorSnapshotSchema.make(f(current)),
          );
          yield* publish(bus, next);
          return next;
        });

      const ifCurrent = (
        requestRef: string,
        f: (current: IdeCursorSnapshot) => IdeCursorSnapshot,
      ) => update((current) => (current.activeRequestRef === requestRef ? f(current) : current));

      const failCurrent = (failure: IdeCursorFailure) =>
        ifCurrent(failure.requestRef, (current) =>
          current.state === "stopped"
            ? current
            : {
                ...current,
                finalDisclosure: null,
                failure: IdeCursorFailureSchema.make(failure),
                state: "failed",
              },
        );

      const invalidOutput = (
        request: IdeCursorProviderInput["request"],
        detail: string,
      ): Effect.Effect<IdeCursorSnapshot> =>
        failCurrent(
          IdeCursorFailureSchema.make({
            requestRef: request.requestRef,
            attemptRef: request.attemptRef,
            reason: "invalid_output",
            detail,
          }),
        );

      const handle = Effect.fn("IdeCursor.handleEvent")(function* (
        request: IdeCursorProviderInput["request"],
        value: unknown,
      ) {
        const event = yield* decodeInput(
          "IdeCursor.handleEvent",
          IdeCursorStreamEventSchema,
          value,
        );
        const current = yield* Ref.get(state);
        if (
          current.activeRequestRef !== request.requestRef ||
          current.activeAttemptRef !== request.attemptRef ||
          current.state !== "running"
        )
          return;
        switch (event._tag) {
          case "Identity": {
            if (
              event.requestRef !== request.requestRef ||
              event.attemptRef !== request.attemptRef ||
              !sameIdentity(event.identity, request.identity) ||
              !validIdentityProgress(event.identity, request.attemptRef)
            ) {
              yield* invalidOutput(
                request,
                "The provider emitted an identity that does not match the admitted attempt.",
              );
              return;
            }
            const previous = (yield* Ref.get(identities)).get(request.attemptRef);
            if (previous !== undefined && !sameIdentity(previous.identity, event.identity)) {
              yield* invalidOutput(
                request,
                "The provider changed effective identity after the attempt started.",
              );
              return;
            }
            yield* Ref.update(identities, (entries) =>
              new Map(entries).set(request.attemptRef, event),
            );
            return;
          }
          case "Candidate": {
            const candidate = yield* decodeInput(
              "IdeCursor.handleCandidate",
              IdeCursorCandidateSchema,
              event.candidate,
            );
            const identity = (yield* Ref.get(identities)).get(request.attemptRef);
            if (identity === undefined) {
              yield* invalidOutput(
                request,
                "The provider emitted content before its effective identity.",
              );
              return;
            }
            if (
              candidate.requestRef !== request.requestRef ||
              candidate.attemptRef !== request.attemptRef ||
              candidate.sequence !== request.sequence ||
              !sameIdentity(candidate.identity, request.identity) ||
              !sameIdentity(candidate.identity, identity.identity)
            ) {
              yield* invalidOutput(
                request,
                "The candidate identity, attempt, or sequence does not match the admitted request.",
              );
              return;
            }
            if (!eligibleCandidate(request.intent, candidate)) {
              yield* invalidOutput(
                request,
                "The provider emitted a candidate kind that is not eligible for the admitted intent.",
              );
              return;
            }
            if (
              !sameAnchor(candidate.anchor, request.anchor) ||
              candidate.staleness._tag !== "Fresh"
            ) {
              yield* failCurrent(
                IdeCursorFailureSchema.make({
                  requestRef: request.requestRef,
                  attemptRef: request.attemptRef,
                  reason: "stale",
                  detail:
                    candidate.staleness._tag === "Stale"
                      ? `The provider marked the candidate stale (${candidate.staleness.reason}).`
                      : "The candidate anchor does not match the admitted Monaco document state.",
                }),
              );
              return;
            }
            yield* authority.validate(candidate.anchor);
            const duplicate = current.candidates.find(
              (item) => item.candidateRef === candidate.candidateRef,
            );
            if (duplicate !== undefined && !sameCandidate(duplicate, candidate)) {
              yield* invalidOutput(
                request,
                "The provider reused a candidate ref for different content.",
              );
              return;
            }
            yield* ifCurrent(request.requestRef, (snapshot) => ({
              ...snapshot,
              state: "running",
              // A candidate ref is immutable. A provider retry must use a new ref.
              candidates:
                duplicate !== undefined
                  ? snapshot.candidates
                  : [...snapshot.candidates, candidate].slice(-32),
            }));
            return;
          }
          case "Failed": {
            if (
              event.requestRef !== request.requestRef ||
              event.attemptRef !== request.attemptRef
            ) {
              yield* invalidOutput(
                request,
                "The provider emitted a failure for a different attempt.",
              );
              return;
            }
            yield* failCurrent(IdeCursorFailureSchema.make(event));
            return;
          }
          case "Finished": {
            const identity = (yield* Ref.get(identities)).get(request.attemptRef);
            if (
              event.requestRef !== request.requestRef ||
              event.attemptRef !== request.attemptRef ||
              identity === undefined
            ) {
              yield* invalidOutput(
                request,
                "The provider finished without the admitted attempt identity.",
              );
              return;
            }
            yield* ifCurrent(request.requestRef, (snapshot) => ({
              ...snapshot,
              finalDisclosure: event.disclosure,
              failure: null,
              state: "complete",
            }));
            return;
          }
        }
      });

      const start = Effect.fn("IdeCursor.start")(function* (value: IdeCursorProviderInput) {
        const decoded = yield* decodeInput("IdeCursor.start", IdeCursorProviderInputSchema, value);
        const capabilities = yield* decodeInput(
          "IdeCursor.start.capabilities",
          IdeCursorCapabilitiesSchema,
          provider.capabilities,
        );
        return yield* lock.withPermit(
          Effect.gen(function* () {
            const current = yield* Ref.get(state);
            if (current.state === "stopped")
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.start",
                  reason: "stopped",
                  detail: "service is stopped",
                }),
              );
            if (decoded.request.sequence !== current.latestSequence + 1)
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.start",
                  reason: "sequence",
                  detail: "sequence must advance by exactly one",
                }),
              );
            if (
              !validIdentityProgress(decoded.request.identity, decoded.request.attemptRef) ||
              decoded.request.identity.admitted.provider.value !== capabilities.providerRef ||
              decoded.request.identity.effective.provider.value !== capabilities.providerRef ||
              !capabilities.modelRefs.includes(decoded.request.identity.admitted.model.value) ||
              !capabilities.modelRefs.includes(decoded.request.identity.effective.model.value)
            ) {
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.start",
                  reason: "identity",
                  detail: "provider admission does not match the selected capability",
                }),
              );
            }
            if (!capabilities.intents.includes(requiredCapability(decoded.request.intent))) {
              return yield* Effect.fail(
                new IdeCursorInvalidInput({
                  operation: "IdeCursor.start",
                  detail: "the selected provider does not admit the requested cursor intent",
                }),
              );
            }
            if (
              decoded.request.identity.effective.networkPosture === "offline" &&
              !capabilities.supportsOffline
            ) {
              return yield* Effect.fail(
                new IdeCursorInvalidInput({
                  operation: "IdeCursor.start",
                  detail: "the selected provider does not admit an offline attempt",
                }),
              );
            }
            if (contentDigest(decoded.documentText) !== decoded.request.anchor.contentDigest) {
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.start",
                  reason: "anchor",
                  detail: "document text does not match the anchored content digest",
                }),
              );
            }
            yield* authority.validate(decoded.request.anchor);
            const previous = yield* Ref.get(active);
            if (previous !== null) yield* Fiber.interrupt(previous);
            yield* Ref.set(identities, new Map());
            const started = yield* update((snapshot) => ({
              ...snapshot,
              latestSequence: decoded.request.sequence,
              activeRequestRef: decoded.request.requestRef,
              activeAttemptRef: decoded.request.attemptRef,
              finalDisclosure: null,
              failure: null,
              state: "running",
            }));
            const consume = Stream.runForEach(provider.generate(decoded), (event) =>
              handle(decoded.request, event),
            ).pipe(
              Effect.catch(
                (
                  error:
                    | IdeCursorProviderFailure
                    | IdeCursorInvalidInput
                    | IdeCursorAuthorityFailure,
                ) => {
                  if (error instanceof IdeCursorInvalidInput) {
                    return failCurrent(
                      IdeCursorFailureSchema.make({
                        requestRef: decoded.request.requestRef,
                        attemptRef: decoded.request.attemptRef,
                        reason: "invalid_output",
                        detail: error.detail,
                      }),
                    );
                  }
                  if (error instanceof IdeCursorAuthorityFailure) {
                    return failCurrent(
                      IdeCursorFailureSchema.make({
                        requestRef: decoded.request.requestRef,
                        attemptRef: decoded.request.attemptRef,
                        reason:
                          error.reason === "stale" || error.reason === "conflict"
                            ? "stale"
                            : "unavailable",
                        detail: error.detail,
                      }),
                    );
                  }
                  return failCurrent(
                    IdeCursorFailureSchema.make({
                      requestRef: decoded.request.requestRef,
                      attemptRef: decoded.request.attemptRef,
                      reason:
                        error.reason === "interrupted"
                          ? "cancelled"
                          : error.reason === "unavailable"
                            ? "unavailable"
                            : error.reason === "invalid_event"
                              ? "invalid_output"
                              : "provider",
                      detail: error.detail,
                    }),
                  );
                },
              ),
              Effect.asVoid,
            );
            const fiber = yield* Effect.forkDetach(consume);
            yield* Ref.set(active, fiber);
            return started;
          }),
        );
      });

      const decide = Effect.fn("IdeCursor.decide")(function* (value: IdeCursorDecision) {
        const decision = yield* decodeInput("IdeCursor.decide", IdeCursorDecisionSchema, value);
        return yield* lock.withPermit(
          Effect.gen(function* () {
            const current = yield* Ref.get(state);
            if (current.state === "stopped")
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.decide",
                  reason: "stopped",
                  detail: "service is stopped",
                }),
              );
            if (decision.sequence !== current.latestSequence)
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.decide",
                  reason: "sequence",
                  detail: "decision sequence is not current",
                }),
              );
            if (decision.requestRef !== current.activeRequestRef)
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.decide",
                  reason: "candidate",
                  detail: "decision request is not the current request",
                }),
              );
            const candidate =
              "candidateRef" in decision && decision.candidateRef !== null
                ? current.candidates.find(
                    (item) =>
                      item.candidateRef === decision.candidateRef &&
                      item.requestRef === decision.requestRef &&
                      item.sequence === decision.sequence,
                  )
                : undefined;
            if (decision._tag !== "Cancel" && candidate === undefined)
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.decide",
                  reason: "candidate",
                  detail: "candidate is missing or superseded",
                }),
              );
            if (decision._tag === "Accept" && candidate !== undefined &&
              decision.resultDigest !== candidate.resultDigest) {
              return yield* Effect.fail(new IdeCursorStale({
                operation: "IdeCursor.decide",
                reason: "candidate",
                detail: "The accept decision result digest does not match the immutable candidate.",
              }));
            }
            if (decision._tag === "Undo" && candidate !== undefined &&
              decision.resultDigest !== candidate.anchor.contentDigest) {
              return yield* Effect.fail(new IdeCursorStale({
                operation: "IdeCursor.decide",
                reason: "candidate",
                detail: "The undo decision does not bind the candidate's exact base digest.",
              }));
            }
            if (decision._tag === "Undo" && candidate !== undefined &&
              candidate._tag !== "Completion" && candidate._tag !== "NextEdit") {
              return yield* Effect.fail(
                new IdeCursorStale({
                  operation: "IdeCursor.decide",
                  reason: "candidate",
                  detail:
                    "only completion and next-edit candidates are eligible for direct document mutation",
                }),
              );
            }
            let applied: IdeCursorAppliedResult | null = null;
            let proposalSubmitted = false;
            if (decision._tag === "Accept" && candidate !== undefined) {
              if (candidate._tag === "Completion" || candidate._tag === "NextEdit") {
                yield* authority.validate(candidate.anchor);
                applied = yield* authority.accept(candidate, decision.granularity);
              } else if (candidate._tag === "Proposal") {
                if (Option.isNone(proposalAuthority)) {
                  return yield* Effect.fail(new IdeCursorAuthorityFailure({
                    operation: "IdeCursor.decide",
                    reason: "unavailable",
                    detail: "The IDE-08 proposal authority is unavailable.",
                  }));
                }
                yield* proposalAuthority.value.submit(candidate);
                proposalSubmitted = true;
              } else {
                return yield* Effect.fail(new IdeCursorStale({
                  operation: "IdeCursor.decide",
                  reason: "candidate",
                  detail: "An ask-only answer cannot be accepted as a document mutation.",
                }));
              }
            }
            if (decision._tag === "Undo" && candidate !== undefined) {
              const accepted = current.receipts.some(
                (receipt) =>
                  receipt.decision._tag === "Accept" &&
                  receipt.decision.candidateRef === candidate.candidateRef &&
                  receipt.applied,
              );
              if (!accepted)
                return yield* Effect.fail(
                  new IdeCursorStale({
                    operation: "IdeCursor.decide",
                    reason: "candidate",
                    detail: "candidate has no applied accept receipt to undo",
                  }),
                );
              applied = yield* authority.undo(candidate);
            }
            if (decision._tag === "Cancel") {
              const fiber = yield* Ref.get(active);
              if (fiber !== null) yield* Fiber.interrupt(fiber);
              yield* Ref.set(active, null);
            }
            const receipt = IdeCursorDecisionReceiptSchema.make({
              schemaVersion: "openagents.ide-cursor.v1",
              decision,
              recordedAt: now(),
              previousContentDigest: applied?.previousContentDigest ?? null,
              resultContentDigest: applied?.resultContentDigest ?? null,
              proposalRef: candidate?._tag === "Proposal" ? candidate.proposalRef : null,
              proposalSubmitted,
              applied: applied !== null,
              staleRejected: false,
            });
            return yield* update((snapshot) => ({
              ...snapshot,
              decisions: [...snapshot.decisions, decision].slice(-500),
              receipts: [...snapshot.receipts, receipt].slice(-500),
              state: decision._tag === "Cancel" ? "idle" : snapshot.state,
            }));
          }),
        );
      });

      const stop = Effect.fn("IdeCursor.stop")(function* (_reason: string) {
        const fiber = yield* Ref.get(active);
        if (fiber !== null) yield* Fiber.interrupt(fiber);
        yield* Ref.set(active, null);
        return yield* update((current) => ({
          ...current,
          activeRequestRef: null,
          activeAttemptRef: null,
          state: "stopped",
        }));
      });

      yield* Effect.addFinalizer(() =>
        Effect.gen(function* () {
          const fiber = yield* Ref.get(active);
          if (fiber !== null) yield* Fiber.interrupt(fiber);
          yield* PubSub.shutdown(bus);
        }),
      );

      return IdeCursorService.of({
        snapshot: Ref.get(state),
        changes: Stream.fromPubSub(bus),
        start,
        decide,
        stop,
      });
    }),
  );

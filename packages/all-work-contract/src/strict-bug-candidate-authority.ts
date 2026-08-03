import { createHash, randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import {
  decodeStrictBugCandidate,
  decodeStrictBugCandidateExecuteRequest,
  decodeStrictBugCandidateLedger,
  decodeStrictBugCandidateReadRequest,
  type StrictBugCandidate,
  type StrictBugCandidateExecuteRequest,
  type StrictBugCandidateExecuteResult,
  StrictBugCandidateExecuteResultSchema,
  type StrictBugCandidateLedger,
  StrictBugCandidateLedgerSchema,
  type StrictBugCandidateReadResult,
  StrictBugCandidateReadResultSchema,
  type StrictBugCandidateReceipt,
  StrictBugCandidateReceiptSchema,
} from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

export const STRICT_BUG_CANDIDATE_AUTHORITY_STATE_SCHEMA =
  "openagents.strict_bug_candidate_authority_state.v1" as const;
export const STRICT_BUG_INGEST_CAPABILITY = "capability:strict-bug-candidate:ingest" as const;
export const STRICT_BUG_TRIAGE_CAPABILITY = "capability:strict-bug-candidate:triage" as const;

export const StrictBugCandidateAuthorityStateSchema = S.Struct({
  schema: S.Literal(STRICT_BUG_CANDIDATE_AUTHORITY_STATE_SCHEMA),
  ingressPrincipalRefs: S.Array(S.String),
  triagePrincipalRefs: S.Array(S.String),
  ledger: StrictBugCandidateLedgerSchema,
  receipts: S.Array(StrictBugCandidateReceiptSchema),
});
export interface StrictBugCandidateAuthorityState extends S.Schema.Type<
  typeof StrictBugCandidateAuthorityStateSchema
> {}

export class StrictBugCandidateAuthorityError extends S.TaggedErrorClass<StrictBugCandidateAuthorityError>()(
  "StrictBugCandidateAuthority.Error",
  {
    reason: S.Literals([
      "invalid_state",
      "invalid_request",
      "storage_unavailable",
      "revision_conflict",
      "idempotency_conflict",
      "forbidden",
      "candidate_exists",
      "candidate_not_found",
      "candidate_revision_conflict",
      "candidate_already_disposed",
      "unsafe_content",
      "invalid_source",
      "invalid_disposition",
    ]),
    detail: S.String,
  },
) {}

export interface StrictBugCandidateStateStoreShape {
  readonly load: Effect.Effect<
    StrictBugCandidateAuthorityState | null,
    StrictBugCandidateAuthorityError
  >;
  readonly save: (
    expectedRevision: number,
    state: StrictBugCandidateAuthorityState,
  ) => Effect.Effect<void, StrictBugCandidateAuthorityError>;
}

export class StrictBugCandidateStateStore extends Context.Service<
  StrictBugCandidateStateStore,
  StrictBugCandidateStateStoreShape
>()("StrictBugCandidateAuthority.StateStore") {}

const unsafeContent =
  /(?:\/Users\/|\/home\/|bearer\s+|cookie|gh[op]_[A-Za-z0-9_]+|github_pat_|lnbc|lntb|lno1|mnemonic|oauth|password|payment[_-]?(?:preimage|secret)|private[_-]?(?:key|prompt|repo)|provider[_-]?(?:payload|secret|token)|raw[_-]?(?:payload|prompt|trace|webhook)|recovery[_-]?phrase|secret[_-]?(?:key|token|value)|sk-[A-Za-z0-9_-]{12,}|token\s*[:=]|wallet[_-]?(?:key|mnemonic|secret)|xprv)/iu;

const digest = (value: unknown): string =>
  createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex");
const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)].sort((left, right) => left.localeCompare(right));
const requiredConfirmations = [
  "exact_reproduction_and_evidence",
  "malformed_report_policy_understood",
  "searched_existing_reports",
  "sensitive_material_removed",
  "specific_reproducible_bug",
] as const;

export const emptyStrictBugCandidateAuthorityState = (
  observedAt: string,
  ingressPrincipalRefs: ReadonlyArray<string>,
  triagePrincipalRefs: ReadonlyArray<string>,
): StrictBugCandidateAuthorityState => ({
  schema: STRICT_BUG_CANDIDATE_AUTHORITY_STATE_SCHEMA,
  ingressPrincipalRefs: uniqueSorted(ingressPrincipalRefs),
  triagePrincipalRefs: uniqueSorted(triagePrincipalRefs),
  ledger: decodeStrictBugCandidateLedger({
    contractVersion: "openagents.all_work_boundary.v1",
    revision: 0,
    eventCursor: "cursor:strict-bug-candidate:0",
    candidates: [],
    completeness: { state: "complete", cursor: "cursor:strict-bug-candidate:0", gapRefs: [] },
    freshness: { state: "fresh", observedAt },
  }),
  receipts: [],
});

const decodeState = (input: unknown) =>
  S.decodeUnknownEffect(StrictBugCandidateAuthorityStateSchema)(input, {
    onExcessProperty: "error",
  }).pipe(
    Effect.mapError(
      () => new StrictBugCandidateAuthorityError({ reason: "invalid_state", detail: "decode" }),
    ),
  );

const githubIssueSourceIsAllowed = (
  sourceUrl: string,
  repositoryRef: string,
  issueNumber: number,
): boolean => {
  try {
    const url = new URL(sourceUrl);
    const parts = url.pathname.split("/").filter(Boolean);
    return (
      url.protocol === "https:" &&
      url.hostname === "github.com" &&
      parts[0] === "OpenAgentsInc" &&
      ((parts[1] === "openagents" && repositoryRef === "repository:openagents") ||
        (parts[1] === "omega" && repositoryRef === "repository:omega")) &&
      parts[2] === "issues" &&
      parts[3] === String(issueNumber) &&
      issueNumber > 0 &&
      parts.length === 4
    );
  } catch {
    return false;
  }
};

const assertPublicSafe = (values: ReadonlyArray<string>): void => {
  if (values.some((value) => unsafeContent.test(value))) {
    throw new StrictBugCandidateAuthorityError({
      reason: "unsafe_content",
      detail: "candidate contains secret-shaped or private-path content",
    });
  }
};

const advance = (
  ledger: StrictBugCandidateLedger,
  candidates: ReadonlyArray<StrictBugCandidate>,
  occurredAt: string,
): StrictBugCandidateLedger => {
  const revision = ledger.revision + 1;
  return decodeStrictBugCandidateLedger({
    ...ledger,
    revision,
    eventCursor: `cursor:strict-bug-candidate:${revision}`,
    candidates: [...candidates].sort((left, right) =>
      left.candidateRef.localeCompare(right.candidateRef),
    ),
    completeness: {
      state: "complete",
      cursor: `cursor:strict-bug-candidate:${revision}`,
      gapRefs: [],
    },
    freshness: { state: "fresh", observedAt: occurredAt },
  });
};

const applyCommand = (
  state: StrictBugCandidateAuthorityState,
  request: StrictBugCandidateExecuteRequest,
): StrictBugCandidateLedger => {
  const command = request.command;
  if (command.command === "ingest") {
    if (
      request.capabilityRef !== STRICT_BUG_INGEST_CAPABILITY ||
      !state.ingressPrincipalRefs.includes(request.effectivePrincipalRef)
    ) {
      throw new StrictBugCandidateAuthorityError({ reason: "forbidden", detail: "ingress" });
    }
    if (
      !githubIssueSourceIsAllowed(command.sourceUrl, command.repositoryRef, command.issueNumber)
    ) {
      throw new StrictBugCandidateAuthorityError({
        reason: "invalid_source",
        detail: "source must be an OpenAgentsInc strict bug issue URL",
      });
    }
    const repositorySlug = command.repositoryRef === "repository:omega" ? "omega" : "openagents";
    if (
      command.candidateRef !== `strict-bug-candidate:${repositorySlug}:${command.issueNumber}` ||
      command.sourceRef !== `source:github:${repositorySlug}:issue:${command.issueNumber}` ||
      request.idempotencyKey !== `github-delivery:${command.deliveryRef}`
    ) {
      throw new StrictBugCandidateAuthorityError({
        reason: "invalid_source",
        detail: "candidate, source, and idempotency identities must derive from the GitHub issue",
      });
    }
    assertPublicSafe([
      command.candidateRef,
      command.sourceRef,
      command.deliveryRef,
      command.title,
      command.affectedSurface,
      command.actualBehavior,
      command.expectedBehavior,
      command.reproductionSteps,
      command.publicSafeEvidence,
      command.environment,
      command.reporterRef,
      command.signatureVerificationRef,
      ...command.attachmentRefs,
    ]);
    if (
      JSON.stringify(uniqueSorted(command.requiredConfirmations)) !==
      JSON.stringify(requiredConfirmations)
    ) {
      throw new StrictBugCandidateAuthorityError({
        reason: "invalid_request",
        detail: "all strict bug confirmations are required",
      });
    }
    if (
      state.ledger.candidates.some(
        (candidate) =>
          candidate.candidateRef === command.candidateRef ||
          candidate.sourceRef === command.sourceRef ||
          candidate.deliveryRef === command.deliveryRef,
      )
    ) {
      throw new StrictBugCandidateAuthorityError({
        reason: "candidate_exists",
        detail: command.candidateRef,
      });
    }
    const candidate = decodeStrictBugCandidate({
      contractVersion: "openagents.all_work_boundary.v1",
      candidateRef: command.candidateRef,
      sourceRef: command.sourceRef,
      deliveryRef: command.deliveryRef,
      repositoryRef: command.repositoryRef,
      issueNumber: command.issueNumber,
      sourceUrl: command.sourceUrl,
      title: command.title,
      affectedSurface: command.affectedSurface,
      actualBehavior: command.actualBehavior,
      expectedBehavior: command.expectedBehavior,
      reproductionSteps: command.reproductionSteps,
      publicSafeEvidence: command.publicSafeEvidence,
      severity: command.severity,
      environment: command.environment,
      safetyRedaction: command.safetyRedaction,
      requiredConfirmations: uniqueSorted(command.requiredConfirmations),
      reporterRef: command.reporterRef,
      attachmentRefs: uniqueSorted(command.attachmentRefs),
      signatureVerificationRef: command.signatureVerificationRef,
      untrusted: true,
      disposition: "pending",
      linkedWorkRef: null,
      dispositionReceiptRef: null,
      observedAt: request.occurredAt,
      revision: 1,
    });
    return advance(state.ledger, [...state.ledger.candidates, candidate], request.occurredAt);
  }
  if (
    request.capabilityRef !== STRICT_BUG_TRIAGE_CAPABILITY ||
    !state.triagePrincipalRefs.includes(request.effectivePrincipalRef)
  ) {
    throw new StrictBugCandidateAuthorityError({ reason: "forbidden", detail: "triage" });
  }
  const candidate = state.ledger.candidates.find(
    (entry) => entry.candidateRef === command.candidateRef,
  );
  if (candidate === undefined) {
    throw new StrictBugCandidateAuthorityError({
      reason: "candidate_not_found",
      detail: command.candidateRef,
    });
  }
  if (candidate.revision !== command.expectedCandidateRevision) {
    throw new StrictBugCandidateAuthorityError({
      reason: "candidate_revision_conflict",
      detail: command.candidateRef,
    });
  }
  if (candidate.disposition !== "pending") {
    throw new StrictBugCandidateAuthorityError({
      reason: "candidate_already_disposed",
      detail: command.candidateRef,
    });
  }
  if (command.disposition === "pending") {
    throw new StrictBugCandidateAuthorityError({
      reason: "invalid_disposition",
      detail: "pending is not a terminal triage decision",
    });
  }
  const needsWork = command.disposition !== "rejected";
  if (needsWork !== (command.linkedWorkRef !== null)) {
    throw new StrictBugCandidateAuthorityError({
      reason: "invalid_disposition",
      detail: "non-rejected disposition requires exactly one linked Work ref",
    });
  }
  const next = decodeStrictBugCandidate({
    ...candidate,
    disposition: command.disposition,
    linkedWorkRef: command.linkedWorkRef,
    dispositionReceiptRef: command.dispositionReceiptRef,
    revision: candidate.revision + 1,
  });
  return advance(
    state.ledger,
    state.ledger.candidates.map((entry) =>
      entry.candidateRef === next.candidateRef ? next : entry,
    ),
    request.occurredAt,
  );
};

export interface StrictBugCandidateAuthorityShape {
  readonly read: (
    input: unknown,
  ) => Effect.Effect<StrictBugCandidateReadResult, StrictBugCandidateAuthorityError>;
  readonly execute: (
    input: unknown,
  ) => Effect.Effect<StrictBugCandidateExecuteResult, StrictBugCandidateAuthorityError>;
}

export class StrictBugCandidateAuthority extends Context.Service<
  StrictBugCandidateAuthority,
  StrictBugCandidateAuthorityShape
>()("StrictBugCandidateAuthority.Service") {}

export const StrictBugCandidateAuthorityLive = Layer.effect(
  StrictBugCandidateAuthority,
  Effect.gen(function* () {
    const store = yield* StrictBugCandidateStateStore;
    const load = store.load.pipe(
      Effect.flatMap((state) =>
        state === null
          ? Effect.fail(
              new StrictBugCandidateAuthorityError({
                reason: "invalid_state",
                detail: "store is empty",
              }),
            )
          : Effect.succeed(state),
      ),
    );
    return StrictBugCandidateAuthority.of({
      read: Effect.fn("StrictBugCandidateAuthority.read")(function* (input: unknown) {
        const request = yield* Effect.try({
          try: () => decodeStrictBugCandidateReadRequest(input),
          catch: () =>
            new StrictBugCandidateAuthorityError({ reason: "invalid_request", detail: "decode" }),
        });
        const state = yield* load;
        const candidates =
          request.candidateRef == null
            ? state.ledger.candidates
            : state.ledger.candidates.filter(
                (candidate) => candidate.candidateRef === request.candidateRef,
              );
        return yield* S.decodeUnknownEffect(StrictBugCandidateReadResultSchema)({
          ledger: { ...state.ledger, candidates },
        }).pipe(
          Effect.mapError(
            () => new StrictBugCandidateAuthorityError({ reason: "invalid_state", detail: "read" }),
          ),
        );
      }),
      execute: Effect.fn("StrictBugCandidateAuthority.execute")(function* (input: unknown) {
        const request = yield* Effect.try({
          try: () => decodeStrictBugCandidateExecuteRequest(input),
          catch: () =>
            new StrictBugCandidateAuthorityError({ reason: "invalid_request", detail: "decode" }),
        });
        const state = yield* load;
        const commandDigest = digest(request);
        const prior = state.receipts.find(
          (receipt) => receipt.idempotencyKey === request.idempotencyKey,
        );
        if (prior !== undefined) {
          if (prior.commandDigest !== commandDigest) {
            return yield* new StrictBugCandidateAuthorityError({
              reason: "idempotency_conflict",
              detail: request.idempotencyKey,
            });
          }
          return yield* S.decodeUnknownEffect(StrictBugCandidateExecuteResultSchema)({
            ledger: state.ledger,
            receipt: prior,
          }).pipe(
            Effect.mapError(
              () =>
                new StrictBugCandidateAuthorityError({
                  reason: "invalid_state",
                  detail: "replay",
                }),
            ),
          );
        }
        if (state.ledger.revision !== request.expectedRevision) {
          return yield* new StrictBugCandidateAuthorityError({
            reason: "revision_conflict",
            detail: `expected ${request.expectedRevision}, found ${state.ledger.revision}`,
          });
        }
        const ledger = yield* Effect.try({
          try: () => applyCommand(state, request),
          catch: (error) =>
            error instanceof StrictBugCandidateAuthorityError
              ? error
              : new StrictBugCandidateAuthorityError({
                  reason: "invalid_request",
                  detail: "transition",
                }),
        });
        const receipt = yield* S.decodeUnknownEffect(StrictBugCandidateReceiptSchema)({
          intentRef: request.intentRef,
          idempotencyKey: request.idempotencyKey,
          commandDigest,
          candidateRef: request.command.candidateRef,
          previousRevision: state.ledger.revision,
          revision: ledger.revision,
          eventCursor: ledger.eventCursor,
          effectivePrincipalRef: request.effectivePrincipalRef,
          acceptedAt: request.occurredAt,
          githubWriteCount: 0,
        }).pipe(
          Effect.mapError(
            () =>
              new StrictBugCandidateAuthorityError({ reason: "invalid_state", detail: "receipt" }),
          ),
        );
        const next = yield* decodeState({
          ...state,
          ledger,
          receipts: [...state.receipts, receipt],
        });
        yield* store.save(state.ledger.revision, next);
        return yield* S.decodeUnknownEffect(StrictBugCandidateExecuteResultSchema)({
          ledger: next.ledger,
          receipt,
        }).pipe(
          Effect.mapError(
            () =>
              new StrictBugCandidateAuthorityError({ reason: "invalid_state", detail: "result" }),
          ),
        );
      }),
    });
  }),
);

export const inMemoryStrictBugCandidateStateStoreLayer = (
  initial: StrictBugCandidateAuthorityState,
): Layer.Layer<StrictBugCandidateStateStore> =>
  Layer.effect(
    StrictBugCandidateStateStore,
    Effect.gen(function* () {
      const state = yield* Ref.make(initial);
      return StrictBugCandidateStateStore.of({
        load: Ref.get(state),
        save: (expectedRevision, next) =>
          Ref.modify(state, (current) =>
            current.ledger.revision !== expectedRevision
              ? [
                  Effect.fail(
                    new StrictBugCandidateAuthorityError({
                      reason: "revision_conflict",
                      detail: `expected ${expectedRevision}, found ${current.ledger.revision}`,
                    }),
                  ),
                  current,
                ]
              : [Effect.void, next],
          ).pipe(Effect.flatten),
      });
    }),
  );

export const strictBugCandidateStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "strict-bug-candidates.v1.json");
const storageError = (detail: string) =>
  new StrictBugCandidateAuthorityError({ reason: "storage_unavailable", detail });
const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";
const readFileState = (
  filePath: string,
): Effect.Effect<StrictBugCandidateAuthorityState | null, StrictBugCandidateAuthorityError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (error) => (isNotFound(error) ? storageError("not_found") : storageError("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({ try: () => JSON.parse(contents), catch: () => storageError("json") }),
    ),
    Effect.flatMap(decodeState),
    Effect.catch((error) =>
      error.detail === "not_found" ? Effect.succeed(null) : Effect.fail(error),
    ),
  );
const atomicWrite = (
  filePath: string,
  state: StrictBugCandidateAuthorityState,
): Effect.Effect<void, StrictBugCandidateAuthorityError> =>
  Effect.tryPromise({
    try: async () => {
      const directory = path.dirname(filePath);
      await mkdir(directory, { recursive: true, mode: 0o700 });
      const temporary = path.join(
        directory,
        `.${path.basename(filePath)}.${randomBytes(8).toString("hex")}.tmp`,
      );
      await writeFile(temporary, `${JSON.stringify(state, null, 2)}\n`, {
        encoding: "utf8",
        mode: 0o600,
      });
      await rename(temporary, filePath);
    },
    catch: () => storageError("write"),
  });
export const fileStrictBugCandidateStateStoreLayer = (
  rootDir: string,
): Layer.Layer<StrictBugCandidateStateStore> => {
  const filePath = strictBugCandidateStatePath(rootDir);
  const load = readFileState(filePath);
  return Layer.succeed(
    StrictBugCandidateStateStore,
    StrictBugCandidateStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.gen(function* () {
          const current = yield* load;
          if (current === null || current.ledger.revision !== expectedRevision) {
            return yield* new StrictBugCandidateAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${current?.ledger.revision ?? "none"}`,
            });
          }
          yield* atomicWrite(filePath, state);
        }),
    }),
  );
};
export const initializeFileStrictBugCandidateState = (
  rootDir: string,
  state: StrictBugCandidateAuthorityState,
): Effect.Effect<void, StrictBugCandidateAuthorityError> => {
  const filePath = strictBugCandidateStatePath(rootDir);
  return readFileState(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};

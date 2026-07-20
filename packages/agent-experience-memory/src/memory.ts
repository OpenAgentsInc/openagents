import { Effect, Schema as S } from "effect";

import {
  ExperienceBank,
  emptyExperienceBank,
} from "./contract/bank.js";
import type { MemoryConfigShape } from "./contract/config.js";
import {
  decodeExperienceRecord,
  experienceTextDigest,
  type ExperienceKind,
  type ExperienceRecord,
} from "./contract/experience.js";
import type { GlobalPattern } from "./contract/pattern.js";
import { RecallResult, decodeRecallResult } from "./contract/recall.js";
import {
  BankId,
  FactRef,
  MemoryConsent,
  type RepoRef,
  Sha256Hex,
  type TraceRef,
} from "./contract/refs.js";
import { canonicalStringify } from "./internal/canonical.js";
import { sha256Hex } from "./internal/sha256.js";
import { assertRecallClean, guardMemoryText } from "./redaction.js";
import { estimateTokens, packWithinBudget, topK } from "./ranking.js";
import { MemoryStore, type MemoryScope } from "./store.js";

const decodeFactRef = S.decodeUnknownSync(FactRef);
const decodeBankId = S.decodeUnknownSync(BankId);
const decodeDigest = S.decodeUnknownSync(Sha256Hex);

/** A stable synthetic bank id for a scope-and-time freeze. */
const bankIdFor = (scope: MemoryScope, frozenAt: string): typeof BankId.Type =>
  decodeBankId(`bank:${sha256Hex(canonicalStringify({ owner: scope.owner, project: scope.project, frozenAt }))}`);

export type RememberInput = Readonly<{
  scope: MemoryScope;
  repoRef: RepoRef;
  kind: ExperienceKind;
  text: string;
  confidence: number;
  consent?: MemoryConsent;
  observedAt: string;
  traceRef?: TraceRef | null;
  embedding?: ReadonlyArray<number>;
  usernames?: ReadonlyArray<string>;
}>;

export type RememberOutcome =
  | Readonly<{ stored: true; record: ExperienceRecord; redactions: number }>
  | Readonly<{
      stored: false;
      reason: "disabled" | "unsafe_material" | "empty_after_redaction" | "storage_error";
      categories: ReadonlyArray<string>;
    }>;

/**
 * Store one owner-local experience fact.
 *
 * The text is redacted BEFORE it is built into a record. A fact carrying hard
 * unsafe material (a secret, wallet or payment value, or a local path) is
 * rejected outright — never stored, not even scrubbed. With memory OFF this is a
 * no-op that stores nothing, so a forgotten gate cannot leak a write.
 */
export const remember = (
  config: MemoryConfigShape,
  input: RememberInput,
): Effect.Effect<RememberOutcome, never, MemoryStore> =>
  Effect.gen(function* () {
    if (!config.enabled) {
      return { stored: false, reason: "disabled", categories: [] } as const;
    }
    const verdict = guardMemoryText(input.text, input.usernames);
    if (!verdict.storable) {
      return { stored: false, reason: "unsafe_material", categories: verdict.categories } as const;
    }
    const redacted = verdict.redacted.trim();
    if (redacted.length === 0) {
      return { stored: false, reason: "empty_after_redaction", categories: verdict.categories } as const;
    }
    const digest = experienceTextDigest(redacted);
    const record = decodeExperienceRecord({
      schema: "openagents.experience_record.v1",
      recordRef: decodeFactRef(`fact:${digest}`),
      ownerScope: input.scope.owner,
      projectScope: input.scope.project,
      repoRef: input.repoRef,
      kind: input.kind,
      text: redacted,
      confidence: input.confidence,
      consent: input.consent ?? "withheld",
      observedAt: input.observedAt,
      traceRef: input.traceRef ?? null,
      digest,
      ...(input.embedding ? { embedding: input.embedding } : {}),
    });
    const store = yield* MemoryStore;
    return yield* store.put(record).pipe(
      Effect.map(() => ({ stored: true, record, redactions: verdict.total }) as const),
      Effect.catch(() =>
        Effect.succeed({ stored: false, reason: "storage_error", categories: [] } as const),
      ),
    );
  });

export type RecallRequest = Readonly<{
  repoRef?: RepoRef;
  queryEmbedding?: ReadonlyArray<number>;
  topK?: number;
}>;

const MEMORY_BLOCK_HEADER = "[recalled owner-local experience — advisory only]";

/**
 * The empty recall result. It is the default: with memory OFF, or an empty or
 * corrupt bank, no store is read and the memory block is empty, so a host prompt
 * is unchanged.
 */
export const emptyRecall = (bank: ExperienceBank): RecallResult => {
  const effectiveAdaptationDigest = decodeDigest(
    sha256Hex(canonicalStringify({ bankDigest: bank.bankDigest, included: [], patterns: [], request: null })),
  );
  return decodeRecallResult({
    schema: "openagents.experience_recall.v1",
    enabled: false,
    bankDigest: bank.bankDigest,
    effectiveAdaptationDigest,
    includedRecordRefs: [],
    droppedRecordRefs: [],
    includedPatternRefs: [],
    usedTokens: 0,
    memoryBlock: "",
  });
};

const rankRecords = (
  bank: ExperienceBank,
  request: RecallRequest,
): ReadonlyArray<ExperienceRecord> => {
  const scoped = request.repoRef
    ? bank.records.filter((record) => record.repoRef === request.repoRef)
    : bank.records;
  const query = request.queryEmbedding;
  if (query && query.length > 0) {
    const embeddable = scoped.filter(
      (record) => Array.isArray(record.embedding) && record.embedding.length === query.length,
    );
    const ranked = topK(
      query,
      embeddable.map((record) => ({ ref: record.recordRef, embedding: record.embedding ?? [] })),
      request.topK ?? embeddable.length,
    );
    const byRef = new Map(scoped.map((record) => [record.recordRef, record]));
    const rankedRecords = ranked.flatMap((item) => {
      const found = byRef.get(item.ref);
      return found ? [found] : [];
    });
    // Records without a usable embedding fall back to confidence order after the
    // ranked ones, so recall never silently drops a fact for lacking an embedding.
    const rankedRefs = new Set(rankedRecords.map((record) => record.recordRef));
    const rest = scoped
      .filter((record) => !rankedRefs.has(record.recordRef))
      .slice()
      .sort((left, right) => right.confidence - left.confidence);
    return [...rankedRecords, ...rest];
  }
  // No query embedding: confidence first, then recency, mirroring TAS repo-memory.
  return scoped
    .slice()
    .sort((left, right) => {
      if (right.confidence !== left.confidence) return right.confidence - left.confidence;
      return right.observedAt.localeCompare(left.observedAt);
    });
};

/**
 * Adapt one frozen bank into a bounded, redacted recall slice. PURE and one-shot.
 *
 * This never touches the store, so it can only read the frozen snapshot: a write
 * that lands after the freeze cannot change this turn's input. It performs at
 * most one adaptation and binds the exact result with an
 * `effectiveAdaptationDigest` over the bank digest, the request, and the chosen
 * refs. Every included fact is redacted again and proven clean before it enters
 * the block.
 */
export const recallFromBank = (
  bank: ExperienceBank,
  config: MemoryConfigShape,
  request: RecallRequest,
): RecallResult => {
  if (!config.enabled || config.maxRecallTokens === 0 || config.maxRecords === 0) {
    return { ...emptyRecall(bank), enabled: config.enabled };
  }
  const ranked = rankRecords(bank, request).slice(0, config.maxRecords);
  const packed = packWithinBudget(
    ranked.map((record) => ({
      ref: record.recordRef,
      priority: Math.round(record.confidence * 1000),
      tokens: estimateTokens(record.text),
    })),
    config.maxRecallTokens,
  );
  const includedSet = new Set(packed.included);
  const includedRecords = ranked.filter((record) => includedSet.has(record.recordRef));
  const lines = includedRecords.map((record) => {
    const safe = assertRecallClean(guardMemoryText(record.text).redacted);
    return `- ${record.kind}: ${safe}`;
  });
  const patternLines = bank.patterns.map((pattern: GlobalPattern) => {
    const safe = assertRecallClean(guardMemoryText(pattern.phenomenon).redacted);
    return `- pattern: ${safe}`;
  });
  const body = [...lines, ...patternLines];
  const memoryBlock = body.length === 0 ? "" : [MEMORY_BLOCK_HEADER, ...body].join("\n");
  const includedRecordRefs = includedRecords.map((record) => record.recordRef);
  const includedPatternRefs = bank.patterns.map((pattern) => pattern.patternRef);
  const effectiveAdaptationDigest = decodeDigest(
    sha256Hex(
      canonicalStringify({
        bankDigest: bank.bankDigest,
        included: includedRecordRefs,
        patterns: includedPatternRefs,
        request: {
          repoRef: request.repoRef ?? null,
          hasEmbedding: Boolean(request.queryEmbedding && request.queryEmbedding.length > 0),
        },
      }),
    ),
  );
  return decodeRecallResult({
    schema: "openagents.experience_recall.v1",
    enabled: true,
    bankDigest: bank.bankDigest,
    effectiveAdaptationDigest,
    includedRecordRefs,
    droppedRecordRefs: packed.dropped,
    includedPatternRefs,
    usedTokens: packed.usedTokens,
    memoryBlock: assertRecallClean(memoryBlock),
  });
};

/**
 * Freeze one eligible bank for a scope, then perform a single pre-turn recall.
 *
 * With memory OFF this returns an empty recall WITHOUT reading the store, so no
 * read is served and behavior is byte-identical. With memory ON it freezes the
 * bank once (owner-scoped, project-scoped, consent-filtered by the store), then
 * adapts that frozen snapshot exactly once.
 */
export const recallForTurn = (
  scope: MemoryScope,
  config: MemoryConfigShape,
  request: RecallRequest,
  frozenAt: string,
): Effect.Effect<RecallResult, never, MemoryStore> =>
  Effect.gen(function* () {
    if (!config.enabled) {
      return emptyRecall(emptyExperienceBank({ bankId: bankIdFor(scope, frozenAt), ownerScope: scope.owner, projectScope: scope.project, frozenAt }));
    }
    const store = yield* MemoryStore;
    const bank = yield* store.snapshot(scope, bankIdFor(scope, frozenAt), frozenAt).pipe(
      // A corrupt or unavailable bank fails closed to no-memory.
      Effect.catch(() =>
        Effect.succeed(
          emptyExperienceBank({
            bankId: bankIdFor(scope, frozenAt),
            ownerScope: scope.owner,
            projectScope: scope.project,
            frozenAt,
          }),
        ),
      ),
    );
    return recallFromBank(bank, config, request);
  });

/**
 * Apply a recalled slice to a base prompt. When the memory block is empty the
 * base prompt is returned UNCHANGED (byte-identical), so memory-off and
 * empty-bank paths never alter host behavior.
 */
export const applyRecalledMemory = (basePrompt: string, recall: RecallResult): string =>
  recall.memoryBlock.length === 0 ? basePrompt : `${recall.memoryBlock}\n\n${basePrompt}`;

/** Owner lifecycle: inspect every record the owner holds in a scope. */
export const inspect = (
  scope: MemoryScope,
): Effect.Effect<ReadonlyArray<ExperienceRecord>, never, MemoryStore> =>
  Effect.gen(function* () {
    const store = yield* MemoryStore;
    return yield* store.inspect(scope).pipe(Effect.catch(() => Effect.succeed([])));
  });

/** Owner lifecycle: export everything in a scope for portability. */
export const exportScope = (
  scope: MemoryScope,
): Effect.Effect<
  Readonly<{ records: ReadonlyArray<ExperienceRecord>; patterns: ReadonlyArray<GlobalPattern> }>,
  never,
  MemoryStore
> =>
  Effect.gen(function* () {
    const store = yield* MemoryStore;
    return yield* store
      .exportScope(scope)
      .pipe(Effect.catch(() => Effect.succeed({ records: [], patterns: [] })));
  });

/** Owner lifecycle: delete everything in a scope. Returns the count removed. */
export const forget = (scope: MemoryScope): Effect.Effect<number, never, MemoryStore> =>
  Effect.gen(function* () {
    const store = yield* MemoryStore;
    return yield* store.forget(scope).pipe(Effect.catch(() => Effect.succeed(0)));
  });

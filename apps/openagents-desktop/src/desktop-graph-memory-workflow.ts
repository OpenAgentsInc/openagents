import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  graphMemoryScopeRefFor,
  guardMemoryText,
  ownerScopeId,
  projectScopeId,
  type GraphMemoryScope,
} from "@openagentsinc/agent-experience-memory";
import type {
  DeterministicGraphExtractor,
  GraphExtractionCandidates,
  GraphExtractionLimits,
} from "@openagentsinc/dse";
import { buildInlineCorpusInput, makeInlineCorpusHandle } from "@openagentsinc/rlm";
import { Effect } from "effect";

import type { DesktopGraphMemoryPreferences } from "./desktop-preferences-contract.js";
import type { DesktopGraphMemoryStore } from "./desktop-graph-memory-store.js";
import {
  runDesktopGraphMemoryTurn,
  type DesktopGraphMemoryTurnEvidence,
} from "./desktop-graph-memory-turn.js";
import type { ProviderLaneHistoryMessage } from "./provider-lane.js";

const MAX_HISTORY_ENTRIES = 32;
const MAX_ENTRY_CHARACTERS = 2_048;
const MAX_HISTORY_CHARACTERS = 32_768;
const MAX_EVIDENCE_RECORDS = 2_048;
const EVIDENCE_STORE_SCHEMA = "openagents.desktop.graph_memory_evidence_store.v1";

export const DESKTOP_GRAPH_MEMORY_EXTRACTION_LIMITS: GraphExtractionLimits = {
  maxEntries: MAX_HISTORY_ENTRIES,
  maxCharacters: MAX_HISTORY_CHARACTERS,
  maxInputTokens: MAX_HISTORY_CHARACTERS,
  maxOutputTokens: MAX_HISTORY_CHARACTERS * 4,
  maxOutputCharacters: MAX_HISTORY_CHARACTERS * 4,
  maxModelCalls: 1,
  maxWallClockMs: 5_000,
  maxConcurrency: 1,
  maxEntriesPerBatch: MAX_HISTORY_ENTRIES,
  maxCharactersPerBatch: MAX_HISTORY_CHARACTERS,
  maxInputTokensPerBatch: MAX_HISTORY_CHARACTERS,
};

export const DESKTOP_GRAPH_MEMORY_RECALL_LIMITS = {
  maxDepth: 1,
  maxVisitedElements: 64,
  maxReturnedElements: 8,
  maxSourceAddresses: 16,
  maxCharactersPerResult: 4_096,
  maxObservationCharacters: 2_048,
} as const;

const sha256 = (value: string): string =>
  createHash("sha256").update(value, "utf8").digest("hex");

export const desktopGraphMemoryRecallQueryFor = (message: string): string => {
  const stopWords = new Set(["about", "active", "does", "from", "have", "that", "the", "this", "what", "when", "where", "which", "with"]);
  return message
    .normalize("NFC")
    .toLocaleLowerCase("en-US")
    .match(/[\p{L}\p{N}._-]+/gu)
    ?.find((word) => word.length >= 4 && !stopWords.has(word)) ?? message;
};

export const desktopGraphMemoryDeterministicExtractor: DeterministicGraphExtractor = {
  parserRef: "parser.desktop.foreground-history",
  parserVersion: "version.1",
  extract: ({ entries }) => {
    const candidates: GraphExtractionCandidates = {
      mentions: entries.map((entry, index) => ({
        candidateKey: `mention.${index}.${sha256(entry.entryKey).slice(0, 16)}`,
        identityNamespace: "desktop-foreground-history",
        canonicalKey: entry.text.slice(0, MAX_ENTRY_CHARACTERS),
        supportEntryKey: entry.entryKey,
        confidence: 1,
      })),
      entities: [],
      relations: [],
      merges: [],
    };
    return Effect.succeed(candidates);
  },
};

export interface DesktopGraphMemoryWorkflowDependencies {
  readonly preferences: () => DesktopGraphMemoryPreferences;
  readonly ownerScope: () => string;
  readonly projectScope: () => string;
  readonly openStore: () => Promise<DesktopGraphMemoryStore>;
  readonly emitEvidence: (evidence: DesktopGraphMemoryTurnEvidence) => Promise<void>;
  readonly now?: () => Date;
}

export interface DesktopGraphMemoryWorkflowInput {
  readonly turnRef: string;
  readonly threadRef: string;
  readonly history: ReadonlyArray<ProviderLaneHistoryMessage>;
  readonly message: string;
}

export interface DesktopGraphMemoryEvidenceStore {
  readonly record: (evidence: DesktopGraphMemoryTurnEvidence) => void;
  readonly list: () => ReadonlyArray<DesktopGraphMemoryTurnEvidence>;
}

/** Open a bounded, owner-local evidence ledger. It stores no recalled text. */
export const openDesktopGraphMemoryEvidenceStore = (
  filePath: string,
): DesktopGraphMemoryEvidenceStore => {
  let records: DesktopGraphMemoryTurnEvidence[] = [];
  try {
    const decoded = JSON.parse(readFileSync(filePath, "utf8")) as unknown;
    if (
      typeof decoded === "object" &&
      decoded !== null &&
      "schema" in decoded &&
      decoded.schema === EVIDENCE_STORE_SCHEMA &&
      "records" in decoded &&
      Array.isArray(decoded.records)
    ) {
      records = decoded.records.slice(-MAX_EVIDENCE_RECORDS) as DesktopGraphMemoryTurnEvidence[];
    }
  } catch {
    records = [];
  }
  const persist = (): void => {
    const parent = path.dirname(filePath);
    mkdirSync(parent, { recursive: true, mode: 0o700 });
    if (process.platform !== "win32") chmodSync(parent, 0o700);
    const temporary = `${filePath}.pending`;
    writeFileSync(
      temporary,
      `${JSON.stringify({ schema: EVIDENCE_STORE_SCHEMA, records })}\n`,
      { encoding: "utf8", mode: 0o600 },
    );
    if (process.platform !== "win32") chmodSync(temporary, 0o600);
    renameSync(temporary, filePath);
    if (process.platform !== "win32") chmodSync(filePath, 0o600);
  };
  return {
    record: (evidence) => {
      records = [...records, evidence].slice(-MAX_EVIDENCE_RECORDS);
      persist();
    },
    list: () => [...records],
  };
};

const scopeFor = (dependencies: DesktopGraphMemoryWorkflowDependencies): GraphMemoryScope => ({
  owner: ownerScopeId(`owner.${sha256(dependencies.ownerScope())}`),
  project: projectScopeId(`project.${sha256(dependencies.projectScope())}`),
});

const authorizedHistory = (
  scope: GraphMemoryScope,
  threadRef: string,
  history: ReadonlyArray<ProviderLaneHistoryMessage>,
) => {
  const threadDigest = sha256(threadRef);
  let characters = 0;
  return history
    .filter((entry) => entry.role === "user" || entry.role === "assistant")
    .slice(-MAX_HISTORY_ENTRIES)
    .flatMap((entry, index) => {
      const guarded = guardMemoryText(entry.text.slice(0, MAX_ENTRY_CHARACTERS));
      if (!guarded.storable || guarded.redacted.trim() === "") return [];
      if (characters + guarded.redacted.length > MAX_HISTORY_CHARACTERS) return [];
      characters += guarded.redacted.length;
      const entryDigest = sha256(`${threadDigest}:${index}:${guarded.redacted}`);
      return [{
        corpusRef: `corpus.desktop-history.${entryDigest}`,
        scopeRef: graphMemoryScopeRefFor(scope),
        entryRef: `entry.${entryDigest}`,
        sourceKind: "desktop-foreground-history",
        sourceAddress: {
          addressSchemaId: "openagents.desktop.history_address.v1",
          encodedAddress: `memory://thread/${threadDigest}/entry/${index}`,
        },
        text: guarded.redacted,
        visibility: "private" as const,
        // The entry bytes are the output of the redaction boundary. They are
        // clean corpus text, even when the source needed a soft redaction.
        redactionClass: "none" as const,
      }];
    });
};

/** Compose the released graph SDK into the direct Desktop foreground path. */
export const makeDesktopGraphMemoryWorkflow = (
  dependencies: DesktopGraphMemoryWorkflowDependencies,
) => ({
  beforeTurn: async (input: DesktopGraphMemoryWorkflowInput): Promise<{ message: string }> => {
    const preferences = dependencies.preferences();
    if (!preferences.graphExtractionEnabled && !preferences.graphRecallEnabled) {
      return { message: input.message };
    }
    const scope = scopeFor(dependencies);
    const entries = authorizedHistory(scope, input.threadRef, input.history);
    if (entries.length === 0) return { message: input.message };
    const leaves = await Promise.all(
      entries.map((entry) =>
        Effect.runPromise(
          makeInlineCorpusHandle(
            buildInlineCorpusInput({
              corpusRef: entry.corpusRef,
              scopeRef: entry.scopeRef,
              policy: {
                includeVisibilities: ["private"],
                includeRedactionClasses: ["none"],
              },
              entries: [{
                entryRef: entry.entryRef,
                scopeRef: entry.scopeRef,
                sourcePlane: "evidence_pack",
                sourceKind: entry.sourceKind,
                sourceAddress: entry.sourceAddress,
                text: entry.text,
                visibility: entry.visibility,
                redactionClass: entry.redactionClass,
              }],
            }),
          ),
        ),
      ),
    );
    const store = await dependencies.openStore();
    const result = await Effect.runPromise(
      runDesktopGraphMemoryTurn(
        {
          turnRef: input.turnRef,
          mode: "foreground",
          prompt: input.message,
          recallQuery: desktopGraphMemoryRecallQueryFor(input.message),
          scope,
          extractionEnabled: preferences.graphExtractionEnabled,
          recallEnabled: preferences.graphRecallEnabled,
          admission: {
            consent: "granted",
            consentRef: "consent.desktop.graph-memory.preferences.v1",
            policyRef: "policy.desktop.graph-memory.foreground.v1",
            redactionState: "already_redacted",
          },
          policy: {
            includeVisibilities: ["private"],
            includeRedactionClasses: ["none"],
          },
        },
        {
          resolveSources: () =>
            Effect.succeed(leaves.map((handle) => ({ handle, redactionState: "already_redacted" }))),
          extraction: { _tag: "Deterministic", extractor: desktopGraphMemoryDeterministicExtractor },
          extractionLimits: DESKTOP_GRAPH_MEMORY_EXTRACTION_LIMITS,
          recallLimits: DESKTOP_GRAPH_MEMORY_RECALL_LIMITS,
          countTokens: (text) => text.length,
          monotonicMs: () => Math.floor(performance.now()),
          now: () => (dependencies.now ?? (() => new Date()))().toISOString(),
          emitEvidence: (evidence) => Effect.promise(() => dependencies.emitEvidence(evidence)),
        },
      ).pipe(Effect.provide(store.layer)),
    );
    const after = scopeFor(dependencies);
    if (after.owner !== scope.owner || after.project !== scope.project) {
      return { message: input.message };
    }
    return { message: result.prompt };
  },
});

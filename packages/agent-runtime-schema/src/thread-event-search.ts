import { Schema as S } from "effect";

import { ThreadExportArtifact, type ThreadExportArtifact as ThreadExportArtifactType } from "./thread-export-artifact.js";

export const ThreadEventSearchProjectionSchemaLiteral =
  "openagents.thread_event_search_projection.v1" as const;

const MAX_ARTIFACTS = 1_000;
const MAX_INDEXED_EVENTS = 10_000;
const MAX_STRING_LEAVES_PER_EVENT = 256;
const MAX_INDEXED_TEXT_CHARS_PER_EVENT = 20_000;
const MAX_QUERY_CHARS = 200;
const MAX_RESULTS = 100;
const MAX_SNIPPET_CHARS = 240;

type ExportedEvent = ThreadExportArtifactType["events"][number];

export type ThreadEventSearchResult = Readonly<{
  threadRef: string;
  eventRef: string;
  sequence: number;
  authority: ExportedEvent["authority"];
  snippet: string;
  score: 1 | 2 | 3;
}>;

export type ThreadEventSearchProjection = Readonly<{
  schema: typeof ThreadEventSearchProjectionSchemaLiteral;
  query: string;
  results: ReadonlyArray<ThreadEventSearchResult>;
  indexedEvents: number;
  totalMatches: number;
  indexTruncated: boolean;
  resultsTruncated: boolean;
}>;

export type SearchCanonicalThreadEventsInput = Readonly<{
  artifacts: ReadonlyArray<unknown>;
  query: unknown;
  limit?: number;
}>;

const decodeArtifact = S.decodeUnknownSync(ThreadExportArtifact);
const compareStrings = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;
const normalize = (value: string): string => value.toLowerCase().replace(/\s+/gu, " ").trim();

const validateAuthority = (event: ExportedEvent): void => {
  const { authority } = event;
  if (
    authority.relationRefs.length === 0 ||
    new Set(authority.relationRefs).size !== authority.relationRefs.length
  ) {
    throw new Error("Thread event search requires unique non-empty authority relations");
  }
  if (
    authority.state === "superseded" &&
    authority.supersededByEventRef === event.eventRef
  ) {
    throw new Error("Thread event search rejects self-supersession");
  }
  if (
    authority.state === "reverted" &&
    (authority.revertedByEventRef === event.eventRef ||
      authority.restoredEventRef === event.eventRef ||
      authority.revertedByEventRef === authority.restoredEventRef)
  ) {
    throw new Error("Thread event search rejects invalid revert authority");
  }
};

const validateArtifact = (artifact: ThreadExportArtifactType): void => {
  const eventRefs = new Set<string>();
  const sequences = new Set<number>();
  for (const event of artifact.events) {
    if (eventRefs.has(event.eventRef) || sequences.has(event.sequence)) {
      throw new Error("Thread event search rejects duplicate event identity");
    }
    eventRefs.add(event.eventRef);
    sequences.add(event.sequence);
    validateAuthority(event);
  }
};

type StringProjection = Readonly<{ values: ReadonlyArray<string>; truncated: boolean }>;

const stringLeaves = (data: S.Json): StringProjection => {
  const values: string[] = [];
  let characters = 0;
  let truncated = false;

  const visit = (value: S.Json): void => {
    if (truncated) return;
    if (typeof value === "string") {
      if (
        values.length >= MAX_STRING_LEAVES_PER_EVENT ||
        characters + value.length > MAX_INDEXED_TEXT_CHARS_PER_EVENT
      ) {
        truncated = true;
        return;
      }
      values.push(value);
      characters += value.length;
      return;
    }
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
      return;
    }
    if (typeof value === "object" && value !== null) {
      const object = value as S.JsonObject;
      for (const key of Object.keys(object).sort(compareStrings)) visit(object[key]!);
    }
  };

  visit(data);
  return { values, truncated };
};

const matchingSnippet = (
  values: ReadonlyArray<string>,
  needle: string,
): Readonly<{ snippet: string; score: 1 | 2 | 3 }> | null => {
  for (const value of values) {
    const compact = value.replace(/\s+/gu, " ").trim();
    const normalized = compact.toLowerCase();
    const at = normalized.indexOf(needle);
    if (at < 0) continue;
    const start = Math.max(0, at - 80);
    const end = Math.min(compact.length, at + needle.length + 80);
    const snippet = `${start > 0 ? "…" : ""}${compact.slice(start, end)}${
      end < compact.length ? "…" : ""
    }`.slice(0, MAX_SNIPPET_CHARS);
    return {
      snippet,
      score: normalized === needle ? 3 : at === 0 ? 2 : 1,
    };
  }
  return null;
};

/**
 * Search a rebuildable owner-local projection of canonical accepted events.
 * This is bounded deterministic text filtering after the search route has
 * already been selected; it is not intent routing and grants no authority.
 */
export const searchCanonicalThreadEvents = (
  input: SearchCanonicalThreadEventsInput,
): ThreadEventSearchProjection => {
  if (typeof input.query !== "string" || input.query.length > MAX_QUERY_CHARS) {
    throw new Error(`Thread event search query must be at most ${MAX_QUERY_CHARS} characters`);
  }
  const query = input.query.replace(/\s+/gu, " ").trim();
  const needle = normalize(query);
  const limit = input.limit ?? 40;
  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_RESULTS) {
    throw new Error(`Thread event search limit must be an integer from 1 to ${MAX_RESULTS}`);
  }
  if (input.artifacts.length > MAX_ARTIFACTS) {
    throw new Error(`Thread event search exceeds ${MAX_ARTIFACTS} artifacts`);
  }
  if (needle === "") {
    return {
      schema: ThreadEventSearchProjectionSchemaLiteral,
      query,
      results: [],
      indexedEvents: 0,
      totalMatches: 0,
      indexTruncated: false,
      resultsTruncated: false,
    };
  }

  const artifacts = input.artifacts.map((artifact) => decodeArtifact(artifact));
  const threadRefs = new Set<string>();
  for (const artifact of artifacts) {
    if (threadRefs.has(artifact.threadRef)) {
      throw new Error("Thread event search rejects duplicate thread artifacts");
    }
    threadRefs.add(artifact.threadRef);
    validateArtifact(artifact);
  }
  artifacts.sort((left, right) => compareStrings(left.threadRef, right.threadRef));

  const matches: ThreadEventSearchResult[] = [];
  let indexedEvents = 0;
  let indexTruncated = false;
  outer: for (const artifact of artifacts) {
    const events = [...artifact.events].sort(
      (left, right) =>
        left.sequence - right.sequence || compareStrings(left.eventRef, right.eventRef),
    );
    for (const event of events) {
      if (indexedEvents >= MAX_INDEXED_EVENTS) {
        indexTruncated = true;
        break outer;
      }
      indexedEvents += 1;
      const projection = stringLeaves(event.data);
      indexTruncated ||= projection.truncated;
      const match = matchingSnippet(projection.values, needle);
      if (match === null) continue;
      matches.push({
        threadRef: artifact.threadRef,
        eventRef: event.eventRef,
        sequence: event.sequence,
        authority: event.authority,
        ...match,
      });
    }
  }
  matches.sort(
    (left, right) =>
      right.score - left.score ||
      compareStrings(left.threadRef, right.threadRef) ||
      left.sequence - right.sequence ||
      compareStrings(left.eventRef, right.eventRef),
  );

  return {
    schema: ThreadEventSearchProjectionSchemaLiteral,
    query,
    results: matches.slice(0, limit),
    indexedEvents,
    totalMatches: matches.length,
    indexTruncated,
    resultsTruncated: matches.length > limit,
  };
};

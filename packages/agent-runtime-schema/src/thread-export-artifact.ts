import { Schema as S } from "effect";

import { decodeThreadDisclosureIntent, type ThreadDisclosureIntent } from "./thread-disclosure.js";
import {
  projectThreadEventAuthority,
  type ThreadEventAuthorityProjection,
} from "./thread-event-authority.js";

export const ThreadExportArtifactSchemaLiteral = "openagents.thread_export_artifact.v1" as const;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Sequence = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

export const CanonicalThreadExportEvent = S.Struct({
  eventRef: Ref,
  threadRef: Ref,
  sequence: Sequence,
  data: S.Json,
});
export type CanonicalThreadExportEvent = typeof CanonicalThreadExportEvent.Type;

const ExportedAuthority = S.Union([
  S.Struct({
    state: S.Literal("accepted"),
    relationRefs: S.Array(Ref),
  }),
  S.Struct({
    state: S.Literal("superseded"),
    relationRefs: S.Array(Ref),
    supersededByEventRef: Ref,
  }),
  S.Struct({
    state: S.Literal("reverted"),
    relationRefs: S.Array(Ref),
    revertedByEventRef: Ref,
    restoredEventRef: Ref,
  }),
]);

export const ThreadExportArtifact = S.Struct({
  schema: S.Literal(ThreadExportArtifactSchemaLiteral),
  intentRef: Ref,
  threadRef: Ref,
  format: S.Literal("canonical_event_bundle"),
  artifactAudience: S.Struct({ kind: S.Literal("owner_only") }),
  events: S.Array(
    S.Struct({
      eventRef: Ref,
      sequence: Sequence,
      authority: ExportedAuthority,
      data: S.Json,
    }),
  ),
});
export type ThreadExportArtifact = typeof ThreadExportArtifact.Type;

export type ThreadExportArtifactCompilation = Readonly<{
  artifact: ThreadExportArtifact;
  encoded: string;
  bytes: Uint8Array;
  artifactSha256: string;
}>;

export type CompileThreadExportArtifactInput = Readonly<{
  intent: unknown;
  events: ReadonlyArray<unknown>;
  relations: ReadonlyArray<unknown>;
  sha256: (bytes: Uint8Array) => string;
}>;

const MAX_EVENTS = 1_000;
const MAX_ARTIFACT_BYTES = 4 * 1024 * 1024;
const SHA256 = /^[a-f0-9]{64}$/;
const decodeEvent = S.decodeUnknownSync(CanonicalThreadExportEvent);
const decodeArtifact = S.decodeUnknownSync(ThreadExportArtifact);

const requireExportIntent = (
  raw: unknown,
): Extract<ThreadDisclosureIntent, { kind: "thread.export.create" }> => {
  const intent = decodeThreadDisclosureIntent(raw);
  if (intent.kind !== "thread.export.create") {
    throw new Error("Thread export artifact requires a thread.export.create intent");
  }
  if (intent.format !== "canonical_event_bundle") {
    throw new Error("Thread export artifact supports only canonical_event_bundle");
  }
  if (intent.artifactAudience.kind !== "owner_only") {
    throw new Error("Thread export artifact requires owner_only audience authority");
  }
  return intent;
};

const resolvedAuthority = (
  projection: ThreadEventAuthorityProjection,
): ThreadExportArtifact["events"][number]["authority"] => {
  if (projection.status !== "resolved") {
    throw new Error(
      `Thread export event authority is ${projection.status}${
        projection.status === "conflict" ? `: ${projection.reason}` : ""
      }`,
    );
  }
  if (projection.state === "accepted") {
    return { state: "accepted", relationRefs: projection.relationRefs };
  }
  if (projection.state === "superseded") {
    if (projection.supersededByEventRef === undefined) {
      throw new Error("Resolved superseded authority is incomplete");
    }
    return {
      state: "superseded",
      relationRefs: projection.relationRefs,
      supersededByEventRef: projection.supersededByEventRef,
    };
  }
  if (projection.revertedByEventRef === undefined || projection.restoredEventRef === undefined) {
    throw new Error("Resolved reverted authority is incomplete");
  }
  return {
    state: "reverted",
    relationRefs: projection.relationRefs,
    revertedByEventRef: projection.revertedByEventRef,
    restoredEventRef: projection.restoredEventRef,
  };
};

const isJsonArray = (value: S.Json): value is S.JsonArray => Array.isArray(value);
const compareStrings = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const canonicalize = (value: S.Json): S.Json => {
  if (isJsonArray(value)) return value.map(canonicalize);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => compareStrings(left, right))
      .map(([key, child]) => [key, canonicalize(child)]),
  );
};

/**
 * Compile exact owner-local event data into deterministic bytes. This function
 * does not persist, transport, disclose, or accept the artifact.
 */
export const compileThreadExportArtifact = (
  input: CompileThreadExportArtifactInput,
): ThreadExportArtifactCompilation => {
  const intent = requireExportIntent(input.intent);
  if (input.events.length > MAX_EVENTS) {
    throw new Error(`Thread export artifact exceeds ${MAX_EVENTS} events`);
  }

  const seenEventRefs = new Set<string>();
  const seenSequences = new Set<number>();
  const events = input.events.map((raw) => {
    const event = decodeEvent(raw);
    if (event.threadRef !== intent.threadRef) {
      throw new Error("Thread export event does not match the intent thread");
    }
    if (seenEventRefs.has(event.eventRef)) {
      throw new Error("Thread export event ref is duplicated");
    }
    if (seenSequences.has(event.sequence)) {
      throw new Error("Thread export event sequence is duplicated");
    }
    seenEventRefs.add(event.eventRef);
    seenSequences.add(event.sequence);
    return {
      eventRef: event.eventRef,
      sequence: event.sequence,
      authority: resolvedAuthority(
        projectThreadEventAuthority({
          threadRef: intent.threadRef,
          eventRef: event.eventRef,
          relations: input.relations,
        }),
      ),
      data: canonicalize(event.data),
    };
  });
  events.sort(
    (left, right) =>
      left.sequence - right.sequence || compareStrings(left.eventRef, right.eventRef),
  );

  const artifact = decodeArtifact({
    schema: ThreadExportArtifactSchemaLiteral,
    intentRef: intent.intentRef,
    threadRef: intent.threadRef,
    format: "canonical_event_bundle",
    artifactAudience: { kind: "owner_only" },
    events,
  });
  const encoded = JSON.stringify(canonicalize(artifact));
  const bytes = new TextEncoder().encode(encoded);
  if (bytes.byteLength > MAX_ARTIFACT_BYTES) {
    throw new Error(`Thread export artifact exceeds ${MAX_ARTIFACT_BYTES} bytes`);
  }
  const artifactSha256 = input.sha256(bytes);
  if (!SHA256.test(artifactSha256)) {
    throw new Error("Thread export artifact SHA-256 is invalid");
  }
  return { artifact, encoded, bytes, artifactSha256 };
};

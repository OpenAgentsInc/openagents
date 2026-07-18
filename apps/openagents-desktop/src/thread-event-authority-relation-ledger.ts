import { randomUUID } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  decodeThreadEventAuthorityRelation,
  type ThreadEventAuthorityRelation,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

const LEDGER_SCHEMA = "openagents.desktop_thread_event_authority_relation_ledger.v1";
const MAX_RELATIONS = 1_000;
const MAX_LEDGER_BYTES = 1024 * 1024;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const decodeRef = S.decodeUnknownSync(Ref);
const LedgerEnvelope = S.Struct({
  schema: S.Literal(LEDGER_SCHEMA),
  relations: S.Array(S.Unknown),
});
const decodeLedgerEnvelope = S.decodeUnknownSync(LedgerEnvelope);

type TerminalRelation = Extract<
  ThreadEventAuthorityRelation,
  Readonly<{ kind: "superseded" | "reverted" }>
>;

export type DesktopThreadEventAuthorityRelationLedgerRecordResult =
  | Readonly<{ status: "stored" | "unchanged"; relationCount: number }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_relation"
        | "conflicting_identity"
        | "capacity_exceeded"
        | "corrupt_ledger"
        | "persistence_failed";
    }>;

export type DesktopThreadEventAuthorityRelationLedgerListResult =
  | Readonly<{ status: "available"; relations: ReadonlyArray<TerminalRelation> }>
  | Readonly<{ status: "rejected"; reason: "invalid_request" | "corrupt_ledger" }>;

const ownKeysAre = (value: unknown, expected: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  return (
    actual.length === sortedExpected.length &&
    actual.every((key, index) => key === sortedExpected[index])
  );
};

const canonicalTerminalRelation = (raw: unknown): TerminalRelation | null => {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return null;
  const kind = Reflect.get(raw, "kind");
  const exact =
    kind === "superseded"
      ? ownKeysAre(raw, [
          "schema",
          "relationRef",
          "threadRef",
          "eventRef",
          "observedAt",
          "kind",
          "supersededByEventRef",
        ])
      : kind === "reverted"
        ? ownKeysAre(raw, [
            "schema",
            "relationRef",
            "threadRef",
            "eventRef",
            "observedAt",
            "kind",
            "revertedByEventRef",
            "restoredEventRef",
          ])
        : false;
  if (!exact) return null;
  try {
    const relation = decodeThreadEventAuthorityRelation(raw);
    return relation.kind === "accepted" ? null : relation;
  } catch {
    return null;
  }
};

const sameRelation = (left: TerminalRelation, right: TerminalRelation): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

const sameEventIdentity = (left: TerminalRelation, right: TerminalRelation): boolean =>
  left.threadRef === right.threadRef && left.eventRef === right.eventRef;

const compareRelations = (left: TerminalRelation, right: TerminalRelation): number =>
  left.threadRef.localeCompare(right.threadRef) ||
  left.eventRef.localeCompare(right.eventRef) ||
  left.observedAt.localeCompare(right.observedAt) ||
  left.relationRef.localeCompare(right.relationRef);

type LedgerLoad =
  | Readonly<{ status: "available"; relations: ReadonlyArray<TerminalRelation> }>
  | Readonly<{ status: "missing" }>
  | Readonly<{ status: "corrupt" }>;

const loadLedger = (file: string): LedgerLoad => {
  let bytes: Uint8Array;
  try {
    bytes = readFileSync(file);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return { status: "missing" };
    }
    return { status: "corrupt" };
  }
  if (bytes.byteLength > MAX_LEDGER_BYTES) return { status: "corrupt" };

  try {
    const parsed: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
    if (!ownKeysAre(parsed, ["schema", "relations"])) return { status: "corrupt" };
    const envelope = decodeLedgerEnvelope(parsed);
    if (envelope.relations.length > MAX_RELATIONS) return { status: "corrupt" };

    const relations: TerminalRelation[] = [];
    for (const raw of envelope.relations) {
      const relation = canonicalTerminalRelation(raw);
      if (
        relation === null ||
        relations.some(
          (existing) =>
            existing.relationRef === relation.relationRef || sameEventIdentity(existing, relation),
        )
      ) {
        return { status: "corrupt" };
      }
      relations.push(relation);
    }
    relations.sort(compareRelations);
    return { status: "available", relations };
  } catch {
    return { status: "corrupt" };
  }
};

const encodeLedger = (relations: ReadonlyArray<TerminalRelation>): Uint8Array =>
  new TextEncoder().encode(JSON.stringify({ schema: LEDGER_SCHEMA, relations }));

/**
 * Owner-private persistence for terminal authority facts observed elsewhere.
 * This ledger validates and retains ref-only evidence; it does not observe,
 * infer, authorize, or create supersession or reversion authority.
 */
export const openDesktopThreadEventAuthorityRelationLedger = (directory: string) => {
  const file = path.join(directory, "terminal-authority-relations.json");

  const listForThread = (
    rawThreadRef: unknown,
  ): DesktopThreadEventAuthorityRelationLedgerListResult => {
    let threadRef: string;
    try {
      threadRef = decodeRef(rawThreadRef);
    } catch {
      return { status: "rejected", reason: "invalid_request" };
    }
    const loaded = loadLedger(file);
    if (loaded.status === "corrupt") return { status: "rejected", reason: "corrupt_ledger" };
    return {
      status: "available",
      relations:
        loaded.status === "missing"
          ? []
          : loaded.relations.filter((relation) => relation.threadRef === threadRef),
    };
  };

  const record = (raw: unknown): DesktopThreadEventAuthorityRelationLedgerRecordResult => {
    const incoming = canonicalTerminalRelation(raw);
    if (incoming === null) return { status: "rejected", reason: "invalid_relation" };

    const loaded = loadLedger(file);
    if (loaded.status === "corrupt") return { status: "rejected", reason: "corrupt_ledger" };
    const relations = loaded.status === "missing" ? [] : [...loaded.relations];
    const existing = relations.find(
      (relation) =>
        relation.relationRef === incoming.relationRef || sameEventIdentity(relation, incoming),
    );
    if (existing !== undefined) {
      return sameRelation(existing, incoming)
        ? { status: "unchanged", relationCount: relations.length }
        : { status: "rejected", reason: "conflicting_identity" };
    }
    if (relations.length >= MAX_RELATIONS) {
      return { status: "rejected", reason: "capacity_exceeded" };
    }

    relations.push(incoming);
    relations.sort(compareRelations);
    const encoded = encodeLedger(relations);
    if (encoded.byteLength > MAX_LEDGER_BYTES) {
      return { status: "rejected", reason: "capacity_exceeded" };
    }

    let temporary: string | undefined;
    try {
      mkdirSync(directory, { recursive: true, mode: 0o700 });
      if (process.platform !== "win32") chmodSync(directory, 0o700);
      temporary = path.join(directory, `.terminal-authority-relations.${randomUUID()}.tmp`);
      writeFileSync(temporary, encoded, { flag: "wx", mode: 0o600 });
      if (process.platform !== "win32") chmodSync(temporary, 0o600);
      renameSync(temporary, file);
      temporary = undefined;
      if (process.platform !== "win32") chmodSync(file, 0o600);
      return { status: "stored", relationCount: relations.length };
    } catch {
      if (temporary !== undefined) rmSync(temporary, { force: true });
      return { status: "rejected", reason: "persistence_failed" };
    }
  };

  return { record, listForThread } as const;
};

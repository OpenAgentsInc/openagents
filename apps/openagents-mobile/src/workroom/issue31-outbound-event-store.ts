import type { Issue31SignedNostrEvent } from "@openagentsinc/sarah/issue31-nostr";
import {
  decodeIssue31PairingRecord,
  type Issue31PairingRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import { verifyEvent } from "nostr-effect/pure";

import type { Issue31OutboundEventStore } from "./issue31-nostr-client.ts";

declare const require: (id: string) => unknown;

interface Issue31SQLiteDatabase {
  readonly execSync: (sql: string) => void;
  readonly runSync: (sql: string, ...params: ReadonlyArray<string | number>) => unknown;
  readonly getAllSync: <Row>(
    sql: string,
    ...params: ReadonlyArray<string | number>
  ) => ReadonlyArray<Row>;
  readonly closeSync: () => void;
}

export interface Issue31LocalPairingRecord {
  readonly canonicalRecordId: string;
  readonly event: Issue31SignedNostrEvent;
  readonly record: Issue31PairingRecord;
}

export interface Issue31LocalPairingRecordStore {
  readonly load: () => ReadonlyArray<Issue31LocalPairingRecord>;
  readonly put: (record: Issue31LocalPairingRecord) => void;
  readonly close: () => void;
}

export const issue31PersistedPairingEventsForRequeue = (
  records: ReadonlyArray<Issue31LocalPairingRecord>,
  input: Readonly<{
    selectedHostPublicKeyHex: string | null;
    devicePublicKeyHex: string;
    admittedHostPublicKeys: ReadonlySet<string>;
  }>,
): ReadonlyArray<Issue31SignedNostrEvent> => {
  if (
    input.selectedHostPublicKeyHex === null ||
    !input.admittedHostPublicKeys.has(input.selectedHostPublicKeyHex)
  ) {
    return [];
  }
  return records.flatMap(({ event, record }) =>
    (record.recordType === "pairing_request" || record.recordType === "pairing_response") &&
    record.hostPublicKeyHex === input.selectedHostPublicKeyHex &&
    record.devicePublicKeyHex === input.devicePublicKeyHex
      ? [event]
      : [],
  );
};

const HEX_64 = /^[0-9a-f]{64}$/;
const HEX_128 = /^[0-9a-f]{128}$/;

const decodeStoredSignedEvent = (value: unknown): Issue31SignedNostrEvent => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Issue 31 persisted event is invalid.");
  }
  const candidate = value as Readonly<Record<string, unknown>>;
  if (
    typeof candidate["id"] !== "string" ||
    !HEX_64.test(candidate["id"]) ||
    typeof candidate["pubkey"] !== "string" ||
    !HEX_64.test(candidate["pubkey"]) ||
    typeof candidate["sig"] !== "string" ||
    !HEX_128.test(candidate["sig"]) ||
    !Number.isSafeInteger(candidate["created_at"]) ||
    typeof candidate["kind"] !== "number" ||
    !Number.isSafeInteger(candidate["kind"]) ||
    !Array.isArray(candidate["tags"]) ||
    candidate["tags"].length > 128 ||
    !candidate["tags"].every(
      (tag) =>
        Array.isArray(tag) &&
        tag.length <= 16 &&
        tag.every((part) => typeof part === "string" && part.length <= 4_096),
    ) ||
    typeof candidate["content"] !== "string" ||
    candidate["content"].length > 524_288
  ) {
    throw new Error("Issue 31 persisted event has an invalid shape.");
  }
  const event = candidate as unknown as Issue31SignedNostrEvent;
  if (!verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })) {
    throw new Error("Issue 31 persisted event signature is invalid.");
  }
  return event;
};

export const openExpoIssue31OutboundEventStore = (
  maximumEvents = 256,
): Issue31OutboundEventStore => {
  if (!Number.isSafeInteger(maximumEvents) || maximumEvents < 1 || maximumEvents > 1_024) {
    throw new Error("Issue 31 persisted outbound queue bound is invalid.");
  }
  const sqlite = require("expo-sqlite") as Readonly<{
    openDatabaseSync: (name: string) => Issue31SQLiteDatabase;
  }>;
  const database = sqlite.openDatabaseSync("openagents-omega-issue31.db");
  database.execSync(`
    PRAGMA journal_mode = WAL;
    CREATE TABLE IF NOT EXISTS issue31_outbound_events (
      sequence INTEGER PRIMARY KEY AUTOINCREMENT,
      event_id TEXT NOT NULL UNIQUE,
      event_json TEXT NOT NULL
    );
  `);
  return {
    load: () =>
      database
        .getAllSync<Readonly<{ event_json: string }>>(
          "SELECT event_json FROM issue31_outbound_events ORDER BY sequence ASC LIMIT ?",
          maximumEvents,
        )
        .map((row) => decodeStoredSignedEvent(JSON.parse(row.event_json) as unknown)),
    put: (event) => {
      const existing = database.getAllSync<Readonly<{ event_json: string }>>(
        "SELECT event_json FROM issue31_outbound_events WHERE event_id = ? LIMIT 1",
        event.id,
      )[0];
      const eventJson = JSON.stringify(event);
      if (existing !== undefined) {
        if (existing.event_json !== eventJson) {
          throw new Error("Issue 31 persisted event identifier conflicts.");
        }
        return;
      }
      const count = database.getAllSync<Readonly<{ count: number }>>(
        "SELECT COUNT(*) AS count FROM issue31_outbound_events",
      )[0]?.count;
      if (count === undefined || count >= maximumEvents) {
        throw new Error("Issue 31 persisted outbound queue is full.");
      }
      database.runSync(
        "INSERT INTO issue31_outbound_events (event_id, event_json) VALUES (?, ?)",
        event.id,
        eventJson,
      );
    },
    delete: (eventId) => {
      database.runSync("DELETE FROM issue31_outbound_events WHERE event_id = ?", eventId);
    },
    close: () => database.closeSync(),
  };
};

export const openExpoIssue31LocalPairingRecordStore = (
  maximumRecords = 32,
): Issue31LocalPairingRecordStore => {
  if (!Number.isSafeInteger(maximumRecords) || maximumRecords < 4 || maximumRecords > 128) {
    throw new Error("Issue 31 local pairing record bound is invalid.");
  }
  const sqlite = require("expo-sqlite") as Readonly<{
    openDatabaseSync: (name: string) => Issue31SQLiteDatabase;
  }>;
  const database = sqlite.openDatabaseSync("openagents-omega-issue31.db");
  database.execSync(`
    PRAGMA journal_mode = WAL;
    CREATE TABLE IF NOT EXISTS issue31_local_pairing_records (
      sequence INTEGER PRIMARY KEY AUTOINCREMENT,
      canonical_record_id TEXT NOT NULL UNIQUE,
      event_json TEXT NOT NULL,
      record_json TEXT NOT NULL
    );
  `);
  return {
    load: () =>
      database
        .getAllSync<
          Readonly<{
            canonical_record_id: string;
            event_json: string;
            record_json: string;
          }>
        >(
          "SELECT canonical_record_id, event_json, record_json FROM issue31_local_pairing_records ORDER BY sequence ASC LIMIT ?",
          maximumRecords,
        )
        .map((row) => {
          if (!HEX_64.test(row.canonical_record_id)) {
            throw new Error("Issue 31 persisted canonical record identifier is invalid.");
          }
          return {
            canonicalRecordId: row.canonical_record_id,
            event: decodeStoredSignedEvent(JSON.parse(row.event_json) as unknown),
            record: decodeIssue31PairingRecord(JSON.parse(row.record_json) as unknown),
          };
        }),
    put: (record) => {
      const eventJson = JSON.stringify(record.event);
      const recordJson = JSON.stringify(record.record);
      const existing = database.getAllSync<Readonly<{ event_json: string; record_json: string }>>(
        "SELECT event_json, record_json FROM issue31_local_pairing_records WHERE canonical_record_id = ? LIMIT 1",
        record.canonicalRecordId,
      )[0];
      if (existing !== undefined) {
        if (existing.event_json !== eventJson || existing.record_json !== recordJson) {
          throw new Error("Issue 31 local pairing record identifier conflicts.");
        }
        return;
      }
      database.runSync(
        "INSERT INTO issue31_local_pairing_records (canonical_record_id, event_json, record_json) VALUES (?, ?, ?)",
        record.canonicalRecordId,
        eventJson,
        recordJson,
      );
      database.runSync(
        "DELETE FROM issue31_local_pairing_records WHERE sequence NOT IN (SELECT sequence FROM issue31_local_pairing_records ORDER BY sequence DESC LIMIT ?)",
        maximumRecords,
      );
    },
    close: () => database.closeSync(),
  };
};

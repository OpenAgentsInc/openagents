/**
 * The owner-private local record store, against a real SQLite database.
 *
 * omega#46 exit 4. Two things must hold for the withheld-source signal to mean
 * anything after a restart or a wipe:
 *
 * - the coverage statement survives a restart, or a phone that has been told
 *   its view is short forgets that the moment it relaunches;
 * - a local wipe removes the coverage statement together with the projections
 *   it describes, or the room asserts "this is everything" over a list the
 *   owner has just emptied.
 *
 * The database is `@openagentsinc/sqlite-runtime` over a real file, not a fake:
 * the store verifies every persisted signature on load, so the records here are
 * really signed and really re-decoded.
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { openNodeSqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_PAIRING_SCHEMA,
  ISSUE31_PRIVATE_GIFT_WRAP_KIND,
  decodeIssue31OwnerProjectionRecord,
  decodeIssue31PairingRecord,
  decodeIssue31WithheldSourcesRecord,
  type Issue31PrivateRecord,
  type Issue31SignedNostrEvent,
} from "@openagentsinc/sarah/issue31-nostr";

import {
  createIssue31LocalConfirmedRecordStore,
  type Issue31SQLiteDatabase,
} from "../src/workroom/issue31-outbound-event-store.ts";

const hostPublicKeyHex = "1".repeat(64);
const devicePublicKeyHex = "2".repeat(64);
const sarahPublicKeyHex = "3".repeat(64);
const grantRef = "grant.omega.device_1";

const openRealDatabase = (path: string): Issue31SQLiteDatabase => {
  const database = openNodeSqliteDatabase(path);
  return {
    execSync: (sql) => database.exec(sql),
    runSync: (sql, ...params) => database.run(sql, [...params]),
    getAllSync: <Row,>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.all<Row>(sql, [...params]),
    closeSync: () => database.close(),
  };
};

const secretKey = generateSecretKey();
const publicKey = getPublicKey(secretKey);

const signedEvent = (content: string): Issue31SignedNostrEvent =>
  finalizeEvent(
    {
      kind: ISSUE31_PRIVATE_GIFT_WRAP_KIND,
      created_at: Math.floor(Date.now() / 1_000),
      tags: [["p", publicKey]],
      content,
    },
    secretKey,
  ) as unknown as Issue31SignedNostrEvent;

const ownerProjection = decodeIssue31OwnerProjectionRecord({
  schema: "openagents.omega.issue31.owner_projection.v1",
  recordType: "owner_projection",
  hostRef: "omega.host.local",
  hostPublicKeyHex,
  devicePublicKeyHex,
  grantRef,
  expectedGeneration: 3,
  sourceEventId: "e".repeat(64),
  sourceAuthorPublicKeyHex: sarahPublicKeyHex,
  sourceRole: "sarah",
  sourceKind: 30_174,
  sourceCreatedAt: 1_784_937_650,
  projectedAt: 1_784_937_651,
  projection: {
    kind: "engram",
    dTag: "f".repeat(64),
    plaintext: JSON.stringify({ slug: "mem/release_evidence", value: "Notarized." }),
  },
});

const coverage = decodeIssue31WithheldSourcesRecord({
  schema: "openagents.omega.issue31.withheld_sources.v1",
  recordType: "withheld_sources",
  hostRef: "omega.host.local",
  hostPublicKeyHex,
  devicePublicKeyHex,
  grantRef,
  expectedGeneration: 3,
  observedAt: 1_784_937_651,
  coverage: "partial",
  withheld: [
    {
      cause: "quarantined",
      count: 2,
      exact: true,
      reasonRef: "reason.omega.invalid_projection_source",
    },
  ],
});

const pairing = decodeIssue31PairingRecord({
  schema: ISSUE31_PAIRING_SCHEMA,
  recordType: "scoped_grant",
  hostRef: "omega.host.local",
  hostPublicKeyHex,
  sarahPublicKeyHex,
  devicePublicKeyHex,
  issuedAt: 950,
  pairingResponseEventId: "c".repeat(64),
  grantRef,
  generation: 3,
  scopes: ["observe_issue31"],
  expiresAt: 2_000_000_000,
});

let directory: string;

beforeAll(() => {
  directory = mkdtempSync(join(tmpdir(), "issue31-owner-private-store-"));
});

afterAll(() => {
  rmSync(directory, { recursive: true, force: true });
});

const seed = (path: string) => {
  const store = createIssue31LocalConfirmedRecordStore(openRealDatabase(path));
  const put = (canonicalRecordId: string, record: Issue31PrivateRecord): void => {
    store.put({
      canonicalRecordId,
      event: signedEvent(`ciphertext ${canonicalRecordId}`),
      record,
    });
  };
  put("a".repeat(64), pairing);
  put("b".repeat(64), ownerProjection);
  put("d".repeat(64), coverage);
  return store;
};

describe("Issue 31 owner-private local record store", () => {
  test("a coverage statement survives a restart, so a short view stays known to be short", () => {
    const path = join(directory, "restart.db");
    seed(path).close();

    const reopened = createIssue31LocalConfirmedRecordStore(openRealDatabase(path));
    try {
      const schemas = reopened.load().map((row) => row.record.schema);
      expect(schemas).toContain("openagents.omega.issue31.withheld_sources.v1");
      const restored = reopened
        .load()
        .find((row) => row.record.schema === "openagents.omega.issue31.withheld_sources.v1");
      // Re-decoded from disk through the same decoder, not trusted as stored.
      expect(restored?.record).toEqual(coverage);
    } finally {
      reopened.close();
    }
  });

  test("a local wipe removes the coverage statement with the projections it describes", () => {
    const path = join(directory, "wipe.db");
    const store = seed(path);
    try {
      store.clearOwnerProjections();
      const schemas = store.load().map((row) => row.record.schema);
      // Both owner-private schemas go. Leaving the statement behind would leave
      // the room asserting completeness over a list the owner just emptied.
      expect(schemas).not.toContain("openagents.omega.issue31.owner_projection.v1");
      expect(schemas).not.toContain("openagents.omega.issue31.withheld_sources.v1");
      // The wipe is scoped: pairing is how the device knows it is paired at all.
      expect(schemas).toEqual([ISSUE31_PAIRING_SCHEMA]);
    } finally {
      store.close();
    }
  });
});

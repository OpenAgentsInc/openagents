import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  compileThreadExportArtifact,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import { searchDesktopPersistedCanonicalThreadEvents } from "./thread-event-search-artifact-source.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";

const roots: string[] = [];
const now = "2026-07-17T23:37:07.000Z";
const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const makeRoot = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-search-receipts-"));
  roots.push(root);
  return root;
};

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.search.catalog.1",
  idempotencyKey: "idempotency.search.catalog.1",
  threadRef: "thread.search.catalog.1",
  actorRef: "owner.search.catalog.1",
  expectedVisibilityVersion: { state: "known" as const, value: 1 },
  createdAt: now,
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const compilation = () =>
  compileThreadExportArtifact({
    intent,
    events: [
      {
        eventRef: "event.search.catalog.1",
        threadRef: intent.threadRef,
        sequence: 1,
        data: { text: "persistent canonical search phrase" },
      },
    ],
    relations: [
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: "relation.search.catalog.1",
        threadRef: intent.threadRef,
        eventRef: "event.search.catalog.1",
        observedAt: now,
        kind: "accepted",
      },
    ],
    sha256,
  });

const receipt = (
  digest: string,
  overrides: Partial<ThreadDisclosureReceipt> = {},
): ThreadDisclosureReceipt => ({
  schema: "openagents.thread_disclosure_receipt.v1",
  receiptRef: "receipt.search.catalog.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: now,
  kind: "thread.export.create",
  result: {
    status: "export_created",
    artifactRef: `artifact.thread_export.sha256.${digest}`,
    artifactSha256: digest,
    format: "canonical_event_bundle",
    artifactAudience: { kind: "owner_only" },
  },
  ...overrides,
});

describe("Desktop canonical-export receipt catalog", () => {
  test("reopens private ref-only receipts and feeds FF-D1-35 search", () => {
    const root = makeRoot();
    const artifacts = openDesktopThreadExportArtifactStore(path.join(root, "artifacts"));
    const built = compilation();
    const persisted = artifacts.persist({
      intent,
      compilation: built,
      receiptRef: "receipt.search.catalog.1",
      observedAt: now,
    });
    expect(persisted.status).toBe("stored");
    if (persisted.status === "rejected") throw new Error("expected stored artifact");

    const directory = path.join(root, "catalog");
    const catalog = openDesktopThreadEventSearchReceiptCatalog(directory);
    expect(catalog.record(persisted.receipt)).toEqual({ status: "stored", receiptCount: 1 });
    const reopened = openDesktopThreadEventSearchReceiptCatalog(directory).list();
    expect(reopened).toMatchObject({ status: "available", receipts: [persisted.receipt] });
    if (reopened.status === "rejected") throw new Error("expected available catalog");

    const result = searchDesktopPersistedCanonicalThreadEvents(
      { loadArtifact: artifacts.load },
      { receipts: reopened.receipts, query: "canonical search" },
    );
    expect(result).toMatchObject({
      status: "available",
      projection: { totalMatches: 1, results: [{ eventRef: "event.search.catalog.1" }] },
    });
    const file = path.join(directory, "canonical-export-receipts.json");
    expect(readFileSync(file, "utf8")).not.toContain("persistent canonical search phrase");
    expect(JSON.stringify(reopened)).not.toContain(root);
    if (process.platform !== "win32") {
      expect(statSync(directory).mode & 0o777).toBe(0o700);
      expect(statSync(file).mode & 0o777).toBe(0o600);
    }
  });

  test("keeps exact replay unchanged without rewriting the catalog", () => {
    const directory = path.join(makeRoot(), "catalog");
    const catalog = openDesktopThreadEventSearchReceiptCatalog(directory);
    const value = receipt("1".repeat(64));
    expect(catalog.record(value)).toEqual({ status: "stored", receiptCount: 1 });
    const file = path.join(directory, "canonical-export-receipts.json");
    const before = readFileSync(file);
    expect(catalog.record(value)).toEqual({ status: "unchanged", receiptCount: 1 });
    expect(readFileSync(file)).toEqual(before);
  });

  test("rejects conflicting receipt, intent, idempotency, and artifact identity", () => {
    const directory = path.join(makeRoot(), "catalog");
    const catalog = openDesktopThreadEventSearchReceiptCatalog(directory);
    const digest = "2".repeat(64);
    const original = receipt(digest);
    expect(catalog.record(original).status).toBe("stored");
    const file = path.join(directory, "canonical-export-receipts.json");
    const before = readFileSync(file);

    const conflicts = [
      receipt("3".repeat(64), { observedAt: "2026-07-17T23:37:08.000Z" }),
      receipt("3".repeat(64), {
        receiptRef: "receipt.search.catalog.intent-conflict",
        observedAt: "2026-07-17T23:37:08.000Z",
      }),
      receipt("3".repeat(64), {
        receiptRef: "receipt.search.catalog.idempotency-conflict",
        intentRef: "intent.search.catalog.other",
        observedAt: "2026-07-17T23:37:08.000Z",
      }),
      receipt(digest, {
        receiptRef: "receipt.search.catalog.artifact-conflict",
        intentRef: "intent.search.catalog.other",
        idempotencyKey: "idempotency.search.catalog.other",
        threadRef: "thread.search.catalog.other",
      }),
    ];
    for (const conflict of conflicts) {
      expect(catalog.record(conflict)).toEqual({
        status: "rejected",
        reason: "conflicting_identity",
      });
      expect(readFileSync(file)).toEqual(before);
    }
  });

  test("rejects invalid input and corrupted or smuggled persisted state", () => {
    const root = makeRoot();
    const cases: ReadonlyArray<unknown> = [
      { schema: "unknown", receipts: [] },
      {
        schema: "openagents.desktop_thread_event_search_receipt_catalog.v1",
        receipts: [],
        extra: 1,
      },
      {
        schema: "openagents.desktop_thread_event_search_receipt_catalog.v1",
        receipts: [{ ...receipt("4".repeat(64)), content: "forbidden" }],
      },
    ];
    for (const [index, value] of cases.entries()) {
      const directory = path.join(root, `catalog-${index}`);
      mkdirSync(directory, { recursive: true });
      writeFileSync(path.join(directory, "canonical-export-receipts.json"), JSON.stringify(value));
      const catalog = openDesktopThreadEventSearchReceiptCatalog(directory);
      expect(catalog.list()).toEqual({ status: "rejected", reason: "corrupt_catalog" });
      expect(catalog.record(receipt("5".repeat(64)))).toEqual({
        status: "rejected",
        reason: "corrupt_catalog",
      });
    }
    for (const [name, bytes] of [
      ["invalid-json", new TextEncoder().encode("{")],
      ["oversized", new Uint8Array(1024 * 1024 + 1)],
    ] as const) {
      const directory = path.join(root, name);
      mkdirSync(directory, { recursive: true });
      writeFileSync(path.join(directory, "canonical-export-receipts.json"), bytes);
      expect(openDesktopThreadEventSearchReceiptCatalog(directory).list()).toEqual({
        status: "rejected",
        reason: "corrupt_catalog",
      });
    }
    expect(
      openDesktopThreadEventSearchReceiptCatalog(path.join(root, "invalid")).record({ bad: true }),
    ).toEqual({ status: "rejected", reason: "invalid_receipt" });
  });

  test("rejects capacity overflow and a non-directory persistence root", () => {
    const root = makeRoot();
    const directory = path.join(root, "full");
    mkdirSync(directory, { recursive: true });
    const receipts = Array.from({ length: 1_000 }, (_, index) => {
      const suffix = index.toString(16).padStart(64, "0");
      return receipt(suffix, {
        receiptRef: `receipt.search.catalog.${index}`,
        intentRef: `intent.search.catalog.${index}`,
        idempotencyKey: `idempotency.search.catalog.${index}`,
        threadRef: `thread.search.catalog.${index}`,
      });
    });
    writeFileSync(
      path.join(directory, "canonical-export-receipts.json"),
      JSON.stringify({
        schema: "openagents.desktop_thread_event_search_receipt_catalog.v1",
        receipts,
      }),
    );
    expect(
      openDesktopThreadEventSearchReceiptCatalog(directory).record(
        receipt("f".repeat(64), {
          receiptRef: "receipt.search.catalog.overflow",
          intentRef: "intent.search.catalog.overflow",
          idempotencyKey: "idempotency.search.catalog.overflow",
          threadRef: "thread.search.catalog.overflow",
        }),
      ),
    ).toEqual({ status: "rejected", reason: "capacity_exceeded" });

    const invalidRoot = path.join(root, "not-a-directory");
    writeFileSync(invalidRoot, "file");
    expect(
      openDesktopThreadEventSearchReceiptCatalog(invalidRoot).record(receipt("6".repeat(64))),
    ).toEqual({ status: "rejected", reason: "corrupt_catalog" });
  });
});

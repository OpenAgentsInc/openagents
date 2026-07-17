import { createHash } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  compileThreadExportArtifact,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import { searchDesktopPersistedCanonicalThreadEvents } from "./thread-event-search-artifact-source.ts";

const now = "2026-07-17T23:18:04.000Z";
const sha256 = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex");

const intent = (threadRef = "thread.search.persisted.1", intentRef = "intent.search.persisted.1") => ({
  schema: "openagents.thread_disclosure_intent.v1",
  intentRef,
  idempotencyKey: `idempotency.${intentRef}`,
  threadRef,
  actorRef: "owner.search.1",
  expectedVisibilityVersion: { state: "known", value: 1 },
  createdAt: now,
  kind: "thread.export.create",
  format: "canonical_event_bundle",
  artifactAudience: { kind: "owner_only" },
} as const);

const compilation = (
  threadRef = "thread.search.persisted.1",
  intentRef = "intent.search.persisted.1",
  text = "Original accepted release guidance",
) =>
  compileThreadExportArtifact({
    intent: intent(threadRef, intentRef),
    events: [{ eventRef: `event.${intentRef}`, threadRef, sequence: 1, data: { text } }],
    relations: [
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: `relation.accepted.${intentRef}`,
        threadRef,
        eventRef: `event.${intentRef}`,
        observedAt: now,
        kind: "accepted",
      },
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: `relation.superseded.${intentRef}`,
        threadRef,
        eventRef: `event.${intentRef}`,
        observedAt: "2026-07-17T23:18:05.000Z",
        kind: "superseded",
        supersededByEventRef: `event.replacement.${intentRef}`,
      },
    ],
    sha256,
  });

const receiptFor = (
  built: ReturnType<typeof compilation>,
  threadRef = "thread.search.persisted.1",
  intentRef = "intent.search.persisted.1",
  receiptRef = "receipt.search.persisted.1",
): ThreadDisclosureReceipt => ({
  schema: "openagents.thread_disclosure_receipt.v1",
  receiptRef,
  intentRef,
  idempotencyKey: `idempotency.${intentRef}`,
  threadRef,
  observedAt: now,
  kind: "thread.export.create",
  result: {
    status: "export_created",
    artifactRef: `artifact.thread_export.sha256.${built.artifactSha256}`,
    artifactSha256: built.artifactSha256,
    format: "canonical_event_bundle",
    artifactAudience: { kind: "owner_only" },
  },
});

describe("Desktop persisted canonical-event search acquisition", () => {
  test("loads a real private-store artifact and preserves its exact superseded original", () => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "oa-search-source-"));
    try {
      const store = openDesktopThreadExportArtifactStore(directory);
      const built = compilation();
      const persisted = store.persist({
        intent: intent(),
        compilation: built,
        receiptRef: "receipt.search.persisted.1",
        observedAt: now,
      });
      expect(persisted.status).toBe("stored");
      if (persisted.status === "rejected") throw new Error("expected stored artifact");

      const result = searchDesktopPersistedCanonicalThreadEvents(
        { loadArtifact: store.load },
        { receipts: [persisted.receipt], query: "release guidance" },
      );
      expect(result).toMatchObject({
        status: "available",
        projection: {
          totalMatches: 1,
          results: [
            {
              threadRef: "thread.search.persisted.1",
              eventRef: "event.intent.search.persisted.1",
              authority: {
                state: "superseded",
                supersededByEventRef: "event.replacement.intent.search.persisted.1",
              },
            },
          ],
        },
      });
      expect(JSON.stringify(result)).not.toContain(built.encoded);
      expect(JSON.stringify(result)).not.toContain(directory);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  test("dedupes exact receipt replay and exact shared-artifact receipts to one load", () => {
    const built = compilation();
    const first = receiptFor(built);
    const second = receiptFor(built, first.threadRef, first.intentRef, "receipt.search.persisted.2");
    let loads = 0;
    const result = searchDesktopPersistedCanonicalThreadEvents(
      {
        loadArtifact: () => {
          loads += 1;
          return { status: "found", bytes: built.bytes };
        },
      },
      { receipts: [first, first, second], query: "release" },
    );
    expect(result.status).toBe("available");
    expect(loads).toBe(1);
  });

  test("does not read private artifacts for a blank query", () => {
    let loads = 0;
    const result = searchDesktopPersistedCanonicalThreadEvents(
      {
        loadArtifact: () => {
          loads += 1;
          return { status: "rejected", reason: "missing" };
        },
      },
      { receipts: [{ invalid: true }], query: "   " },
    );
    expect(result).toMatchObject({ status: "available", projection: { indexedEvents: 0 } });
    expect(loads).toBe(0);
  });

  test("rejects malformed receipts and conflicting receipt-ref reuse before load", () => {
    const built = compilation();
    const valid = receiptFor(built);
    let loads = 0;
    const dependencies = {
      loadArtifact: () => {
        loads += 1;
        return { status: "found" as const, bytes: built.bytes };
      },
    };
    expect(
      searchDesktopPersistedCanonicalThreadEvents(dependencies, {
        receipts: [{ bad: true }],
        query: "release",
      }),
    ).toEqual({ status: "unavailable", reason: "invalid_request" });
    expect(
      searchDesktopPersistedCanonicalThreadEvents(dependencies, {
        receipts: [valid, { ...valid, threadRef: "thread.other" }],
        query: "release",
      }),
    ).toEqual({ status: "unavailable", reason: "invalid_request" });
    expect(loads).toBe(0);
  });

  test("reports missing, corrupt, and digest-mismatched store evidence without bytes", () => {
    const built = compilation();
    const receipt = receiptFor(built);
    expect(
      searchDesktopPersistedCanonicalThreadEvents(
        { loadArtifact: () => ({ status: "rejected", reason: "missing" }) },
        { receipts: [receipt], query: "release" },
      ),
    ).toEqual({ status: "unavailable", reason: "artifact_unavailable" });
    expect(
      searchDesktopPersistedCanonicalThreadEvents(
        { loadArtifact: () => ({ status: "rejected", reason: "corrupt_artifact" }) },
        { receipts: [receipt], query: "release" },
      ),
    ).toEqual({ status: "unavailable", reason: "artifact_corrupt" });
    expect(
      searchDesktopPersistedCanonicalThreadEvents(
        { loadArtifact: () => ({ status: "found", bytes: new TextEncoder().encode("{}") }) },
        { receipts: [receipt], query: "release" },
      ),
    ).toEqual({ status: "unavailable", reason: "artifact_corrupt" });
  });

  test("fails closed when verified bytes do not match receipt identity", () => {
    const expected = compilation();
    const other = compilation("thread.other", "intent.other", "release guidance");
    expect(
      searchDesktopPersistedCanonicalThreadEvents(
        { loadArtifact: () => ({ status: "found", bytes: other.bytes }) },
        { receipts: [receiptFor(expected)], query: "release" },
      ),
    ).toEqual({ status: "unavailable", reason: "artifact_corrupt" });

    const forgedReceipt = receiptFor(other, "thread.wrong", "intent.other");
    expect(
      searchDesktopPersistedCanonicalThreadEvents(
        { loadArtifact: () => ({ status: "found", bytes: other.bytes }) },
        { receipts: [forgedReceipt], query: "release" },
      ),
    ).toEqual({ status: "unavailable", reason: "identity_mismatch" });
  });

  test("rejects two distinct persisted artifacts claiming the same thread", () => {
    const first = compilation("thread.same", "intent.same.1", "kernel one");
    const second = compilation("thread.same", "intent.same.2", "kernel two");
    const bytes = new Map([
      [first.artifactSha256, first.bytes],
      [second.artifactSha256, second.bytes],
    ]);
    const result = searchDesktopPersistedCanonicalThreadEvents(
      {
        loadArtifact: ({ artifactSha256 }) => {
          const artifactBytes = bytes.get(artifactSha256);
          return artifactBytes === undefined
            ? { status: "rejected", reason: "missing" }
            : { status: "found", bytes: artifactBytes };
        },
      },
      {
        receipts: [
          receiptFor(first, "thread.same", "intent.same.1", "receipt.same.1"),
          receiptFor(second, "thread.same", "intent.same.2", "receipt.same.2"),
        ],
        query: "kernel",
      },
    );
    expect(result).toEqual({ status: "unavailable", reason: "projection_rejected" });
  });
});

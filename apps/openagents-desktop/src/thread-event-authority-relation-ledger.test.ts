import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadEventAuthorityRelationLedger } from "./thread-event-authority-relation-ledger.ts";

const roots: string[] = [];
const now = "2026-07-18T03:27:35.000Z";

afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const makeDirectory = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-authority-ledger-"));
  roots.push(root);
  return path.join(root, "private-authority");
};

const superseded = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_event_authority.v1",
  relationRef: "relation.superseded.ledger.1",
  threadRef: "thread.authority.ledger.1",
  eventRef: "event.authority.ledger.1",
  observedAt: now,
  kind: "superseded",
  supersededByEventRef: "event.authority.ledger.2",
  ...overrides,
});

const reverted = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_event_authority.v1",
  relationRef: "relation.reverted.ledger.1",
  threadRef: "thread.authority.ledger.1",
  eventRef: "event.authority.ledger.3",
  observedAt: "2026-07-18T03:27:36.000Z",
  kind: "reverted",
  revertedByEventRef: "event.authority.ledger.revert.1",
  restoredEventRef: "event.authority.ledger.4",
  ...overrides,
});

describe("Desktop terminal thread-event authority relation ledger", () => {
  test("reopens deterministic ref-only terminal relations with private permissions", () => {
    const directory = makeDirectory();
    const ledger = openDesktopThreadEventAuthorityRelationLedger(directory);
    expect(ledger.record(reverted())).toEqual({ status: "stored", relationCount: 1 });
    expect(ledger.record(superseded())).toEqual({ status: "stored", relationCount: 2 });

    const reopened = openDesktopThreadEventAuthorityRelationLedger(directory);
    expect(reopened.listForThread("thread.authority.ledger.1")).toEqual({
      status: "available",
      relations: [superseded(), reverted()],
    });
    expect(reopened.listForThread("thread.authority.other")).toEqual({
      status: "available",
      relations: [],
    });

    const file = path.join(directory, "terminal-authority-relations.json");
    const encoded = readFileSync(file, "utf8");
    expect(encoded).not.toMatch(/body|summary|prompt|provider|credential|path/i);
    if (process.platform !== "win32") {
      expect(statSync(directory).mode & 0o777).toBe(0o700);
      expect(statSync(file).mode & 0o777).toBe(0o600);
    }
  });

  test("keeps exact replay unchanged without rewriting the ledger", () => {
    const directory = makeDirectory();
    const ledger = openDesktopThreadEventAuthorityRelationLedger(directory);
    const relation = superseded();
    expect(ledger.record(relation)).toEqual({ status: "stored", relationCount: 1 });
    const file = path.join(directory, "terminal-authority-relations.json");
    const before = readFileSync(file);
    expect(ledger.record(relation)).toEqual({ status: "unchanged", relationCount: 1 });
    expect(readFileSync(file)).toEqual(before);
  });

  test("rejects accepted, malformed, self-referential, and broader relations", () => {
    const directory = makeDirectory();
    const ledger = openDesktopThreadEventAuthorityRelationLedger(directory);
    for (const relation of [
      { ...superseded(), kind: "accepted", supersededByEventRef: undefined },
      { ...superseded(), supersededByEventRef: "event.authority.ledger.1" },
      { ...superseded(), body: "private transcript" },
      { ...reverted(), restoredEventRef: "event.authority.ledger.revert.1" },
      { invalid: true },
    ]) {
      expect(ledger.record(relation)).toEqual({
        status: "rejected",
        reason: "invalid_relation",
      });
    }
    expect(ledger.listForThread("thread/unsafe")).toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(existsSync(directory)).toBe(false);
  });

  test("rejects relation-ref and event-terminal conflicts without mutation", () => {
    const directory = makeDirectory();
    const ledger = openDesktopThreadEventAuthorityRelationLedger(directory);
    expect(ledger.record(superseded())).toEqual({ status: "stored", relationCount: 1 });
    const file = path.join(directory, "terminal-authority-relations.json");
    const before = readFileSync(file);
    for (const conflict of [
      reverted({ relationRef: "relation.superseded.ledger.1" }),
      reverted({ eventRef: "event.authority.ledger.1" }),
    ]) {
      expect(ledger.record(conflict)).toEqual({
        status: "rejected",
        reason: "conflicting_identity",
      });
      expect(readFileSync(file)).toEqual(before);
    }
  });

  test("fails closed on corrupt, smuggled, duplicate, and oversized persisted state", () => {
    const root = makeDirectory();
    const cases: ReadonlyArray<unknown> = [
      { schema: "unknown", relations: [] },
      {
        schema: "openagents.desktop_thread_event_authority_relation_ledger.v1",
        relations: [],
        filePath: "/private/ledger",
      },
      {
        schema: "openagents.desktop_thread_event_authority_relation_ledger.v1",
        relations: [superseded(), superseded()],
      },
      {
        schema: "openagents.desktop_thread_event_authority_relation_ledger.v1",
        relations: [superseded({ summary: "private" })],
      },
    ];
    for (const [index, value] of cases.entries()) {
      const directory = path.join(root, `case-${index}`);
      mkdirSync(directory, { recursive: true });
      writeFileSync(
        path.join(directory, "terminal-authority-relations.json"),
        JSON.stringify(value),
      );
      const ledger = openDesktopThreadEventAuthorityRelationLedger(directory);
      expect(ledger.listForThread("thread.authority.ledger.1")).toEqual({
        status: "rejected",
        reason: "corrupt_ledger",
      });
      expect(ledger.record(reverted())).toEqual({
        status: "rejected",
        reason: "corrupt_ledger",
      });
    }

    const oversized = path.join(root, "oversized");
    mkdirSync(oversized, { recursive: true });
    writeFileSync(
      path.join(oversized, "terminal-authority-relations.json"),
      new Uint8Array(1024 * 1024 + 1),
    );
    expect(
      openDesktopThreadEventAuthorityRelationLedger(oversized).listForThread(
        "thread.authority.ledger.1",
      ),
    ).toEqual({ status: "rejected", reason: "corrupt_ledger" });
  });

  test("refuses a full ledger without rewriting it", () => {
    const directory = makeDirectory();
    mkdirSync(directory, { recursive: true });
    const relations = Array.from({ length: 1_000 }, (_, index) =>
      superseded({
        relationRef: `relation.superseded.capacity.${index}`,
        eventRef: `event.authority.capacity.${index}`,
        supersededByEventRef: `event.authority.capacity.next.${index}`,
      }),
    );
    const file = path.join(directory, "terminal-authority-relations.json");
    writeFileSync(
      file,
      JSON.stringify({
        schema: "openagents.desktop_thread_event_authority_relation_ledger.v1",
        relations,
      }),
    );
    const before = readFileSync(file);
    expect(openDesktopThreadEventAuthorityRelationLedger(directory).record(reverted())).toEqual({
      status: "rejected",
      reason: "capacity_exceeded",
    });
    expect(readFileSync(file)).toEqual(before);
  });
});

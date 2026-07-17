import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "vite-plus/test";
import { decodeRuntimeControlOutcome } from "@openagentsinc/agent-runtime-schema";

import {
  DesktopRuntimeControlOutcomeLedgerSchema,
  decodeDesktopRuntimeControlOutcomeLookup,
  decodeDesktopRuntimeControlOutcomeLookupResult,
} from "./runtime-control-outcome-contract.ts";
import { openDesktopRuntimeControlOutcomeStore } from "./runtime-control-outcome-store.ts";

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) rmSync(roots.pop()!, { recursive: true, force: true });
});

const fixture = (overrides: Record<string, unknown> = {}) => ({
  threadRef: "thread.desktop.1",
  outcome: decodeRuntimeControlOutcome({
    schema: "openagents.runtime_control_outcome.v1",
    outcomeRef: "outcome.desktop.1",
    intentRef: "intent.desktop.1",
    idempotencyKey: "idem.desktop.1",
    observedAt: "2026-07-17T12:00:00Z",
    admission: { status: "pending" },
    delivery: { status: "pending" },
    terminal: { status: "pending" },
    ...overrides,
  }),
});

const makeFile = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-control-outcomes-"));
  roots.push(root);
  return path.join(root, "control-outcomes", "ledger.json");
};

describe("Desktop runtime control outcome store", () => {
  test("lookup boundary accepts only the exact ref-only request and typed result", () => {
    const lookup = {
      threadRef: "thread.desktop.1",
      intentRef: "intent.desktop.1",
      idempotencyKey: "idem.desktop.1",
    };
    expect(decodeDesktopRuntimeControlOutcomeLookup(lookup)).toEqual(lookup);
    expect(decodeDesktopRuntimeControlOutcomeLookup({ ...lookup, message: "secret" })).toBeNull();
    expect(decodeDesktopRuntimeControlOutcomeLookupResult({ status: "found", record: fixture() }))
      .toEqual({ status: "found", record: fixture() });
    expect(decodeDesktopRuntimeControlOutcomeLookupResult({ status: "found", body: "secret" }))
      .toBeNull();
  });

  test("atomically persists private bounded evidence across reopen", () => {
    const file = makeFile();
    expect(openDesktopRuntimeControlOutcomeStore(file).record(fixture()).status).toBe("stored");
    const reopened = openDesktopRuntimeControlOutcomeStore(file);
    expect(reopened.list()).toEqual([fixture()]);
    expect(JSON.parse(readFileSync(file, "utf8")).schema).toBe(
      DesktopRuntimeControlOutcomeLedgerSchema,
    );
    if (process.platform !== "win32") expect(statSync(file).mode & 0o777).toBe(0o600);
  });

  test("replays only an exact identity after reopen", () => {
    const file = makeFile();
    openDesktopRuntimeControlOutcomeStore(file).record(fixture());
    const reopened = openDesktopRuntimeControlOutcomeStore(file);
    const lookup = {
      threadRef: "thread.desktop.1",
      intentRef: "intent.desktop.1",
      idempotencyKey: "idem.desktop.1",
    };
    expect(reopened.lookup(lookup)).toEqual({ status: "found", record: fixture() });
    expect(reopened.lookup({
      ...lookup,
      intentRef: "intent.missing",
      idempotencyKey: "idem.missing",
    })).toEqual({ status: "missing" });
    expect(reopened.lookup({ ...lookup, threadRef: "thread.desktop.2" })).toEqual({
      status: "rejected",
      reason: "identity_conflict",
    });
    expect(reopened.lookup({ ...lookup, idempotencyKey: "idem.other" })).toEqual({
      status: "rejected",
      reason: "identity_conflict",
    });
  });

  test("keeps exact retries idempotent and advances pending axes monotonically", () => {
    const file = makeFile();
    const store = openDesktopRuntimeControlOutcomeStore(file);
    expect(store.record(fixture()).status).toBe("stored");
    expect(store.record(fixture()).status).toBe("unchanged");
    const advanced = fixture({
      observedAt: "2026-07-17T12:00:01Z",
      admission: { status: "accepted", acceptedAt: "2026-07-17T12:00:01Z" },
      delivery: { status: "queued", queueRef: "queue.desktop.1" },
    });
    expect(store.record(advanced)).toMatchObject({ status: "advanced", record: advanced });
    expect(store.record(fixture()).status).toBe("unchanged");
  });

  test("rejects identity reuse, conflicting terminal evidence, and raw invalid records", () => {
    const store = openDesktopRuntimeControlOutcomeStore(makeFile());
    expect(store.record(fixture()).status).toBe("stored");
    expect(store.record({ ...fixture(), threadRef: "thread.desktop.2" })).toMatchObject({
      status: "rejected",
      reason: "identity_conflict",
    });
    expect(
      store.record(fixture({ delivery: { status: "failed", reasonRef: "reason.one" } })).status,
    ).toBe("advanced");
    expect(
      store.record(fixture({ delivery: { status: "unsupported", reasonRef: "reason.two" } })),
    ).toMatchObject({ status: "rejected", reason: "evidence_conflict" });
    expect(
      store.record({ threadRef: "thread.desktop.1", outcome: { body: "secret" } }),
    ).toMatchObject({ status: "rejected", reason: "invalid_request" });
    const valid = fixture()
    expect(store.record({ ...valid, outcome: { ...valid.outcome, body: "secret" } })).toMatchObject({
      status: "rejected",
      reason: "invalid_request",
    });
  });

  test("fails closed on a corrupt ledger instead of replacing evidence", () => {
    const file = makeFile();
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, "not-json", { mode: 0o600 });
    const store = openDesktopRuntimeControlOutcomeStore(file);
    expect(store.list()).toEqual([]);
    expect(store.lookup({
      threadRef: "thread.desktop.1",
      intentRef: "intent.desktop.1",
      idempotencyKey: "idem.desktop.1",
    })).toMatchObject({ status: "rejected", reason: "corrupt_ledger" });
    expect(store.record(fixture())).toMatchObject({ status: "rejected", reason: "corrupt_ledger" });
    expect(readFileSync(file, "utf8")).toBe("not-json");
  });
});

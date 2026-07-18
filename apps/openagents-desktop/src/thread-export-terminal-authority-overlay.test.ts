import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { compileThreadExportArtifact } from "@openagentsinc/agent-runtime-schema";
import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadEventAuthorityRelationLedger } from "./thread-event-authority-relation-ledger.ts";
import { readDesktopThreadExportEvidenceFromConfirmedTimeline } from "./thread-export-confirmed-timeline-evidence.ts";
import { readDesktopThreadExportEvidenceWithTerminalAuthority } from "./thread-export-terminal-authority-overlay.ts";

const THREAD = "thread.overlay.1";
const RUN = "run.overlay.1";
const roots: string[] = [];

afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const makeDirectory = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-authority-overlay-"));
  roots.push(root);
  return path.join(root, "private-authority");
};

const event = (index: number) => ({
  eventRef: `event.overlay.${index}`,
  runRef: RUN,
  sequence: index,
  eventType: "text.delta",
  summary: `Confirmed ${index}`,
  status: null,
  artifactRefs: [],
  item: { kind: "text", messageRef: `message.overlay.${index}`, text: `Confirmed ${index}` },
  createdAt: `2026-07-18T03:40:0${index}Z`,
  version: index,
});

const snapshot = () => ({
  status: { phase: "live", cursor: 4, pendingMutationCount: 0 },
  run: {
    runRef: RUN,
    routeRef: "route.overlay.1",
    runtime: "openagents_native",
    backend: "hosted",
    status: "completed",
    createdAt: "2026-07-18T03:40:00Z",
    updatedAt: "2026-07-18T03:40:05Z",
    startedAt: "2026-07-18T03:40:00Z",
    completedAt: "2026-07-18T03:40:05Z",
    failedAt: null,
    canceledAt: null,
    version: 4,
  },
  events: [1, 2, 3, 4].map(event),
});

const superseded = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_event_authority.v1",
  relationRef: "relation.overlay.superseded.1",
  threadRef: THREAD,
  eventRef: "event.overlay.1",
  observedAt: "2026-07-18T03:41:00Z",
  kind: "superseded",
  supersededByEventRef: "event.overlay.2",
  ...overrides,
});

const reverted = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_event_authority.v1",
  relationRef: "relation.overlay.reverted.3",
  threadRef: THREAD,
  eventRef: "event.overlay.3",
  observedAt: "2026-07-18T03:41:01Z",
  kind: "reverted",
  revertedByEventRef: "event.overlay.4",
  restoredEventRef: "event.overlay.2",
  ...overrides,
});

const run = (directory: string, threadRef: unknown = THREAD) =>
  Effect.runPromise(
    readDesktopThreadExportEvidenceWithTerminalAuthority(
      { authorityLedgerDirectory: directory, snapshotForThread: () => snapshot() },
      threadRef,
    ),
  );

describe("Desktop confirmed-timeline terminal-authority overlay", () => {
  test("merges complete already-observed terminal histories into canonical export", async () => {
    const directory = makeDirectory();
    const ledger = openDesktopThreadEventAuthorityRelationLedger(directory);
    expect(ledger.record(reverted())).toMatchObject({ status: "stored" });
    expect(ledger.record(superseded())).toMatchObject({ status: "stored" });

    const evidence = await run(directory);
    if (evidence.status !== "available") throw new Error("expected available evidence");
    const compiled = compileThreadExportArtifact({
      intent: {
        schema: "openagents.thread_disclosure_intent.v1",
        intentRef: "intent.overlay.1",
        idempotencyKey: "idempotency.overlay.1",
        threadRef: THREAD,
        actorRef: "actor.owner.1",
        expectedVisibilityVersion: { state: "known", value: 4 },
        createdAt: "2026-07-18T03:42:00Z",
        kind: "thread.export.create",
        format: "canonical_event_bundle",
        artifactAudience: { kind: "owner_only" },
      },
      events: evidence.events,
      relations: evidence.relations,
      sha256: () => "a".repeat(64),
    });

    expect(compiled.artifact.events.map(({ authority }) => authority.state)).toEqual([
      "superseded",
      "accepted",
      "reverted",
      "accepted",
    ]);
    expect(evidence.relations).toHaveLength(6);
    expect(JSON.stringify(evidence.relations)).not.toMatch(/summary|text|prompt|provider|path/i);
  });

  test("preserves confirmed evidence exactly when no terminal fact exists", async () => {
    const directory = makeDirectory();
    const overlay = await run(directory);
    const confirmed = await Effect.runPromise(
      readDesktopThreadExportEvidenceFromConfirmedTimeline(
        { snapshotForThread: () => snapshot() },
        THREAD,
      ),
    );
    expect(overlay).toEqual(confirmed);
  });

  test("withholds terminal relations whose event-reference graph is incomplete", async () => {
    for (const relation of [
      superseded({ supersededByEventRef: "event.overlay.missing" }),
      superseded({ eventRef: "event.overlay.missing" }),
    ]) {
      const directory = makeDirectory();
      expect(
        openDesktopThreadEventAuthorityRelationLedger(directory).record(relation),
      ).toMatchObject({
        status: "stored",
      });
      await expect(run(directory)).resolves.toEqual({ status: "unavailable" });
    }
  });

  test("withholds terminal facts that precede their accepted authority", async () => {
    const directory = makeDirectory();
    expect(
      openDesktopThreadEventAuthorityRelationLedger(directory).record(
        superseded({ observedAt: "2026-07-18T03:39:59Z" }),
      ),
    ).toMatchObject({ status: "stored" });
    await expect(run(directory)).resolves.toEqual({ status: "unavailable" });
  });

  test("withholds corrupt private ledger state without projecting native detail", async () => {
    const directory = makeDirectory();
    mkdirSync(directory, { recursive: true });
    writeFileSync(path.join(directory, "terminal-authority-relations.json"), "private-path");
    await expect(run(directory)).resolves.toEqual({ status: "unavailable" });
  });

  test("rejects an unsafe thread before confirmed or ledger lookup", async () => {
    const directory = makeDirectory();
    let reads = 0;
    const result = await Effect.runPromise(
      readDesktopThreadExportEvidenceWithTerminalAuthority(
        {
          authorityLedgerDirectory: directory,
          snapshotForThread: () => {
            reads += 1;
            return snapshot();
          },
        },
        "thread/unsafe",
      ),
    );
    expect(result).toEqual({ status: "unavailable" });
    expect(reads).toBe(0);
  });
});

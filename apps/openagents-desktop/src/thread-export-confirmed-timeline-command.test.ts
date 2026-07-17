import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import {
  openDesktopThreadExportCommandFromConfirmedTimeline,
  type DesktopThreadExportConfirmedTimelineCommandDependencies,
} from "./thread-export-confirmed-timeline-command.ts";

const THREAD = "thread.confirmed.command.1";
const RUN = "run.confirmed.command.1";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.confirmed.command.1",
  idempotencyKey: "idempotency.confirmed.command.1",
  threadRef: THREAD,
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 8 },
  createdAt: "2026-07-17T22:08:00Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const snapshot = (phase: "live" | "catching_up" = "live") => ({
  status: { phase, cursor: 8, pendingMutationCount: 0 },
  run: {
    runRef: RUN,
    routeRef: "route.confirmed.command.1",
    runtime: "openagents_native",
    backend: "hosted",
    status: "completed",
    createdAt: "2026-07-17T22:07:00Z",
    updatedAt: "2026-07-17T22:07:02Z",
    startedAt: "2026-07-17T22:07:01Z",
    completedAt: "2026-07-17T22:07:02Z",
    failedAt: null,
    canceledAt: null,
    version: 2,
  },
  events: [
    {
      eventRef: "event.confirmed.command.1",
      runRef: RUN,
      sequence: 1,
      eventType: "text.delta",
      summary: "Confirmed command evidence",
      status: null,
      artifactRefs: [],
      item: {
        kind: "text",
        messageRef: "message.confirmed.command.1",
        text: "Confirmed command evidence",
      },
      createdAt: "2026-07-17T22:07:02Z",
      version: 3,
    },
  ],
});

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const sha256 = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex");

const harness = (
  overrides: Partial<DesktopThreadExportConfirmedTimelineCommandDependencies> = {},
) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-confirmed-export-command-"));
  roots.push(root);
  const directory = path.join(root, "thread-exports");
  const store = openDesktopThreadExportArtifactStore(directory);
  const reads: string[] = [];
  let persisted = 0;
  const dependencies: DesktopThreadExportConfirmedTimelineCommandDependencies = {
    snapshotForThread: (threadRef) => {
      reads.push(threadRef);
      return snapshot();
    },
    persist: (request) => {
      persisted += 1;
      return store.persist(request);
    },
    makeReceiptRef: () => "receipt.confirmed.command.1",
    observedAt: () => "2026-07-17T22:08:01Z",
    sha256,
    ...overrides,
  };
  return { directory, reads, persisted: () => persisted, dependencies };
};

describe("Desktop confirmed-timeline export-command composition", () => {
  test("reads the exact confirmed thread and persists its accepted canonical artifact", async () => {
    const value = harness();
    const result = await openDesktopThreadExportCommandFromConfirmedTimeline(
      value.dependencies,
    ).execute(intent);

    expect(value.reads).toEqual([THREAD]);
    expect(value.persisted()).toBe(1);
    expect(result).toMatchObject({
      status: "stored",
      receipt: {
        intentRef: intent.intentRef,
        threadRef: THREAD,
        result: { status: "export_created", artifactAudience: { kind: "owner_only" } },
      },
    });
    if (result.status !== "stored" || result.receipt.result.status !== "export_created") {
      throw new Error("expected stored export");
    }
    const encoded = readFileSync(
      path.join(value.directory, `${result.receipt.result.artifactSha256}.json`),
      "utf8",
    );
    expect(encoded).toContain("Confirmed command evidence");
    expect(encoded).toContain('"state":"accepted"');
    expect(JSON.stringify(result)).not.toContain("Confirmed command evidence");
    expect(JSON.stringify(result)).not.toContain(value.directory);
  });

  test("defers timeline lookup until the existing command admits the intent", async () => {
    const value = harness();
    const command = openDesktopThreadExportCommandFromConfirmedTimeline(value.dependencies);

    await expect(command.execute({ ...intent, transcript: "private" })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_intent",
    });
    await expect(
      command.execute({ ...intent, artifactAudience: { kind: "internet_readable" } }),
    ).resolves.toEqual({ status: "rejected", reason: "unsupported_export" });
    expect(value.reads).toEqual([]);
    expect(value.persisted()).toBe(0);
  });

  test("preserves fail-closed evidence-unavailable behavior", async () => {
    for (const snapshotForThread of [
      () => snapshot("catching_up"),
      () => {
        throw new Error("/private/source/timeline");
      },
    ]) {
      const value = harness({ snapshotForThread });
      await expect(
        openDesktopThreadExportCommandFromConfirmedTimeline(value.dependencies).execute(intent),
      ).resolves.toEqual({ status: "rejected", reason: "evidence_unavailable" });
      expect(value.persisted()).toBe(0);
    }
  });

  test("preserves host-metadata rejection without persistence", async () => {
    const value = harness({ sha256: () => "invalid-digest" });
    await expect(
      openDesktopThreadExportCommandFromConfirmedTimeline(value.dependencies).execute(intent),
    ).resolves.toEqual({ status: "rejected", reason: "host_metadata_invalid" });
    expect(value.reads).toEqual([THREAD]);
    expect(value.persisted()).toBe(0);
  });
});

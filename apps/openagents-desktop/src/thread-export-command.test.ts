import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import {
  openDesktopThreadExportCommand,
  type DesktopThreadExportCommandDependencies,
  type DesktopThreadExportEvidenceSnapshot,
} from "./thread-export-command.ts";

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.command.1",
  idempotencyKey: "idempotency.export.command.1",
  threadRef: "thread.command.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 3 },
  createdAt: "2026-07-17T15:01:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const evidence = (): DesktopThreadExportEvidenceSnapshot => ({
  status: "available",
  threadRef: intent.threadRef,
  events: [
    {
      eventRef: "event.command.1",
      threadRef: intent.threadRef,
      sequence: 1,
      data: { role: "assistant", text: "private canonical evidence" },
    },
  ],
  relations: [
    {
      schema: "openagents.thread_event_authority.v1",
      relationRef: "relation.command.accepted.1",
      threadRef: intent.threadRef,
      eventRef: "event.command.1",
      observedAt: "2026-07-17T15:01:01.000Z",
      kind: "accepted",
    },
  ],
});

const makeStore = () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-export-command-"));
  roots.push(root);
  const directory = path.join(root, "thread-exports");
  return { directory, store: openDesktopThreadExportArtifactStore(directory) };
};

const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

const dependencies = (overrides: Partial<DesktopThreadExportCommandDependencies> = {}) => {
  const { directory, store } = makeStore();
  const base: DesktopThreadExportCommandDependencies = {
    readEvidence: async () => evidence(),
    persist: store.persist,
    makeReceiptRef: () => "receipt.export.command.1",
    observedAt: () => "2026-07-17T15:01:02.000Z",
    sha256,
  };
  return { directory, store, value: { ...base, ...overrides } };
};

describe("Desktop thread export command", () => {
  test("consumes host-owned authority evidence and privately persists a ref-only artifact", async () => {
    const requestedThreads: string[] = [];
    const fixture = dependencies({
      readEvidence: async (threadRef) => {
        requestedThreads.push(threadRef);
        return evidence();
      },
    });
    const result = await openDesktopThreadExportCommand(fixture.value).execute(intent);

    expect(requestedThreads).toEqual([intent.threadRef]);
    expect(result).toMatchObject({
      status: "stored",
      receipt: {
        intentRef: intent.intentRef,
        threadRef: intent.threadRef,
        result: {
          status: "export_created",
          format: "canonical_event_bundle",
          artifactAudience: { kind: "owner_only" },
        },
      },
    });
    expect(JSON.stringify(result)).not.toContain("private canonical evidence");
    expect(JSON.stringify(result)).not.toContain(fixture.directory);
    if (result.status !== "stored") throw new Error("expected stored export");
    const digest = result.receipt.result;
    if (digest.status !== "export_created") throw new Error("expected export receipt");
    expect(
      readFileSync(path.join(fixture.directory, `${digest.artifactSha256}.json`), "utf8"),
    ).toContain("private canonical evidence");
  });

  test("reuses the exact private artifact on command retry and verified reopen", async () => {
    const fixture = dependencies();
    const command = openDesktopThreadExportCommand(fixture.value);
    const first = await command.execute(intent);
    const second = await command.execute(intent);

    expect(first.status).toBe("stored");
    expect(second).toEqual({
      status: "unchanged",
      receipt: first.status === "stored" ? first.receipt : undefined,
    });
    if (first.status !== "stored" || first.receipt.result.status !== "export_created") {
      throw new Error("expected export receipt");
    }
    expect(
      openDesktopThreadExportArtifactStore(fixture.directory).load({
        artifactRef: first.receipt.result.artifactRef,
        artifactSha256: first.receipt.result.artifactSha256,
      }).status,
    ).toBe("found");
  });

  test("rejects malformed and unsupported intents before reading canonical evidence", async () => {
    let reads = 0;
    const fixture = dependencies({
      readEvidence: async () => {
        reads += 1;
        return evidence();
      },
    });
    const command = openDesktopThreadExportCommand(fixture.value);

    await expect(command.execute({ ...intent, transcript: "private" })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_intent",
    });
    await expect(
      command.execute({ ...intent, artifactAudience: { kind: "internet_readable" } }),
    ).resolves.toEqual({ status: "rejected", reason: "unsupported_export" });
    await expect(command.execute({ ...intent, format: "json" })).resolves.toEqual({
      status: "rejected",
      reason: "unsupported_export",
    });
    expect(reads).toBe(0);
  });

  test("fails closed for unavailable, throwing, or cross-thread evidence sources", async () => {
    const unavailable = dependencies({ readEvidence: async () => ({ status: "unavailable" }) });
    await expect(
      openDesktopThreadExportCommand(unavailable.value).execute(intent),
    ).resolves.toEqual({
      status: "rejected",
      reason: "evidence_unavailable",
    });

    const throwing = dependencies({
      readEvidence: async () => {
        throw new Error("private transport detail");
      },
    });
    await expect(openDesktopThreadExportCommand(throwing.value).execute(intent)).resolves.toEqual({
      status: "rejected",
      reason: "evidence_unavailable",
    });

    const mismatch = dependencies({
      readEvidence: async () => ({ ...evidence(), threadRef: "thread.other.1" }),
    });
    await expect(openDesktopThreadExportCommand(mismatch.value).execute(intent)).resolves.toEqual({
      status: "rejected",
      reason: "evidence_thread_mismatch",
    });
  });

  test("separates invalid authority evidence, host metadata, and persistence refusal", async () => {
    const invalidEvidence = dependencies({
      readEvidence: async () => ({ ...evidence(), relations: [] }),
    });
    await expect(
      openDesktopThreadExportCommand(invalidEvidence.value).execute(intent),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_evidence" });

    const unboundedEvidence = dependencies({
      readEvidence: async () => ({ ...evidence(), relations: Array.from({ length: 2_001 }) }),
    });
    await expect(
      openDesktopThreadExportCommand(unboundedEvidence.value).execute(intent),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_evidence" });

    const invalidDigest = dependencies({ sha256: () => "not-a-digest" });
    await expect(
      openDesktopThreadExportCommand(invalidDigest.value).execute(intent),
    ).resolves.toEqual({
      status: "rejected",
      reason: "host_metadata_invalid",
    });

    const invalidReceipt = dependencies({ makeReceiptRef: () => "../private" });
    await expect(
      openDesktopThreadExportCommand(invalidReceipt.value).execute(intent),
    ).resolves.toEqual({ status: "rejected", reason: "host_metadata_invalid" });

    const refused = dependencies({
      persist: () => ({ status: "rejected", reason: "identity_mismatch" }),
    });
    await expect(openDesktopThreadExportCommand(refused.value).execute(intent)).resolves.toEqual({
      status: "rejected",
      reason: "persistence_refused",
    });
  });
});

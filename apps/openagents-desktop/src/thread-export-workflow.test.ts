import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  runDesktopThreadExportWorkflow,
  type DesktopThreadExportWorkflowDependencies,
} from "./thread-export-workflow.ts";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.workflow.1",
  idempotencyKey: "idempotency.export.workflow.1",
  threadRef: "thread.export.workflow.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 7 },
  createdAt: "2026-07-17T17:03:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.export.workflow.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: "2026-07-17T17:03:01.000Z",
  kind: "thread.export.create" as const,
  result: {
    status: "export_created" as const,
    artifactRef: "artifact.export.workflow.1",
    artifactSha256: "e".repeat(64),
    format: "canonical_event_bundle" as const,
    artifactAudience: { kind: "owner_only" as const },
  },
};

const written = {
  status: "written" as const,
  artifactRef: receipt.result.artifactRef,
  artifactSha256: receipt.result.artifactSha256,
  replaceAuthorized: false,
};

const fixture = (overrides: Partial<DesktopThreadExportWorkflowDependencies> = {}) => {
  const creates: unknown[] = [];
  const writes: unknown[] = [];
  const dependencies: DesktopThreadExportWorkflowDependencies = {
    create: async (request) => {
      creates.push(request);
      return { status: "stored", receipt };
    },
    write: async (request) => {
      writes.push(request);
      return written;
    },
    ...overrides,
  };
  const run = (input: unknown) =>
    Effect.runPromise(runDesktopThreadExportWorkflow(dependencies, input));
  return { creates, writes, run };
};

describe("Desktop canonical-export create-then-write workflow", () => {
  test("rejects malformed and broader input before either host operation", async () => {
    const value = fixture();
    await expect(
      value.run({ intent: { ...intent, events: [{ private: true }] } }),
    ).resolves.toEqual({ status: "rejected", stage: "create", reason: "invalid_request" });
    await expect(
      value.run({ intent: { ...intent, artifactAudience: { kind: "internet_readable" } } }),
    ).resolves.toEqual({ status: "rejected", stage: "create", reason: "invalid_request" });
    expect(value.creates).toEqual([]);
    expect(value.writes).toEqual([]);
  });

  test("creates before writing and delegates only the decoded receipt", async () => {
    const value = fixture();
    const result = await value.run({ intent });
    expect(result).toEqual(written);
    expect(value.creates).toEqual([{ intent }]);
    expect(value.writes).toEqual([{ receipt }]);
    expect(JSON.stringify(value.writes)).not.toContain("events");
    expect(JSON.stringify(result)).not.toContain("receiptRef");
  });

  test("accepts an unchanged idempotent creation before writing", async () => {
    const value = fixture({ create: async () => ({ status: "unchanged", receipt }) });
    await expect(value.run({ intent })).resolves.toEqual(written);
    expect(value.writes).toEqual([{ receipt }]);
  });

  test("stops at bounded creation rejection and collapses unsafe creation failures", async () => {
    const rejected = fixture({
      create: async () => ({ status: "rejected", reason: "evidence_unavailable" }),
    });
    await expect(rejected.run({ intent })).resolves.toEqual({
      status: "rejected",
      stage: "create",
      reason: "evidence_unavailable",
    });
    expect(rejected.writes).toEqual([]);

    for (const create of [
      async () => ({ status: "stored", receipt, filePath: "/private/export.json" }),
      async () => {
        throw new Error("/private/export.json");
      },
    ]) {
      const unavailable = fixture({ create });
      await expect(unavailable.run({ intent })).resolves.toEqual({
        status: "rejected",
        stage: "create",
        reason: "command_unavailable",
      });
      expect(unavailable.writes).toEqual([]);
    }
  });

  test("preserves cancellation and bounded write rejection", async () => {
    const cancelled = fixture({ write: async () => ({ status: "cancelled" }) });
    await expect(cancelled.run({ intent })).resolves.toEqual({ status: "cancelled" });

    const rejected = fixture({
      write: async () => ({ status: "rejected", reason: "destination_exists" }),
    });
    await expect(rejected.run({ intent })).resolves.toEqual({
      status: "rejected",
      stage: "write",
      reason: "destination_exists",
    });
  });

  test("fails closed on thrown, malformed, path-leaking, or mismatched write results", async () => {
    const outputs: ReadonlyArray<() => Promise<unknown>> = [
      async () => ({ ...written, artifactRef: "artifact.other" }),
      async () => ({ ...written, artifactSha256: "f".repeat(64) }),
      async () => ({ ...written, filePath: "/private/export.json" }),
      async () => ({ status: "rejected", reason: "native_error" }),
      async () => {
        throw new Error("/private/export.json");
      },
    ];
    for (const write of outputs) {
      const value = fixture({ write });
      const result = await value.run({ intent });
      expect(result).toEqual({
        status: "rejected",
        stage: "write",
        reason: "transport_unavailable",
      });
      expect(JSON.stringify(result)).not.toContain("/private");
      expect(JSON.stringify(result)).not.toContain("receiptRef");
    }
  });
});

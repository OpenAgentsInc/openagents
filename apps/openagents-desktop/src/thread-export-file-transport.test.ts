import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";

import { compileThreadExportArtifact } from "@openagentsinc/agent-runtime-schema";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import {
  openDesktopThreadExportFileTransport,
  type DesktopThreadExportFileTransportDependencies,
} from "./thread-export-file-transport.ts";

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.transport.1",
  idempotencyKey: "idempotency.export.transport.1",
  threadRef: "thread:transport:1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 4 },
  createdAt: "2026-07-17T15:10:19.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

const fixture = () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-export-transport-"));
  roots.push(root);
  const store = openDesktopThreadExportArtifactStore(path.join(root, "private-artifacts"));
  const compilation = compileThreadExportArtifact({
    intent,
    events: [
      {
        eventRef: "event.transport.1",
        threadRef: intent.threadRef,
        sequence: 1,
        data: { role: "assistant", text: "owner-selected export" },
      },
    ],
    relations: [
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: "relation.transport.accepted.1",
        threadRef: intent.threadRef,
        eventRef: "event.transport.1",
        observedAt: "2026-07-17T15:10:20.000Z",
        kind: "accepted",
      },
    ],
    sha256,
  });
  const persisted = store.persist({
    intent,
    compilation,
    receiptRef: "receipt.export.transport.1",
    observedAt: "2026-07-17T15:10:21.000Z",
  });
  if (persisted.status !== "stored") throw new Error("expected stored fixture");
  return { root, store, compilation, receipt: persisted.receipt };
};

const dependencies = (
  filePath: string,
  load: DesktopThreadExportFileTransportDependencies["load"],
  replaceExisting = false,
): DesktopThreadExportFileTransportDependencies => ({
  load,
  selectDestination: async () => ({ status: "selected", filePath, replaceExisting }),
});

describe("Desktop thread export file transport", () => {
  test("writes exact verified bytes atomically with a sanitized suggestion and path-free result", async () => {
    const value = fixture();
    const destination = path.join(value.root, "selected", "thread.json");
    writeFileSync(path.join(value.root, "placeholder"), "keep");
    const selected = path.dirname(destination);
    mkdirSync(selected);
    const suggestions: string[] = [];
    const transport = openDesktopThreadExportFileTransport({
      load: value.store.load,
      selectDestination: async ({ suggestedName }) => {
        suggestions.push(suggestedName);
        return { status: "selected", filePath: destination, replaceExisting: false };
      },
    });
    const result = await transport.write(value.receipt);

    expect(suggestions).toEqual([
      `openagents-thread-transport-1-${value.compilation.artifactSha256.slice(0, 12)}.json`,
    ]);
    expect(result).toEqual({
      status: "written",
      artifactRef: `artifact.thread_export.sha256.${value.compilation.artifactSha256}`,
      artifactSha256: value.compilation.artifactSha256,
      replaceAuthorized: false,
    });
    expect(JSON.stringify(result)).not.toContain(destination);
    expect(readFileSync(destination)).toEqual(Buffer.from(value.compilation.bytes));
    if (process.platform !== "win32") expect(statSync(destination).mode & 0o777).toBe(0o600);
    expect(readdirSync(selected)).toEqual(["thread.json"]);
  });

  test("treats owner cancellation as a non-error without creating a destination", async () => {
    const value = fixture();
    const transport = openDesktopThreadExportFileTransport({
      load: value.store.load,
      selectDestination: async () => ({ status: "cancelled" }),
    });
    await expect(transport.write(value.receipt)).resolves.toEqual({ status: "cancelled" });
  });

  test("rejects malformed or broader receipts before loading or selecting a destination", async () => {
    let loads = 0;
    let selections = 0;
    const transport = openDesktopThreadExportFileTransport({
      load: () => {
        loads += 1;
        return { status: "rejected", reason: "missing" };
      },
      selectDestination: async () => {
        selections += 1;
        return { status: "cancelled" };
      },
    });
    await expect(transport.write({ body: "private" })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_receipt",
    });
    const value = fixture();
    await expect(
      transport.write({
        ...value.receipt,
        result: { ...value.receipt.result, artifactAudience: { kind: "internet_readable" } },
      }),
    ).resolves.toEqual({ status: "rejected", reason: "unsupported_export" });
    expect({ loads, selections }).toEqual({ loads: 0, selections: 0 });
  });

  test("distinguishes missing or corrupt private artifacts before destination selection", async () => {
    const value = fixture();
    let selections = 0;
    const missing = openDesktopThreadExportFileTransport({
      load: () => ({ status: "rejected", reason: "missing" }),
      selectDestination: async () => {
        selections += 1;
        return { status: "cancelled" };
      },
    });
    await expect(missing.write(value.receipt)).resolves.toEqual({
      status: "rejected",
      reason: "artifact_missing",
    });
    const corrupt = openDesktopThreadExportFileTransport({
      load: () => ({ status: "rejected", reason: "corrupt_artifact" }),
      selectDestination: async () => ({ status: "cancelled" }),
    });
    await expect(corrupt.write(value.receipt)).resolves.toEqual({
      status: "rejected",
      reason: "artifact_corrupt",
    });
    expect(selections).toBe(0);
  });

  test("requires explicit replacement authority and preserves or replaces exact destination bytes", async () => {
    const value = fixture();
    const destination = path.join(value.root, "thread.json");
    writeFileSync(destination, "existing", { mode: 0o600 });
    const refusing = openDesktopThreadExportFileTransport(
      dependencies(destination, value.store.load, false),
    );
    await expect(refusing.write(value.receipt)).resolves.toEqual({
      status: "rejected",
      reason: "destination_exists",
    });
    expect(readFileSync(destination, "utf8")).toBe("existing");

    const replacing = openDesktopThreadExportFileTransport(
      dependencies(destination, value.store.load, true),
    );
    await expect(replacing.write(value.receipt)).resolves.toMatchObject({
      status: "written",
      replaceAuthorized: true,
    });
    expect(readFileSync(destination)).toEqual(Buffer.from(value.compilation.bytes));
  });

  test("rejects invalid or unavailable destinations and cleans a failed temporary write", async () => {
    const value = fixture();
    const invalid = openDesktopThreadExportFileTransport({
      load: value.store.load,
      selectDestination: async () => ({
        status: "selected",
        filePath: "relative/private.json",
        replaceExisting: false,
      }),
    });
    await expect(invalid.write(value.receipt)).resolves.toEqual({
      status: "rejected",
      reason: "destination_invalid",
    });

    const unavailable = openDesktopThreadExportFileTransport({
      load: value.store.load,
      selectDestination: async () => {
        throw new Error("native picker detail");
      },
    });
    await expect(unavailable.write(value.receipt)).resolves.toEqual({
      status: "rejected",
      reason: "destination_unavailable",
    });

    const missingParent = path.join(value.root, "missing", "thread.json");
    const failed = openDesktopThreadExportFileTransport(
      dependencies(missingParent, value.store.load, false),
    );
    await expect(failed.write(value.receipt)).resolves.toEqual({
      status: "rejected",
      reason: "write_failed",
    });
    expect(readdirSync(value.root).some((name) => name.endsWith(".tmp"))).toBe(false);
  });
});

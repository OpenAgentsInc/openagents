import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import os from "node:os";
import path from "node:path";

import {
  compileThreadExportArtifact,
  type ThreadExportArtifactCompilation,
} from "@openagentsinc/agent-runtime-schema";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.desktop.1",
  idempotencyKey: "idempotency.export.desktop.1",
  threadRef: "thread.desktop.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 2 },
  createdAt: "2026-07-17T14:40:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

const compilation = (): ThreadExportArtifactCompilation =>
  compileThreadExportArtifact({
    intent,
    events: [
      {
        eventRef: "event.desktop.1",
        threadRef: intent.threadRef,
        sequence: 1,
        data: { role: "assistant", text: "owner-local export" },
      },
    ],
    relations: [
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: "relation.accepted.desktop.1",
        threadRef: intent.threadRef,
        eventRef: "event.desktop.1",
        observedAt: "2026-07-17T14:40:01.000Z",
        kind: "accepted",
      },
    ],
    sha256,
  });

const makeDirectory = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-exports-"));
  roots.push(root);
  return path.join(root, "thread-exports");
};

const request = (compiled = compilation()) => ({
  intent,
  compilation: compiled,
  receiptRef: "receipt.export.desktop.1",
  observedAt: "2026-07-17T14:40:02.000Z",
});

describe("Desktop thread export artifact store", () => {
  test("atomically persists exact private bytes and returns only a typed receipt", () => {
    const directory = makeDirectory();
    const compiled = compilation();
    const result = openDesktopThreadExportArtifactStore(directory).persist(request(compiled));

    expect(result).toMatchObject({
      status: "stored",
      receipt: {
        intentRef: intent.intentRef,
        threadRef: intent.threadRef,
        result: {
          status: "export_created",
          artifactSha256: compiled.artifactSha256,
          artifactAudience: { kind: "owner_only" },
        },
      },
    });
    expect(JSON.stringify(result)).not.toContain(directory);
    const file = path.join(directory, `${compiled.artifactSha256}.json`);
    expect(readFileSync(file)).toEqual(Buffer.from(compiled.bytes));
    if (process.platform !== "win32") {
      expect(statSync(directory).mode & 0o777).toBe(0o700);
      expect(statSync(file).mode & 0o777).toBe(0o600);
    }
    expect(readdirSync(directory)).toEqual([`${compiled.artifactSha256}.json`]);
  });

  test("reopens and loads only an exact ref and digest while exact persist retry is unchanged", () => {
    const directory = makeDirectory();
    const compiled = compilation();
    const first = openDesktopThreadExportArtifactStore(directory);
    const stored = first.persist(request(compiled));
    expect(stored.status).toBe("stored");

    const reopened = openDesktopThreadExportArtifactStore(directory);
    expect(reopened.persist(request(compiled)).status).toBe("unchanged");
    const artifactRef = `artifact.thread_export.sha256.${compiled.artifactSha256}`;
    expect(reopened.load({ artifactRef, artifactSha256: compiled.artifactSha256 })).toEqual({
      status: "found",
      bytes: compiled.bytes,
    });
    expect(
      reopened.load({
        artifactRef: `${artifactRef}.other`,
        artifactSha256: compiled.artifactSha256,
      }),
    ).toEqual({ status: "rejected", reason: "invalid_request" });
    expect(reopened.load({ artifactRef, artifactSha256: "0".repeat(64) })).toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
  });

  test("rejects mismatched intent, bytes, digest, and receipt metadata before persistence", () => {
    const directory = makeDirectory();
    const compiled = compilation();
    const store = openDesktopThreadExportArtifactStore(directory);

    expect(
      store.persist({ ...request(compiled), intent: { ...intent, threadRef: "thread.other.1" } }),
    ).toEqual({ status: "rejected", reason: "identity_mismatch" });
    expect(
      store.persist({
        ...request(compiled),
        compilation: { ...compiled, bytes: new TextEncoder().encode("different") },
      }),
    ).toEqual({ status: "rejected", reason: "identity_mismatch" });
    expect(
      store.persist({
        ...request(compiled),
        compilation: { ...compiled, artifactSha256: "0".repeat(64) },
      }),
    ).toEqual({ status: "rejected", reason: "digest_mismatch" });
    expect(store.persist({ ...request(compiled), receiptRef: "../escape" })).toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(exists(directory)).toBe(false);
  });

  test("refuses corrupt existing bytes without overwriting and rejects corrupt reload", () => {
    const directory = makeDirectory();
    const compiled = compilation();
    mkdirSync(directory, { recursive: true, mode: 0o700 });
    const file = path.join(directory, `${compiled.artifactSha256}.json`);
    writeFileSync(file, "corrupt", { mode: 0o600 });
    const store = openDesktopThreadExportArtifactStore(directory);

    expect(store.persist(request(compiled))).toEqual({
      status: "rejected",
      reason: "existing_artifact_conflict",
    });
    expect(readFileSync(file, "utf8")).toBe("corrupt");
    expect(
      store.load({
        artifactRef: `artifact.thread_export.sha256.${compiled.artifactSha256}`,
        artifactSha256: compiled.artifactSha256,
      }),
    ).toEqual({ status: "rejected", reason: "corrupt_artifact" });
  });

  test("fails closed when the persistence root is a file", () => {
    const directory = makeDirectory();
    mkdirSync(path.dirname(directory), { recursive: true });
    writeFileSync(directory, "not-a-directory");
    expect(openDesktopThreadExportArtifactStore(directory).persist(request())).toEqual({
      status: "rejected",
      reason: "persistence_failed",
    });
  });
});

const exists = (value: string): boolean => {
  try {
    statSync(value);
    return true;
  } catch {
    return false;
  }
};

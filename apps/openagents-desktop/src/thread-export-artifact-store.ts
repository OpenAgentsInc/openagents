import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createHash, randomUUID } from "node:crypto";
import path from "node:path";

import {
  decodeThreadDisclosureIntent,
  decodeThreadDisclosureReceipt,
  ThreadExportArtifact,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

const MAX_ARTIFACT_BYTES = 4 * 1024 * 1024;
const SHA256 = /^[a-f0-9]{64}$/;
const ARTIFACT_REF_PREFIX = "artifact.thread_export.sha256.";
const decodeArtifact = S.decodeUnknownSync(ThreadExportArtifact);

export type DesktopThreadExportPersistRequest = Readonly<{
  intent: unknown;
  compilation: unknown;
  receiptRef: string;
  observedAt: string;
}>;

export type DesktopThreadExportPersistResult =
  | Readonly<{
      status: "stored" | "unchanged";
      receipt: ThreadDisclosureReceipt;
    }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_request"
        | "identity_mismatch"
        | "artifact_too_large"
        | "digest_mismatch"
        | "existing_artifact_conflict"
        | "persistence_failed";
    }>;

export type DesktopThreadExportLoadResult =
  | Readonly<{ status: "found"; bytes: Uint8Array }>
  | Readonly<{
      status: "rejected";
      reason: "invalid_request" | "missing" | "corrupt_artifact";
    }>;

type ValidatedCompilation = Readonly<{
  artifact: typeof ThreadExportArtifact.Type;
  bytes: Uint8Array;
  artifactSha256: string;
}>;

const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

const equalBytes = (left: Uint8Array, right: Uint8Array): boolean =>
  left.byteLength === right.byteLength && left.every((value, index) => value === right[index]);

const artifactRef = (digest: string): string => `${ARTIFACT_REF_PREFIX}${digest}`;

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const validateCompilation = (
  value: unknown,
):
  | Readonly<{ status: "valid"; value: ValidatedCompilation }>
  | Readonly<{
      status: "rejected";
      reason: "invalid_request" | "identity_mismatch" | "artifact_too_large" | "digest_mismatch";
    }> => {
  const encoded = field(value, "encoded");
  const bytes = field(value, "bytes");
  const digest = field(value, "artifactSha256");
  const suppliedArtifact = field(value, "artifact");
  if (
    typeof encoded !== "string" ||
    !(bytes instanceof Uint8Array) ||
    typeof digest !== "string" ||
    !SHA256.test(digest)
  ) {
    return { status: "rejected", reason: "invalid_request" };
  }
  if (bytes.byteLength > MAX_ARTIFACT_BYTES) {
    return { status: "rejected", reason: "artifact_too_large" };
  }
  try {
    if (new TextDecoder("utf-8", { fatal: true }).decode(bytes) !== encoded) {
      return { status: "rejected", reason: "identity_mismatch" };
    }
    const encodedBytes = new TextEncoder().encode(encoded);
    if (!equalBytes(encodedBytes, bytes)) {
      return { status: "rejected", reason: "identity_mismatch" };
    }
    const artifact = decodeArtifact(JSON.parse(encoded));
    if (JSON.stringify(suppliedArtifact) !== JSON.stringify(artifact)) {
      return { status: "rejected", reason: "identity_mismatch" };
    }
    if (artifact.artifactAudience.kind !== "owner_only") {
      return { status: "rejected", reason: "identity_mismatch" };
    }
    if (sha256(bytes) !== digest) {
      return { status: "rejected", reason: "digest_mismatch" };
    }
    return {
      status: "valid",
      value: { artifact, bytes: Uint8Array.from(bytes), artifactSha256: digest },
    };
  } catch {
    return { status: "rejected", reason: "invalid_request" };
  }
};

const validateStoredBytes = (bytes: Uint8Array, digest: string): boolean => {
  if (bytes.byteLength > MAX_ARTIFACT_BYTES || sha256(bytes) !== digest) return false;
  try {
    const artifact = decodeArtifact(
      JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)),
    );
    return artifact.artifactAudience.kind === "owner_only";
  } catch {
    return false;
  }
};

/**
 * Main-process-only private artifact store. No path or raw bytes cross to a
 * renderer through this module's persistence result.
 */
export const openDesktopThreadExportArtifactStore = (directory: string) => {
  const destination = (digest: string): string => path.join(directory, `${digest}.json`);

  const load = (
    input: Readonly<{
      artifactRef: string;
      artifactSha256: string;
    }>,
  ): DesktopThreadExportLoadResult => {
    if (
      !SHA256.test(input.artifactSha256) ||
      input.artifactRef !== artifactRef(input.artifactSha256)
    ) {
      return { status: "rejected", reason: "invalid_request" };
    }
    try {
      const bytes = readFileSync(destination(input.artifactSha256));
      if (!validateStoredBytes(bytes, input.artifactSha256)) {
        return { status: "rejected", reason: "corrupt_artifact" };
      }
      return { status: "found", bytes: Uint8Array.from(bytes) };
    } catch (error) {
      if (error instanceof Error && "code" in error && error.code === "ENOENT") {
        return { status: "rejected", reason: "missing" };
      }
      return { status: "rejected", reason: "corrupt_artifact" };
    }
  };

  const persist = (input: DesktopThreadExportPersistRequest): DesktopThreadExportPersistResult => {
    const validated = validateCompilation(input.compilation);
    if (validated.status === "rejected") return validated;

    let intent: ReturnType<typeof decodeThreadDisclosureIntent>;
    try {
      intent = decodeThreadDisclosureIntent(input.intent);
    } catch {
      return { status: "rejected", reason: "invalid_request" };
    }
    const { artifact, bytes, artifactSha256 } = validated.value;
    if (
      intent.kind !== "thread.export.create" ||
      intent.format !== "canonical_event_bundle" ||
      intent.artifactAudience.kind !== "owner_only" ||
      intent.intentRef !== artifact.intentRef ||
      intent.threadRef !== artifact.threadRef ||
      intent.format !== artifact.format
    ) {
      return { status: "rejected", reason: "identity_mismatch" };
    }

    let receipt: ThreadDisclosureReceipt;
    try {
      receipt = decodeThreadDisclosureReceipt({
        schema: "openagents.thread_disclosure_receipt.v1",
        receiptRef: input.receiptRef,
        intentRef: intent.intentRef,
        idempotencyKey: intent.idempotencyKey,
        threadRef: intent.threadRef,
        observedAt: input.observedAt,
        kind: intent.kind,
        result: {
          status: "export_created",
          artifactRef: artifactRef(artifactSha256),
          artifactSha256,
          format: artifact.format,
          artifactAudience: artifact.artifactAudience,
        },
      });
    } catch {
      return { status: "rejected", reason: "invalid_request" };
    }

    const file = destination(artifactSha256);
    if (existsSync(file)) {
      const existing = load({
        artifactRef: artifactRef(artifactSha256),
        artifactSha256,
      });
      if (existing.status !== "found" || !equalBytes(existing.bytes, bytes)) {
        return { status: "rejected", reason: "existing_artifact_conflict" };
      }
      return { status: "unchanged", receipt };
    }

    let temporary: string | undefined;
    try {
      mkdirSync(directory, { recursive: true, mode: 0o700 });
      if (process.platform !== "win32") chmodSync(directory, 0o700);
      temporary = path.join(directory, `.${artifactSha256}.${randomUUID()}.tmp`);
      writeFileSync(temporary, bytes, { flag: "wx", mode: 0o600 });
      if (process.platform !== "win32") chmodSync(temporary, 0o600);
      renameSync(temporary, file);
      temporary = undefined;
      if (process.platform !== "win32") chmodSync(file, 0o600);
      return { status: "stored", receipt };
    } catch {
      if (temporary !== undefined) rmSync(temporary, { force: true });
      if (existsSync(file)) {
        const existing = load({
          artifactRef: artifactRef(artifactSha256),
          artifactSha256,
        });
        if (existing.status === "found" && equalBytes(existing.bytes, bytes)) {
          return { status: "unchanged", receipt };
        }
      }
      return { status: "rejected", reason: "persistence_failed" };
    }
  };

  return { persist, load } as const;
};

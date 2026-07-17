import { createHash } from "node:crypto";

import {
  decodeThreadDisclosureReceipt,
  searchCanonicalThreadEvents,
  ThreadExportArtifact,
  type ThreadDisclosureReceipt,
  type ThreadEventSearchProjection,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

import type { DesktopThreadExportLoadResult } from "./thread-export-artifact-store.ts";

const MAX_RECEIPTS = 1_000;
const MAX_ARTIFACT_BYTES = 4 * 1024 * 1024;
const SHA256 = /^[a-f0-9]{64}$/;
const ARTIFACT_REF_PREFIX = "artifact.thread_export.sha256.";
const decodeArtifact = S.decodeUnknownSync(ThreadExportArtifact);

type CanonicalExportReceipt = ThreadDisclosureReceipt & Readonly<{
  kind: "thread.export.create";
  result: Readonly<{
    status: "export_created";
    artifactRef: string;
    artifactSha256: string;
    format: "canonical_event_bundle";
    artifactAudience: Readonly<{ kind: "owner_only" }>;
  }>;
}>;

export type DesktopThreadEventSearchArtifactSourceDependencies = Readonly<{
  loadArtifact: (input: Readonly<{
    artifactRef: string;
    artifactSha256: string;
  }>) => DesktopThreadExportLoadResult;
}>;

export type DesktopThreadEventSearchArtifactSourceInput = Readonly<{
  receipts: ReadonlyArray<unknown>;
  query: unknown;
  limit?: number;
}>;

export type DesktopThreadEventSearchArtifactSourceResult =
  | Readonly<{ status: "available"; projection: ThreadEventSearchProjection }>
  | Readonly<{
      status: "unavailable";
      reason:
        | "invalid_request"
        | "artifact_unavailable"
        | "artifact_corrupt"
        | "identity_mismatch"
        | "projection_rejected";
    }>;

const sha256 = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex");

const decodeCanonicalReceipt = (raw: unknown): CanonicalExportReceipt | null => {
  try {
    const receipt = decodeThreadDisclosureReceipt(raw);
    if (
      receipt.kind !== "thread.export.create" ||
      receipt.result.status !== "export_created" ||
      receipt.result.format !== "canonical_event_bundle" ||
      receipt.result.artifactAudience.kind !== "owner_only" ||
      !SHA256.test(receipt.result.artifactSha256) ||
      receipt.result.artifactRef !==
        `${ARTIFACT_REF_PREFIX}${receipt.result.artifactSha256}`
    ) {
      return null;
    }
    return {
      schema: receipt.schema,
      receiptRef: receipt.receiptRef,
      intentRef: receipt.intentRef,
      idempotencyKey: receipt.idempotencyKey,
      threadRef: receipt.threadRef,
      observedAt: receipt.observedAt,
      kind: "thread.export.create",
      result: {
        status: "export_created",
        artifactRef: receipt.result.artifactRef,
        artifactSha256: receipt.result.artifactSha256,
        format: "canonical_event_bundle",
        artifactAudience: { kind: "owner_only" },
      },
    };
  } catch {
    return null;
  }
};

type ReceiptGroup = Readonly<{
  artifactRef: string;
  artifactSha256: string;
  receipts: ReadonlyArray<CanonicalExportReceipt>;
}>;

const groupReceipts = (
  raws: ReadonlyArray<unknown>,
): ReadonlyArray<ReceiptGroup> | null => {
  if (raws.length > MAX_RECEIPTS) return null;
  const receiptIdentity = new Map<string, string>();
  const groups = new Map<string, { artifactSha256: string; receipts: CanonicalExportReceipt[] }>();

  for (const raw of raws) {
    const receipt = decodeCanonicalReceipt(raw);
    if (receipt === null || receipt.result.status !== "export_created") return null;
    const encoded = JSON.stringify(receipt);
    const existingReceipt = receiptIdentity.get(receipt.receiptRef);
    if (existingReceipt !== undefined) {
      if (existingReceipt !== encoded) return null;
      continue;
    }
    receiptIdentity.set(receipt.receiptRef, encoded);

    const existing = groups.get(receipt.result.artifactRef);
    if (existing !== undefined) {
      if (existing.artifactSha256 !== receipt.result.artifactSha256) return null;
      existing.receipts.push(receipt);
      continue;
    }
    groups.set(receipt.result.artifactRef, {
      artifactSha256: receipt.result.artifactSha256,
      receipts: [receipt],
    });
  }

  return [...groups.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([artifactRef, group]) => ({ artifactRef, ...group }));
};

const verifiedArtifact = (
  dependencies: DesktopThreadEventSearchArtifactSourceDependencies,
  group: ReceiptGroup,
):
  | Readonly<{ status: "verified"; artifact: typeof ThreadExportArtifact.Type }>
  | Readonly<{
      status: "unavailable";
      reason: "artifact_unavailable" | "artifact_corrupt" | "identity_mismatch";
    }> => {
  const loaded = dependencies.loadArtifact({
    artifactRef: group.artifactRef,
    artifactSha256: group.artifactSha256,
  });
  if (loaded.status !== "found") {
    return {
      status: "unavailable",
      reason: loaded.reason === "missing" ? "artifact_unavailable" : "artifact_corrupt",
    };
  }
  if (
    !(loaded.bytes instanceof Uint8Array) ||
    loaded.bytes.byteLength > MAX_ARTIFACT_BYTES ||
    sha256(loaded.bytes) !== group.artifactSha256
  ) {
    return { status: "unavailable", reason: "artifact_corrupt" };
  }

  let artifact: typeof ThreadExportArtifact.Type;
  try {
    artifact = decodeArtifact(
      JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(loaded.bytes)),
    );
  } catch {
    return { status: "unavailable", reason: "artifact_corrupt" };
  }
  if (
    artifact.format !== "canonical_event_bundle" ||
    artifact.artifactAudience.kind !== "owner_only" ||
    group.receipts.some(
      (receipt) =>
        receipt.intentRef !== artifact.intentRef ||
        receipt.threadRef !== artifact.threadRef ||
        receipt.result.status !== "export_created" ||
        receipt.result.format !== artifact.format ||
        receipt.result.artifactAudience.kind !== artifact.artifactAudience.kind,
    )
  ) {
    return { status: "unavailable", reason: "identity_mismatch" };
  }
  return { status: "verified", artifact };
};

/**
 * Acquire canonical bundles only through verified ref-only export receipts,
 * then delegate search to FF-D1-34. No artifact bytes or store authority are
 * returned to the caller.
 */
export const searchDesktopPersistedCanonicalThreadEvents = (
  dependencies: DesktopThreadEventSearchArtifactSourceDependencies,
  input: DesktopThreadEventSearchArtifactSourceInput,
): DesktopThreadEventSearchArtifactSourceResult => {
  if (typeof input.query !== "string") {
    return { status: "unavailable", reason: "invalid_request" };
  }
  if (input.query.trim() === "") {
    try {
      return {
        status: "available",
        projection: searchCanonicalThreadEvents({
          artifacts: [],
          query: input.query,
          ...(input.limit === undefined ? {} : { limit: input.limit }),
        }),
      };
    } catch {
      return { status: "unavailable", reason: "invalid_request" };
    }
  }

  const groups = groupReceipts(input.receipts);
  if (groups === null) return { status: "unavailable", reason: "invalid_request" };
  const artifacts = [];
  for (const group of groups) {
    const verified = verifiedArtifact(dependencies, group);
    if (verified.status === "unavailable") return verified;
    artifacts.push(verified.artifact);
  }
  try {
    return {
      status: "available",
      projection: searchCanonicalThreadEvents({
        artifacts,
        query: input.query,
        ...(input.limit === undefined ? {} : { limit: input.limit }),
      }),
    };
  } catch {
    return { status: "unavailable", reason: "projection_rejected" };
  }
};

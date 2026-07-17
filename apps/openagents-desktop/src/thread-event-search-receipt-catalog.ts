import { randomUUID } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

const CATALOG_SCHEMA = "openagents.desktop_thread_event_search_receipt_catalog.v1";
const MAX_RECEIPTS = 1_000;
const MAX_CATALOG_BYTES = 1024 * 1024;
const SHA256 = /^[a-f0-9]{64}$/;
const ARTIFACT_REF_PREFIX = "artifact.thread_export.sha256.";

const CatalogEnvelope = S.Struct({
  schema: S.Literal(CATALOG_SCHEMA),
  receipts: S.Array(S.Unknown),
});
const decodeCatalogEnvelope = S.decodeUnknownSync(CatalogEnvelope);

type CanonicalExportReceipt = ThreadDisclosureReceipt &
  Readonly<{
    kind: "thread.export.create";
    result: Readonly<{
      status: "export_created";
      artifactRef: string;
      artifactSha256: string;
      format: "canonical_event_bundle";
      artifactAudience: Readonly<{ kind: "owner_only" }>;
    }>;
  }>;

export type DesktopThreadEventSearchReceiptCatalogRecordResult =
  | Readonly<{ status: "stored" | "unchanged"; receiptCount: number }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_receipt"
        | "conflicting_identity"
        | "capacity_exceeded"
        | "corrupt_catalog"
        | "persistence_failed";
    }>;

export type DesktopThreadEventSearchReceiptCatalogListResult =
  | Readonly<{ status: "available"; receipts: ReadonlyArray<ThreadDisclosureReceipt> }>
  | Readonly<{ status: "rejected"; reason: "corrupt_catalog" }>;

const canonicalReceipt = (raw: unknown): CanonicalExportReceipt | null => {
  try {
    const receipt = decodeThreadDisclosureReceipt(raw);
    if (
      receipt.kind !== "thread.export.create" ||
      receipt.result.status !== "export_created" ||
      receipt.result.format !== "canonical_event_bundle" ||
      receipt.result.artifactAudience.kind !== "owner_only" ||
      !SHA256.test(receipt.result.artifactSha256) ||
      receipt.result.artifactRef !== `${ARTIFACT_REF_PREFIX}${receipt.result.artifactSha256}`
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

const sameReceipt = (left: CanonicalExportReceipt, right: CanonicalExportReceipt): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

const identityConflict = (
  existing: CanonicalExportReceipt,
  incoming: CanonicalExportReceipt,
): boolean =>
  existing.receiptRef === incoming.receiptRef ||
  existing.intentRef === incoming.intentRef ||
  existing.idempotencyKey === incoming.idempotencyKey ||
  existing.result.artifactRef === incoming.result.artifactRef;

const isExactEnvelope = (value: unknown): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return Object.keys(value).sort().join(",") === "receipts,schema";
};

type CatalogLoad =
  | Readonly<{ status: "available"; receipts: ReadonlyArray<CanonicalExportReceipt> }>
  | Readonly<{ status: "missing" }>
  | Readonly<{ status: "corrupt" }>;

const loadCatalog = (file: string): CatalogLoad => {
  let bytes: Uint8Array;
  try {
    bytes = readFileSync(file);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return { status: "missing" };
    }
    return { status: "corrupt" };
  }
  if (bytes.byteLength > MAX_CATALOG_BYTES) return { status: "corrupt" };

  try {
    const parsed: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
    if (!isExactEnvelope(parsed)) return { status: "corrupt" };
    const envelope = decodeCatalogEnvelope(parsed);
    if (envelope.receipts.length > MAX_RECEIPTS) return { status: "corrupt" };

    const receipts: CanonicalExportReceipt[] = [];
    for (const raw of envelope.receipts) {
      const receipt = canonicalReceipt(raw);
      if (
        receipt === null ||
        receipts.some(
          (existing) => identityConflict(existing, receipt) || sameReceipt(existing, receipt),
        )
      ) {
        return { status: "corrupt" };
      }
      receipts.push(receipt);
    }
    receipts.sort((left, right) => left.receiptRef.localeCompare(right.receiptRef));
    return { status: "available", receipts };
  } catch {
    return { status: "corrupt" };
  }
};

const encodedCatalog = (receipts: ReadonlyArray<CanonicalExportReceipt>): Uint8Array =>
  new TextEncoder().encode(JSON.stringify({ schema: CATALOG_SCHEMA, receipts }));

/**
 * Owner-private, main-process receipt catalog. It persists only decoded,
 * ref-only evidence and never returns its file path or any artifact bytes.
 */
export const openDesktopThreadEventSearchReceiptCatalog = (directory: string) => {
  const file = path.join(directory, "canonical-export-receipts.json");

  const list = (): DesktopThreadEventSearchReceiptCatalogListResult => {
    const loaded = loadCatalog(file);
    if (loaded.status === "corrupt") return { status: "rejected", reason: "corrupt_catalog" };
    return {
      status: "available",
      receipts: loaded.status === "missing" ? [] : loaded.receipts,
    };
  };

  const record = (raw: unknown): DesktopThreadEventSearchReceiptCatalogRecordResult => {
    const incoming = canonicalReceipt(raw);
    if (incoming === null) return { status: "rejected", reason: "invalid_receipt" };

    const loaded = loadCatalog(file);
    if (loaded.status === "corrupt") return { status: "rejected", reason: "corrupt_catalog" };
    const receipts = loaded.status === "missing" ? [] : [...loaded.receipts];
    const existing = receipts.find((receipt) => identityConflict(receipt, incoming));
    if (existing !== undefined) {
      return sameReceipt(existing, incoming)
        ? { status: "unchanged", receiptCount: receipts.length }
        : { status: "rejected", reason: "conflicting_identity" };
    }
    if (receipts.length >= MAX_RECEIPTS) {
      return { status: "rejected", reason: "capacity_exceeded" };
    }

    receipts.push(incoming);
    receipts.sort((left, right) => left.receiptRef.localeCompare(right.receiptRef));
    const encoded = encodedCatalog(receipts);
    if (encoded.byteLength > MAX_CATALOG_BYTES) {
      return { status: "rejected", reason: "capacity_exceeded" };
    }

    let temporary: string | undefined;
    try {
      mkdirSync(directory, { recursive: true, mode: 0o700 });
      if (process.platform !== "win32") chmodSync(directory, 0o700);
      temporary = path.join(directory, `.canonical-export-receipts.${randomUUID()}.tmp`);
      writeFileSync(temporary, encoded, { flag: "wx", mode: 0o600 });
      if (process.platform !== "win32") chmodSync(temporary, 0o600);
      renameSync(temporary, file);
      temporary = undefined;
      if (process.platform !== "win32") chmodSync(file, 0o600);
      return { status: "stored", receiptCount: receipts.length };
    } catch {
      if (temporary !== undefined) rmSync(temporary, { force: true });
      return { status: "rejected", reason: "persistence_failed" };
    }
  };

  return { record, list } as const;
};

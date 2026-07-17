import {
  compileThreadExportArtifact,
  decodeThreadDisclosureIntent,
  type ThreadDisclosureIntent,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";

import type {
  DesktopThreadExportPersistRequest,
  DesktopThreadExportPersistResult,
} from "./thread-export-artifact-store.ts";

export type DesktopThreadExportEvidenceSnapshot =
  | Readonly<{
      status: "available";
      threadRef: string;
      events: ReadonlyArray<unknown>;
      relations: ReadonlyArray<unknown>;
    }>
  | Readonly<{ status: "unavailable" }>;

export type DesktopThreadExportCommandDependencies = Readonly<{
  readEvidence: (threadRef: string) => Promise<unknown>;
  persist: (request: DesktopThreadExportPersistRequest) => DesktopThreadExportPersistResult;
  makeReceiptRef: (
    intent: Extract<ThreadDisclosureIntent, { kind: "thread.export.create" }>,
  ) => string;
  observedAt: () => string;
  sha256: (bytes: Uint8Array) => string;
}>;

const MAX_EVENTS = 1_000;
const MAX_RELATIONS = 2_000;

const evidenceField = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const decodeEvidenceSnapshot = (
  input: unknown,
): DesktopThreadExportEvidenceSnapshot | undefined => {
  const status = evidenceField(input, "status");
  if (status === "unavailable") return { status };
  const threadRef = evidenceField(input, "threadRef");
  const events = evidenceField(input, "events");
  const relations = evidenceField(input, "relations");
  if (
    status !== "available" ||
    typeof threadRef !== "string" ||
    !Array.isArray(events) ||
    !Array.isArray(relations) ||
    events.length > MAX_EVENTS ||
    relations.length > MAX_RELATIONS
  ) {
    return undefined;
  }
  return { status, threadRef, events, relations };
};

export type DesktopThreadExportCommandResult =
  | Readonly<{
      status: "stored" | "unchanged";
      receipt: ThreadDisclosureReceipt;
    }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_intent"
        | "unsupported_export"
        | "evidence_unavailable"
        | "evidence_thread_mismatch"
        | "invalid_evidence"
        | "host_metadata_invalid"
        | "persistence_refused"
        | "existing_artifact_conflict"
        | "persistence_failed";
    }>;

const requireOwnerOnlyCanonicalExport = (
  input: unknown,
):
  | Readonly<{
      status: "valid";
      intent: Extract<ThreadDisclosureIntent, { kind: "thread.export.create" }>;
    }>
  | Readonly<{ status: "rejected"; reason: "invalid_intent" | "unsupported_export" }> => {
  let intent: ThreadDisclosureIntent;
  try {
    intent = decodeThreadDisclosureIntent(input);
  } catch {
    return { status: "rejected", reason: "invalid_intent" };
  }
  if (
    intent.kind !== "thread.export.create" ||
    intent.format !== "canonical_event_bundle" ||
    intent.artifactAudience.kind !== "owner_only"
  ) {
    return { status: "rejected", reason: "unsupported_export" };
  }
  return { status: "valid", intent };
};

const mapPersistenceRejection = (
  reason: Extract<DesktopThreadExportPersistResult, { status: "rejected" }>["reason"],
): Extract<DesktopThreadExportCommandResult, { status: "rejected" }> => {
  if (reason === "invalid_request") {
    return { status: "rejected", reason: "host_metadata_invalid" };
  }
  if (reason === "existing_artifact_conflict" || reason === "persistence_failed") {
    return { status: "rejected", reason };
  }
  return { status: "rejected", reason: "persistence_refused" };
};

/**
 * Main-process command coordinator. The caller supplies only a disclosure
 * intent; canonical event data and authority remain behind a host-owned source.
 */
export const openDesktopThreadExportCommand = (
  dependencies: DesktopThreadExportCommandDependencies,
) => {
  const execute = async (input: unknown): Promise<DesktopThreadExportCommandResult> => {
    const decoded = requireOwnerOnlyCanonicalExport(input);
    if (decoded.status === "rejected") return decoded;

    let rawEvidence: unknown;
    try {
      rawEvidence = await dependencies.readEvidence(decoded.intent.threadRef);
    } catch {
      return { status: "rejected", reason: "evidence_unavailable" };
    }
    const evidence = decodeEvidenceSnapshot(rawEvidence);
    if (evidence === undefined) {
      return { status: "rejected", reason: "invalid_evidence" };
    }
    if (evidence.status === "unavailable") {
      return { status: "rejected", reason: "evidence_unavailable" };
    }
    if (evidence.threadRef !== decoded.intent.threadRef) {
      return { status: "rejected", reason: "evidence_thread_mismatch" };
    }

    let digestAttempted = false;
    let compilation: ReturnType<typeof compileThreadExportArtifact>;
    try {
      compilation = compileThreadExportArtifact({
        intent: decoded.intent,
        events: evidence.events,
        relations: evidence.relations,
        sha256: (bytes) => {
          digestAttempted = true;
          return dependencies.sha256(bytes);
        },
      });
    } catch {
      return {
        status: "rejected",
        reason: digestAttempted ? "host_metadata_invalid" : "invalid_evidence",
      };
    }

    let receiptRef: string;
    let observedAt: string;
    try {
      receiptRef = dependencies.makeReceiptRef(decoded.intent);
      observedAt = dependencies.observedAt();
    } catch {
      return { status: "rejected", reason: "host_metadata_invalid" };
    }
    const persisted = dependencies.persist({
      intent: decoded.intent,
      compilation,
      receiptRef,
      observedAt,
    });
    return persisted.status === "rejected" ? mapPersistenceRejection(persisted.reason) : persisted;
  };

  return { execute } as const;
};

import { chmodSync, existsSync, linkSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { randomUUID } from "node:crypto";
import path from "node:path";

import {
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";

import type { DesktopThreadExportLoadResult } from "./thread-export-artifact-store.ts";

export type DesktopThreadExportFileTransportDependencies = Readonly<{
  load: (
    input: Readonly<{ artifactRef: string; artifactSha256: string }>,
  ) => DesktopThreadExportLoadResult;
  selectDestination: (input: Readonly<{ suggestedName: string }>) => Promise<unknown>;
}>;

export type DesktopThreadExportFileTransportResult =
  | Readonly<{ status: "cancelled" }>
  | Readonly<{
      status: "written";
      artifactRef: string;
      artifactSha256: string;
      replaceAuthorized: boolean;
    }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_receipt"
        | "unsupported_export"
        | "artifact_missing"
        | "artifact_corrupt"
        | "destination_unavailable"
        | "destination_invalid"
        | "destination_exists"
        | "write_failed";
    }>;

type ExportReceipt = Readonly<{
  threadRef: string;
  result: Extract<ThreadDisclosureReceipt["result"], { status: "export_created" }>;
}>;

type SelectedDestination = Readonly<{
  status: "selected";
  filePath: string;
  replaceExisting: boolean;
}>;

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const decodeExportReceipt = (
  input: unknown,
):
  | Readonly<{ status: "valid"; receipt: ExportReceipt }>
  | Readonly<{ status: "rejected"; reason: "invalid_receipt" | "unsupported_export" }> => {
  let receipt: ThreadDisclosureReceipt;
  try {
    receipt = decodeThreadDisclosureReceipt(input);
  } catch {
    return { status: "rejected", reason: "invalid_receipt" };
  }
  if (
    receipt.kind !== "thread.export.create" ||
    receipt.result.status !== "export_created" ||
    receipt.result.format !== "canonical_event_bundle" ||
    receipt.result.artifactAudience.kind !== "owner_only"
  ) {
    return { status: "rejected", reason: "unsupported_export" };
  }
  return { status: "valid", receipt: { threadRef: receipt.threadRef, result: receipt.result } };
};

const decodeDestination = (
  input: unknown,
): SelectedDestination | Readonly<{ status: "cancelled" }> | undefined => {
  const status = field(input, "status");
  if (status === "cancelled") return { status };
  const filePath = field(input, "filePath");
  const replaceExisting = field(input, "replaceExisting");
  if (
    status !== "selected" ||
    typeof filePath !== "string" ||
    typeof replaceExisting !== "boolean" ||
    filePath.length === 0 ||
    filePath.length > 4_096 ||
    filePath.includes("\0") ||
    !path.isAbsolute(filePath) ||
    path.extname(filePath).toLowerCase() !== ".json"
  ) {
    return undefined;
  }
  return { status, filePath, replaceExisting };
};

const suggestedName = (receipt: ExportReceipt): string => {
  const thread = receipt.threadRef.replace(/[^A-Za-z0-9._-]/g, "-").slice(0, 80);
  return `openagents-${thread}-${receipt.result.artifactSha256.slice(0, 12)}.json`;
};

const isAlreadyExists = (error: unknown): boolean =>
  error instanceof Error && "code" in error && error.code === "EEXIST";

/**
 * Main-process-only local file transport. Destination paths and artifact bytes
 * remain inside this module; the result is ref-only.
 */
export const openDesktopThreadExportFileTransport = (
  dependencies: DesktopThreadExportFileTransportDependencies,
) => {
  const write = async (input: unknown): Promise<DesktopThreadExportFileTransportResult> => {
    const decoded = decodeExportReceipt(input);
    if (decoded.status === "rejected") return decoded;

    const loaded = dependencies.load({
      artifactRef: decoded.receipt.result.artifactRef,
      artifactSha256: decoded.receipt.result.artifactSha256,
    });
    if (loaded.status === "rejected") {
      return {
        status: "rejected",
        reason: loaded.reason === "missing" ? "artifact_missing" : "artifact_corrupt",
      };
    }

    let rawDestination: unknown;
    try {
      rawDestination = await dependencies.selectDestination({
        suggestedName: suggestedName(decoded.receipt),
      });
    } catch {
      return { status: "rejected", reason: "destination_unavailable" };
    }
    const destination = decodeDestination(rawDestination);
    if (destination === undefined) {
      return { status: "rejected", reason: "destination_invalid" };
    }
    if (destination.status === "cancelled") return destination;
    if (!destination.replaceExisting && existsSync(destination.filePath)) {
      return { status: "rejected", reason: "destination_exists" };
    }

    const temporary = path.join(
      path.dirname(destination.filePath),
      `.${path.basename(destination.filePath)}.${randomUUID()}.tmp`,
    );
    let exclusiveDestinationCreated = false;
    try {
      writeFileSync(temporary, loaded.bytes, { flag: "wx", mode: 0o600 });
      if (process.platform !== "win32") chmodSync(temporary, 0o600);
      if (destination.replaceExisting) {
        renameSync(temporary, destination.filePath);
      } else {
        linkSync(temporary, destination.filePath);
        exclusiveDestinationCreated = true;
        rmSync(temporary);
      }
      return {
        status: "written",
        artifactRef: decoded.receipt.result.artifactRef,
        artifactSha256: decoded.receipt.result.artifactSha256,
        replaceAuthorized: destination.replaceExisting,
      };
    } catch (error) {
      rmSync(temporary, { force: true });
      if (exclusiveDestinationCreated) rmSync(destination.filePath, { force: true });
      return {
        status: "rejected",
        reason:
          !destination.replaceExisting && isAlreadyExists(error)
            ? "destination_exists"
            : "write_failed",
      };
    }
  };

  return { write } as const;
};

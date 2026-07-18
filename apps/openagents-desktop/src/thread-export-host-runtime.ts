import { Effect } from "effect";

import {
  openDesktopThreadExportArtifactStore,
  type DesktopThreadExportPersistResult,
} from "./thread-export-artifact-store.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";
import {
  openDesktopThreadExportCommandFromConfirmedTimeline,
  type DesktopThreadExportConfirmedTimelineCommandDependencies,
} from "./thread-export-confirmed-timeline-command.ts";
import {
  openDesktopThreadExportFileTransport,
  type DesktopThreadExportFileTransportDependencies,
} from "./thread-export-file-transport.ts";
import {
  openDesktopThreadExportMainComposition,
  type DesktopThreadExportMainCompositionDependencies,
} from "./thread-export-main-composition.ts";

export type DesktopThreadExportHostRuntimeDependencies = Readonly<{
  storeDirectory: string;
  receiptCatalogDirectory: string;
  authorityLedgerDirectory: string;
  snapshotForThread: DesktopThreadExportConfirmedTimelineCommandDependencies["snapshotForThread"];
  selectDestination: DesktopThreadExportFileTransportDependencies["selectDestination"];
  registerWrite: DesktopThreadExportMainCompositionDependencies["registerWrite"];
  registerCreate: DesktopThreadExportMainCompositionDependencies["registerCreate"];
  isTrustedSender: DesktopThreadExportMainCompositionDependencies["isTrustedSender"];
  makeReceiptRef: DesktopThreadExportConfirmedTimelineCommandDependencies["makeReceiptRef"];
  observedAt: DesktopThreadExportConfirmedTimelineCommandDependencies["observedAt"];
  sha256: DesktopThreadExportConfirmedTimelineCommandDependencies["sha256"];
}>;

const sameCatalogOperation = (
  left: Parameters<ReturnType<typeof openDesktopThreadEventSearchReceiptCatalog>["record"]>[0],
  right: Parameters<ReturnType<typeof openDesktopThreadEventSearchReceiptCatalog>["record"]>[0],
): boolean => {
  if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) {
    return false;
  }
  const leftIntent = Reflect.get(left, "intentRef");
  const rightIntent = Reflect.get(right, "intentRef");
  const leftIdempotency = Reflect.get(left, "idempotencyKey");
  const rightIdempotency = Reflect.get(right, "idempotencyKey");
  return (
    (typeof leftIntent === "string" && leftIntent === rightIntent) ||
    (typeof leftIdempotency === "string" && leftIdempotency === rightIdempotency)
  );
};

const sameExportIdentity = (
  left: Extract<DesktopThreadExportPersistResult, { status: "stored" | "unchanged" }>["receipt"],
  right: Extract<DesktopThreadExportPersistResult, { status: "stored" | "unchanged" }>["receipt"],
): boolean =>
  left.intentRef === right.intentRef &&
  left.idempotencyKey === right.idempotencyKey &&
  left.threadRef === right.threadRef &&
  left.kind === right.kind &&
  left.result.status === "export_created" &&
  right.result.status === "export_created" &&
  left.result.artifactRef === right.result.artifactRef &&
  left.result.artifactSha256 === right.result.artifactSha256 &&
  left.result.format === right.result.format &&
  left.result.artifactAudience.kind === right.result.artifactAudience.kind;

/**
 * Acquire the complete owner-only canonical-export main-process graph behind
 * one close-only lifetime. Store bytes, destination paths, and host authority
 * remain inside the composed dependencies and never enter the returned value.
 */
export const openDesktopThreadExportHostRuntime = Effect.fn("DesktopThreadExportHostRuntime.open")(
  function* (dependencies: DesktopThreadExportHostRuntimeDependencies) {
    const store = openDesktopThreadExportArtifactStore(dependencies.storeDirectory);
    const receiptCatalog = openDesktopThreadEventSearchReceiptCatalog(
      dependencies.receiptCatalogDirectory,
    );
    const command = openDesktopThreadExportCommandFromConfirmedTimeline({
      snapshotForThread: dependencies.snapshotForThread,
      authorityLedgerDirectory: dependencies.authorityLedgerDirectory,
      persist: (request): DesktopThreadExportPersistResult => {
        const listed = receiptCatalog.list();
        if (listed.status === "rejected") {
          return { status: "rejected", reason: "persistence_failed" };
        }
        const prior = listed.receipts.find((receipt) =>
          sameCatalogOperation(receipt, request.intent),
        );
        const persisted = store.persist(request);
        if (persisted.status === "rejected") return persisted;
        if (prior !== undefined) {
          return sameExportIdentity(prior, persisted.receipt)
            ? { status: "unchanged", receipt: prior }
            : { status: "rejected", reason: "existing_artifact_conflict" };
        }
        const recorded = receiptCatalog.record(persisted.receipt);
        return recorded.status === "rejected"
          ? { status: "rejected", reason: "persistence_failed" }
          : persisted;
      },
      makeReceiptRef: dependencies.makeReceiptRef,
      observedAt: dependencies.observedAt,
      sha256: dependencies.sha256,
    });
    const transport = openDesktopThreadExportFileTransport({
      load: store.load,
      selectDestination: dependencies.selectDestination,
    });

    return yield* openDesktopThreadExportMainComposition({
      registerWrite: dependencies.registerWrite,
      registerCreate: dependencies.registerCreate,
      isTrustedSender: dependencies.isTrustedSender,
      write: transport.write,
      execute: command.execute,
    });
  },
);

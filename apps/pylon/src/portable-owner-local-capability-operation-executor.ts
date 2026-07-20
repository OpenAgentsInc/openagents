import { createHash } from "node:crypto";

import type { SecretMaterial } from "@openagentsinc/portable-session-contract";
import type { OwnerLocalPortableCapabilityInstallationPort } from "@openagentsinc/khala-sync-server/portable-capability-installation-ports";

import {
  capabilityMaterialRequest,
  type PylonPortableOwnerLocalCapabilityMaterialClient,
} from "./portable-owner-local-capability-material-client.js";
import type { PylonPortableOwnerLocalCapabilityExecutor } from "./portable-owner-local-capability-operation-worker.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const receiptRef = (action: "install" | "wipe", operationRef: string): string =>
  `receipt.pylon.portable-capability-${action}.${createHash("sha256").update(operationRef).digest("hex").slice(0, 32)}`;

export type MakePylonPortableOwnerLocalCapabilityOperationExecutorOptions = Readonly<{
  materialClient: PylonPortableOwnerLocalCapabilityMaterialClient;
  installationPort: Pick<OwnerLocalPortableCapabilityInstallationPort, "install" | "wipe">;
}>;

/**
 * Compose the refs-only queue worker with owner-local custody. Material exists
 * only between redemption and the installation call and is always cleared.
 */
export const makePylonPortableOwnerLocalCapabilityOperationExecutor = (
  options: MakePylonPortableOwnerLocalCapabilityOperationExecutorOptions,
): PylonPortableOwnerLocalCapabilityExecutor => ({
  recoverySemantics: async () => "operation_ref_idempotent",
  execute: async (request, claim, signal) => {
    if (signal.aborted) throw signal.reason;
    if (request.action === "wipe") {
      if (
        request.capability !== null ||
        request.installationRef === null ||
        request.permissionRefs.length !== 0 ||
        !SAFE_REF.test(request.installationRef)
      )
        throw new Error("owner-local capability wipe scope is invalid");
      const wiped = await options.installationPort.wipe({
        leaseRef: request.sourceLeaseRef,
        targetRef: request.targetRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: request.attachmentGeneration,
        installationRef: request.installationRef,
      });
      if (!SAFE_REF.test(wiped.wipeReceiptRef))
        throw new Error("owner-local capability wipe receipt is invalid");
      return {
        outcome: {
          status: "completed",
          resultInstallationRef: null,
          receiptRef: wiped.wipeReceiptRef,
          evidenceRefs: [],
          errorRef: null,
        },
      };
    }

    if (
      request.capability === null ||
      request.installationRef !== null ||
      request.permissionRefs.length === 0
    )
      throw new Error("owner-local capability install scope is invalid");
    const material = await options.materialClient.redeem(
      capabilityMaterialRequest(request, claim),
      signal,
    );
    try {
      if (signal.aborted) throw signal.reason;
      const installed = await options.installationPort.install({
        lease: {
          leaseRef: request.destinationLeaseRef,
          ownerRef: request.ownerRef,
          sessionRef: request.sessionRef,
          attachmentRef: request.attachmentRef,
          attachmentGeneration: request.attachmentGeneration,
          targetRef: request.targetRef,
          capability: request.capability,
          expiresAt: request.expiresAt,
          state: "issued",
        },
        permissions: request.permissionRefs,
        material: material as SecretMaterial,
      });
      if (!SAFE_REF.test(installed.installationRef) || !SAFE_REF.test(installed.evidenceRef))
        throw new Error("owner-local capability installation result is invalid");
      return {
        outcome: {
          status: "completed",
          resultInstallationRef: installed.installationRef,
          receiptRef: receiptRef("install", request.operationRef),
          evidenceRefs: [installed.evidenceRef],
          errorRef: null,
        },
      };
    } finally {
      material.fill(0);
    }
  },
});

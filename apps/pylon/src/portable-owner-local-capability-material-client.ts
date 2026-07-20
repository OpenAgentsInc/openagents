import { Schema } from "effect";

import type { PylonPortableOwnerLocalCapabilityExecutionClaim } from "./portable-owner-local-capability-operation-worker.js";
import type { PortableOwnerLocalCapabilityOperationRequest } from "@openagentsinc/portable-session-contract";

const MAX_MATERIAL_BYTES = 1024 * 1024;
const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const Ref = Schema.String.check(Schema.isPattern(SAFE_REF));

const MaterialRequestSchema = Schema.Struct({
  schema: Schema.Literal("openagents.portable_owner_local_capability_operation.v1"),
  operationRef: Ref,
  commandExecutionClaimRef: Ref,
  claimRef: Ref,
  pylonRef: Ref,
  targetRef: Ref,
  sessionRef: Ref,
  attachmentRef: Ref,
  attachmentGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  workerInstanceRef: Ref,
  claimGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  expectedLeaseRevision: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  expectedLeaseExpiresAt: Schema.String,
  destinationGrantRef: Ref,
});
const decodeMaterialRequest = Schema.decodeUnknownSync(MaterialRequestSchema);

export type PylonPortableOwnerLocalCapabilityMaterialRequest = typeof MaterialRequestSchema.Type;

export class PylonPortableOwnerLocalCapabilityMaterialError extends Error {
  override readonly name = "PylonPortableOwnerLocalCapabilityMaterialError";
  constructor(
    readonly reason:
      | "bad_response"
      | "cancelled"
      | "invalid_request"
      | "network_failed"
      | "not_authorized"
      | "unavailable",
  ) {
    super(`Pylon capability material redemption failed: ${reason}`);
  }
}

export type PylonPortableOwnerLocalCapabilityMaterialClient = Readonly<{
  redeem: (
    request: PylonPortableOwnerLocalCapabilityMaterialRequest,
    signal: AbortSignal,
  ) => Promise<Uint8Array>;
}>;

export type MakePylonPortableOwnerLocalCapabilityMaterialClientOptions = Readonly<{
  agentToken: string;
  baseUrl: string;
  pylonRef: string;
  targetRef: string;
  fetchImpl?: typeof globalThis.fetch;
  requestTimeoutMs?: number;
}>;

const failure = (reason: PylonPortableOwnerLocalCapabilityMaterialError["reason"]) =>
  new PylonPortableOwnerLocalCapabilityMaterialError(reason);

const baseUrl = (value: string): URL => {
  try {
    const parsed = new URL(value);
    const loopback =
      parsed.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname);
    if (
      (parsed.protocol !== "https:" && !loopback) ||
      parsed.username !== "" ||
      parsed.password !== ""
    )
      throw failure("invalid_request");
    return parsed;
  } catch (error) {
    if (error instanceof PylonPortableOwnerLocalCapabilityMaterialError) throw error;
    throw failure("invalid_request");
  }
};

export const capabilityMaterialRequest = (
  request: PortableOwnerLocalCapabilityOperationRequest,
  claim: PylonPortableOwnerLocalCapabilityExecutionClaim,
): PylonPortableOwnerLocalCapabilityMaterialRequest =>
  decodeMaterialRequest(
    {
      schema: request.schema,
      operationRef: request.operationRef,
      commandExecutionClaimRef: request.commandExecutionClaimRef,
      claimRef: claim.claimRef,
      pylonRef: request.pylonRef,
      targetRef: request.targetRef,
      sessionRef: request.sessionRef,
      attachmentRef: request.attachmentRef,
      attachmentGeneration: request.attachmentGeneration,
      workerInstanceRef: claim.workerInstanceRef,
      claimGeneration: claim.claimGeneration,
      expectedLeaseRevision: claim.expectedLeaseRevision,
      expectedLeaseExpiresAt: claim.expectedLeaseExpiresAt,
      destinationGrantRef: request.destinationGrantRef,
    },
    { onExcessProperty: "error" },
  );

export const makePylonPortableOwnerLocalCapabilityMaterialClient = (
  options: MakePylonPortableOwnerLocalCapabilityMaterialClientOptions,
): PylonPortableOwnerLocalCapabilityMaterialClient => {
  const origin = baseUrl(options.baseUrl);
  if (
    !SAFE_REF.test(options.pylonRef) ||
    !SAFE_REF.test(options.targetRef) ||
    options.agentToken.trim() === ""
  )
    throw failure("invalid_request");
  const timeoutMs = options.requestTimeoutMs ?? 15_000;
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1_000 || timeoutMs > 60_000)
    throw failure("invalid_request");
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;

  return {
    redeem: async (input, signal) => {
      const exact = decodeMaterialRequest(input, {
        onExcessProperty: "error",
      });
      if (exact.pylonRef !== options.pylonRef || exact.targetRef !== options.targetRef)
        throw failure("invalid_request");
      const path = `/api/pylons/${encodeURIComponent(options.pylonRef)}/portable-targets/${encodeURIComponent(options.targetRef)}/capability-operations/${encodeURIComponent(exact.operationRef)}/material`;
      let response: Response;
      try {
        response = await fetchImpl(new URL(path, origin), {
          method: "POST",
          headers: {
            Accept: "application/octet-stream",
            Authorization: `Bearer ${options.agentToken}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify(exact),
          signal: AbortSignal.any([signal, AbortSignal.timeout(timeoutMs)]),
        });
      } catch {
        if (signal.aborted) throw failure("cancelled");
        throw failure("network_failed");
      }
      if (!response.ok) {
        if ([401, 403, 404, 409].includes(response.status)) throw failure("not_authorized");
        throw failure("unavailable");
      }
      if (
        response.headers.get("content-type") !== "application/octet-stream" ||
        !response.headers
          .get("cache-control")
          ?.toLowerCase()
          .split(",")
          .map((value) => value.trim())
          .includes("no-store")
      )
        throw failure("bad_response");
      const contentLength = response.headers.get("content-length");
      if (contentLength !== null) {
        const declared = Number(contentLength);
        if (!Number.isSafeInteger(declared) || declared < 1 || declared > MAX_MATERIAL_BYTES)
          throw failure("bad_response");
      }
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.byteLength < 1 || bytes.byteLength > MAX_MATERIAL_BYTES) {
        bytes.fill(0);
        throw failure("bad_response");
      }
      return bytes;
    },
  };
};

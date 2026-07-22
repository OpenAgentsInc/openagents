import {
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  PortableOwnerLocalCapabilityOperationRecordSchema,
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
  type PortableOwnerLocalCapabilityOperationClaimRequest,
  type PortableOwnerLocalCapabilityOperationRecord,
  type PortableOwnerLocalCapabilityOperationRenewRequest,
  type PortableOwnerLocalCapabilityOperationResultRequest,
} from "@openagentsinc/portable-session-contract";
import { Schema as S } from "effect";

const RESPONSE_SCHEMA =
  "openagents.portable_owner_local_capability_operation_transport.v1" as const;
const MAX_RESPONSE_BYTES = 512 * 1_024;
const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

const PendingResponse = S.Struct({
  schema: S.Literal(RESPONSE_SCHEMA),
  operations: S.Array(PortableOwnerLocalCapabilityOperationRecordSchema),
});
const ReconcileResponse = S.Struct({
  schema: S.Literal(RESPONSE_SCHEMA),
  operation: PortableOwnerLocalCapabilityOperationRecordSchema,
  status: S.Literal("reconciled"),
});
const ClaimResponse = S.Struct({
  schema: S.Literal(RESPONSE_SCHEMA),
  operation: PortableOwnerLocalCapabilityOperationRecordSchema,
  status: S.Literals(["claimed", "replayed"]),
});
const RenewResponse = S.Struct({
  schema: S.Literal(RESPONSE_SCHEMA),
  operation: PortableOwnerLocalCapabilityOperationRecordSchema,
  status: S.Literals(["renewed", "replayed"]),
});
const CompleteResponse = S.Struct({
  schema: S.Literal(RESPONSE_SCHEMA),
  operation: PortableOwnerLocalCapabilityOperationRecordSchema,
  status: S.Literals(["completed", "failed", "replayed"]),
});

export type PylonPortableOwnerLocalCapabilityTransportFailure =
  | "bad_response"
  | "cancelled"
  | "claim_conflict"
  | "invalid_request"
  | "network_failed"
  | "not_authorized"
  | "unavailable";

export class PylonPortableOwnerLocalCapabilityTransportError extends Error {
  override readonly name = "PylonPortableOwnerLocalCapabilityTransportError";

  constructor(readonly failure: PylonPortableOwnerLocalCapabilityTransportFailure) {
    super(`Pylon portable owner-local capability transport failed: ${failure}`);
  }
}

export type PylonPortableOwnerLocalCapabilityOperationClient = Readonly<{
  pending: (
    limit: number,
    signal?: AbortSignal,
  ) => Promise<ReadonlyArray<PortableOwnerLocalCapabilityOperationRecord>>;
  read: (
    operationRef: string,
    signal?: AbortSignal,
  ) => Promise<PortableOwnerLocalCapabilityOperationRecord>;
  claim: (
    request: PortableOwnerLocalCapabilityOperationClaimRequest,
    signal?: AbortSignal,
  ) => Promise<
    Readonly<{
      operation: PortableOwnerLocalCapabilityOperationRecord;
      status: "claimed" | "replayed";
    }>
  >;
  renew: (
    request: PortableOwnerLocalCapabilityOperationRenewRequest,
    signal?: AbortSignal,
  ) => Promise<
    Readonly<{
      operation: PortableOwnerLocalCapabilityOperationRecord;
      status: "renewed" | "replayed";
    }>
  >;
  complete: (
    request: PortableOwnerLocalCapabilityOperationResultRequest,
    signal?: AbortSignal,
  ) => Promise<
    Readonly<{
      operation: PortableOwnerLocalCapabilityOperationRecord;
      status: "completed" | "failed" | "replayed";
    }>
  >;
}>;

export type MakePylonPortableOwnerLocalCapabilityOperationClientOptions = Readonly<{
  agentToken: string;
  baseUrl: string;
  pylonRef: string;
  targetRef: string;
  fetchImpl?: typeof globalThis.fetch;
  requestTimeoutMs?: number;
}>;

const transportError = (failure: PylonPortableOwnerLocalCapabilityTransportFailure) =>
  new PylonPortableOwnerLocalCapabilityTransportError(failure);

const validateBaseUrl = (value: string): URL => {
  try {
    const parsed = new URL(value);
    const loopbackHttp =
      parsed.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname);
    if (
      (parsed.protocol !== "https:" && !loopbackHttp) ||
      parsed.username !== "" ||
      parsed.password !== ""
    ) {
      throw transportError("invalid_request");
    }
    return parsed;
  } catch (error) {
    if (error instanceof PylonPortableOwnerLocalCapabilityTransportError) throw error;
    throw transportError("invalid_request");
  }
};

const mapStatus = (status: number): PylonPortableOwnerLocalCapabilityTransportError => {
  if (status === 400 || status === 405) return transportError("invalid_request");
  if (status === 401 || status === 403 || status === 404) return transportError("not_authorized");
  if (status === 409 || status === 410) return transportError("claim_conflict");
  return transportError("unavailable");
};

const readBoundedText = async (response: Response): Promise<string> => {
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > MAX_RESPONSE_BYTES)
    throw transportError("bad_response");
  const body = await response.text();
  if (new TextEncoder().encode(body).byteLength > MAX_RESPONSE_BYTES)
    throw transportError("bad_response");
  return body;
};

export const makePylonPortableOwnerLocalCapabilityOperationClient = (
  options: MakePylonPortableOwnerLocalCapabilityOperationClientOptions,
): PylonPortableOwnerLocalCapabilityOperationClient => {
  const baseUrl = validateBaseUrl(options.baseUrl);
  if (!SAFE_REF.test(options.pylonRef) || !SAFE_REF.test(options.targetRef)) {
    throw transportError("invalid_request");
  }
  if (options.agentToken.trim() === "") throw transportError("not_authorized");
  const requestTimeoutMs = options.requestTimeoutMs ?? 15_000;
  if (
    !Number.isInteger(requestTimeoutMs) ||
    requestTimeoutMs < 1_000 ||
    requestTimeoutMs > 60_000
  ) {
    throw transportError("invalid_request");
  }
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const route = `/api/pylons/${encodeURIComponent(options.pylonRef)}/portable-targets/${encodeURIComponent(options.targetRef)}/capability-operations`;

  const request = async (
    suffix: string,
    init: RequestInit,
    query?: URLSearchParams,
    signal?: AbortSignal,
  ): Promise<string> => {
    const timeout = AbortSignal.timeout(requestTimeoutMs);
    const requestSignal = signal === undefined ? timeout : AbortSignal.any([signal, timeout]);
    let response: Response;
    try {
      const url = new URL(`${route}${suffix}`, baseUrl);
      if (query !== undefined) url.search = query.toString();
      response = await fetchImpl(url, {
        ...init,
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${options.agentToken}`,
          ...init.headers,
        },
        signal: requestSignal,
      });
    } catch {
      if (signal?.aborted === true) throw transportError("cancelled");
      throw transportError("network_failed");
    }
    if (!response.ok) throw mapStatus(response.status);
    return readBoundedText(response);
  };

  const decode = <A>(schema: S.Decoder<A>, raw: string): A => {
    try {
      return S.decodeUnknownSync(S.fromJsonString(schema))(raw, { onExcessProperty: "error" });
    } catch {
      throw transportError("bad_response");
    }
  };

  return {
    pending: async (limit, signal) => {
      if (!Number.isSafeInteger(limit) || limit < 1 || limit > 32)
        throw transportError("invalid_request");
      const url = new URL(route, baseUrl);
      url.searchParams.set("limit", String(limit));
      const raw = await request("", { method: "GET" }, url.searchParams, signal);
      const decoded = decode(PendingResponse, raw);
      if (
        decoded.operations.some(
          (operation) =>
            operation.request.pylonRef !== options.pylonRef ||
            operation.request.targetRef !== options.targetRef,
        )
      )
        throw transportError("bad_response");
      return decoded.operations;
    },
    read: async (operationRef, signal) => {
      if (!SAFE_REF.test(operationRef)) throw transportError("invalid_request");
      const raw = await request(
        `/reconcile/${encodeURIComponent(operationRef)}`,
        { method: "GET" },
        undefined,
        signal,
      );
      const decoded = decode(ReconcileResponse, raw);
      if (
        decoded.operation.request.operationRef !== operationRef ||
        decoded.operation.request.pylonRef !== options.pylonRef ||
        decoded.operation.request.targetRef !== options.targetRef
      ) {
        throw transportError("bad_response");
      }
      return decoded.operation;
    },
    claim: async (body, signal) => {
      const exact = S.decodeUnknownSync(PortableOwnerLocalCapabilityOperationClaimRequestSchema)(
        body,
        {
          onExcessProperty: "error",
        },
      );
      if (exact.pylonRef !== options.pylonRef || exact.targetRef !== options.targetRef)
        throw transportError("invalid_request");
      const raw = await request(
        "/claim",
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(exact),
        },
        undefined,
        signal,
      );
      return decode(ClaimResponse, raw);
    },
    renew: async (body, signal) => {
      const exact = S.decodeUnknownSync(PortableOwnerLocalCapabilityOperationRenewRequestSchema)(
        body,
        {
          onExcessProperty: "error",
        },
      );
      if (exact.pylonRef !== options.pylonRef || exact.targetRef !== options.targetRef)
        throw transportError("invalid_request");
      const raw = await request(
        "/renew",
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(exact),
        },
        undefined,
        signal,
      );
      return decode(RenewResponse, raw);
    },
    complete: async (body, signal) => {
      const exact = S.decodeUnknownSync(PortableOwnerLocalCapabilityOperationResultRequestSchema)(
        body,
        {
          onExcessProperty: "error",
        },
      );
      if (exact.pylonRef !== options.pylonRef || exact.targetRef !== options.targetRef)
        throw transportError("invalid_request");
      const raw = await request(
        "/complete",
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(exact),
        },
        undefined,
        signal,
      );
      return decode(CompleteResponse, raw);
    },
  };
};

import {
  OwnerManagedEnvironmentEnrollmentSchema,
  type OwnerManagedEnvironmentEnrollment,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

export type PylonOwnerManagedEnrollmentClient = Readonly<{
  admitOrRenew: (health?: "ready" | "draining") => Promise<OwnerManagedEnvironmentEnrollment>;
  revoke: () => Promise<OwnerManagedEnvironmentEnrollment>;
  current: () => OwnerManagedEnvironmentEnrollment | null;
  isCurrent: () => boolean;
}>;

export type PylonOwnerManagedEnrollmentClientOptions = Readonly<{
  agentToken: string;
  baseUrl: string;
  pylonRef: string;
  targetRef: string;
  workerInstanceRef: string;
  adapterRef: string;
  compatibilityRef: string;
  isolation: "owner_host_process" | "owner_host_container";
  checkpointKeyRef: string;
  regionRef: string;
  networkDestinationRefs: ReadonlyArray<string>;
  dataDestinationRefs: ReadonlyArray<string>;
  retentionSeconds: number;
  costPolicyRef: string;
  generation: number;
  evidenceRefs: ReadonlyArray<string>;
  fetchImpl?: typeof globalThis.fetch;
}>;

const validBaseUrl = (value: string): URL => {
  const url = new URL(value);
  const loopback =
    url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if ((url.protocol !== "https:" && !loopback) || url.username !== "" || url.password !== "") {
    throw new Error("owner-managed enrollment base URL is invalid");
  }
  return url;
};

export const makePylonOwnerManagedEnrollmentClient = (
  options: PylonOwnerManagedEnrollmentClientOptions,
): PylonOwnerManagedEnrollmentClient => {
  const refs = [
    options.pylonRef,
    options.targetRef,
    options.workerInstanceRef,
    options.adapterRef,
    options.compatibilityRef,
    options.checkpointKeyRef,
    options.regionRef,
    options.costPolicyRef,
    ...options.networkDestinationRefs,
    ...options.dataDestinationRefs,
    ...options.evidenceRefs,
  ];
  if (
    options.agentToken.trim() === "" ||
    !refs.every((ref) => SAFE_REF.test(ref)) ||
    !Number.isSafeInteger(options.generation) ||
    options.generation < 1 ||
    !Number.isSafeInteger(options.retentionSeconds) ||
    options.retentionSeconds < 0 ||
    options.retentionSeconds > 31_536_000
  ) {
    throw new Error("owner-managed enrollment configuration is invalid");
  }
  const origin = validBaseUrl(options.baseUrl);
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const route = `/api/pylons/${encodeURIComponent(options.pylonRef)}/owner-managed-environments/${encodeURIComponent(options.targetRef)}`;
  let enrollment: OwnerManagedEnvironmentEnrollment | null = null;

  const send = async (method: "POST" | "DELETE", health: "ready" | "draining") => {
    const expectedRevision = enrollment?.revision;
    const action =
      method === "DELETE" ? "revoke" : expectedRevision === undefined ? "admit" : "renew";
    const idempotencyKey = [
      "owner-managed-enrollment",
      options.pylonRef,
      options.targetRef,
      String(options.generation),
      action,
      String(expectedRevision ?? 0),
    ].join(":");
    const response = await fetchImpl(new URL(route, origin), {
      method,
      headers: {
        authorization: `Bearer ${options.agentToken}`,
        "content-type": "application/json",
        "idempotency-key": idempotencyKey,
      },
      body: JSON.stringify({
        schema: "openagents.owner_managed_environment_enrollment.request.v1",
        workerInstanceRef: options.workerInstanceRef,
        adapterRef: options.adapterRef,
        compatibilityRef: options.compatibilityRef,
        isolation: options.isolation,
        checkpointKeyRef: options.checkpointKeyRef,
        regionRef: options.regionRef,
        networkDestinationRefs: options.networkDestinationRefs,
        dataDestinationRefs: options.dataDestinationRefs,
        retentionSeconds: options.retentionSeconds,
        costPolicyRef: options.costPolicyRef,
        generation: options.generation,
        health,
        evidenceRefs: options.evidenceRefs,
        ...(expectedRevision === undefined ? {} : { expectedRevision }),
      }),
    });
    if (!response.ok) {
      throw new Error(`owner-managed enrollment request failed (${response.status})`);
    }
    const body: unknown = await response.json();
    if (typeof body !== "object" || body === null || !("enrollment" in body)) {
      throw new Error("owner-managed enrollment response is invalid");
    }
    const decoded = Schema.decodeUnknownSync(OwnerManagedEnvironmentEnrollmentSchema)(
      body.enrollment,
      { onExcessProperty: "error" },
    );
    if (
      decoded.pylonRef !== options.pylonRef ||
      decoded.targetRef !== options.targetRef ||
      decoded.workerInstanceRef !== options.workerInstanceRef ||
      decoded.generation !== options.generation ||
      decoded.checkpointKeyRef !== options.checkpointKeyRef ||
      decoded.custodyPolicy !== "owner_held_key"
    ) {
      throw new Error("owner-managed enrollment response scope is not exact");
    }
    enrollment = decoded;
    return decoded;
  };

  return {
    admitOrRenew: (health = "ready") => send("POST", health),
    revoke: () => send("DELETE", "draining"),
    current: () => enrollment,
    isCurrent: () =>
      enrollment?.state === "active" &&
      enrollment.health === "ready" &&
      Date.parse(enrollment.expiresAt) > Date.now(),
  };
};

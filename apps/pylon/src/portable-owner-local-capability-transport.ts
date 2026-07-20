import { createHash, timingSafeEqual } from "node:crypto";

import type { PortableCapabilityLease, SecretMaterial } from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const MAX_MATERIAL_BYTES = 16 * 1024 * 1024;
const CAPABILITIES = ["provider", "scm_read", "scm_write", "tool", "api"] as const;
export const PYLON_OWNER_LOCAL_CAPABILITY_PATH = "/v1/portable-owner-local-capabilities";

const Ref = Schema.String.check(Schema.isPattern(SAFE_REF));
export const PylonOwnerLocalCapabilityAuthoritySchema = Schema.Struct({
  commandExecutionClaimRef: Ref, ownerRef: Ref, pylonRef: Ref, sessionRef: Ref,
  attachmentRef: Ref,
  attachmentGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  targetRef: Ref,
});
export type PylonOwnerLocalCapabilityAuthority = typeof PylonOwnerLocalCapabilityAuthoritySchema.Type;

export type PylonOwnerLocalCapabilityInstallationPort = Readonly<{
  install: (input: Readonly<{
    lease: PortableCapabilityLease;
    permissions: ReadonlyArray<string>;
    material: SecretMaterial;
  }>) => Promise<Readonly<{ installationRef: string; evidenceRef: string }>>;
  wipe: (input: Readonly<{
    leaseRef: string;
    targetRef: string;
    attachmentRef: string;
    attachmentGeneration: number;
    installationRef: string;
  }>) => Promise<Readonly<{ wipeReceiptRef: string }>>;
}>;

export class PylonOwnerLocalCapabilityTransportError extends Schema.TaggedErrorClass<PylonOwnerLocalCapabilityTransportError>()(
  "PylonOwnerLocalCapabilityTransportError",
  { reason: Schema.String },
) {}

const digest = (values: ReadonlyArray<string>): string =>
  createHash("sha256").update(values.join("\u0000")).digest("hex");

export const pylonOwnerLocalCapabilityOperationRef = (input: Readonly<{
  action: "install" | "wipe";
  authority: PylonOwnerLocalCapabilityAuthority;
  leaseRef: string;
  installationRef?: string;
  permissions?: ReadonlyArray<string>;
}>): string => `operation.owner-local-capability.${digest([
  input.action,
  input.authority.commandExecutionClaimRef,
  input.authority.ownerRef,
  input.authority.pylonRef,
  input.authority.sessionRef,
  input.authority.attachmentRef,
  String(input.authority.attachmentGeneration),
  input.authority.targetRef,
  input.leaseRef,
  input.installationRef ?? "none",
  ...[...(input.permissions ?? [])].sort(),
])}`;

const safe = (value: string | null): value is string => value !== null && SAFE_REF.test(value);
const isCapability = (value: string | null): value is PortableCapabilityLease["capability"] =>
  value !== null && CAPABILITIES.some(candidate => candidate === value);
const authorizedBearer = (request: Request, expected: string): boolean => {
  const header = request.headers.get("authorization");
  if (header === null || !header.startsWith("Bearer ")) return false;
  const actual = Buffer.from(header.slice(7));
  const wanted = Buffer.from(expected);
  try {
    return actual.length === wanted.length && timingSafeEqual(actual, wanted);
  } finally {
    actual.fill(0);
    wanted.fill(0);
  }
};

const json = (body: unknown, status = 200): Response => Response.json(body, {
  status,
  headers: { "cache-control": "no-store" },
});

export const makePylonOwnerLocalCapabilityTransportHandler = (config: Readonly<{
  bearerToken: string;
  port?: PylonOwnerLocalCapabilityInstallationPort;
  portForAuthority?: (
    authority: PylonOwnerLocalCapabilityAuthority,
  ) => PylonOwnerLocalCapabilityInstallationPort;
  authorize: (authority: PylonOwnerLocalCapabilityAuthority) => Promise<boolean>;
}>): ((request: Request) => Promise<Response>) => {
  if (
    config.bearerToken.length < 16 ||
    (config.port === undefined) === (config.portForAuthority === undefined)
  ) throw new PylonOwnerLocalCapabilityTransportError({ reason: "invalid_config" });
  return async request => {
    const url = new URL(request.url);
    const action = url.pathname === `${PYLON_OWNER_LOCAL_CAPABILITY_PATH}/install`
      ? "install"
      : url.pathname === `${PYLON_OWNER_LOCAL_CAPABILITY_PATH}/wipe` ? "wipe" : undefined;
    if (request.method !== "POST" || action === undefined) return json({ error: "not_found" }, 404);
    if (!authorizedBearer(request, config.bearerToken)) return json({ error: "unauthorized" }, 401);
    const h = request.headers;
    const generation = Number(h.get("x-openagents-attachment-generation"));
    let authority: PylonOwnerLocalCapabilityAuthority;
    try {
      authority = Schema.decodeUnknownSync(PylonOwnerLocalCapabilityAuthoritySchema)({
        commandExecutionClaimRef: h.get("x-openagents-command-claim-ref"),
        ownerRef: h.get("x-openagents-owner-ref"), pylonRef: h.get("x-openagents-pylon-ref"),
        sessionRef: h.get("x-openagents-session-ref"), attachmentRef: h.get("x-openagents-attachment-ref"),
        attachmentGeneration: generation, targetRef: h.get("x-openagents-target-ref"),
      }, { onExcessProperty: "error" });
    } catch { return json({ error: "authority_refused" }, 403); }
    const leaseRef = h.get("x-openagents-lease-ref");
    if (!safe(leaseRef) || !(await config.authorize(authority))) {
      return json({ error: "authority_refused" }, 403);
    }
    const port = config.portForAuthority?.(authority) ?? config.port;
    if (port === undefined) return json({ error: "authority_refused" }, 403);
    const permissions = (h.get("x-openagents-permissions") ?? "").split(",").filter(Boolean);
    const installationRef = h.get("x-openagents-installation-ref") ?? undefined;
    const operationRef = pylonOwnerLocalCapabilityOperationRef({
      action, authority, leaseRef,
      ...(installationRef === undefined ? {} : { installationRef }),
      ...(action === "install" ? { permissions } : {}),
    });
    if (h.get("idempotency-key") !== operationRef || h.get("x-openagents-operation-ref") !== operationRef) {
      return json({ error: "claim_mismatch" }, 409);
    }
    try {
      if (action === "wipe") {
        if (installationRef === undefined || !SAFE_REF.test(installationRef) || permissions.length !== 0) return json({ error: "invalid_scope" }, 400);
        const result = await port.wipe({ leaseRef, targetRef: authority.targetRef,
          attachmentRef: authority.attachmentRef, attachmentGeneration: generation, installationRef });
        if (!SAFE_REF.test(result.wipeReceiptRef)) throw new Error("unsafe receipt");
        return json({ schema: "openagents.owner_local_capability_transport.v1", status: "wiped",
          operationRef, leaseRef, wipeReceiptRef: result.wipeReceiptRef, material: "excluded" });
      }
      const capability = h.get("x-openagents-capability");
      const expiresAt = h.get("x-openagents-expires-at");
      if (!isCapability(capability) || permissions.length === 0 || permissions.length > 64 ||
          new Set(permissions).size !== permissions.length || permissions.some(value => !SAFE_REF.test(value)) ||
          expiresAt === null || !Number.isFinite(Date.parse(expiresAt))) return json({ error: "invalid_scope" }, 400);
      const declared = Number(h.get("content-length"));
      if (!Number.isSafeInteger(declared) || declared < 1 || declared > MAX_MATERIAL_BYTES) return json({ error: "invalid_size" }, 413);
      const bytes = new Uint8Array(await request.arrayBuffer());
      if (bytes.byteLength !== declared) { bytes.fill(0); return json({ error: "invalid_size" }, 400); }
      try {
        const result = await port.install({
          lease: { leaseRef, ownerRef: authority.ownerRef, sessionRef: authority.sessionRef,
            attachmentRef: authority.attachmentRef, attachmentGeneration: generation,
            targetRef: authority.targetRef, capability, expiresAt, state: "issued" },
          permissions,
          material: bytes as SecretMaterial,
        });
        if (!SAFE_REF.test(result.installationRef) || !SAFE_REF.test(result.evidenceRef)) throw new Error("unsafe receipt");
        return json({ schema: "openagents.owner_local_capability_transport.v1", status: "installed",
          operationRef, leaseRef, installationRef: result.installationRef,
          evidenceRef: result.evidenceRef, material: "excluded" });
      } finally { bytes.fill(0); }
    } catch {
      return json({ error: "target_refused" }, 409);
    }
  };
};

export const runPylonOwnerLocalCapabilityRequest = Effect.fn("PylonOwnerLocalCapabilityTransport.handle")(
  function* (handler: (request: Request) => Promise<Response>, request: Request) {
    return yield* Effect.promise(() => handler(request));
  },
);

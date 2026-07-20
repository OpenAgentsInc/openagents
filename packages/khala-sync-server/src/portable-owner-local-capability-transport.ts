import { createHash } from "node:crypto";

import { Schema } from "effect";
import type { PortableCapabilityLease, SecretMaterial } from "@openagentsinc/portable-session-contract";

import type { PortableCapabilityTargetInstallationPort } from "./portable-capability-runtime-adapters.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const LOOPBACK = new Set(["127.0.0.1", "localhost", "::1"]);
const RESPONSE = "openagents.owner_local_capability_transport.v1" as const;
const FORBIDDEN = /"(?:token|authorization|secret|credential|path|hostname|processId)"\s*:/iu;

const Ref = Schema.String.check(Schema.isPattern(SAFE_REF));
const InstallResponse = Schema.Struct({
  schema: Schema.Literal(RESPONSE), status: Schema.Literal("installed"),
  operationRef: Ref, leaseRef: Ref, installationRef: Ref, evidenceRef: Ref,
  material: Schema.Literal("excluded"),
});
const WipeResponse = Schema.Struct({
  schema: Schema.Literal(RESPONSE), status: Schema.Literal("wiped"),
  operationRef: Ref, leaseRef: Ref, wipeReceiptRef: Ref,
  material: Schema.Literal("excluded"),
});

export const OwnerLocalRemoteCapabilityAuthoritySchema = Schema.Struct({
  commandExecutionClaimRef: Ref, ownerRef: Ref, pylonRef: Ref, sessionRef: Ref,
  attachmentRef: Ref,
  attachmentGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  targetRef: Ref,
});
export type OwnerLocalRemoteCapabilityAuthority = typeof OwnerLocalRemoteCapabilityAuthoritySchema.Type;

export class OwnerLocalRemoteCapabilityTransportError extends Schema.TaggedErrorClass<OwnerLocalRemoteCapabilityTransportError>()(
  "OwnerLocalRemoteCapabilityTransportError", { reason: Schema.String },
) {}

const digest = (values: ReadonlyArray<string>): string =>
  createHash("sha256").update(values.join("\u0000")).digest("hex");
const operationRef = (action: "install" | "wipe", authority: OwnerLocalRemoteCapabilityAuthority,
  leaseRef: string, installationRef?: string, permissions: ReadonlyArray<string> = []): string =>
  `operation.owner-local-capability.${digest([action, authority.commandExecutionClaimRef,
    authority.ownerRef, authority.pylonRef, authority.sessionRef, authority.attachmentRef,
    String(authority.attachmentGeneration), authority.targetRef, leaseRef,
    installationRef ?? "none", ...[...permissions].sort()])}`;

export class OwnerLocalRemoteCapabilityInstallationPort implements PortableCapabilityTargetInstallationPort {
  private readonly origin: URL;
  private readonly fetch: typeof globalThis.fetch;
  private readonly timeoutMs: number;

  constructor(private readonly config: Readonly<{
    baseUrl: string;
    bearerToken: string;
    authority: OwnerLocalRemoteCapabilityAuthority;
    fetch?: typeof globalThis.fetch;
    timeoutMs?: number;
  }>) {
    try { this.origin = new URL(config.baseUrl); } catch { throw new OwnerLocalRemoteCapabilityTransportError({ reason: "invalid_config" }); }
    this.timeoutMs = config.timeoutMs ?? 30_000;
    if ((this.origin.protocol !== "https:" && !(this.origin.protocol === "http:" && LOOPBACK.has(this.origin.hostname))) ||
        this.origin.username !== "" || this.origin.password !== "" ||
        !Number.isSafeInteger(this.timeoutMs) || this.timeoutMs < 1_000 || this.timeoutMs > 120_000 ||
        config.bearerToken.length < 16) {
      throw new OwnerLocalRemoteCapabilityTransportError({ reason: "invalid_config" });
    }
    try { Schema.decodeUnknownSync(OwnerLocalRemoteCapabilityAuthoritySchema)(config.authority,
      { onExcessProperty: "error" }); } catch {
      throw new OwnerLocalRemoteCapabilityTransportError({ reason: "invalid_config" });
    }
    this.fetch = config.fetch ?? globalThis.fetch;
  }

  private headers(leaseRef: string, ref: string): Record<string, string> {
    const a = this.config.authority;
    return { authorization: `Bearer ${this.config.bearerToken}`, "idempotency-key": ref,
      "x-openagents-operation-ref": ref, "x-openagents-command-claim-ref": a.commandExecutionClaimRef,
      "x-openagents-owner-ref": a.ownerRef, "x-openagents-pylon-ref": a.pylonRef,
      "x-openagents-session-ref": a.sessionRef, "x-openagents-attachment-ref": a.attachmentRef,
      "x-openagents-attachment-generation": String(a.attachmentGeneration),
      "x-openagents-target-ref": a.targetRef, "x-openagents-lease-ref": leaseRef };
  }

  private async send(path: string, init: RequestInit): Promise<unknown> {
    let response: Response;
    try {
      response = await this.fetch(new URL(path, this.origin), { ...init,
        signal: AbortSignal.timeout(this.timeoutMs) });
    } catch { throw new OwnerLocalRemoteCapabilityTransportError({ reason: "unavailable" }); }
    if (!response.ok) throw new OwnerLocalRemoteCapabilityTransportError({ reason: "refused" });
    const value: unknown = await response.json();
    if (FORBIDDEN.test(JSON.stringify(value))) throw new OwnerLocalRemoteCapabilityTransportError({ reason: "unsafe_response" });
    return value;
  }

  async install(input: Readonly<{ lease: PortableCapabilityLease; permissions: ReadonlyArray<string>;
    material: SecretMaterial; managedMarkerPath?: string | undefined }>) {
    const a = this.config.authority;
    if (input.managedMarkerPath !== undefined || input.lease.ownerRef !== a.ownerRef ||
        input.lease.sessionRef !== a.sessionRef || input.lease.attachmentRef !== a.attachmentRef ||
        input.lease.attachmentGeneration !== a.attachmentGeneration || input.lease.targetRef !== a.targetRef ||
        input.material.byteLength === 0 || !SAFE_REF.test(input.lease.leaseRef) ||
        input.lease.state !== "issued" || !Number.isFinite(Date.parse(input.lease.expiresAt)) ||
        input.permissions.length === 0 || input.permissions.length > 64 ||
        new Set(input.permissions).size !== input.permissions.length ||
        input.permissions.some(value => !SAFE_REF.test(value))) {
      throw new OwnerLocalRemoteCapabilityTransportError({ reason: "invalid_scope" });
    }
    const ref = operationRef("install", a, input.lease.leaseRef, undefined, input.permissions);
    const outboundBody = Buffer.from(input.material);
    let value: unknown;
    try {
      value = await this.send("/v1/portable-owner-local-capabilities/install", {
        method: "POST", headers: { ...this.headers(input.lease.leaseRef, ref),
          "content-type": "application/octet-stream", "content-length": String(outboundBody.byteLength),
          "x-openagents-capability": input.lease.capability, "x-openagents-expires-at": input.lease.expiresAt,
          "x-openagents-permissions": [...input.permissions].sort().join(",") }, body: outboundBody,
      });
    } finally {
      outboundBody.fill(0);
    }
    const decoded = Schema.decodeUnknownSync(InstallResponse)(value, { onExcessProperty: "error" });
    if (decoded.operationRef !== ref || decoded.leaseRef !== input.lease.leaseRef) {
      throw new OwnerLocalRemoteCapabilityTransportError({ reason: "scope_mismatch" });
    }
    return { installationRef: decoded.installationRef, evidenceRef: decoded.evidenceRef };
  }

  async wipe(input: Readonly<{ leaseRef: string; targetRef: string; attachmentRef: string;
    attachmentGeneration: number; installationRef?: string | undefined }>) {
    const a = this.config.authority;
    if (input.targetRef !== a.targetRef || input.attachmentRef !== a.attachmentRef ||
        input.attachmentGeneration !== a.attachmentGeneration || input.installationRef === undefined ||
        !SAFE_REF.test(input.installationRef)) throw new OwnerLocalRemoteCapabilityTransportError({ reason: "invalid_scope" });
    const ref = operationRef("wipe", a, input.leaseRef, input.installationRef);
    const value = await this.send("/v1/portable-owner-local-capabilities/wipe", { method: "POST",
      headers: { ...this.headers(input.leaseRef, ref), "x-openagents-installation-ref": input.installationRef } });
    const decoded = Schema.decodeUnknownSync(WipeResponse)(value, { onExcessProperty: "error" });
    if (decoded.operationRef !== ref || decoded.leaseRef !== input.leaseRef) {
      throw new OwnerLocalRemoteCapabilityTransportError({ reason: "scope_mismatch" });
    }
    return { wipeReceiptRef: decoded.wipeReceiptRef };
  }
}

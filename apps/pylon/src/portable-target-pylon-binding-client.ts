import { createHash } from "node:crypto";

import { Schema } from "effect";

import type { PylonPortableControlBinding } from "./portable-session-operation-ledger.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[0-9a-f]{64}$/u;

const BindingResponse = Schema.Struct({
  binding: Schema.Struct({
    bindingRef: Schema.String,
    sessionRef: Schema.String,
    targetRef: Schema.String,
    pylonRef: Schema.String,
    workerInstanceRef: Schema.String,
    bindingDigest: Schema.String,
    revision: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
    state: Schema.Literals(["active", "revoked"]),
    health: Schema.Literals(["ready", "draining", "offline", "revoked"]),
    expiresAt: Schema.String,
  }),
});

export type PylonPortableTargetBindingClient = Readonly<{
  admitOrRenew: (health?: "ready" | "draining") => Promise<void>;
  revoke: () => Promise<void>;
  isCurrent: () => boolean;
}>;

export type PylonPortableTargetBindingClientOptions = Readonly<{
  agentToken: string;
  baseUrl: string;
  pylonRef: string;
  sessionRef: string;
  targetRef: string;
  workerInstanceRef: string;
  binding: PylonPortableControlBinding;
  fetchImpl?: typeof globalThis.fetch;
}>;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Readonly<Record<string, unknown>>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

export const portableTargetPylonBindingDigest = (
  options: Omit<PylonPortableTargetBindingClientOptions, "agentToken" | "baseUrl" | "fetchImpl">,
): `sha256:${string}` =>
  `sha256:${createHash("sha256")
    .update(
      canonical({
        pylonRef: options.pylonRef,
        sessionRef: options.sessionRef,
        targetRef: options.targetRef,
        workerInstanceRef: options.workerInstanceRef,
        binding: {
          sessionRef: options.binding.sessionRef,
          attachmentRef: options.binding.attachmentRef,
          generation: options.binding.generation,
          runtimeInstanceRef: options.binding.runtimeInstanceRef,
          state: options.binding.state,
          revision: options.binding.revision,
          agents: options.binding.agents.map((agent) => ({
            agentRef: agent.agentRef,
            controlSessionRef: agent.controlSessionRef,
            workspaceRef: agent.workspaceRef,
          })),
        },
      }),
    )
    .digest("hex")}`;

const validBaseUrl = (value: string): URL => {
  const url = new URL(value);
  const loopback =
    url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if ((url.protocol !== "https:" && !loopback) || url.username !== "" || url.password !== "") {
    throw new Error("portable target Pylon binding base URL is invalid");
  }
  return url;
};

export const makePylonPortableTargetBindingClient = (
  options: PylonPortableTargetBindingClientOptions,
): PylonPortableTargetBindingClient => {
  if (
    options.agentToken.trim() === "" ||
    ![options.pylonRef, options.sessionRef, options.targetRef, options.workerInstanceRef].every(
      (ref) => SAFE_REF.test(ref),
    ) ||
    options.binding.sessionRef !== options.sessionRef
  )
    throw new Error("portable target Pylon binding configuration is invalid");
  const origin = validBaseUrl(options.baseUrl);
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const digest = portableTargetPylonBindingDigest(options);
  if (!SHA256.test(digest)) throw new Error("portable target Pylon binding digest is invalid");
  const evidenceRefs = [`evidence.portable-target-pylon.${digest.slice("sha256:".length)}`];
  const route = `/api/pylons/${encodeURIComponent(options.pylonRef)}/portable-target-bindings/${encodeURIComponent(options.targetRef)}`;
  let revision: number | undefined;
  let current = false;
  let currentUntil = 0;

  const readCurrent = async () => {
    const url = new URL(route, origin);
    url.searchParams.set("sessionRef", options.sessionRef);
    const response = await fetchImpl(url, {
      method: "GET",
      headers: { authorization: `Bearer ${options.agentToken}` },
    });
    if (response.status === 404) {
      current = false;
      return;
    }
    if (!response.ok)
      throw new Error(`portable target Pylon binding read failed (${response.status})`);
    const decoded = Schema.decodeUnknownSync(BindingResponse)(await response.json(), {
      onExcessProperty: "preserve",
    });
    if (
      decoded.binding.state !== "active" ||
      decoded.binding.sessionRef !== options.sessionRef ||
      decoded.binding.targetRef !== options.targetRef ||
      decoded.binding.pylonRef !== options.pylonRef ||
      decoded.binding.workerInstanceRef !== options.workerInstanceRef ||
      decoded.binding.bindingDigest !== digest ||
      !Number.isFinite(Date.parse(decoded.binding.expiresAt))
    )
      throw new Error("portable target Pylon binding read is not exact");
    revision = decoded.binding.revision;
    current = decoded.binding.health === "ready";
    currentUntil = Date.parse(decoded.binding.expiresAt);
  };

  const send = async (method: "POST" | "DELETE", health: "ready" | "draining") => {
    const expectedRevision = revision;
    const action =
      method === "DELETE" ? "revoke" : expectedRevision === undefined ? "admit" : "renew";
    const idempotencyKey = `portable-target-pylon:${digest}:${action}:${expectedRevision ?? 0}`;
    const response = await fetchImpl(new URL(route, origin), {
      method,
      headers: {
        authorization: `Bearer ${options.agentToken}`,
        "content-type": "application/json",
        "idempotency-key": idempotencyKey,
      },
      body: JSON.stringify({
        schema: "openagents.portable_target_pylon_binding.request.v1",
        sessionRef: options.sessionRef,
        workerInstanceRef: options.workerInstanceRef,
        bindingDigest: digest,
        health,
        evidenceRefs,
        ...(expectedRevision === undefined ? {} : { expectedRevision }),
      }),
    });
    if (!response.ok)
      throw new Error(`portable target Pylon binding request failed (${response.status})`);
    const decoded = Schema.decodeUnknownSync(BindingResponse)(await response.json(), {
      onExcessProperty: "preserve",
    });
    if (
      decoded.binding.sessionRef !== options.sessionRef ||
      decoded.binding.targetRef !== options.targetRef ||
      decoded.binding.pylonRef !== options.pylonRef ||
      decoded.binding.workerInstanceRef !== options.workerInstanceRef ||
      decoded.binding.bindingDigest !== digest ||
      !Number.isFinite(Date.parse(decoded.binding.expiresAt))
    ) {
      throw new Error("portable target Pylon binding response is not exact");
    }
    if (method === "POST" && decoded.binding.state !== "active") {
      throw new Error("portable target Pylon binding response is not active");
    }
    if (method === "DELETE" && decoded.binding.state !== "revoked") {
      throw new Error("portable target Pylon binding response is not revoked");
    }
    revision = decoded.binding.revision;
    current = method === "POST" && decoded.binding.health === "ready";
    currentUntil = Date.parse(decoded.binding.expiresAt);
  };

  return {
    admitOrRenew: async (health = "ready") => {
      if (revision === undefined) await readCurrent();
      await send("POST", health);
    },
    revoke: () => send("DELETE", "draining"),
    isCurrent: () => current && currentUntil > Date.now(),
  };
};

import { createHash } from "node:crypto";

import { Schema } from "effect";
import type {
  PortableTargetPylonBindingRecord,
  PostgresPortableTargetPylonBindingStore,
} from "@openagentsinc/khala-sync-server";

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Digest = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/));
const RequestBody = Schema.Struct({
  schema: Schema.Literal("openagents.portable_target_pylon_binding.request.v1"),
  sessionRef: Ref,
  workerInstanceRef: Ref,
  bindingDigest: Digest,
  health: Schema.Literals(["ready", "draining"]),
  evidenceRefs: Schema.Array(Ref).check(Schema.isMaxLength(64)),
  expectedRevision: Schema.optionalKey(
    Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  ),
});

export type PortableTargetPylonBindingActor = Readonly<{
  ownerUserId: string;
  ownerAgentUserId: string;
}>;

export type PortableTargetPylonBindingRouteDependencies<Bindings> = Readonly<{
  authenticate: (
    request: Request,
    env: Bindings,
    pylonRef: string,
  ) => Promise<PortableTargetPylonBindingActor | undefined>;
  withStore: <A>(
    env: Bindings,
    use: (
      store: Pick<PostgresPortableTargetPylonBindingStore, "admit" | "read" | "revoke">,
    ) => Promise<A>,
  ) => Promise<A>;
}>;

const response = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });

const projection = (binding: PortableTargetPylonBindingRecord) => ({
  schema: "openagents.portable_target_pylon_binding.v1",
  bindingRef: binding.bindingRef,
  sessionRef: binding.sessionRef,
  targetRef: binding.targetRef,
  pylonRef: binding.pylonRef,
  workerInstanceRef: binding.workerInstanceRef,
  bindingDigest: binding.bindingDigest,
  revision: binding.revision,
  state: binding.state,
  health: binding.health,
  evidenceRefs: binding.evidenceRefs,
  expiresAt: binding.expiresAt,
  revokedAt: binding.revokedAt,
});

const idempotencyHash = (request: Request): string | undefined => {
  const key = request.headers.get("idempotency-key")?.trim();
  return key === undefined || key.length < 8 || key.length > 512
    ? undefined
    : `sha256:${createHash("sha256").update(key).digest("hex")}`;
};

export const makePortableTargetPylonBindingRoutes = <Bindings>(
  dependencies: PortableTargetPylonBindingRouteDependencies<Bindings>,
) => ({
  route: (request: Request, env: Bindings): Promise<Response> | undefined => {
    const match = /^\/api\/pylons\/([^/]+)\/portable-target-bindings\/([^/]+)$/.exec(
      new URL(request.url).pathname,
    );
    if (match === null) return undefined;
    return (async () => {
      let pylonRef: string;
      let targetRef: string;
      try {
        pylonRef = decodeURIComponent(match[1] ?? "");
        targetRef = decodeURIComponent(match[2] ?? "");
        Schema.decodeUnknownSync(Ref)(pylonRef);
        Schema.decodeUnknownSync(Ref)(targetRef);
      } catch {
        return response({ error: "invalid_scope" }, 400);
      }
      const actor = await dependencies.authenticate(request, env, pylonRef).catch(() => undefined);
      if (actor === undefined) return response({ error: "unauthorized" }, 401);
      if (request.method === "GET") {
        const sessionRef = new URL(request.url).searchParams.get("sessionRef");
        if (sessionRef === null) return response({ error: "invalid_scope" }, 400);
        try {
          Schema.decodeUnknownSync(Ref)(sessionRef);
          const binding = await dependencies.withStore(env, (store) =>
            store.read(actor.ownerUserId, sessionRef, targetRef),
          );
          return binding === undefined
            ? response({ error: "not_found" }, 404)
            : response({ binding: projection(binding) });
        } catch {
          return response({ error: "binding_unavailable" }, 503);
        }
      }
      if (request.method !== "POST" && request.method !== "DELETE") {
        return response({ error: "method_not_allowed" }, 405);
      }
      const hash = idempotencyHash(request);
      if (hash === undefined) return response({ error: "idempotency_required" }, 400);
      let body: typeof RequestBody.Type;
      try {
        body = Schema.decodeUnknownSync(RequestBody)(await request.json(), {
          onExcessProperty: "error",
        });
      } catch {
        return response({ error: "invalid_request" }, 400);
      }
      try {
        const input = {
          idempotencyKeyHash: hash,
          ownerUserId: actor.ownerUserId,
          ownerAgentUserId: actor.ownerAgentUserId,
          sessionRef: body.sessionRef,
          targetRef,
          pylonRef,
          workerInstanceRef: body.workerInstanceRef,
          bindingDigest: body.bindingDigest,
          health: body.health,
          evidenceRefs: body.evidenceRefs,
          ...(body.expectedRevision === undefined
            ? {}
            : { expectedRevision: body.expectedRevision }),
        } as const;
        const binding = await dependencies.withStore(env, (store) =>
          request.method === "DELETE" ? store.revoke(input) : store.admit(input),
        );
        return response(
          { binding: projection(binding) },
          request.method === "POST" && binding.revision === 1 ? 201 : 200,
        );
      } catch (error) {
        const reason =
          error instanceof Error && "reason" in error ? String(error.reason) : "unavailable";
        const status =
          reason === "conflict" || reason === "stale_revision"
            ? 409
            : reason === "authority_mismatch"
              ? 403
              : reason === "not_found"
                ? 404
                : 503;
        return response({ error: `portable_target_pylon_binding_${reason}` }, status);
      }
    })();
  },
});

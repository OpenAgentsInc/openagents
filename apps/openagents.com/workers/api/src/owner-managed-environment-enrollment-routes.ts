import { createHash } from "node:crypto";

import {
  OwnerManagedEnvironmentEnrollmentRequestSchema,
  type OwnerManagedEnvironmentEnrollment,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";
import type { PostgresOwnerManagedEnvironmentEnrollmentStore } from "@openagentsinc/khala-sync-server";

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);

export type OwnerManagedEnvironmentEnrollmentActor = Readonly<{
  ownerUserId: string;
  ownerAgentUserId: string;
}>;

export type OwnerManagedEnvironmentEnrollmentRouteDependencies<Bindings> = Readonly<{
  authenticate: (
    request: Request,
    env: Bindings,
    pylonRef: string,
  ) => Promise<OwnerManagedEnvironmentEnrollmentActor | undefined>;
  withStore: <A>(
    env: Bindings,
    use: (
      store: Pick<PostgresOwnerManagedEnvironmentEnrollmentStore, "admit" | "read" | "revoke">,
    ) => Promise<A>,
  ) => Promise<A>;
}>;

const response = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });

const projection = (enrollment: OwnerManagedEnvironmentEnrollment) => ({
  enrollment,
});

const idempotencyHash = (request: Request): string | undefined => {
  const key = request.headers.get("idempotency-key")?.trim();
  return key === undefined || key.length < 8 || key.length > 512
    ? undefined
    : `sha256:${createHash("sha256").update(key).digest("hex")}`;
};

export const makeOwnerManagedEnvironmentEnrollmentRoutes = <Bindings>(
  dependencies: OwnerManagedEnvironmentEnrollmentRouteDependencies<Bindings>,
) => ({
  route: (request: Request, env: Bindings): Promise<Response> | undefined => {
    const match = /^\/api\/pylons\/([^/]+)\/owner-managed-environments\/([^/]+)$/.exec(
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
        try {
          const enrollment = await dependencies.withStore(env, (store) =>
            store.read(actor.ownerUserId, targetRef),
          );
          return enrollment === undefined
            ? response({ error: "not_found" }, 404)
            : response(projection(enrollment));
        } catch {
          return response({ error: "owner_managed_enrollment_unavailable" }, 503);
        }
      }
      if (request.method !== "POST" && request.method !== "DELETE") {
        return response({ error: "method_not_allowed" }, 405);
      }
      const hash = idempotencyHash(request);
      if (hash === undefined) return response({ error: "idempotency_required" }, 400);
      let body: typeof OwnerManagedEnvironmentEnrollmentRequestSchema.Type;
      try {
        body = Schema.decodeUnknownSync(OwnerManagedEnvironmentEnrollmentRequestSchema)(
          await request.json(),
          { onExcessProperty: "error" },
        );
      } catch {
        return response({ error: "invalid_request" }, 400);
      }
      try {
        const input = {
          ...body,
          idempotencyKeyHash: hash,
          ownerUserId: actor.ownerUserId,
          ownerAgentUserId: actor.ownerAgentUserId,
          targetRef,
          pylonRef,
        };
        const enrollment = await dependencies.withStore(env, (store) =>
          request.method === "DELETE" ? store.revoke(input) : store.admit(input),
        );
        return response(
          projection(enrollment),
          request.method === "POST" && enrollment.revision === 1 ? 201 : 200,
        );
      } catch (error) {
        const reason =
          error instanceof Error && "reason" in error ? String(error.reason) : "unavailable";
        const status =
          reason === "conflict" || reason === "stale_revision" || reason === "stale_generation"
            ? 409
            : reason === "authority_mismatch"
              ? 403
              : reason === "not_found"
                ? 404
                : 503;
        return response({ error: `owner_managed_enrollment_${reason}` }, status);
      }
    })();
  },
});

import {
  PortablePhaseOperationClaimRequestSchema,
  PortablePhaseOperationRenewRequestSchema,
  PortablePhaseOperationResultRequestSchema,
  PortablePhaseOperationStoreError,
  type PostgresPortablePhaseOperationStore,
  type SyncSql,
} from "@openagentsinc/khala-sync-server";
import { Schema as S } from "effect";

import { methodNotAllowed, noStoreJsonResponse } from "./http/responses";
import { readJsonObject } from "./json-boundary";

type HttpResponse = globalThis.Response;

export const PORTABLE_PHASE_OPERATION_ROUTE_PATTERN =
  "/api/pylons/:pylonRef/portable-targets/:targetRef/phase-operations/:operation?" as const;
export const PORTABLE_PHASE_OPERATION_RESPONSE_SCHEMA =
  "openagents.portable_phase_operation_transport.v1" as const;

export type PortablePhaseRouteActor = Readonly<{
  agentUserId: string;
  ownerUserId: string;
}>;

type PortablePhaseExchange = Pick<
  PostgresPortablePhaseOperationStore,
  "claim" | "complete" | "pending" | "read" | "renew"
>;

export type PortablePhaseOperationRouteDependencies<Bindings> = Readonly<{
  authenticate: (request: Request, env: Bindings) => Promise<PortablePhaseRouteActor | undefined>;
  readPylonOwnerAgentUserId: (env: Bindings, pylonRef: string) => Promise<string | undefined>;
  withExchange: <A>(
    env: Bindings,
    use: (exchange: PortablePhaseExchange) => Promise<A>,
  ) => Promise<A>;
  resolveExactTarget: (
    env: Bindings,
    input: Readonly<{
      ownerUserId: string;
      pylonRef: string;
      targetRef: string;
    }>,
  ) => Promise<"ready" | "unavailable" | "not_found">;
}>;

const response = (
  body: Readonly<Record<string, unknown>>,
  status = 200,
  headers?: HeadersInit,
): HttpResponse =>
  noStoreJsonResponse(
    { schema: PORTABLE_PHASE_OPERATION_RESPONSE_SCHEMA, ...body },
    { status, ...(headers === undefined ? {} : { headers }) },
  );

const errorResponse = (error: unknown): HttpResponse => {
  if (error instanceof PortablePhaseOperationStoreError) {
    if (error.code === "invalid" || error.code === "unsafe_material") {
      return response({ error: "invalid_request", retryable: false }, 400);
    }
    if (error.code === "not_found") {
      return response({ error: "phase_operation_not_found", retryable: false }, 404);
    }
    return response({ error: `phase_operation_${error.code}`, retryable: false }, 409);
  }
  return response({ error: "phase_exchange_unavailable", retryable: true }, 503);
};

const decodeBody = async <A>(request: Request, schema: S.Decoder<A>): Promise<A> =>
  S.decodeUnknownSync(schema)(await readJsonObject(request));
const decodePathRef = S.decodeUnknownSync(
  S.String.check(
    S.isMinLength(3),
    S.isMaxLength(256),
    S.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
  ),
);

const exactPathBinding = (
  body: Readonly<{ pylonRef: string; targetRef: string }>,
  pylonRef: string,
  targetRef: string,
): boolean => body.pylonRef === pylonRef && body.targetRef === targetRef;

/**
 * Resolve only a current server-admitted target/Pylon/owner binding. The
 * binding exists before a phase row, so command dispatch does not depend on
 * the phase row that it is about to create.
 */
export const resolvePortablePhaseTarget = async (
  sql: SyncSql,
  input: Readonly<{
    ownerUserId: string;
    pylonRef: string;
    targetRef: string;
  }>,
): Promise<"ready" | "unavailable" | "not_found"> => {
  const rows: Array<{ health: string }> = await sql`
    SELECT target.health
    FROM khala_sync_portable_targets AS target
    WHERE target.target_ref = ${input.targetRef}
      AND target.owner_user_id = ${input.ownerUserId}
      AND EXISTS (
        SELECT 1
        FROM khala_sync_portable_target_pylon_bindings AS binding
        WHERE binding.owner_user_id = ${input.ownerUserId}
          AND binding.pylon_ref = ${input.pylonRef}
          AND binding.target_ref = ${input.targetRef}
          AND binding.state = 'active'
          AND binding.health IN ('ready', 'draining')
          AND binding.expires_at > CURRENT_TIMESTAMP
      )
    LIMIT 1
  `;
  const row = rows[0];
  if (row === undefined) return "not_found";
  return row.health === "ready" ? "ready" : "unavailable";
};

export const makePortablePhaseOperationRoutes = <Bindings>(
  dependencies: PortablePhaseOperationRouteDependencies<Bindings>,
) => ({
  routePortablePhaseOperationRequest: (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> | undefined => {
    const url = new URL(request.url);
    const match =
      /^\/api\/pylons\/([^/]+)\/portable-targets\/([^/]+)\/phase-operations(?:\/(claim|renew|complete)|\/reconcile\/([^/]+))?$/.exec(
        url.pathname,
      );
    if (match === null) return undefined;

    return (async () => {
      let pylonRef: string;
      let targetRef: string;
      try {
        pylonRef = decodePathRef(decodeURIComponent(match[1]!));
        targetRef = decodePathRef(decodeURIComponent(match[2]!));
      } catch {
        return response({ error: "invalid_path", retryable: false }, 400);
      }
      const operation = match[3];
      let reconcileOperationRef: string | undefined;
      try {
        reconcileOperationRef =
          match[4] === undefined ? undefined : decodePathRef(decodeURIComponent(match[4]));
      } catch {
        return response({ error: "invalid_path", retryable: false }, 400);
      }
      const expectedMethod = operation === undefined ? "GET" : "POST";
      if (request.method !== expectedMethod) {
        return methodNotAllowed([expectedMethod]);
      }

      let actor: PortablePhaseRouteActor | undefined;
      try {
        actor = await dependencies.authenticate(request, env);
      } catch {
        return response({ error: "authentication_unavailable", retryable: true }, 503);
      }
      if (actor === undefined) {
        return response({ error: "authentication_required", retryable: false }, 401, {
          "www-authenticate": "Bearer",
        });
      }
      let registeredOwner: string | undefined;
      try {
        registeredOwner = await dependencies.readPylonOwnerAgentUserId(env, pylonRef);
      } catch {
        return response({ error: "pylon_registry_unavailable", retryable: true }, 503);
      }
      if (registeredOwner === undefined) {
        return response({ error: "pylon_not_registered", retryable: false }, 404);
      }
      if (registeredOwner !== actor.agentUserId) {
        return response({ error: "pylon_not_owned", retryable: false }, 403);
      }

      let targetState: "ready" | "unavailable" | "not_found";
      try {
        targetState = await dependencies.resolveExactTarget(env, {
          ownerUserId: actor.ownerUserId,
          pylonRef,
          targetRef,
        });
      } catch {
        return response({ error: "phase_exchange_unavailable", retryable: true }, 503);
      }
      if (targetState === "not_found") {
        return response({ error: "portable_target_not_authorized", retryable: false }, 403);
      }
      if (targetState !== "ready") {
        return response({ error: "portable_target_unavailable", retryable: true }, 409);
      }

      try {
        if (operation === undefined) {
          if (reconcileOperationRef !== undefined) {
            const exact = await dependencies.withExchange(env, (exchange) =>
              exchange.read(pylonRef, targetRef, reconcileOperationRef),
            );
            return response({ operation: exact, status: "reconciled" });
          }
          const rawLimit = url.searchParams.get("limit") ?? "32";
          const limit = Number(rawLimit);
          if (!Number.isSafeInteger(limit) || limit < 1 || limit > 32) {
            return response({ error: "invalid_limit", retryable: false }, 400);
          }
          const operations = await dependencies.withExchange(env, (exchange) =>
            exchange.pending(pylonRef, targetRef, limit),
          );
          return response({ operations });
        }

        if (operation === "claim") {
          const body = await decodeBody(request, PortablePhaseOperationClaimRequestSchema);
          if (!exactPathBinding(body, pylonRef, targetRef)) {
            return response({ error: "phase_scope_mismatch", retryable: false }, 409);
          }
          const result = await dependencies.withExchange(env, (exchange) => exchange.claim(body));
          return response({ operation: result.operation, status: result.status });
        }

        if (operation === "renew") {
          const body = await decodeBody(request, PortablePhaseOperationRenewRequestSchema);
          if (!exactPathBinding(body, pylonRef, targetRef)) {
            return response({ error: "phase_scope_mismatch", retryable: false }, 409);
          }
          const result = await dependencies.withExchange(env, (exchange) => exchange.renew(body));
          return response({ operation: result.operation, status: result.status });
        }

        const body = await decodeBody(request, PortablePhaseOperationResultRequestSchema);
        if (!exactPathBinding(body, pylonRef, targetRef)) {
          return response({ error: "phase_scope_mismatch", retryable: false }, 409);
        }
        const result = await dependencies.withExchange(env, (exchange) => exchange.complete(body));
        return response({ operation: result.operation, status: result.status });
      } catch (error) {
        return errorResponse(error);
      }
    })();
  },
});

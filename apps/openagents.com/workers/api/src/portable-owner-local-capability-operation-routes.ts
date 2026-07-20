import {
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
  PortableOwnerLocalCapabilityOperationStoreError,
  type PostgresPortableOwnerLocalCapabilityOperationStore,
} from "@openagentsinc/khala-sync-server";
import { Schema } from "effect";

import { methodNotAllowed, noStoreJsonResponse } from "./http/responses";
import { readJsonObject } from "./json-boundary";

type HttpResponse = globalThis.Response;

export const PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_ROUTE_PATTERN =
  "/api/pylons/:pylonRef/portable-targets/:targetRef/capability-operations/:operation?" as const;
export const PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_RESPONSE_SCHEMA =
  "openagents.portable_owner_local_capability_operation_transport.v1" as const;

export type PortableOwnerLocalCapabilityRouteActor = Readonly<{
  agentUserId: string;
  ownerUserId: string;
}>;

type PortableOwnerLocalCapabilityExchange = Pick<
  PostgresPortableOwnerLocalCapabilityOperationStore,
  "claim" | "complete" | "pending" | "read" | "renew"
>;

export type PortableOwnerLocalCapabilityOperationRouteDependencies<Bindings> = Readonly<{
  authenticate: (
    request: Request,
    env: Bindings,
  ) => Promise<PortableOwnerLocalCapabilityRouteActor | undefined>;
  readPylonOwnerAgentUserId: (env: Bindings, pylonRef: string) => Promise<string | undefined>;
  withExchange: <A>(
    env: Bindings,
    use: (exchange: PortableOwnerLocalCapabilityExchange) => Promise<A>,
  ) => Promise<A>;
}>;

const response = (
  body: Readonly<Record<string, unknown>>,
  status = 200,
  headers?: HeadersInit,
): HttpResponse =>
  noStoreJsonResponse(
    {
      schema: PORTABLE_OWNER_LOCAL_CAPABILITY_OPERATION_RESPONSE_SCHEMA,
      ...body,
    },
    { status, ...(headers === undefined ? {} : { headers }) },
  );

const errorResponse = (error: unknown): HttpResponse => {
  if (error instanceof PortableOwnerLocalCapabilityOperationStoreError) {
    if (error.code === "invalid" || error.code === "unsafe_material") {
      return response({ error: "invalid_request", retryable: false }, 400);
    }
    if (error.code === "not_found") {
      return response({ error: "capability_operation_not_found", retryable: false }, 404);
    }
    return response({ error: `capability_operation_${error.code}`, retryable: false }, 409);
  }
  return response({ error: "capability_exchange_unavailable", retryable: true }, 503);
};

const decodeClaimBody = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
);
const decodeRenewBody = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
);
const decodeResultBody = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
);
const decodePathRef = Schema.decodeUnknownSync(
  Schema.String.check(
    Schema.isMinLength(3),
    Schema.isMaxLength(256),
    Schema.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
  ),
);

const exactPathBinding = (
  body: Readonly<{ pylonRef: string; targetRef: string }>,
  pylonRef: string,
  targetRef: string,
): boolean => body.pylonRef === pylonRef && body.targetRef === targetRef;

/**
 * Creates the authenticated outbound-poll route. The caller injects the
 * registered-agent authentication and exact Pylon-owner lookup. Enqueue is
 * intentionally absent from this Pylon-facing surface.
 */
export const makePortableOwnerLocalCapabilityOperationRoutes = <Bindings>(
  dependencies: PortableOwnerLocalCapabilityOperationRouteDependencies<Bindings>,
) => ({
  routePortableOwnerLocalCapabilityOperationRequest: (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> | undefined => {
    const url = new URL(request.url);
    const match =
      /^\/api\/pylons\/([^/]+)\/portable-targets\/([^/]+)\/capability-operations(?:\/(claim|renew|complete)|\/reconcile\/([^/]+))?$/.exec(
        url.pathname,
      );
    if (match === null) return undefined;

    return (async () => {
      let pylonRef: string;
      let targetRef: string;
      let reconcileOperationRef: string | undefined;
      try {
        pylonRef = decodePathRef(decodeURIComponent(match[1]!));
        targetRef = decodePathRef(decodeURIComponent(match[2]!));
        reconcileOperationRef =
          match[4] === undefined ? undefined : decodePathRef(decodeURIComponent(match[4]));
      } catch {
        return response({ error: "invalid_path", retryable: false }, 400);
      }
      const operation = match[3];
      const expectedMethod = operation === undefined ? "GET" : "POST";
      if (request.method !== expectedMethod) return methodNotAllowed([expectedMethod]);

      let actor: PortableOwnerLocalCapabilityRouteActor | undefined;
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

      try {
        if (operation === undefined) {
          if (reconcileOperationRef !== undefined) {
            const exact = await dependencies.withExchange(env, (exchange) =>
              exchange.read(actor.ownerUserId, pylonRef, targetRef, reconcileOperationRef),
            );
            return response({ operation: exact, status: "reconciled" });
          }
          const limit = Number(url.searchParams.get("limit") ?? "32");
          if (!Number.isSafeInteger(limit) || limit < 1 || limit > 32) {
            return response({ error: "invalid_limit", retryable: false }, 400);
          }
          const operations = await dependencies.withExchange(env, (exchange) =>
            exchange.pending(actor.ownerUserId, pylonRef, targetRef, limit),
          );
          return response({ operations });
        }

        if (operation === "claim") {
          const body = decodeClaimBody(await readJsonObject(request));
          if (!exactPathBinding(body, pylonRef, targetRef)) {
            return response({ error: "capability_scope_mismatch", retryable: false }, 409);
          }
          const result = await dependencies.withExchange(env, (exchange) =>
            exchange.claim(actor.ownerUserId, body),
          );
          return response({ operation: result.operation, status: result.status });
        }

        if (operation === "renew") {
          const body = decodeRenewBody(await readJsonObject(request));
          if (!exactPathBinding(body, pylonRef, targetRef)) {
            return response({ error: "capability_scope_mismatch", retryable: false }, 409);
          }
          const result = await dependencies.withExchange(env, (exchange) =>
            exchange.renew(actor.ownerUserId, body),
          );
          return response({ operation: result.operation, status: result.status });
        }

        const body = decodeResultBody(await readJsonObject(request));
        if (!exactPathBinding(body, pylonRef, targetRef)) {
          return response({ error: "capability_scope_mismatch", retryable: false }, 409);
        }
        const result = await dependencies.withExchange(env, (exchange) =>
          exchange.complete(actor.ownerUserId, body),
        );
        return response({ operation: result.operation, status: result.status });
      } catch (error) {
        return errorResponse(error);
      }
    })();
  },
});

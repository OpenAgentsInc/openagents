import {
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  PortableOwnerLocalCapabilityOperationRenewRequestSchema,
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
  PortableOwnerLocalCapabilityOperationStoreError,
  type PostgresPortableOwnerLocalCapabilityOperationStore,
} from "@openagentsinc/khala-sync-server";
import {
  PortableOwnerLocalCapabilityMaterialRedemptionRequestSchema,
  type PortableOwnerLocalCapabilityKind,
  type PortableOwnerLocalCapabilityMaterialRedemptionRequest,
} from "@openagentsinc/portable-session-contract";
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

export type PortableOwnerLocalCapabilityMaterialAuthority = Readonly<
  PortableOwnerLocalCapabilityMaterialRedemptionRequest & {
    actorAgentUserId: string;
    ownerRef: string;
    capability: PortableOwnerLocalCapabilityKind;
    permissionRefs: ReadonlyArray<string>;
    operationExpiresAt: string;
    sourceGrantRef: string;
  }
>;

const MAX_CAPABILITY_MATERIAL_BYTES = 1024 * 1024;

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
  /**
   * Resolves the exact destination grant without an HTTP service bearer. The
   * implementation must recheck the command claim, owner-local target/Pylon
   * binding, and operation claim immediately before and after it reads bytes.
   */
  redeemDestinationGrantMaterial: (
    env: Bindings,
    authority: PortableOwnerLocalCapabilityMaterialAuthority,
  ) => Promise<Uint8Array>;
  now?: (() => Date) | undefined;
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
const decodeMaterialBody = Schema.decodeUnknownSync(
  PortableOwnerLocalCapabilityMaterialRedemptionRequestSchema,
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

const materialResponse = (material: Uint8Array): HttpResponse => {
  if (material.byteLength === 0 || material.byteLength > MAX_CAPABILITY_MATERIAL_BYTES) {
    material.fill(0);
    throw new Error("capability material size is invalid");
  }
  try {
    // Response owns this copy. The authority-owned source buffer is cleared as
    // soon as the one-shot transport body is constructed.
    return new Response(material.slice(), {
      status: 200,
      headers: {
        "cache-control": "no-store",
        "content-type": "application/octet-stream",
        pragma: "no-cache",
        "x-content-type-options": "nosniff",
      },
    });
  } finally {
    material.fill(0);
  }
};

const exactMaterialAuthority = (
  record: Awaited<ReturnType<PortableOwnerLocalCapabilityExchange["read"]>>,
  body: PortableOwnerLocalCapabilityMaterialRedemptionRequest,
  actorAgentUserId: string,
  now: Date,
): PortableOwnerLocalCapabilityMaterialAuthority | undefined => {
  const request = record.request;
  if (
    request.action !== "install" ||
    request.capability === null ||
    record.state !== "claimed" ||
    record.claimRef !== body.claimRef ||
    record.workerInstanceRef !== body.workerInstanceRef ||
    record.claimGeneration !== body.claimGeneration ||
    record.leaseRevision !== body.expectedLeaseRevision ||
    record.leaseExpiresAt !== body.expectedLeaseExpiresAt ||
    request.operationRef !== body.operationRef ||
    request.commandExecutionClaimRef !== body.commandExecutionClaimRef ||
    request.pylonRef !== body.pylonRef ||
    request.targetRef !== body.targetRef ||
    request.sessionRef !== body.sessionRef ||
    request.attachmentRef !== body.attachmentRef ||
    request.attachmentGeneration !== body.attachmentGeneration ||
    request.destinationGrantRef !== body.destinationGrantRef ||
    Date.parse(record.leaseExpiresAt) <= now.getTime() ||
    Date.parse(request.expiresAt) <= now.getTime()
  ) {
    return undefined;
  }
  return {
    ...body,
    actorAgentUserId,
    ownerRef: request.ownerRef,
    capability: request.capability,
    permissionRefs: request.permissionRefs,
    operationExpiresAt: request.expiresAt,
    sourceGrantRef: request.sourceGrantRef,
  };
};

export const portableOwnerLocalCapabilityMaterialAuthorityMatchesRecord = (
  record: Awaited<ReturnType<PortableOwnerLocalCapabilityExchange["read"]>>,
  authority: PortableOwnerLocalCapabilityMaterialAuthority,
  now: Date,
): boolean => {
  const exact = exactMaterialAuthority(record, authority, authority.actorAgentUserId, now);
  return (
    exact !== undefined &&
    exact.ownerRef === authority.ownerRef &&
    exact.capability === authority.capability &&
    JSON.stringify(exact.permissionRefs) === JSON.stringify(authority.permissionRefs) &&
    exact.operationExpiresAt === authority.operationExpiresAt &&
    exact.sourceGrantRef === authority.sourceGrantRef
  );
};

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
      /^\/api\/pylons\/([^/]+)\/portable-targets\/([^/]+)\/capability-operations(?:\/(claim|renew|complete)|\/reconcile\/([^/]+)|\/([^/]+)\/material)?$/.exec(
        url.pathname,
      );
    if (match === null) return undefined;

    return (async () => {
      let pylonRef: string;
      let targetRef: string;
      let reconcileOperationRef: string | undefined;
      let materialOperationRef: string | undefined;
      try {
        pylonRef = decodePathRef(decodeURIComponent(match[1]!));
        targetRef = decodePathRef(decodeURIComponent(match[2]!));
        reconcileOperationRef =
          match[4] === undefined ? undefined : decodePathRef(decodeURIComponent(match[4]));
        materialOperationRef =
          match[5] === undefined ? undefined : decodePathRef(decodeURIComponent(match[5]));
      } catch {
        return response({ error: "invalid_path", retryable: false }, 400);
      }
      const operation = match[3];
      const expectedMethod =
        operation === undefined && materialOperationRef === undefined ? "GET" : "POST";
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
        if (materialOperationRef !== undefined) {
          let body: PortableOwnerLocalCapabilityMaterialRedemptionRequest;
          try {
            body = decodeMaterialBody(await readJsonObject(request), {
              onExcessProperty: "error",
            });
          } catch {
            return response({ error: "invalid_request", retryable: false }, 400);
          }
          if (
            !exactPathBinding(body, pylonRef, targetRef) ||
            body.operationRef !== materialOperationRef
          ) {
            return response({ error: "capability_scope_mismatch", retryable: false }, 409);
          }
          const exact = await dependencies.withExchange(env, (exchange) =>
            exchange.read(actor.ownerUserId, pylonRef, targetRef, materialOperationRef),
          );
          const authority = exactMaterialAuthority(
            exact,
            body,
            actor.agentUserId,
            (dependencies.now ?? (() => new Date()))(),
          );
          if (authority === undefined || authority.ownerRef !== actor.ownerUserId) {
            return response({ error: "capability_material_authority_lost", retryable: false }, 409);
          }
          const material = await dependencies.redeemDestinationGrantMaterial(env, authority);
          try {
            const finalRecord = await dependencies.withExchange(env, (exchange) =>
              exchange.read(actor.ownerUserId, pylonRef, targetRef, materialOperationRef),
            );
            if (
              !portableOwnerLocalCapabilityMaterialAuthorityMatchesRecord(
                finalRecord,
                authority,
                (dependencies.now ?? (() => new Date()))(),
              )
            ) {
              material.fill(0);
              return response(
                { error: "capability_material_authority_lost", retryable: false },
                409,
              );
            }
            return materialResponse(material);
          } catch (error) {
            material.fill(0);
            throw error;
          }
        }

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

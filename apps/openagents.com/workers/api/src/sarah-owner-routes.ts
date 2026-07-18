import { decodePushRequest } from "@openagentsinc/khala-sync";
import { resolveAuthorityDecision } from "@openagentsinc/authority";
import {
  chatMutators,
  executePush,
  makeMutatorRegistry,
  type SyncSql,
} from "@openagentsinc/khala-sync-server";
import {
  ROOT_AUTHORITY_PROFILE_REF,
  ROOT_AUTHORITY_REVISION,
  SARAH_AUTHORITY_PROFILE_REF,
  SARAH_AUTHORITY_REVISION,
  SARAH_CAPABILITIES,
  SARAH_PRINCIPAL_SCHEMA,
  SARAH_RUNTIME_AUTHORITY_PROFILE,
  type SarahPrincipalProjection,
} from "@openagentsinc/sarah";
import { Effect, Schema as S } from "effect";

import {
  type HttpHeadersDecorator,
  type JsonHttpResult,
  decorateJsonHttpResultHeaders,
  methodNotAllowedResult,
  noStoreJsonResult,
} from "./http/responses";
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from "./khala-sync-push-routes";
import { defaultMakeKhalaSyncSqlClient } from "./khala-sync-push-routes";

export const SARAH_OWNER_PATH = "/api/mobile/sarah";
export const SARAH_OWNER_ROUTE_REF = "route.mobile.sarah.principal.v1";

export type SarahOwnerAuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

type SarahOwnerRouteEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined;
}>;

export type SarahOwnerRouteDependencies<Bindings extends SarahOwnerRouteEnv> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<SarahOwnerAuthenticatedOwner | undefined>;
  bindingForEnv?: ((env: Bindings) => KhalaSyncHyperdriveBinding | undefined) | undefined;
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined;
  ensurePrincipal?:
    | ((sql: SyncSql, ownerUserId: string) => Promise<SarahPrincipalProjection>)
    | undefined;
}>;

class SarahOwnerStorageError extends S.TaggedErrorClass<SarahOwnerStorageError>()(
  "SarahOwnerStorageError",
  { cause: S.Defect() },
) {}

const digestOwner = async (ownerUserId: string): Promise<string> => {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(ownerUserId));
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("");
};

/** Stable and opaque across devices. The raw owner id never enters the ref. */
export const sarahThreadRefForOwner = async (ownerUserId: string): Promise<string> =>
  `thread.sarah.${(await digestOwner(ownerUserId)).slice(0, 24)}`;

export const isSarahThreadForOwner = async (
  ownerUserId: string,
  threadRef: string,
): Promise<boolean> => threadRef === (await sarahThreadRefForOwner(ownerUserId));

/**
 * A Sarah-shaped thread id is not authority by itself. Hosted inference only
 * adopts the Sarah principal after the authenticated owner route has emitted
 * the admitted bootstrap receipt for this exact owner and thread.
 */
export const hasSarahThreadAuthority = async (
  sql: SyncSql,
  ownerUserId: string,
  threadRef: string,
): Promise<boolean> => {
  if (!(await isSarahThreadForOwner(ownerUserId, threadRef))) return false;
  const rows: Array<{ receipt_ref: string }> = await sql`
    SELECT receipt_ref
      FROM sarah_authority_decision_receipts
     WHERE owner_user_id = ${ownerUserId}
       AND thread_ref = ${threadRef}
       AND profile_ref = ${SARAH_AUTHORITY_PROFILE_REF}
       AND profile_revision = ${SARAH_AUTHORITY_REVISION}
       AND action_ref = 'maintain_owner_contact'
       AND outcome = 'succeeded'
     LIMIT 1
  `;
  return rows[0] !== undefined;
};

export const ensureSarahPrincipal = async (
  sql: SyncSql,
  ownerUserId: string,
): Promise<SarahPrincipalProjection> => {
  const threadRef = await sarahThreadRefForOwner(ownerUserId);
  const startedAt = new Date();
  const decision = await Effect.runPromise(
    resolveAuthorityDecision(SARAH_RUNTIME_AUTHORITY_PROFILE, {
      requestRef: `request.sarah.bootstrap.${threadRef}`,
      actorRef: "principal.sarah",
      actorRole: "sarah_orchestrator",
      action: "maintain_owner_contact",
      resource: "owner_private_conversation",
      programRef: "program.sarah_company_operations",
      triggerRef: "current_owner_direction_2026-07-18_sarah_reboot",
      conditionResults: [
        {
          conditionRef: "condition.owner_scope",
          passed: true,
          evidenceRefs: [`owner_scope:${threadRef}`],
        },
        {
          conditionRef: "condition.redaction",
          passed: true,
          evidenceRefs: ["schema:openagents.sarah.principal.v1"],
        },
        {
          conditionRef: "condition.citations",
          passed: true,
          evidenceRefs: ["context:source_refs_required"],
        },
      ],
      startedAt: startedAt.toISOString(),
    }),
  );
  if (decision._tag !== "Allowed") {
    throw new SarahOwnerStorageError({ cause: decision.reason });
  }
  const rows: Array<{ thread_id: string }> = await sql`
    SELECT thread_id
      FROM khala_sync_chat_threads
     WHERE thread_id = ${threadRef}
       AND owner_user_id = ${ownerUserId}
     LIMIT 1
  `;
  if (rows[0] === undefined) {
    const response = await executePush({
      registry: makeMutatorRegistry([...chatMutators]),
      request: decodePushRequest({
        clientGroupId: `server.sarah.${threadRef}`,
        clientId: `server.sarah.bootstrap.${threadRef}`,
        mutations: [
          {
            argsJson: JSON.stringify({ threadId: threadRef, title: "Sarah" }),
            mutationId: 1,
            name: "chat.createThread",
          },
        ],
        protocolVersion: 1,
        schemaVersion: 1,
      }),
      sql,
      userId: ownerUserId,
    });
    const result = response.results[0];
    if (
      result === undefined ||
      (result.status !== "applied" && result.errorCode !== "thread_exists")
    ) {
      throw new SarahOwnerStorageError({ cause: result ?? "missing_result" });
    }
  }
  const settledAt = new Date().toISOString();
  await sql`
    INSERT INTO sarah_authority_decision_receipts
      (receipt_ref, owner_user_id, thread_ref, profile_ref, profile_revision,
       grant_ref, action_ref, outcome, evidence_refs_json, started_at, settled_at)
    VALUES
      (${`receipt.authority.sarah.bootstrap.${threadRef.slice(-24)}`},
       ${ownerUserId}, ${threadRef}, ${decision.profileRef},
       ${decision.profileRevision}, ${decision.grantRef},
       ${decision.request.action}, 'succeeded',
       ${JSON.stringify(["schema:openagents.sarah.principal.v1"])}::text::jsonb,
       ${startedAt.toISOString()}, ${settledAt})
    ON CONFLICT (receipt_ref) DO NOTHING
  `;
  return {
    schema: SARAH_PRINCIPAL_SCHEMA,
    principalRef: "principal.sarah",
    displayName: "Sarah",
    role: "Owner orchestrator",
    threadRef,
    authorityProfileRef: SARAH_AUTHORITY_PROFILE_REF,
    authorityRevision: SARAH_AUTHORITY_REVISION,
    rootAuthorityProfileRef: ROOT_AUTHORITY_PROFILE_REF,
    rootAuthorityRevision: ROOT_AUTHORITY_REVISION,
    memory: "durable_cited",
    capabilities: SARAH_CAPABILITIES,
  };
};

const bindingConnectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | undefined => {
  const value = binding?.connectionString;
  return typeof value === "string" && value.length > 0 ? value : undefined;
};

export const makeSarahOwnerRoutes = <Bindings extends SarahOwnerRouteEnv>(
  dependencies: SarahOwnerRouteDependencies<Bindings>,
) => ({
  handle: (request: Request, env: Bindings, ctx: ExecutionContext): Effect.Effect<JsonHttpResult> =>
    Effect.gen(function* () {
      if (request.method !== "POST" && request.method !== "GET") {
        return methodNotAllowedResult(["GET", "POST"]);
      }
      const authentication = yield* Effect.tryPromise({
        try: () => dependencies.authenticateOwner(request, env, ctx),
        catch: (cause) => new SarahOwnerStorageError({ cause }),
      }).pipe(Effect.option);
      if (authentication._tag === "None") {
        return noStoreJsonResult(
          { ok: false, error: "authentication_unavailable" },
          { status: 503 },
        );
      }
      const owner = authentication.value;
      if (owner === undefined) {
        return noStoreJsonResult({ ok: false, error: "unauthenticated" }, { status: 401 });
      }
      const respond = <Body>(result: JsonHttpResult<Body>): JsonHttpResult<Body> =>
        owner.decorateResponseHeaders === undefined
          ? result
          : decorateJsonHttpResultHeaders(result, owner.decorateResponseHeaders);
      if ([...new URL(request.url).searchParams.keys()].length > 0) {
        return respond(noStoreJsonResult({ ok: false, error: "invalid_request" }, { status: 400 }));
      }
      const connectionString = bindingConnectionString(
        (dependencies.bindingForEnv ?? ((value) => value.KHALA_SYNC_DB))(env),
      );
      if (connectionString === undefined) {
        return respond(
          noStoreJsonResult({ ok: false, error: "storage_unavailable" }, { status: 503 }),
        );
      }
      const clientResult = yield* Effect.tryPromise({
        try: () => (dependencies.makeSqlClient ?? defaultMakeKhalaSyncSqlClient)(connectionString),
        catch: (cause) => new SarahOwnerStorageError({ cause }),
      }).pipe(Effect.option);
      if (clientResult._tag === "None") {
        return respond(
          noStoreJsonResult({ ok: false, error: "storage_unavailable" }, { status: 503 }),
        );
      }
      const client: KhalaSyncPushSqlClient = clientResult.value;
      const projection = yield* Effect.tryPromise({
        try: () => (dependencies.ensurePrincipal ?? ensureSarahPrincipal)(client.sql, owner.userId),
        catch: (cause) => new SarahOwnerStorageError({ cause }),
      }).pipe(
        Effect.option,
        Effect.ensuring(Effect.promise(() => client.end()).pipe(Effect.ignore)),
      );
      return projection._tag === "None"
        ? respond(noStoreJsonResult({ ok: false, error: "storage_unavailable" }, { status: 503 }))
        : respond(
            noStoreJsonResult({
              ok: true,
              routeRef: SARAH_OWNER_ROUTE_REF,
              principal: projection.value,
            }),
          );
    }),
});

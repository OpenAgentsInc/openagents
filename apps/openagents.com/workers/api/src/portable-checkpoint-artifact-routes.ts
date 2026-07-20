import {
  PortableCommandExecutionClaimSchema,
  type PortableCommandExecutionClaim,
  type PortablePhaseOperationRecord,
} from "@openagentsinc/portable-session-contract";
import type {
  PostgresPortablePhaseOperationStore,
  SyncSql,
} from "@openagentsinc/khala-sync-server";
import { GcsHmacClient } from "@openagentsinc/oa-infra/blob-store-gcs-hmac";
import { Effect, Schema as S } from "effect";

import { methodNotAllowed, noStoreJsonResponse } from "./http/responses";
import { readJsonObject } from "./json-boundary";
import { artifactsBucketForEnv, type ArtifactsEnv } from "./artifacts-binding";
import {
  PortableCheckpointArtifactError,
  makePortableCheckpointArtifactService,
  type PortableCheckpointArtifactAuthority,
  type PortableCheckpointArtifactBucket,
} from "./portable-checkpoint-artifact-service";
import type { PortablePhaseRouteActor } from "./portable-phase-operation-routes";

type HttpResponse = globalThis.Response;

export const PORTABLE_CHECKPOINT_ARTIFACT_ROUTE_PATTERN =
  "/api/pylons/:pylonRef/portable-targets/:targetRef/checkpoint-artifacts/:manifestDigest?/:operation?" as const;
export const PORTABLE_CHECKPOINT_ARTIFACT_RESPONSE_SCHEMA =
  "openagents.portable_checkpoint_artifact_transport.v1" as const;

const MAXIMUM_OBJECT_BYTES = 64 * 1024 * 1024;

const PortableRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
const PrepareRequestSchema = S.Struct({
  operationRef: PortableRef,
  manifest: S.Unknown,
});
const OperationRequestSchema = S.Struct({ operationRef: PortableRef });
const DownloadRequestSchema = S.Struct({
  operationRef: PortableRef,
  redemptionRef: PortableRef,
});

type PhaseReader = Pick<PostgresPortablePhaseOperationStore, "read">;

export type PortableCheckpointArtifactRouteDependencies<Bindings> = Readonly<{
  authenticate: (request: Request, env: Bindings) => Promise<PortablePhaseRouteActor | undefined>;
  readPylonOwnerAgentUserId: (env: Bindings, pylonRef: string) => Promise<string | undefined>;
  resolveExactTarget: (
    env: Bindings,
    input: Readonly<{ ownerUserId: string; pylonRef: string; targetRef: string }>,
  ) => Promise<"ready" | "unavailable" | "not_found">;
  bucket: (env: Bindings) => PortableCheckpointArtifactBucket;
  readAuthority: (
    env: Bindings,
    input: Readonly<{
      pylonRef: string;
      targetRef: string;
      operationRef: string;
    }>,
  ) => Promise<PortableCheckpointArtifactAuthority>;
  now?: () => Date;
  maximumObjectBytes?: number;
}>;

const response = (body: Readonly<Record<string, unknown>>, status = 200): HttpResponse =>
  noStoreJsonResponse(
    { schema: PORTABLE_CHECKPOINT_ARTIFACT_RESPONSE_SCHEMA, ...body },
    { status },
  );

const errorResponse = (error: unknown): HttpResponse => {
  if (error instanceof PortableCheckpointArtifactError) {
    if (error.code === "invalid" || error.code === "too_large") {
      return response({ error: `checkpoint_artifact_${error.code}`, retryable: false }, 400);
    }
    if (error.code === "not_found") {
      return response({ error: "checkpoint_artifact_not_found", retryable: false }, 404);
    }
    if (error.code === "unavailable") {
      return response({ error: "checkpoint_artifact_unavailable", retryable: true }, 503);
    }
    return response({ error: `checkpoint_artifact_${error.code}`, retryable: false }, 409);
  }
  return response({ error: "checkpoint_artifact_unavailable", retryable: true }, 503);
};

const decodePathRef = S.decodeUnknownSync(PortableRef);
const decodeManifestHex = S.decodeUnknownSync(S.String.check(S.isPattern(/^[0-9a-f]{64}$/)));

const readBody = async <A>(request: Request, schema: S.Decoder<A>): Promise<A> => {
  try {
    return S.decodeUnknownSync(schema)(await readJsonObject(request));
  } catch {
    throw new PortableCheckpointArtifactError({
      code: "invalid",
      operation: "request_body",
    });
  }
};

const readBoundedBytes = async (request: Request, maximumBytes: number): Promise<Uint8Array> => {
  const contentLength = request.headers.get("content-length");
  if (
    contentLength !== null &&
    (!/^\d+$/.test(contentLength) || Number(contentLength) > maximumBytes)
  ) {
    throw new PortableCheckpointArtifactError({ code: "too_large", operation: "upload_body" });
  }
  if (request.body === null) {
    throw new PortableCheckpointArtifactError({ code: "invalid", operation: "upload_body" });
  }
  const reader = request.body.getReader();
  const chunks: Array<Uint8Array> = [];
  let size = 0;
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) break;
      size += next.value.byteLength;
      if (size > maximumBytes) {
        await reader.cancel();
        throw new PortableCheckpointArtifactError({
          code: "too_large",
          operation: "upload_body",
        });
      }
      chunks.push(next.value);
    }
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
};

export const portableCheckpointArtifactBucketFromR2 = (
  bucket: R2Bucket,
): PortableCheckpointArtifactBucket => ({
  get: async (key) => {
    const object = await bucket.get(key);
    if (object === null) return null;
    return {
      size: object.size,
      bytes: async () => new Uint8Array(await object.arrayBuffer()),
    };
  },
  putIfAbsent: async (key, bytes, options) => {
    if ((await bucket.head(key)) !== null) return "exists";
    await bucket.put(key, bytes, { httpMetadata: { contentType: options.contentType } });
    return "created";
  },
  delete: (key) => bucket.delete(key),
});

/** Production uses the GCS create-only primitive. The R2 seam is test/local only. */
export const portableCheckpointArtifactBucketForEnv = (
  env: ArtifactsEnv,
): PortableCheckpointArtifactBucket => {
  const bucket = env.ARTIFACTS_GCS_BUCKET;
  const accessKeyId = env.ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID;
  const secretAccessKey = env.ARTIFACTS_GCS_HMAC_SECRET;
  if (
    bucket !== undefined &&
    bucket !== "" &&
    accessKeyId !== undefined &&
    accessKeyId !== "" &&
    secretAccessKey !== undefined &&
    secretAccessKey !== ""
  ) {
    const client = new GcsHmacClient({
      accessKeyId,
      bucket,
      secretAccessKey,
      ...(env.ARTIFACTS_GCS_ENDPOINT === undefined ? {} : { endpoint: env.ARTIFACTS_GCS_ENDPOINT }),
    });
    return {
      get: async (key) => {
        const response = await client.getObject(key);
        if (response === null) return null;
        const bytes = new Uint8Array(await response.arrayBuffer());
        return { size: bytes.byteLength, bytes: async () => Uint8Array.from(bytes) };
      },
      putIfAbsent: async (key, bytes, options) => {
        const result = await client.putObject(key, bytes, {
          contentType: options.contentType,
          ifAbsent: true,
        });
        return result.created ? "created" : "exists";
      },
      delete: (key) => client.deleteObject(key),
    };
  }
  return portableCheckpointArtifactBucketFromR2(artifactsBucketForEnv(env));
};

type CommandExecutionRow = Readonly<{
  claim_ref: string;
  command_ref: string;
  owner_user_id: string;
  session_ref: string;
  command_kind: string;
  command_fingerprint: string;
  claim_fingerprint: string;
  source_attachment_ref: string;
  source_generation: string | number;
  destination_target_ref: string;
  executor_environment_ref: string;
  worker_instance_ref: string;
  claim_generation: string | number;
  lease_revision: string | number;
  state: string;
  claimed_at: Date | string;
  lease_expires_at: Date | string;
  updated_at: Date | string;
  terminal_status: string | null;
  pending_reconcile_ref: string | null;
  outcome_ref: string | null;
  evidence_refs_json: unknown;
}>;

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;

/** Read the current command claim. A manifest copy is never command authority. */
export const readPortableCheckpointCommandClaim = async (
  sql: SyncSql,
  claimRef: string,
): Promise<PortableCommandExecutionClaim> => {
  const rows: Array<CommandExecutionRow> = await sql`
    SELECT claim_ref, command_ref, owner_user_id, session_ref, command_kind,
           command_fingerprint, claim_fingerprint, source_attachment_ref,
           source_generation, destination_target_ref, executor_environment_ref,
           worker_instance_ref, claim_generation, lease_revision, state,
           claimed_at, lease_expires_at, updated_at, terminal_status,
           pending_reconcile_ref, outcome_ref, evidence_refs_json
    FROM khala_sync_portable_command_executions
    WHERE claim_ref = ${claimRef}
    LIMIT 1
  `;
  const row = rows[0];
  if (row === undefined) {
    throw new PortableCheckpointArtifactError({ code: "stale_claim", operation: "command_claim" });
  }
  try {
    return S.decodeUnknownSync(PortableCommandExecutionClaimSchema)({
      schema: "openagents.portable_command_execution.v1",
      claimRef: row.claim_ref,
      commandRef: row.command_ref,
      ownerRef: row.owner_user_id,
      sessionRef: row.session_ref,
      commandKind: row.command_kind,
      commandFingerprint: row.command_fingerprint,
      claimFingerprint: row.claim_fingerprint,
      sourceAttachmentRef: row.source_attachment_ref,
      sourceGeneration: Number(row.source_generation),
      destinationTargetRef: row.destination_target_ref,
      executorEnvironmentRef: row.executor_environment_ref,
      workerInstanceRef: row.worker_instance_ref,
      claimGeneration: Number(row.claim_generation),
      leaseRevision: Number(row.lease_revision),
      state: row.state,
      claimedAt: new Date(row.claimed_at).toISOString(),
      leaseExpiresAt: new Date(row.lease_expires_at).toISOString(),
      updatedAt: new Date(row.updated_at).toISOString(),
      terminalStatus: row.terminal_status,
      pendingReconcileRef: row.pending_reconcile_ref,
      outcomeRef: row.outcome_ref,
      evidenceRefs: parseJson(row.evidence_refs_json),
    });
  } catch {
    throw new PortableCheckpointArtifactError({ code: "stale_claim", operation: "command_claim" });
  }
};

/** Resolve phase and command authority from one database connection. */
export const readPortableCheckpointArtifactAuthority = async (
  sql: SyncSql,
  phaseReader: PhaseReader,
  input: Readonly<{ pylonRef: string; targetRef: string; operationRef: string }>,
): Promise<PortableCheckpointArtifactAuthority> => {
  const operation: PortablePhaseOperationRecord = await phaseReader.read(
    input.pylonRef,
    input.targetRef,
    input.operationRef,
  );
  const commandClaim = await readPortableCheckpointCommandClaim(
    sql,
    operation.request.commandExecutionClaimRef,
  );
  return { operation, commandClaim };
};

export const makePortableCheckpointArtifactRoutes = <Bindings>(
  dependencies: PortableCheckpointArtifactRouteDependencies<Bindings>,
) => ({
  routePortableCheckpointArtifactRequest: (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> | undefined => {
    const url = new URL(request.url);
    const match =
      /^\/api\/pylons\/([^/]+)\/portable-targets\/([^/]+)\/checkpoint-artifacts(?:\/(prepare)|\/([0-9a-f]{64})\/(upload|commit|redeem|download|delete))?$/.exec(
        url.pathname,
      );
    if (match === null) return undefined;

    return (async () => {
      let pylonRef: string;
      let targetRef: string;
      let manifestHex: string | undefined;
      try {
        pylonRef = decodePathRef(decodeURIComponent(match[1]!));
        targetRef = decodePathRef(decodeURIComponent(match[2]!));
        manifestHex = match[4] === undefined ? undefined : decodeManifestHex(match[4]);
      } catch {
        return response({ error: "invalid_path", retryable: false }, 400);
      }
      const action = match[3] ?? match[5];
      const expectedMethod = action === "upload" ? "PUT" : "POST";
      if (action === undefined || request.method !== expectedMethod) {
        return methodNotAllowed([expectedMethod]);
      }

      let actor: PortablePhaseRouteActor | undefined;
      try {
        actor = await dependencies.authenticate(request, env);
      } catch {
        return response({ error: "authentication_unavailable", retryable: true }, 503);
      }
      if (actor === undefined) {
        return noStoreJsonResponse(
          {
            schema: PORTABLE_CHECKPOINT_ARTIFACT_RESPONSE_SCHEMA,
            error: "authentication_required",
            retryable: false,
          },
          { status: 401, headers: { "www-authenticate": "Bearer" } },
        );
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
        return response({ error: "checkpoint_artifact_unavailable", retryable: true }, 503);
      }
      if (targetState === "not_found") {
        return response({ error: "portable_target_not_authorized", retryable: false }, 403);
      }
      if (targetState !== "ready") {
        return response({ error: "portable_target_unavailable", retryable: true }, 409);
      }

      const maximumObjectBytes = dependencies.maximumObjectBytes ?? MAXIMUM_OBJECT_BYTES;
      const service = makePortableCheckpointArtifactService({
        bucket: dependencies.bucket(env),
        readAuthority: (inputPylonRef, inputTargetRef, operationRef) =>
          dependencies.readAuthority(env, {
            pylonRef: inputPylonRef,
            targetRef: inputTargetRef,
            operationRef,
          }),
        ...(dependencies.now === undefined ? {} : { now: dependencies.now }),
        maximumObjectBytes,
      });
      const scope = { ownerRef: actor.ownerUserId, pylonRef, targetRef };
      const manifestDigest = manifestHex === undefined ? undefined : `sha256:${manifestHex}`;

      try {
        if (action === "prepare") {
          const body = await readBody(request, PrepareRequestSchema);
          const prepared = await Effect.runPromise(
            service.prepare(scope, body.operationRef, body.manifest),
          );
          const path = `${url.pathname.replace(/\/prepare$/, "")}/${prepared.manifestDigest.slice(7)}/upload`;
          return response({
            status: "prepared",
            ...prepared,
            upload: {
              transport: "server_mediated",
              method: "PUT",
              path,
              contentType: "application/octet-stream",
              operationRefHeader: "x-openagents-operation-ref",
              expiresAt: prepared.expiresAt,
            },
          });
        }
        if (manifestDigest === undefined) {
          return response({ error: "invalid_path", retryable: false }, 400);
        }
        if (action === "upload") {
          let operationRef: string;
          try {
            operationRef = decodePathRef(request.headers.get("x-openagents-operation-ref"));
          } catch {
            throw new PortableCheckpointArtifactError({
              code: "invalid",
              operation: "operation_ref_header",
            });
          }
          const bytes = await readBoundedBytes(request, maximumObjectBytes);
          const uploaded = await Effect.runPromise(
            service.upload(scope, manifestDigest, operationRef, bytes),
          );
          return response({ status: "uploaded", ...uploaded });
        }
        if (action === "download") {
          const body = await readBody(request, DownloadRequestSchema);
          const bytes = await Effect.runPromise(
            service.download(scope, manifestDigest, body.operationRef, body.redemptionRef),
          );
          return new Response(Uint8Array.from(bytes).buffer, {
            status: 200,
            headers: {
              "cache-control": "no-store",
              "content-type": "application/octet-stream",
              "content-length": String(bytes.byteLength),
              "x-content-type-options": "nosniff",
            },
          });
        }
        const body = await readBody(request, OperationRequestSchema);
        if (action === "commit") {
          return response({
            status: "committed",
            ...(await Effect.runPromise(service.commit(scope, manifestDigest, body.operationRef))),
          });
        }
        if (action === "redeem") {
          return response({
            status: "redeemed",
            ...(await Effect.runPromise(service.redeem(scope, manifestDigest, body.operationRef))),
            download: {
              transport: "server_mediated",
              method: "POST",
              path: `${url.pathname.replace(/\/redeem$/, "")}/download`,
            },
          });
        }
        return response({
          ...(await Effect.runPromise(
            service.deleteObject(scope, manifestDigest, body.operationRef),
          )),
        });
      } catch (error) {
        return errorResponse(error);
      }
    })();
  },
});

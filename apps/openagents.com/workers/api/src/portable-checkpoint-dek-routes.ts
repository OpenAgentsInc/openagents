import { canonicalJson } from "@openagentsinc/khala-sync";
import type { SyncSql } from "@openagentsinc/khala-sync-server";
import {
  PORTABLE_CHECKPOINT_DEK_AUTHORITY_SCHEMA_VERSION,
  PortableCheckpointDekAuthoritySchema,
  type PortableCheckpointDekAuthority,
  type PortableCommandExecutionClaim,
  type PortablePhaseOperationRecord,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import type { GoogleCloudKmsDekClient } from "./google-cloud-kms";
import { methodNotAllowed, noStoreJsonResponse } from "./http/responses";
import type { PortablePhaseRouteActor } from "./portable-phase-operation-routes";

type HttpResponse = globalThis.Response;

export const PORTABLE_CHECKPOINT_DEK_ROUTE_PATTERN =
  "/api/pylons/:pylonRef/portable-targets/:targetRef/checkpoint-deks/:operationRef/:action" as const;
const RESPONSE_SCHEMA = "openagents.portable_checkpoint_dek_transport.v1" as const;
const MAX_WRAPPED_DEK_BYTES = 128 * 1024;

const PortableRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const decodeRef = Schema.decodeUnknownSync(PortableRef);
const decodeAuthority = Schema.decodeUnknownSync(PortableCheckpointDekAuthoritySchema);

export type PortableCheckpointDekCurrentAuthority = Readonly<{
  operation: PortablePhaseOperationRecord;
  commandClaim: PortableCommandExecutionClaim;
}>;

export type PortableCheckpointDekWrapBinding = Readonly<{
  schema: "openagents.portable_checkpoint_dek_wrap_binding.v1";
  algorithm: "aes-256-gcm+google-kms-wrapped-dek";
  policy: "openagents_managed";
  operationRef: string;
  commandExecutionClaimRef: string;
  ownerRef: string;
  pylonRef: string;
  targetRef: string;
  sessionRef: string;
  attachmentRef: string;
  attachmentGeneration: number;
  objectRef: string;
  keyRef: string;
}>;

export type PortableCheckpointDekRouteDependencies<Bindings> = Readonly<{
  authenticate: (
    request: Request,
    env: Bindings,
  ) => Promise<PortablePhaseRouteActor | undefined>;
  readPylonOwnerAgentUserId: (env: Bindings, pylonRef: string) => Promise<string | undefined>;
  resolveExactTarget: (
    env: Bindings,
    input: Readonly<{ ownerUserId: string; pylonRef: string; targetRef: string }>,
  ) => Promise<"ready" | "unavailable" | "not_found">;
  readAuthority: (
    env: Bindings,
    input: Readonly<{ pylonRef: string; targetRef: string; operationRef: string }>,
  ) => Promise<PortableCheckpointDekCurrentAuthority>;
  resolveWrapBinding: (
    env: Bindings,
    input: Readonly<{
      actorOwnerRef: string;
      authority: PortableCheckpointDekAuthority;
      current: PortableCheckpointDekCurrentAuthority;
    }>,
  ) => Promise<PortableCheckpointDekWrapBinding>;
  kmsClient: (env: Bindings) => GoogleCloudKmsDekClient | undefined;
  configuredKeyRef: (env: Bindings) => string | undefined;
  now?: (() => Date) | undefined;
}>;

const response = (body: Readonly<Record<string, unknown>>, status: number): HttpResponse =>
  noStoreJsonResponse({ schema: RESPONSE_SCHEMA, ...body }, { status });

const binaryResponse = (bytes: Uint8Array): HttpResponse => {
  try {
    return new Response(bytes.slice(), {
      status: 200,
      headers: {
        "cache-control": "no-store",
        "content-type": "application/octet-stream",
        pragma: "no-cache",
        "x-content-type-options": "nosniff",
      },
    });
  } finally {
    bytes.fill(0);
  }
};

const exactHeader = (request: Request, name: string): string =>
  decodeRef(request.headers.get(name));

const positiveHeader = (request: Request, name: string): number => {
  const value = request.headers.get(name);
  if (value === null || !/^[1-9][0-9]*$/u.test(value)) throw new Error("invalid_header");
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) throw new Error("invalid_header");
  return parsed;
};

const readHeaders = (
  request: Request,
  path: Readonly<{
    action: "wrap" | "unwrap";
    operationRef: string;
    pylonRef: string;
    targetRef: string;
  }>,
): PortableCheckpointDekAuthority =>
  decodeAuthority(
    {
      schema: PORTABLE_CHECKPOINT_DEK_AUTHORITY_SCHEMA_VERSION,
      algorithm: "aes-256-gcm+google-kms-wrapped-dek",
      policy: "openagents_managed",
      ...path,
      commandExecutionClaimRef: exactHeader(request, "x-openagents-command-claim-ref"),
      phaseClaimRef: exactHeader(request, "x-openagents-phase-claim-ref"),
      sessionRef: exactHeader(request, "x-openagents-session-ref"),
      attachmentRef: exactHeader(request, "x-openagents-attachment-ref"),
      attachmentGeneration: positiveHeader(request, "x-openagents-attachment-generation"),
      workerInstanceRef: exactHeader(request, "x-openagents-worker-instance-ref"),
      claimGeneration: positiveHeader(request, "x-openagents-claim-generation"),
      expectedLeaseRevision: positiveHeader(request, "x-openagents-lease-revision"),
      expectedLeaseExpiresAt: request.headers.get("x-openagents-lease-expires-at"),
      objectRef: exactHeader(request, "x-openagents-object-ref"),
      keyRef: exactHeader(request, "x-openagents-key-ref"),
    },
    { onExcessProperty: "error" },
  );

const readBoundedBytes = async (
  request: Request,
  expected: Readonly<{ exact?: number; maximum: number }>,
): Promise<Uint8Array> => {
  if (request.headers.get("content-type")?.split(";", 1)[0]?.trim() !== "application/octet-stream") {
    throw new Error("invalid_content_type");
  }
  const contentLength = request.headers.get("content-length");
  if (
    contentLength !== null &&
    (!/^[0-9]+$/u.test(contentLength) ||
      Number(contentLength) > expected.maximum ||
      (expected.exact !== undefined && Number(contentLength) !== expected.exact))
  ) {
    throw new Error("invalid_size");
  }
  if (request.body === null) throw new Error("invalid_body");
  const reader = request.body.getReader();
  const chunks: Array<Uint8Array> = [];
  let total = 0;
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) break;
      chunks.push(next.value);
      total += next.value.byteLength;
      if (total > expected.maximum) {
        await reader.cancel();
        throw new Error("invalid_size");
      }
    }
    if (total === 0 || (expected.exact !== undefined && total !== expected.exact)) {
      throw new Error("invalid_size");
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return bytes;
  } finally {
    for (const chunk of chunks) chunk.fill(0);
    reader.releaseLock();
  }
};

const currentAuthorityMatches = (
  current: PortableCheckpointDekCurrentAuthority,
  authority: PortableCheckpointDekAuthority,
  ownerRef: string,
  now: Date,
): boolean => {
  const operation = current.operation;
  const command = current.commandClaim;
  const request = operation.request;
  const common =
    request.operationRef === authority.operationRef &&
    request.commandExecutionClaimRef === authority.commandExecutionClaimRef &&
    request.ownerRef === ownerRef &&
    request.pylonRef === authority.pylonRef &&
    request.targetRef === authority.targetRef &&
    request.sessionRef === authority.sessionRef &&
    request.attachmentRef === authority.attachmentRef &&
    request.attachmentGeneration === authority.attachmentGeneration &&
    operation.state === "claimed" &&
    operation.claimRef === authority.phaseClaimRef &&
    operation.workerInstanceRef === authority.workerInstanceRef &&
    operation.claimGeneration === authority.claimGeneration &&
    operation.leaseRevision === authority.expectedLeaseRevision &&
    operation.leaseExpiresAt === authority.expectedLeaseExpiresAt &&
    Date.parse(authority.expectedLeaseExpiresAt) > now.getTime() &&
    Date.parse(request.expiresAt) > now.getTime() &&
    command.claimRef === authority.commandExecutionClaimRef &&
    command.ownerRef === ownerRef &&
    command.sessionRef === authority.sessionRef &&
    command.state === "claimed" &&
    command.terminalStatus === null &&
    command.outcomeRef === null &&
    Date.parse(command.leaseExpiresAt) > now.getTime() &&
    request.expiresAt <= command.leaseExpiresAt;
  if (!common) return false;
  if (authority.action === "wrap") {
    return (
      request.kind === "checkpoint-create" &&
      request.targetRef === command.executorEnvironmentRef &&
      request.attachmentRef === command.sourceAttachmentRef &&
      request.attachmentGeneration === command.sourceGeneration &&
      request.checkpointRef !== null &&
      request.checkpointObjectRef === null &&
      request.checkpointDigest === null
    );
  }
  return (
    request.kind === "checkpoint-stage" &&
    request.targetRef === command.destinationTargetRef &&
    request.attachmentGeneration === command.sourceGeneration + 1 &&
    request.checkpointObjectRef === authority.objectRef &&
    request.checkpointRef !== null &&
    request.checkpointDigest !== null
  );
};

const sameBinding = (
  left: PortableCheckpointDekWrapBinding,
  right: PortableCheckpointDekWrapBinding,
): boolean => canonicalJson(left) === canonicalJson(right);

const aadBytes = (binding: PortableCheckpointDekWrapBinding): Uint8Array =>
  new TextEncoder().encode(canonicalJson(binding));

export const readPortableCheckpointDekWrapBinding = async (
  sql: SyncSql,
  input: Readonly<{
    action: "wrap" | "unwrap";
    actorOwnerRef: string;
    authority: PortableCheckpointDekAuthority;
    current: PortableCheckpointDekCurrentAuthority;
  }>,
): Promise<PortableCheckpointDekWrapBinding> => {
  const make = (request: PortablePhaseOperationRecord["request"]): PortableCheckpointDekWrapBinding => ({
    schema: "openagents.portable_checkpoint_dek_wrap_binding.v1",
    algorithm: "aes-256-gcm+google-kms-wrapped-dek",
    policy: "openagents_managed",
    operationRef: request.operationRef,
    commandExecutionClaimRef: request.commandExecutionClaimRef,
    ownerRef: request.ownerRef,
    pylonRef: request.pylonRef,
    targetRef: request.targetRef,
    sessionRef: request.sessionRef,
    attachmentRef: request.attachmentRef,
    attachmentGeneration: request.attachmentGeneration,
    objectRef: input.authority.objectRef,
    keyRef: input.authority.keyRef,
  });
  if (input.action === "wrap") return make(input.current.operation.request);
  type Row = Readonly<{
    operation_ref: string;
    command_execution_claim_ref: string;
    owner_user_id: string;
    pylon_ref: string;
    target_ref: string;
    session_ref: string;
    attachment_ref: string;
    attachment_generation: string | number;
  }>;
  const rows: Array<Row> = await sql`
    SELECT operation_ref, command_execution_claim_ref, owner_user_id, pylon_ref,
           target_ref, session_ref, attachment_ref, attachment_generation
    FROM khala_sync_portable_phase_operations
    WHERE command_execution_claim_ref = ${input.authority.commandExecutionClaimRef}
      AND owner_user_id = ${input.actorOwnerRef}
      AND kind = 'checkpoint-create'
      AND state = 'completed'
      AND result_status = 'completed'
      AND result_checkpoint_object_ref = ${input.authority.objectRef}
    LIMIT 1
  `;
  const row = rows[0];
  if (row === undefined) throw new Error("wrap_binding_not_found");
  return {
    schema: "openagents.portable_checkpoint_dek_wrap_binding.v1",
    algorithm: "aes-256-gcm+google-kms-wrapped-dek",
    policy: "openagents_managed",
    operationRef: decodeRef(row.operation_ref),
    commandExecutionClaimRef: decodeRef(row.command_execution_claim_ref),
    ownerRef: decodeRef(row.owner_user_id),
    pylonRef: decodeRef(row.pylon_ref),
    targetRef: decodeRef(row.target_ref),
    sessionRef: decodeRef(row.session_ref),
    attachmentRef: decodeRef(row.attachment_ref),
    attachmentGeneration: Number(row.attachment_generation),
    objectRef: input.authority.objectRef,
    keyRef: input.authority.keyRef,
  };
};

export const makePortableCheckpointDekRoutes = <Bindings>(
  dependencies: PortableCheckpointDekRouteDependencies<Bindings>,
) => ({
  routePortableCheckpointDekRequest: (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> | undefined => {
    const match =
      /^\/api\/pylons\/([^/]+)\/portable-targets\/([^/]+)\/checkpoint-deks\/([^/]+)\/(wrap|unwrap)$/.exec(
        new URL(request.url).pathname,
      );
    if (match === null) return undefined;
    return (async () => {
      if (request.method !== "POST") return methodNotAllowed(["POST"]);
      let pylonRef: string;
      let targetRef: string;
      let operationRef: string;
      let action: "wrap" | "unwrap";
      let authority: PortableCheckpointDekAuthority;
      try {
        pylonRef = decodeRef(decodeURIComponent(match[1]!));
        targetRef = decodeRef(decodeURIComponent(match[2]!));
        operationRef = decodeRef(decodeURIComponent(match[3]!));
        action = match[4] === "wrap" ? "wrap" : "unwrap";
        authority = readHeaders(request, { action, operationRef, pylonRef, targetRef });
        if (!authority.objectRef.startsWith("checkpoint-custody:")) {
          throw new Error("invalid_object_ref");
        }
      } catch {
        return response({ error: "invalid_request", retryable: false }, 400);
      }
      let actor: PortablePhaseRouteActor | undefined;
      try {
        actor = await dependencies.authenticate(request, env);
      } catch {
        return response({ error: "authentication_unavailable", retryable: true }, 503);
      }
      if (actor === undefined) {
        return noStoreJsonResponse(
          { schema: RESPONSE_SCHEMA, error: "authentication_required", retryable: false },
          { status: 401, headers: { "www-authenticate": "Bearer" } },
        );
      }
      try {
        const registeredOwner = await dependencies.readPylonOwnerAgentUserId(env, pylonRef);
        if (registeredOwner === undefined) {
          return response({ error: "pylon_not_registered", retryable: false }, 404);
        }
        if (registeredOwner !== actor.agentUserId) {
          return response({ error: "pylon_not_owned", retryable: false }, 403);
        }
        const target = await dependencies.resolveExactTarget(env, {
          ownerUserId: actor.ownerUserId,
          pylonRef,
          targetRef,
        });
        if (target !== "ready") {
          return response(
            { error: target === "not_found" ? "target_not_authorized" : "target_unavailable", retryable: false },
            target === "not_found" ? 403 : 409,
          );
        }
        const keyRef = dependencies.configuredKeyRef(env);
        const kms = dependencies.kmsClient(env);
        if (keyRef === undefined || keyRef !== authority.keyRef || kms === undefined) {
          return response({ error: "checkpoint_dek_authority_unavailable", retryable: true }, 503);
        }
        const now = dependencies.now ?? (() => new Date());
        const current = await dependencies.readAuthority(env, { pylonRef, targetRef, operationRef });
        if (!currentAuthorityMatches(current, authority, actor.ownerUserId, now())) {
          return response({ error: "checkpoint_dek_authority_lost", retryable: false }, 409);
        }
        const wrapBinding = await dependencies.resolveWrapBinding(env, {
          actorOwnerRef: actor.ownerUserId,
          authority,
          current,
        });
        if (
          wrapBinding.ownerRef !== actor.ownerUserId ||
          wrapBinding.commandExecutionClaimRef !== authority.commandExecutionClaimRef ||
          wrapBinding.objectRef !== authority.objectRef ||
          wrapBinding.keyRef !== authority.keyRef
        ) {
          return response({ error: "checkpoint_dek_binding_mismatch", retryable: false }, 409);
        }
        let input: Uint8Array;
        try {
          input = await readBoundedBytes(
            request,
            action === "wrap"
              ? { exact: 32, maximum: 32 }
              : { maximum: MAX_WRAPPED_DEK_BYTES },
          );
        } catch {
          return response({ error: "invalid_dek_bytes", retryable: false }, 400);
        }
        const aad = aadBytes(wrapBinding);
        let output: Uint8Array | undefined;
        try {
          output =
            action === "wrap"
              ? await kms.wrapDek(input, aad)
              : await kms.unwrapDek(input, aad);
          if (
            output.byteLength === 0 ||
            (action === "wrap" && output.byteLength > MAX_WRAPPED_DEK_BYTES) ||
            (action === "unwrap" && output.byteLength !== 32)
          ) {
            output.fill(0);
            return response({ error: "checkpoint_dek_invalid_result", retryable: false }, 409);
          }
          const finalCurrent = await dependencies.readAuthority(env, {
            pylonRef,
            targetRef,
            operationRef,
          });
          const finalBinding = await dependencies.resolveWrapBinding(env, {
            actorOwnerRef: actor.ownerUserId,
            authority,
            current: finalCurrent,
          });
          const finalRegisteredOwner = await dependencies.readPylonOwnerAgentUserId(
            env,
            pylonRef,
          );
          const finalTarget = await dependencies.resolveExactTarget(env, {
            ownerUserId: actor.ownerUserId,
            pylonRef,
            targetRef,
          });
          if (
            finalRegisteredOwner !== actor.agentUserId ||
            finalTarget !== "ready" ||
            !currentAuthorityMatches(finalCurrent, authority, actor.ownerUserId, now()) ||
            !sameBinding(wrapBinding, finalBinding)
          ) {
            output.fill(0);
            return response({ error: "checkpoint_dek_authority_lost", retryable: false }, 409);
          }
          return binaryResponse(output);
        } finally {
          input.fill(0);
          aad.fill(0);
          output?.fill(0);
        }
      } catch {
        return response({ error: "checkpoint_dek_authority_unavailable", retryable: true }, 503);
      }
    })();
  },
});

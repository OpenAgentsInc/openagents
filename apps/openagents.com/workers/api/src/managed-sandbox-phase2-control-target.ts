import {
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxPhase2Error,
  decodeManagedSandboxCheckpointDeleteReceipt,
  decodeManagedSandboxCheckpointStopOutcome,
  decodeManagedSandboxContentCheckpoint,
  decodeManagedSandboxForkReceipt,
  decodeManagedSandboxRestoreReceipt,
  decodeManagedSandboxPrivateIngressCapability,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import { parseJsonUnknown } from "./json-boundary";
import type { ManagedSandboxPhase2Target } from "./managed-sandbox-phase2-service";

export const MANAGED_SANDBOX_PHASE2_TARGET_SCHEMA_VERSION =
  "openagents.managed_sandbox_phase2_target.v1";

const MAX_RESPONSE_BYTES = 1024 * 1024;
const noEvidence: ReadonlyArray<string> = [];
const forbiddenPrivateMaterial =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

const ControlActionSchema = S.Literals([
  "create_checkpoint",
  "archive_with_checkpoint",
  "verify_checkpoint",
  "observe_resource_generation",
  "fork_from_checkpoint",
  "restore_checkpoint",
  "delete_checkpoint",
  "create_private_ingress",
  "revoke_private_ingress",
  "expire_private_ingress",
]);
type ControlAction = typeof ControlActionSchema.Type;

const ControlResponseSchema = S.Struct({
  schemaVersion: S.Literal(MANAGED_SANDBOX_PHASE2_TARGET_SCHEMA_VERSION),
  action: ControlActionSchema,
  requestRef: S.String,
  result: S.Unknown,
});

const VerifyResultSchema = S.Struct({
  verified: S.Boolean,
  checkpointRef: S.String,
  contentDigest: S.String,
  evidenceRefs: S.Array(S.String),
});

const GenerationResultSchema = S.Struct({
  ownerRef: S.String,
  tenantRef: S.String,
  sandboxRef: S.String,
  resourceGeneration: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  evidenceRefs: S.Array(S.String),
});

const decodeControlResponse = S.decodeUnknownSync(ControlResponseSchema, {
  onExcessProperty: "error",
});
const decodeVerifyResult = S.decodeUnknownSync(VerifyResultSchema, {
  onExcessProperty: "error",
});
const decodeGenerationResult = S.decodeUnknownSync(GenerationResultSchema, {
  onExcessProperty: "error",
});

type RequestContext = Readonly<{
  requestRef: string;
  idempotencyRef?: string;
  checkpointRef?: string;
}>;

const invalid = (
  requestRef: string,
  message: string,
  retryable = false,
): ManagedSandboxPhase2Error => ({
  _tag: "InvalidRequest",
  requestRef,
  message,
  retryable,
  evidenceRefs: noEvidence,
});

const conflict = (idempotencyRef: string): ManagedSandboxPhase2Error => ({
  _tag: "IdempotencyConflict",
  idempotencyRef,
  message: "the Phase 2 target refused conflicting idempotency bytes",
  retryable: false,
  evidenceRefs: noEvidence,
});

const incomplete = (checkpointRef: string): ManagedSandboxPhase2Error => ({
  _tag: "CheckpointIncomplete",
  checkpointRef,
  message: "the completed checkpoint does not exist at the target",
  retryable: false,
  evidenceRefs: noEvidence,
});

const responseFailure = (status: number, context: RequestContext): ManagedSandboxPhase2Error => {
  if (status === 409 && context.idempotencyRef !== undefined) {
    return conflict(context.idempotencyRef);
  }
  if (status === 404 && context.checkpointRef !== undefined) {
    return incomplete(context.checkpointRef);
  }
  return invalid(
    context.requestRef,
    status === 400 || status === 403
      ? "the Phase 2 target refused the request"
      : "the Phase 2 target is unavailable",
    status === 429 || status >= 500,
  );
};

const resultFailure = (context: RequestContext): ManagedSandboxPhase2Error =>
  invalid(context.requestRef, "the Phase 2 target response failed contract validation");

export type ManagedSandboxPhase2ControlTargetOptions = Readonly<{
  baseUrl: string;
  bearerToken: string;
  fetch?: typeof fetch;
}>;

/** Build the private Google Cloud control client. This target does not expose ingress. */
export const makeManagedSandboxPhase2ControlTarget = (
  options: ManagedSandboxPhase2ControlTargetOptions,
): ManagedSandboxPhase2Target => {
  const fetchImpl = options.fetch ?? fetch;
  const endpoint = `${options.baseUrl.replace(/\/$/u, "")}/v1/managed-sandbox/runtime/checkpoints`;
  const configured =
    options.baseUrl.startsWith("https://") && options.bearerToken.trim().length > 0;

  const request = <A>(input: {
    action: ControlAction;
    context: RequestContext;
    payload: Readonly<Record<string, unknown>>;
    decode: (value: unknown) => A;
  }): Effect.Effect<A, ManagedSandboxPhase2Error> =>
    Effect.gen(function* () {
      if (!configured) {
        return yield* Effect.fail(
          invalid(input.context.requestRef, "the Phase 2 target is not configured", true),
        );
      }
      const body = JSON.stringify({
        schemaVersion: MANAGED_SANDBOX_PHASE2_TARGET_SCHEMA_VERSION,
        action: input.action,
        requestRef: input.context.requestRef,
        ...input.payload,
      });
      if (forbiddenPrivateMaterial.test(body)) {
        return yield* Effect.fail(
          invalid(input.context.requestRef, "the Phase 2 target request contains private material"),
        );
      }
      const response = yield* Effect.tryPromise({
        try: () =>
          fetchImpl(endpoint, {
            method: "POST",
            headers: {
              accept: "application/json",
              "cache-control": "no-store",
              "content-type": "application/json",
              "x-openagents-managed-sandbox-token": options.bearerToken,
            },
            body,
          }),
        catch: () => invalid(input.context.requestRef, "the Phase 2 target is unavailable", true),
      });
      const text = yield* Effect.tryPromise({
        try: () => response.text(),
        catch: () => invalid(input.context.requestRef, "the Phase 2 target is unavailable", true),
      });
      if (!response.ok) {
        return yield* Effect.fail(responseFailure(response.status, input.context));
      }
      if (text.length > MAX_RESPONSE_BYTES || forbiddenPrivateMaterial.test(text)) {
        return yield* Effect.fail(resultFailure(input.context));
      }
      return yield* Effect.try({
        try: () => {
          const envelope = decodeControlResponse(parseJsonUnknown(text));
          if (
            envelope.action !== input.action ||
            envelope.requestRef !== input.context.requestRef
          ) {
            throw new Error("response_scope_conflict");
          }
          return input.decode(envelope.result);
        },
        catch: () => resultFailure(input.context),
      });
    });

  const createCheckpoint = Effect.fn("ManagedSandboxPhase2ControlTarget.createCheckpoint")(
    (command: Parameters<ManagedSandboxPhase2Target["createCheckpoint"]>[0]) =>
      request({
        action: "create_checkpoint",
        context: {
          requestRef: command.commandRef,
          idempotencyRef: command.idempotencyRef,
          checkpointRef: command.checkpointRef,
        },
        payload: { command },
        decode: decodeManagedSandboxContentCheckpoint,
      }),
  );

  const archiveWithCheckpoint = Effect.fn(
    "ManagedSandboxPhase2ControlTarget.archiveWithCheckpoint",
  )((command: Parameters<ManagedSandboxPhase2Target["archiveWithCheckpoint"]>[0]) =>
    request({
      action: "archive_with_checkpoint",
      context: {
        requestRef: command.commandRef,
        idempotencyRef: command.idempotencyRef,
        checkpointRef: command.checkpointRef,
      },
      payload: { command },
      decode: decodeManagedSandboxCheckpointStopOutcome,
    }),
  );

  const verifyCheckpoint = Effect.fn("ManagedSandboxPhase2ControlTarget.verifyCheckpoint")(
    (checkpoint: ManagedSandboxContentCheckpoint) =>
      request({
        action: "verify_checkpoint",
        context: {
          requestRef: checkpoint.checkpointRef,
          checkpointRef: checkpoint.checkpointRef,
        },
        payload: { checkpoint },
        decode: (value) => {
          const result = decodeVerifyResult(value);
          if (
            result.checkpointRef !== checkpoint.checkpointRef ||
            result.contentDigest !== checkpoint.contentDigest ||
            result.evidenceRefs.length === 0
          ) {
            throw new Error("checkpoint_verification_scope_conflict");
          }
          return result.verified;
        },
      }),
  );

  const observeResourceGeneration = Effect.fn(
    "ManagedSandboxPhase2ControlTarget.observeResourceGeneration",
  )((input: Parameters<ManagedSandboxPhase2Target["observeResourceGeneration"]>[0]) =>
    request({
      action: "observe_resource_generation",
      context: { requestRef: input.sandboxRef },
      payload: input,
      decode: (value) => {
        const result = decodeGenerationResult(value);
        if (
          result.ownerRef !== input.ownerRef ||
          result.tenantRef !== input.tenantRef ||
          result.sandboxRef !== input.sandboxRef ||
          result.evidenceRefs.length === 0
        ) {
          throw new Error("generation_scope_conflict");
        }
        return result.resourceGeneration;
      },
    }),
  );

  const forkFromCheckpoint = Effect.fn("ManagedSandboxPhase2ControlTarget.forkFromCheckpoint")(
    (
      command: Parameters<ManagedSandboxPhase2Target["forkFromCheckpoint"]>[0],
      checkpoint: ManagedSandboxContentCheckpoint,
    ) =>
      request({
        action: "fork_from_checkpoint",
        context: {
          requestRef: command.commandRef,
          idempotencyRef: command.idempotencyRef,
          checkpointRef: command.checkpointRef,
        },
        payload: { command, checkpoint },
        decode: decodeManagedSandboxForkReceipt,
      }),
  );

  const restoreCheckpoint = Effect.fn("ManagedSandboxPhase2ControlTarget.restoreCheckpoint")(
    (
      command: Parameters<ManagedSandboxPhase2Target["restoreCheckpoint"]>[0],
      checkpoint: ManagedSandboxContentCheckpoint,
    ) =>
      request({
        action: "restore_checkpoint",
        context: {
          requestRef: command.commandRef,
          idempotencyRef: command.idempotencyRef,
          checkpointRef: command.checkpointRef,
        },
        payload: { command, checkpoint },
        decode: decodeManagedSandboxRestoreReceipt,
      }),
  );

  const deleteCheckpoint = Effect.fn("ManagedSandboxPhase2ControlTarget.deleteCheckpoint")(
    (
      command: Parameters<ManagedSandboxPhase2Target["deleteCheckpoint"]>[0],
      checkpoint: ManagedSandboxContentCheckpoint,
    ) =>
      request({
        action: "delete_checkpoint",
        context: {
          requestRef: command.commandRef,
          idempotencyRef: command.idempotencyRef,
          checkpointRef: command.checkpointRef,
        },
        payload: { command, checkpoint },
        decode: decodeManagedSandboxCheckpointDeleteReceipt,
      }),
  );

  const createPrivateIngress = Effect.fn(
    "ManagedSandboxPhase2ControlTarget.createPrivateIngress",
  )((command: Parameters<ManagedSandboxPhase2Target["createPrivateIngress"]>[0]) =>
    request({
      action: "create_private_ingress",
      context: { requestRef: command.commandRef, idempotencyRef: command.idempotencyRef },
      payload: { command },
      decode: decodeManagedSandboxPrivateIngressCapability,
    }),
  );

  const revokePrivateIngress = Effect.fn(
    "ManagedSandboxPhase2ControlTarget.revokePrivateIngress",
  )(
    (
      command: Parameters<ManagedSandboxPhase2Target["revokePrivateIngress"]>[0],
      capability: Parameters<ManagedSandboxPhase2Target["revokePrivateIngress"]>[1],
    ) =>
      request({
        action: "revoke_private_ingress",
        context: { requestRef: command.commandRef, idempotencyRef: command.idempotencyRef },
        payload: { command, capability },
        decode: decodeManagedSandboxPrivateIngressCapability,
      }),
  );

  const expirePrivateIngress = Effect.fn(
    "ManagedSandboxPhase2ControlTarget.expirePrivateIngress",
  )(
    (
      command: Parameters<ManagedSandboxPhase2Target["expirePrivateIngress"]>[0],
      capability: Parameters<ManagedSandboxPhase2Target["expirePrivateIngress"]>[1],
    ) =>
      request({
        action: "expire_private_ingress",
        context: { requestRef: command.commandRef, idempotencyRef: command.idempotencyRef },
        payload: { command, capability },
        decode: decodeManagedSandboxPrivateIngressCapability,
      }),
  );

  return {
    createCheckpoint,
    archiveWithCheckpoint,
    verifyCheckpoint,
    observeResourceGeneration,
    forkFromCheckpoint,
    restoreCheckpoint,
    deleteCheckpoint,
    createPrivateIngress,
    revokePrivateIngress,
    expirePrivateIngress,
  };
};

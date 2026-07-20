import { createHash } from "node:crypto";

import {
  PortableRef,
  type PortableCapabilityLease,
  type PortableOwnerLocalCapabilityOperationRecord,
} from "@openagentsinc/portable-session-contract";
import { Duration, Effect, Schedule, Schema } from "effect";

import type {
  PortableCommandTargetInstallationPortResolution,
  PortableCommandTargetInstallationPortResolver,
} from "./portable-command-broker-factory.js";
import type { PortableCapabilityTargetInstallationPort } from "./portable-capability-runtime-adapters.js";
import {
  PostgresPortableOwnerLocalCapabilityOperationStore,
  PortableOwnerLocalCapabilityOperationStoreError,
  portableOwnerLocalCapabilityOperationRef,
  portableOwnerLocalCapabilityPermissionFingerprint,
} from "./portable-owner-local-capability-operation-store.js";
import type { PortableCommandGrantAuthorityBinding } from "./portable-session-command-runner.js";
import type { PortableCapabilityTransfer } from "./portable-session-move.js";
import type { SyncSql } from "./sql.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

type AuthorityRow = Readonly<{
  target_ref: string;
  target_owner_ref: string;
  target_class: string;
  adapter_ref: string;
  compatibility_ref: string;
  isolation: string;
  data_posture: string;
  target_health: string;
  claim_ref: string;
  claim_owner_ref: string;
  session_ref: string;
  source_attachment_ref: string;
  source_generation: string | number;
  executor_environment_ref: string;
  destination_target_ref: string;
  claim_state: string;
  terminal_status: string | null;
  claim_lease_expires_at: Date | string;
  pylon_ref: string;
  binding_state: string;
  binding_health: string;
  binding_expires_at: Date | string;
}>;

const stableFailureRef = (code: string, scopeRef: string): string =>
  `failure.portable-owner-local-installation.${createHash("sha256")
    .update(`${code}\u0000${scopeRef}`)
    .digest("hex")}`;

export class PortableOwnerLocalCommandInstallationResolverError extends Schema.TaggedErrorClass<PortableOwnerLocalCommandInstallationResolverError>()(
  "PortableOwnerLocalCommandInstallationResolverError",
  {
    code: Schema.Literals([
      "invalid_configuration",
      "invalid_scope",
      "authority_missing",
      "authority_mismatch",
      "authority_expired",
      "operation_failed",
      "operation_timeout",
      "result_mismatch",
    ]),
    failureRef: PortableRef,
  },
) {}

class PendingOwnerLocalCapabilityOperation extends Schema.TaggedErrorClass<PendingOwnerLocalCapabilityOperation>()(
  "PendingOwnerLocalCapabilityOperation",
  { operationRef: PortableRef },
) {}

export type PostgresOwnerLocalPortableCommandInstallationPortResolverConfig = Readonly<{
  sql: SyncSql;
  now?: () => string;
  pollIntervalMs?: number;
  timeoutMs?: number;
}>;

type ResolverInput = Parameters<PortableCommandTargetInstallationPortResolver["resolve"]>[0];

type ExactTransfer = Readonly<{
  transfer: PortableCapabilityTransfer;
  binding: PortableCommandGrantAuthorityBinding;
}>;

const exactTransfer = (
  input: ResolverInput,
  leaseRef: string,
  side: "source" | "destination",
): ExactTransfer | undefined => {
  const matches = input.capabilityTransfers.filter((candidate) =>
    side === "source"
      ? candidate.sourceLeaseRef === leaseRef
      : candidate.destinationLeaseRef === leaseRef,
  );
  if (matches.length !== 1) return undefined;
  const transfer = matches[0];
  if (transfer === undefined) return undefined;
  const bindings = input.grantBindings.filter(
    (candidate) => candidate.sourceLeaseRef === transfer.sourceLeaseRef,
  );
  if (bindings.length !== 1) return undefined;
  const binding = bindings[0];
  return binding === undefined ? undefined : { transfer, binding };
};

class QueuedOwnerLocalCapabilityInstallationPort implements PortableCapabilityTargetInstallationPort {
  constructor(
    private readonly config: Readonly<{
      input: ResolverInput;
      pylonRef: string;
      role: "source" | "destination";
      attachmentRef: string;
      attachmentGeneration: number;
      maximumExpiresAt: string;
      store: PostgresPortableOwnerLocalCapabilityOperationStore;
      now: () => string;
      pollIntervalMs: number;
      maximumPolls: number;
    }>,
  ) {}

  async install(): Promise<never> {
    throw this.failure("invalid_scope", this.config.input.target.targetRef);
  }

  readonly installByReference = async (
    input: Parameters<
      NonNullable<PortableCapabilityTargetInstallationPort["installByReference"]>
    >[0],
  ): Promise<
    Awaited<ReturnType<NonNullable<PortableCapabilityTargetInstallationPort["installByReference"]>>>
  > => {
    if (
      this.config.role !== "destination" ||
      !this.exactLeaseScope(input.lease) ||
      input.lease.attachmentRef !== this.config.attachmentRef ||
      input.lease.attachmentGeneration !== this.config.attachmentGeneration
    ) {
      throw this.failure("invalid_scope", input.lease.leaseRef);
    }
    const mapped = exactTransfer(this.config.input, input.lease.leaseRef, "destination");
    const permissionRefs = [...input.permissions];
    permissionRefs.sort((left, right) => left.localeCompare(right));
    if (
      mapped === undefined ||
      permissionRefs.length === 0 ||
      new Set(permissionRefs).size !== permissionRefs.length ||
      permissionRefs.some((ref) => !SAFE_REF.test(ref))
    ) {
      throw this.failure("invalid_scope", input.lease.leaseRef);
    }
    const operation = await this.enqueueAndAwait({
      action: "install",
      capability: input.lease.capability,
      installationRef: null,
      permissionRefs,
      mapped,
    });
    if (
      operation.resultInstallationRef === null ||
      operation.receiptRef === null ||
      operation.resultEvidenceRefs.length !== 1
    ) {
      throw this.failure("result_mismatch", operation.request.operationRef);
    }
    const evidenceRef = operation.resultEvidenceRefs[0];
    if (evidenceRef === undefined) {
      throw this.failure("result_mismatch", operation.request.operationRef);
    }
    return {
      installationRef: operation.resultInstallationRef,
      evidenceRef,
    };
  };

  async wipe(
    input: Parameters<PortableCapabilityTargetInstallationPort["wipe"]>[0],
  ): Promise<Awaited<ReturnType<PortableCapabilityTargetInstallationPort["wipe"]>>> {
    if (
      this.config.role !== "source" ||
      input.targetRef !== this.config.input.target.targetRef ||
      input.attachmentRef !== this.config.attachmentRef ||
      input.attachmentGeneration !== this.config.attachmentGeneration ||
      input.installationRef === undefined ||
      !SAFE_REF.test(input.installationRef)
    ) {
      throw this.failure("invalid_scope", input.leaseRef);
    }
    const mapped = exactTransfer(this.config.input, input.leaseRef, "source");
    if (mapped === undefined) throw this.failure("invalid_scope", input.leaseRef);
    const operation = await this.enqueueAndAwait({
      action: "wipe",
      capability: null,
      installationRef: input.installationRef,
      permissionRefs: [],
      mapped,
    });
    if (
      operation.resultInstallationRef !== null ||
      operation.receiptRef === null ||
      operation.resultEvidenceRefs.length !== 0
    ) {
      throw this.failure("result_mismatch", operation.request.operationRef);
    }
    return { wipeReceiptRef: operation.receiptRef };
  }

  private exactLeaseScope(lease: PortableCapabilityLease): boolean {
    return (
      lease.ownerRef === this.config.input.ownerRef &&
      lease.sessionRef === this.config.input.sessionRef &&
      lease.targetRef === this.config.input.target.targetRef &&
      lease.state === "issued" &&
      SAFE_REF.test(lease.leaseRef)
    );
  }

  private async enqueueAndAwait(
    input: Readonly<{
      action: "install" | "wipe";
      capability: PortableCapabilityLease["capability"] | null;
      installationRef: string | null;
      permissionRefs: ReadonlyArray<string>;
      mapped: ExactTransfer;
    }>,
  ): Promise<PortableOwnerLocalCapabilityOperationRecord> {
    const now = new Date(this.config.now());
    const maximum = new Date(this.config.maximumExpiresAt);
    if (!Number.isFinite(now.valueOf()) || !Number.isFinite(maximum.valueOf()) || maximum <= now) {
      throw this.failure("authority_expired", this.config.input.commandExecutionClaimRef);
    }
    const identity = {
      action: input.action,
      capability: input.capability,
      commandExecutionClaimRef: this.config.input.commandExecutionClaimRef,
      ownerRef: this.config.input.ownerRef,
      pylonRef: this.config.pylonRef,
      sessionRef: this.config.input.sessionRef,
      attachmentRef: this.config.attachmentRef,
      attachmentGeneration: this.config.attachmentGeneration,
      targetRef: this.config.input.target.targetRef,
      sourceLeaseRef: input.mapped.transfer.sourceLeaseRef,
      sourceGrantRef: input.mapped.binding.grantRef,
      destinationLeaseRef: input.mapped.transfer.destinationLeaseRef,
      destinationGrantRef: input.mapped.transfer.destinationSourceGrantRef,
      installationRef: input.installationRef,
      permissionRefs: input.permissionRefs,
    };
    const request = {
      schema: "openagents.portable_owner_local_capability_operation.v1" as const,
      operationRef: portableOwnerLocalCapabilityOperationRef(identity),
      ...identity,
      permissionFingerprint: portableOwnerLocalCapabilityPermissionFingerprint(
        input.permissionRefs,
      ),
      expiresAt: maximum.toISOString(),
    };
    try {
      await this.config.store.enqueue(request);
    } catch (cause) {
      throw this.storeFailure(cause, request.operationRef);
    }

    const readTerminal = Effect.tryPromise({
      try: async (): Promise<PortableOwnerLocalCapabilityOperationRecord> => {
        const operation = await this.config.store.read(
          this.config.input.ownerRef,
          this.config.pylonRef,
          this.config.input.target.targetRef,
          request.operationRef,
        );
        if (operation.state === "pending" || operation.state === "claimed") {
          throw new PendingOwnerLocalCapabilityOperation({
            operationRef: request.operationRef,
          });
        }
        if (operation.state === "failed" || operation.state === "expired") {
          throw this.failure("operation_failed", request.operationRef);
        }
        return operation;
      },
      catch: (
        cause,
      ):
        | PendingOwnerLocalCapabilityOperation
        | PortableOwnerLocalCommandInstallationResolverError =>
        cause instanceof PendingOwnerLocalCapabilityOperation ||
        cause instanceof PortableOwnerLocalCommandInstallationResolverError
          ? cause
          : this.storeFailure(cause, request.operationRef),
    }).pipe(
      Effect.retry({
        schedule: Schedule.spaced(Duration.millis(this.config.pollIntervalMs)).pipe(
          Schedule.both(Schedule.recurs(this.config.maximumPolls - 1)),
        ),
        while: (error) => error instanceof PendingOwnerLocalCapabilityOperation,
      }),
      Effect.mapError((error) =>
        error instanceof PendingOwnerLocalCapabilityOperation
          ? this.failure("operation_timeout", request.operationRef)
          : error,
      ),
    );
    return Effect.runPromise(readTerminal);
  }

  private storeFailure(cause: unknown, scopeRef: string) {
    if (cause instanceof PortableOwnerLocalCommandInstallationResolverError) return cause;
    const code =
      cause instanceof PortableOwnerLocalCapabilityOperationStoreError && cause.code === "expired"
        ? "authority_expired"
        : "authority_mismatch";
    return this.failure(code, scopeRef);
  }

  private failure(
    code: PortableOwnerLocalCommandInstallationResolverError["code"],
    scopeRef: string,
  ) {
    return new PortableOwnerLocalCommandInstallationResolverError({
      code,
      failureRef: stableFailureRef(code, scopeRef),
    });
  }
}

/** Resolve one durable outbound-poll installation port for one exact owner-local target. */
export const createPostgresOwnerLocalPortableCommandInstallationPortResolver = (
  config: PostgresOwnerLocalPortableCommandInstallationPortResolverConfig,
): PortableCommandTargetInstallationPortResolver => {
  const pollIntervalMs = config.pollIntervalMs ?? 25;
  const timeoutMs = config.timeoutMs ?? 30_000;
  const maximumPolls = Math.max(1, Math.ceil(timeoutMs / pollIntervalMs));
  const now = config.now ?? (() => new Date().toISOString());
  const invalidConfig =
    typeof config.sql !== "function" ||
    !Number.isSafeInteger(pollIntervalMs) ||
    pollIntervalMs < 1 ||
    pollIntervalMs > 10_000 ||
    !Number.isSafeInteger(timeoutMs) ||
    timeoutMs < pollIntervalMs ||
    timeoutMs > 120_000;
  if (invalidConfig) {
    throw new PortableOwnerLocalCommandInstallationResolverError({
      code: "invalid_configuration",
      failureRef: stableFailureRef("invalid_configuration", "configuration"),
    });
  }

  const failure = (
    code: PortableOwnerLocalCommandInstallationResolverError["code"],
    scopeRef: string,
  ) =>
    new PortableOwnerLocalCommandInstallationResolverError({
      code,
      failureRef: stableFailureRef(code, scopeRef),
    });

  return {
    resolve: async (input): Promise<PortableCommandTargetInstallationPortResolution | null> => {
      if (input.target.targetClass !== "owner_local") return null;
      if (
        ![
          input.commandExecutionClaimRef,
          input.ownerRef,
          input.sessionRef,
          input.target.targetRef,
          input.target.adapterRef,
          input.sourceAttachmentRef,
          input.destinationAttachmentRef,
        ].every((ref) => SAFE_REF.test(ref)) ||
        input.target.ownerRef !== input.ownerRef ||
        !Number.isSafeInteger(input.sourceGeneration) ||
        input.sourceGeneration < 1 ||
        input.destinationGeneration !== input.sourceGeneration + 1
      ) {
        throw failure("invalid_scope", input.commandExecutionClaimRef);
      }
      let rows: ReadonlyArray<AuthorityRow>;
      try {
        rows = await config.sql`
          SELECT target.target_ref, target.owner_user_id AS target_owner_ref,
                 target.target_class, target.adapter_ref, target.compatibility_ref,
                 target.isolation, target.data_posture, target.health AS target_health,
                 claim.claim_ref, claim.owner_user_id AS claim_owner_ref,
                 claim.session_ref, claim.source_attachment_ref, claim.source_generation,
                 claim.executor_environment_ref, claim.destination_target_ref,
                 claim.state AS claim_state, claim.terminal_status,
                 claim.lease_expires_at AS claim_lease_expires_at,
                 binding.pylon_ref, binding.state AS binding_state,
                 binding.health AS binding_health, binding.expires_at AS binding_expires_at
          FROM khala_sync_portable_targets AS target
          JOIN khala_sync_portable_command_executions AS claim
            ON claim.owner_user_id = target.owner_user_id
           AND (claim.executor_environment_ref = target.target_ref
             OR claim.destination_target_ref = target.target_ref)
          JOIN khala_sync_portable_target_pylon_bindings AS binding
            ON binding.owner_user_id = target.owner_user_id
           AND binding.session_ref = claim.session_ref
           AND binding.target_ref = target.target_ref
          WHERE target.target_ref = ${input.target.targetRef}
            AND target.owner_user_id = ${input.ownerRef}
            AND claim.claim_ref = ${input.commandExecutionClaimRef}
            AND claim.session_ref = ${input.sessionRef}
        `;
      } catch {
        throw failure("authority_missing", input.commandExecutionClaimRef);
      }
      if (rows.length !== 1) throw failure("authority_missing", input.target.targetRef);
      const row = rows[0];
      if (row === undefined) throw failure("authority_missing", input.target.targetRef);
      const source = row.executor_environment_ref === row.target_ref;
      const destination = row.destination_target_ref === row.target_ref;
      if (
        source === destination ||
        row.target_ref !== input.target.targetRef ||
        row.target_owner_ref !== input.ownerRef ||
        row.target_class !== input.target.targetClass ||
        row.adapter_ref !== input.target.adapterRef ||
        row.compatibility_ref !== input.target.compatibilityRef ||
        row.isolation !== input.target.isolation ||
        row.data_posture !== input.target.dataPosture ||
        row.target_health !== input.target.health ||
        row.target_health !== "ready" ||
        row.claim_ref !== input.commandExecutionClaimRef ||
        row.claim_owner_ref !== input.ownerRef ||
        row.session_ref !== input.sessionRef ||
        row.source_attachment_ref !== input.sourceAttachmentRef ||
        Number(row.source_generation) !== input.sourceGeneration ||
        row.claim_state !== "claimed" ||
        row.terminal_status !== null ||
        row.binding_state !== "active" ||
        !["ready", "draining"].includes(row.binding_health) ||
        !SAFE_REF.test(row.pylon_ref)
      ) {
        throw failure("authority_mismatch", input.target.targetRef);
      }
      const instant = new Date(now());
      const claimExpiry = new Date(row.claim_lease_expires_at);
      const bindingExpiry = new Date(row.binding_expires_at);
      if (
        ![instant, claimExpiry, bindingExpiry].every((value) => Number.isFinite(value.valueOf())) ||
        claimExpiry <= instant ||
        bindingExpiry <= instant
      ) {
        throw failure("authority_expired", input.commandExecutionClaimRef);
      }
      const waitExpiry = new Date(instant.valueOf() + timeoutMs);
      const maximumExpiresAt = new Date(
        Math.min(waitExpiry.valueOf(), claimExpiry.valueOf(), bindingExpiry.valueOf()),
      ).toISOString();
      const role = source ? "source" : "destination";
      const attachmentRef = source ? input.sourceAttachmentRef : input.destinationAttachmentRef;
      const attachmentGeneration = source ? input.sourceGeneration : input.destinationGeneration;
      return {
        targetRef: input.target.targetRef,
        targetClass: "owner_local",
        adapterRef: input.target.adapterRef,
        port: new QueuedOwnerLocalCapabilityInstallationPort({
          input,
          pylonRef: row.pylon_ref,
          role,
          attachmentRef,
          attachmentGeneration,
          maximumExpiresAt,
          store: new PostgresPortableOwnerLocalCapabilityOperationStore(config.sql, now),
          now,
          pollIntervalMs,
          maximumPolls,
        }),
      };
    },
  };
};

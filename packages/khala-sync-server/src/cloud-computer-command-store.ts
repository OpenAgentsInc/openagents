import { createHash } from "node:crypto";
import { canonicalJson } from "@openagentsinc/khala-sync";

import {
  assertVerifiedCloudComputerCommandArtifact,
  cloudComputerCommandArtifactRef,
  type VerifiedCloudComputerCommandArtifact,
} from "./cloud-computer-command-artifact.js";
import {
  assertCloudComputerReverseDialCredential,
  assertCloudComputerCommand,
  assertCloudComputerRuntimeAcknowledgement,
  assertCloudComputerRuntimeReservation,
  CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA,
  createCloudComputerCommandEvent,
  createCloudComputerCommand,
  type CloudComputerCommand as CoreCloudComputerCommand,
  type CloudComputerRuntimeAcknowledgement,
  type CloudComputerRuntimeReservation,
  type CloudComputerReverseDialCredential,
} from "./cloud-computer-command.js";
import type { SyncSql, SyncTransactionSql } from "./sql.js";
import {
  assertCloudComputerCommandRecoveryEvidence,
  type CloudComputerCommandRecoveryEvidence,
} from "./cloud-computer-command-recovery.js";

const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;
const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const LOCK = "openagents.cloud-computer.command";
const MAX_INLINE_EVENT_BYTES = 64 * 1_024;
const MAX_RETAINED_SESSION_EVENTS = 10_000;
const SAFE_TERMINAL_REASON = /^[a-z][a-z0-9_]{0,63}$/u;

export type CloudComputerCommandStatus =
  | "admitted"
  | "not_dispatched"
  | "dispatched"
  | "may_have_started"
  | "running"
  | "completed"
  | "failed"
  | "cancelled"
  | "timed_out"
  | "lost";

export class CloudComputerCommandStoreError extends Error {
  constructor(
    readonly code: "conflict" | "cursor_expired" | "invalid" | "not_found" | "stale_generation",
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerCommandStoreError";
  }
}

export type CloudComputerCommand = Readonly<{
  commandRef: string;
  requestDigest: string;
  computerRef: string;
  workspaceRef: string;
  sessionRef: string;
  runtimeGeneration: number;
  runtimeRef: string;
  providerLeaseRef: string;
  status: CloudComputerCommandStatus;
  providerCommandRef: string | null;
  dispatchRef: string | null;
  terminalRef: string | null;
  terminalDigest: string | null;
  terminalReason: string | null;
  exitCode: number | null;
  outputDigest: string | null;
  terminalSessionSequence: number | null;
  terminalCommandSequence: number | null;
  fence: number;
  replayed: boolean;
}>;

type CommandRow = Readonly<{
  command_ref: string;
  request_digest: string;
  computer_ref: string;
  workspace_ref: string;
  session_ref: string;
  runtime_generation: string | number;
  runtime_ref: string;
  provider_lease_ref: string;
  status: CloudComputerCommandStatus;
  provider_command_ref: string | null;
  dispatch_ref: string | null;
  terminal_ref: string | null;
  terminal_digest: string | null;
  terminal_reason: string | null;
  terminal_exit_code?: string | number | null;
  terminal_output_digest?: string | null;
  terminal_session_sequence: string | number | null;
  terminal_command_sequence: string | number | null;
  fence: string | number;
}>;

const commandFrom = (row: CommandRow, replayed = false): CloudComputerCommand => ({
  commandRef: row.command_ref,
  requestDigest: row.request_digest,
  computerRef: row.computer_ref,
  workspaceRef: row.workspace_ref,
  sessionRef: row.session_ref,
  runtimeGeneration: Number(row.runtime_generation),
  runtimeRef: row.runtime_ref,
  providerLeaseRef: row.provider_lease_ref,
  status: row.status,
  providerCommandRef: row.provider_command_ref,
  dispatchRef: row.dispatch_ref,
  terminalRef: row.terminal_ref,
  terminalDigest: row.terminal_digest,
  terminalReason: row.terminal_reason,
  exitCode: row.terminal_exit_code == null ? null : Number(row.terminal_exit_code),
  outputDigest: row.terminal_output_digest ?? null,
  terminalSessionSequence:
    row.terminal_session_sequence === null ? null : Number(row.terminal_session_sequence),
  terminalCommandSequence:
    row.terminal_command_sequence === null ? null : Number(row.terminal_command_sequence),
  fence: Number(row.fence),
  replayed,
});

const assertRef = (value: string, field: string): void => {
  if (!REF.test(value)) throw new CloudComputerCommandStoreError("invalid", `${field} is invalid`);
};
const assertDigest = (value: string, field: string): void => {
  if (!SHA256.test(value))
    throw new CloudComputerCommandStoreError("invalid", `${field} is invalid`);
};
const assertGeneration = (value: number): void => {
  if (!Number.isSafeInteger(value) || value < 1)
    throw new CloudComputerCommandStoreError("invalid", "generation is invalid");
};
const assertTimestamp = (value: string): void => {
  if (!Number.isFinite(Date.parse(value)))
    throw new CloudComputerCommandStoreError("invalid", "timestamp is invalid");
};

const dispatchClaim = Symbol("cloud-computer-command-dispatch");
export class CloudComputerCommandDispatchClaim {
  readonly #claim = dispatchClaim;

  constructor(
    token: typeof dispatchClaim,
    readonly commandRef: string,
    readonly dispatchRef: string,
    readonly runtimeGeneration: number,
    readonly runtimeRef: string,
    readonly providerLeaseRef: string,
  ) {
    if (token !== dispatchClaim) throw new Error("invalid dispatch claim");
  }

  authentic(): boolean {
    return this.#claim === dispatchClaim;
  }
}

type Fence = Readonly<{
  commandRef: string;
  sessionRef: string;
  attachmentEpoch: number;
  runtimeGeneration: number;
  runtimeRef: string;
  providerLeaseRef: string;
  observedAt: string;
}>;

export class PostgresCloudComputerCommandStore {
  constructor(private readonly sql: SyncSql) {}

  private async serializable<A>(run: (tx: SyncTransactionSql) => Promise<A>): Promise<A> {
    for (let attempt = 0; attempt < 32; attempt += 1) {
      try {
        // eslint-disable-next-line no-await-in-loop -- retries observe the prior SQLSTATE.
        return await this.sql.begin("isolation level serializable", run);
      } catch (error) {
        const code =
          typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
        if (code !== "40001" || attempt === 31) throw error;
      }
    }
    throw new CloudComputerCommandStoreError("conflict", "serialization retries exhausted");
  }

  private lock(tx: SyncTransactionSql, commandRef: string): Promise<unknown> {
    return tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${LOCK}|${commandRef}`}, 3107))`;
  }

  private validateFence(input: Fence): void {
    for (const [field, value] of Object.entries({
      commandRef: input.commandRef,
      sessionRef: input.sessionRef,
      runtimeRef: input.runtimeRef,
      providerLeaseRef: input.providerLeaseRef,
    }))
      assertRef(value, field);
    assertGeneration(input.runtimeGeneration);
    if (!Number.isSafeInteger(input.attachmentEpoch) || input.attachmentEpoch < 1)
      throw new CloudComputerCommandStoreError("invalid", "attachment epoch is invalid");
    assertTimestamp(input.observedAt);
  }

  private async fencedCommand(tx: SyncTransactionSql, input: Fence): Promise<CommandRow> {
    const rows: ReadonlyArray<CommandRow> = await tx`
      SELECT command.command_ref, command.request_digest, command.computer_ref,
             command.workspace_ref, command.session_ref, command.runtime_generation, command.runtime_ref,
             command.provider_lease_ref, command.status, command.provider_command_ref,
             command.dispatch_ref, command.terminal_ref, command.terminal_digest,
             command.terminal_reason, command.terminal_exit_code,
             command.terminal_output_digest, command.terminal_session_sequence,
             command.terminal_command_sequence, command.fence
      FROM khala_sync_cloud_computer_commands AS command
      JOIN khala_sync_cloud_computers AS computer
        ON computer.computer_ref = command.computer_ref
      JOIN khala_sync_cloud_computer_workspaces AS workspace
        ON workspace.workspace_ref = command.workspace_ref
      JOIN khala_sync_cloud_computer_command_sessions AS session
        ON session.session_ref = command.session_ref
      WHERE command.command_ref = ${input.commandRef}
        AND command.runtime_generation = ${input.runtimeGeneration}
        AND command.session_ref = ${input.sessionRef}
        AND command.runtime_ref = ${input.runtimeRef}
        AND command.provider_lease_ref = ${input.providerLeaseRef}
        AND computer.generation = command.runtime_generation
        AND computer.active_lease_ref = command.provider_lease_ref
        AND computer.state IN ('active', 'stopping')
        AND workspace.computer_ref = computer.computer_ref
        AND workspace.runtime_generation = command.runtime_generation
        AND workspace.state = 'active'
        AND session.attachment_epoch = ${input.attachmentEpoch}
        AND session.runtime_generation = command.runtime_generation
        AND session.runtime_ref = command.runtime_ref
        AND session.provider_lease_ref = command.provider_lease_ref
      FOR UPDATE OF command
    `;
    if (rows[0] === undefined)
      throw new CloudComputerCommandStoreError("stale_generation", "command fence differs");
    return rows[0];
  }

  async initializeSession(
    input: Readonly<{
      sessionRef: string;
      computerRef: string;
      workspaceRef: string;
      ownerRef: string;
      tenantRef: string;
      runtimeGeneration: number;
      runtimeRef: string;
      providerLeaseRef: string;
      createdAt: string;
    }>,
  ): Promise<void> {
    for (const [field, value] of Object.entries({
      sessionRef: input.sessionRef,
      computerRef: input.computerRef,
      workspaceRef: input.workspaceRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      runtimeRef: input.runtimeRef,
      providerLeaseRef: input.providerLeaseRef,
    }))
      assertRef(value, field);
    assertGeneration(input.runtimeGeneration);
    assertTimestamp(input.createdAt);
    const rows: ReadonlyArray<{ session_ref: string }> = await this.sql`
      INSERT INTO khala_sync_cloud_computer_command_sessions
        (session_ref, computer_ref, workspace_ref, owner_ref, tenant_ref,
         runtime_generation, runtime_ref, provider_lease_ref, authority_digest,
         created_at, updated_at)
      SELECT ${input.sessionRef}, computer.computer_ref, workspace.workspace_ref,
             computer.owner_ref, computer.tenant_ref, computer.generation,
             ${input.runtimeRef}, computer.active_lease_ref, computer.authority_snapshot_digest,
             ${input.createdAt}, ${input.createdAt}
      FROM khala_sync_cloud_computers AS computer
      JOIN khala_sync_cloud_computer_workspaces AS workspace
        ON workspace.computer_ref = computer.computer_ref
      WHERE computer.computer_ref = ${input.computerRef}
        AND workspace.workspace_ref = ${input.workspaceRef}
        AND computer.owner_ref = ${input.ownerRef} AND computer.tenant_ref = ${input.tenantRef}
        AND computer.generation = ${input.runtimeGeneration}
        AND computer.active_lease_ref = ${input.providerLeaseRef}
        AND computer.state = 'active' AND workspace.runtime_generation = computer.generation
      ON CONFLICT (session_ref) DO UPDATE SET session_ref = EXCLUDED.session_ref
      WHERE khala_sync_cloud_computer_command_sessions.computer_ref = EXCLUDED.computer_ref
        AND khala_sync_cloud_computer_command_sessions.workspace_ref = EXCLUDED.workspace_ref
        AND khala_sync_cloud_computer_command_sessions.owner_ref = EXCLUDED.owner_ref
        AND khala_sync_cloud_computer_command_sessions.tenant_ref = EXCLUDED.tenant_ref
        AND khala_sync_cloud_computer_command_sessions.runtime_generation = EXCLUDED.runtime_generation
        AND khala_sync_cloud_computer_command_sessions.runtime_ref = EXCLUDED.runtime_ref
        AND khala_sync_cloud_computer_command_sessions.provider_lease_ref = EXCLUDED.provider_lease_ref
        AND khala_sync_cloud_computer_command_sessions.authority_digest = EXCLUDED.authority_digest
      RETURNING session_ref
    `;
    if (rows.length !== 1)
      throw new CloudComputerCommandStoreError("stale_generation", "session fence differs");
  }

  async attach(
    input: Readonly<{
      connectionRef: string;
      credential: CloudComputerReverseDialCredential;
      observedAt: string;
    }>,
  ): Promise<
    Readonly<{
      attachmentEpoch: number;
      lastSessionSequence: number;
      retainedThroughSessionSequence: number;
      retentionEpoch: number;
    }>
  > {
    for (const [field, value] of Object.entries({
      connectionRef: input.connectionRef,
    }))
      assertRef(value, field);
    assertCloudComputerReverseDialCredential(input.credential);
    assertTimestamp(input.observedAt);
    const credential = input.credential;
    const observedAt = Date.parse(input.observedAt);
    if (
      observedAt < Date.parse(credential.issuedAt) ||
      observedAt >= Date.parse(credential.expiresAt)
    )
      throw new CloudComputerCommandStoreError("stale_generation", "credential is expired");
    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${LOCK}|session|${credential.sessionRef}`}, 3107))`;
      const rows: ReadonlyArray<{
        attachment_epoch: string | number;
        last_session_sequence: string | number;
        retained_through_session_sequence: string | number;
        retention_epoch: string | number;
      }> = await tx`
        UPDATE khala_sync_cloud_computer_command_sessions AS session
        SET attachment_epoch = attachment_epoch + 1, connection_ref = ${input.connectionRef},
            updated_at = ${input.observedAt}
        FROM khala_sync_cloud_computers AS computer,
             khala_sync_cloud_computer_workspaces AS workspace
        WHERE session.session_ref = ${credential.sessionRef}
          AND session.computer_ref = ${credential.computerRef}
          AND session.workspace_ref = ${credential.workspaceRef}
          AND session.owner_ref = ${credential.ownerRef}
          AND session.tenant_ref = ${credential.tenantRef}
          AND session.runtime_generation = ${credential.runtimeGeneration}
          AND session.runtime_ref = ${credential.runtimeRef}
          AND session.provider_lease_ref = ${credential.providerLeaseRef}
          AND session.authority_digest = ${credential.authorityDigest}
          AND computer.computer_ref = session.computer_ref
          AND computer.generation = session.runtime_generation
          AND computer.active_lease_ref = session.provider_lease_ref
          AND computer.state = 'active'
          AND workspace.workspace_ref = session.workspace_ref
          AND workspace.runtime_generation = session.runtime_generation
          AND NOT EXISTS (
            SELECT 1 FROM khala_sync_cloud_computer_command_credential_nonces AS nonce
            WHERE nonce.nonce = ${credential.nonce}
          )
        RETURNING session.attachment_epoch, session.last_session_sequence,
                  session.retained_through_session_sequence, session.retention_epoch
      `;
      if (rows[0] === undefined)
        throw new CloudComputerCommandStoreError("stale_generation", "credential fence differs");
      await tx`
        INSERT INTO khala_sync_cloud_computer_command_credential_nonces
          (nonce, session_ref, authority_digest, attachment_epoch, consumed_at)
        VALUES (${credential.nonce}, ${credential.sessionRef}, ${credential.authorityDigest},
                ${Number(rows[0].attachment_epoch)}, ${input.observedAt})
      `;
      return {
        attachmentEpoch: Number(rows[0].attachment_epoch),
        lastSessionSequence: Number(rows[0].last_session_sequence),
        retainedThroughSessionSequence: Number(rows[0].retained_through_session_sequence),
        retentionEpoch: Number(rows[0].retention_epoch),
      };
    });
  }

  async admit(
    input: Readonly<{
      commandRef: string;
      idempotencyRef: string;
      requestDigest: string;
      computerRef: string;
      workspaceRef: string;
      sessionRef: string;
      ownerRef: string;
      tenantRef: string;
      runtimeGeneration: number;
      runtimeRef: string;
      providerLeaseRef: string;
      workingDirectory: string;
      capabilityRefs: ReadonlyArray<string>;
      capabilityDigest: string;
      budgetSnapshotDigest: string;
      budgetLimits: Readonly<Record<string, number>>;
      deadlineAt: string;
      request: CoreCloudComputerCommand;
      createdAt: string;
    }>,
  ): Promise<CloudComputerCommand> {
    for (const [field, value] of Object.entries({
      commandRef: input.commandRef,
      idempotencyRef: input.idempotencyRef,
      computerRef: input.computerRef,
      workspaceRef: input.workspaceRef,
      sessionRef: input.sessionRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      runtimeRef: input.runtimeRef,
      providerLeaseRef: input.providerLeaseRef,
    }))
      assertRef(value, field);
    assertDigest(input.requestDigest, "request digest");
    assertDigest(input.capabilityDigest, "capability digest");
    assertDigest(input.budgetSnapshotDigest, "budget snapshot digest");
    input.capabilityRefs.forEach((ref) => assertRef(ref, "capability ref"));
    // `budgetLimits` is an open record, so its `outputBytes` may be absent.
    // An absent output bound is already rejected by the safe-integer test; it is
    // named here so the type says what the test was always enforcing.
    const outputBytes: number | undefined = input.budgetLimits["outputBytes"];
    if (
      !input.workingDirectory.startsWith("/") ||
      input.workingDirectory.includes("\0") ||
      outputBytes === undefined ||
      !Number.isSafeInteger(outputBytes) ||
      outputBytes < 0 ||
      Object.values(input.budgetLimits).some((value) => !Number.isSafeInteger(value) || value < 0)
    )
      throw new CloudComputerCommandStoreError("invalid", "request authority is invalid");
    assertTimestamp(input.deadlineAt);
    if (Date.parse(input.deadlineAt) <= Date.parse(input.createdAt))
      throw new CloudComputerCommandStoreError("invalid", "deadline is invalid");
    assertCloudComputerCommand(input.request);
    const canonicalRequest = createCloudComputerCommand(input.request);
    if (
      canonicalRequest.requestDigest !== input.requestDigest ||
      input.request.commandRef !== input.commandRef ||
      input.request.idempotencyRef !== input.idempotencyRef ||
      input.request.computerRef !== input.computerRef ||
      input.request.workspaceRef !== input.workspaceRef ||
      input.request.ownerRef !== input.ownerRef ||
      input.request.tenantRef !== input.tenantRef ||
      input.request.runtimeGeneration !== input.runtimeGeneration ||
      input.request.runtimeRef !== input.runtimeRef ||
      input.request.providerLeaseRef !== input.providerLeaseRef ||
      input.request.workingDirectory !== input.workingDirectory ||
      input.request.capabilitySnapshotDigest !== input.capabilityDigest ||
      input.request.budgetSnapshotDigest !== input.budgetSnapshotDigest ||
      canonicalJson(input.request.capabilityRefs) !== canonicalJson(input.capabilityRefs) ||
      canonicalJson(input.request.budget) !== canonicalJson(input.budgetLimits) ||
      new Date(input.request.deadlineAt).toISOString() !== new Date(input.deadlineAt).toISOString()
    )
      throw new CloudComputerCommandStoreError("conflict", "canonical request digest differs");
    assertGeneration(input.runtimeGeneration);
    assertTimestamp(input.createdAt);
    return this.serializable(async (tx) => {
      await tx`
        SELECT pg_advisory_xact_lock(hashtextextended(lock_key, 3107))
        FROM (
          SELECT unnest(${[
            `${LOCK}|${input.commandRef}`,
            `${LOCK}|idempotency|${input.computerRef}|${input.runtimeGeneration}|${input.idempotencyRef}`,
          ]}::text[]) AS lock_key
          ORDER BY lock_key
        ) AS ordered_locks
      `;
      const prior: ReadonlyArray<
        CommandRow & {
          idempotency_ref: string;
          owner_ref: string;
          tenant_ref: string;
          working_directory: string;
          capability_refs_json: unknown;
          capability_digest: string;
          budget_snapshot_digest: string;
          budget_limits_json: unknown;
          deadline_at: string | Date;
          request_json: unknown;
        }
      > = await tx`
          SELECT command_ref, idempotency_ref, request_digest, computer_ref, workspace_ref, session_ref,
                 owner_ref, tenant_ref, runtime_generation, runtime_ref, provider_lease_ref,
                 working_directory, capability_refs_json, capability_digest,
                 budget_snapshot_digest, budget_limits_json, deadline_at, request_json,
                 status, provider_command_ref, dispatch_ref, terminal_ref, terminal_digest,
                 terminal_reason, terminal_session_sequence, terminal_command_sequence, fence
          FROM khala_sync_cloud_computer_commands
          WHERE command_ref = ${input.commandRef}
             OR (computer_ref = ${input.computerRef} AND runtime_generation = ${input.runtimeGeneration}
                 AND idempotency_ref = ${input.idempotencyRef})
          FOR UPDATE
        `;
      if (prior[0] !== undefined) {
        const row = prior[0];
        if (
          row.command_ref !== input.commandRef ||
          row.idempotency_ref !== input.idempotencyRef ||
          row.request_digest !== input.requestDigest ||
          row.computer_ref !== input.computerRef ||
          row.workspace_ref !== input.workspaceRef ||
          row.session_ref !== input.sessionRef ||
          row.owner_ref !== input.ownerRef ||
          row.tenant_ref !== input.tenantRef ||
          Number(row.runtime_generation) !== input.runtimeGeneration ||
          row.runtime_ref !== input.runtimeRef ||
          row.provider_lease_ref !== input.providerLeaseRef ||
          row.working_directory !== input.workingDirectory ||
          canonicalJson(row.capability_refs_json) !== canonicalJson(input.capabilityRefs) ||
          row.capability_digest !== input.capabilityDigest ||
          row.budget_snapshot_digest !== input.budgetSnapshotDigest ||
          canonicalJson(row.budget_limits_json) !== canonicalJson(input.budgetLimits) ||
          new Date(row.deadline_at).toISOString() !== new Date(input.deadlineAt).toISOString() ||
          canonicalJson(row.request_json) !== canonicalJson(input.request)
        )
          throw new CloudComputerCommandStoreError("conflict", "idempotency bytes differ");
        return commandFrom(row, true);
      }
      const inserted: ReadonlyArray<CommandRow> = await tx`
        INSERT INTO khala_sync_cloud_computer_commands
          (command_ref, idempotency_ref, request_digest, computer_ref, workspace_ref, session_ref,
           owner_ref, tenant_ref, runtime_generation, runtime_ref, provider_lease_ref,
           working_directory, capability_refs_json, capability_digest,
           budget_snapshot_digest, budget_limits_json, deadline_at, request_json,
           status, created_at, updated_at)
        SELECT ${input.commandRef}, ${input.idempotencyRef}, ${input.requestDigest},
               computer.computer_ref, workspace.workspace_ref, session.session_ref, computer.owner_ref,
               computer.tenant_ref, computer.generation, ${input.runtimeRef},
               computer.active_lease_ref, ${input.workingDirectory}, ${input.capabilityRefs}::jsonb,
               ${input.capabilityDigest}, ${input.budgetSnapshotDigest}, ${input.budgetLimits}::jsonb,
               ${input.deadlineAt}, ${input.request}::jsonb, 'admitted', ${input.createdAt}, ${input.createdAt}
        FROM khala_sync_cloud_computers AS computer
      JOIN khala_sync_cloud_computer_workspaces AS workspace
        ON workspace.computer_ref = computer.computer_ref
      JOIN khala_sync_cloud_computer_command_sessions AS session
        ON session.workspace_ref = workspace.workspace_ref
        WHERE computer.computer_ref = ${input.computerRef}
          AND workspace.workspace_ref = ${input.workspaceRef}
          AND session.session_ref = ${input.sessionRef}
          AND computer.owner_ref = ${input.ownerRef} AND computer.tenant_ref = ${input.tenantRef}
          AND computer.generation = ${input.runtimeGeneration}
          AND computer.active_lease_ref = ${input.providerLeaseRef}
          AND computer.state = 'active'
          AND workspace.runtime_generation = computer.generation
          AND session.runtime_generation = computer.generation
          AND session.runtime_ref = ${input.runtimeRef}
          AND session.provider_lease_ref = computer.active_lease_ref
          AND workspace.state = 'active'
        RETURNING command_ref, request_digest, computer_ref, workspace_ref, session_ref,
                  runtime_generation, runtime_ref, provider_lease_ref, status,
                  provider_command_ref, dispatch_ref, terminal_ref, terminal_digest,
                  terminal_reason, terminal_session_sequence, terminal_command_sequence, fence
      `;
      if (inserted[0] === undefined)
        throw new CloudComputerCommandStoreError("stale_generation", "admission fence differs");
      return commandFrom(inserted[0]);
    });
  }

  async markNotDispatched(input: Fence): Promise<CloudComputerCommand> {
    this.validateFence(input);
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      if (command.terminal_ref !== null)
        throw new CloudComputerCommandStoreError("conflict", "command is terminal");
      const rows: ReadonlyArray<CommandRow> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'not_dispatched', updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND status = 'admitted'
        RETURNING command_ref, request_digest, computer_ref, workspace_ref, session_ref,
                  runtime_generation, runtime_ref, provider_lease_ref, status,
                  provider_command_ref, dispatch_ref, terminal_ref, terminal_digest,
                  terminal_reason, terminal_session_sequence, terminal_command_sequence, fence
      `;
      if (rows[0] === undefined)
        throw new CloudComputerCommandStoreError("conflict", "command was already dispatched");
      return commandFrom(rows[0]);
    });
  }

  async prepareDispatchAttempt(input: Fence & Readonly<{ dispatchRef: string }>): Promise<void> {
    this.validateFence(input);
    assertRef(input.dispatchRef, "dispatch ref");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      await this.fencedCommand(tx, input);
      const prior: ReadonlyArray<{ attempt_ref: string; status: string }> = await tx`
        SELECT attempt_ref, status
        FROM khala_sync_cloud_computer_command_dispatch_attempts
        WHERE command_ref = ${input.commandRef}
        ORDER BY prepared_at DESC, attempt_ref DESC
        LIMIT 1
        FOR UPDATE
      `;
      if (prior[0]?.attempt_ref === input.dispatchRef && prior[0].status === "prepared") return;
      if (prior[0] !== undefined && prior[0].status !== "not_exposed")
        throw new CloudComputerCommandStoreError("conflict", "dispatch attempt is already present");
      const rows: ReadonlyArray<{ command_ref: string }> = await tx`
        INSERT INTO khala_sync_cloud_computer_command_dispatch_attempts
          (attempt_ref, command_ref, runtime_generation, runtime_ref,
           provider_lease_ref, status, prepared_at)
        SELECT ${input.dispatchRef}, command_ref, runtime_generation, runtime_ref,
               provider_lease_ref, 'prepared', ${input.observedAt}
        FROM khala_sync_cloud_computer_commands
        WHERE command_ref = ${input.commandRef} AND status IN ('admitted', 'not_dispatched')
        RETURNING command_ref
      `;
      if (rows.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "dispatch attempt is already present");
    });
  }

  /** Persists write exposure before returning the only claim authorized for provider I/O. */
  async exposeDispatchAttempt(
    input: Fence & Readonly<{ dispatchRef: string }>,
  ): Promise<CloudComputerCommandDispatchClaim> {
    this.validateFence(input);
    assertRef(input.dispatchRef, "dispatch ref");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      await this.fencedCommand(tx, input);
      const deadlines: ReadonlyArray<{ allowed: boolean }> = await tx`
        SELECT (${input.observedAt} <= deadline_at) AS allowed
        FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
      `;
      if (deadlines[0]?.allowed !== true)
        throw new CloudComputerCommandStoreError("conflict", "dispatch exposure exceeded deadline");
      const attempts: ReadonlyArray<{ attempt_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_command_dispatch_attempts
        SET status = 'write_exposed', write_exposed_at = ${input.observedAt},
            revision = revision + 1
        WHERE attempt_ref = ${input.dispatchRef} AND command_ref = ${input.commandRef}
          AND runtime_generation = ${input.runtimeGeneration}
          AND runtime_ref = ${input.runtimeRef} AND provider_lease_ref = ${input.providerLeaseRef}
          AND status = 'prepared'
        RETURNING attempt_ref
      `;
      if (attempts.length !== 1)
        throw new CloudComputerCommandStoreError(
          "conflict",
          "write exposure is already committed and cannot be replayed automatically",
        );
      const commands: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'may_have_started', dispatch_ref = ${input.dispatchRef},
            dispatched_at = ${input.observedAt}, updated_at = ${input.observedAt},
            revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND status IN ('admitted', 'not_dispatched')
        RETURNING command_ref
      `;
      if (commands.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "command dispatch CAS failed");
      return new CloudComputerCommandDispatchClaim(
        dispatchClaim,
        input.commandRef,
        input.dispatchRef,
        input.runtimeGeneration,
        input.runtimeRef,
        input.providerLeaseRef,
      );
    });
  }

  async markMayHaveStarted(input: Fence & Readonly<{ dispatchRef: string }>): Promise<void> {
    this.validateFence(input);
    await this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      await this.fencedCommand(tx, input);
      const attempts: ReadonlyArray<{ attempt_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_command_dispatch_attempts
        SET status = 'may_have_started', settled_at = ${input.observedAt}, revision = revision + 1
        WHERE attempt_ref = ${input.dispatchRef} AND command_ref = ${input.commandRef}
          AND status = 'write_exposed'
        RETURNING attempt_ref
      `;
      const commands: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'may_have_started', updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND dispatch_ref = ${input.dispatchRef}
          AND status = 'may_have_started'
        RETURNING command_ref
      `;
      if (attempts.length !== 1 || commands.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "dispatch uncertainty differs");
    });
  }

  async recordReservation(
    input: Fence &
      Readonly<{
        dispatchRef: string;
        reservationRef: string;
        providerExecutionRef: string;
        providerCommandRef: string;
        reservation: CloudComputerRuntimeReservation;
      }>,
  ): Promise<void> {
    this.validateFence(input);
    assertCloudComputerRuntimeReservation(input.reservation);
    for (const [field, value] of Object.entries({
      dispatchRef: input.dispatchRef,
      reservationRef: input.reservationRef,
      providerExecutionRef: input.providerExecutionRef,
      providerCommandRef: input.providerCommandRef,
    }))
      assertRef(value, field);
    if (
      input.reservation.commandRef !== input.commandRef ||
      input.reservation.reservationRef !== input.reservationRef ||
      input.reservation.providerExecutionRef !== input.providerExecutionRef ||
      input.reservation.runtimeGeneration !== input.runtimeGeneration ||
      input.reservation.runtimeRef !== input.runtimeRef ||
      input.reservation.providerLeaseRef !== input.providerLeaseRef ||
      input.reservation.reservedAt !== input.observedAt
    )
      throw new CloudComputerCommandStoreError("conflict", "runtime reservation authority differs");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      if (input.reservation.requestDigest !== command.request_digest)
        throw new CloudComputerCommandStoreError("conflict", "runtime reservation request differs");
      const attempts: ReadonlyArray<{ attempt_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_command_dispatch_attempts
        SET status = 'reservation_recorded', reservation_ref = ${input.reservationRef},
            provider_execution_ref = ${input.providerExecutionRef},
            provider_command_ref = ${input.providerCommandRef}, revision = revision + 1
        WHERE attempt_ref = ${input.dispatchRef} AND command_ref = ${input.commandRef}
          AND status IN ('write_exposed', 'may_have_started')
        RETURNING attempt_ref
      `;
      const commands: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET reservation_ref = ${input.reservationRef},
            provider_execution_ref = ${input.providerExecutionRef},
            provider_command_ref = ${input.providerCommandRef},
            updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND status = 'may_have_started'
        RETURNING command_ref
      `;
      if (attempts.length !== 1 || commands.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "runtime reservation differs");
    });
  }

  async recordDispatchedAcknowledgement(
    input: Fence &
      Readonly<{
        dispatchRef: string;
        reservationRef: string;
        providerExecutionRef: string;
        providerCommandRef: string;
        acknowledgementEventRef: string;
        acknowledgementEventDigest: string;
        expectedFence: number;
        expectedCommandSequence: number;
        reservation: CloudComputerRuntimeReservation;
        acknowledgement: CloudComputerRuntimeAcknowledgement;
      }>,
  ): Promise<CloudComputerCommand> {
    this.validateFence(input);
    assertCloudComputerRuntimeReservation(input.reservation);
    assertCloudComputerRuntimeAcknowledgement(input.acknowledgement);
    assertRef(input.dispatchRef, "dispatch ref");
    assertRef(input.reservationRef, "reservation ref");
    assertRef(input.providerExecutionRef, "provider execution ref");
    assertRef(input.providerCommandRef, "provider command ref");
    assertRef(input.acknowledgementEventRef, "acknowledgement event ref");
    assertDigest(input.acknowledgementEventDigest, "acknowledgement event digest");
    if (
      input.reservation.commandRef !== input.commandRef ||
      input.reservation.reservationRef !== input.reservationRef ||
      input.reservation.providerExecutionRef !== input.providerExecutionRef ||
      input.acknowledgement.commandRef !== input.commandRef ||
      input.acknowledgement.reservationRef !== input.reservationRef ||
      input.acknowledgement.providerExecutionRef !== input.providerExecutionRef ||
      input.acknowledgement.eventRef !== input.acknowledgementEventRef ||
      input.acknowledgement.eventDigest !== input.acknowledgementEventDigest ||
      input.acknowledgement.acceptedSequence !== input.expectedCommandSequence ||
      input.acknowledgement.fence !== input.expectedFence ||
      input.acknowledgement.observedAt !== input.observedAt
    )
      throw new CloudComputerCommandStoreError(
        "conflict",
        "runtime acknowledgement authority differs",
      );
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      if (
        input.reservation.requestDigest !== command.request_digest ||
        input.acknowledgement.requestDigest !== command.request_digest
      )
        throw new CloudComputerCommandStoreError(
          "conflict",
          "runtime acknowledgement request differs",
        );
      const deadlines: ReadonlyArray<{ allowed: boolean }> = await tx`
        SELECT (${input.observedAt} <= deadline_at) AS allowed
        FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
      `;
      if (deadlines[0]?.allowed !== true)
        throw new CloudComputerCommandStoreError("conflict", "acknowledgement exceeded deadline");
      if (
        command.status === "dispatched" &&
        command.dispatch_ref === input.dispatchRef &&
        command.provider_command_ref === input.providerCommandRef
      ) {
        const bindings: ReadonlyArray<{
          reservation_ref: string;
          provider_execution_ref: string;
          acknowledgement_event_ref: string;
          acknowledgement_event_digest: string;
          command_sequence: string | number;
          fence: string | number;
        }> = await tx`
          SELECT command.reservation_ref, command.provider_execution_ref,
                 command.acknowledgement_event_ref, command.acknowledgement_event_digest,
                 event.command_sequence, event.fence
          FROM khala_sync_cloud_computer_commands AS command
          JOIN khala_sync_cloud_computer_command_events AS event
            ON event.event_ref = command.acknowledgement_event_ref
          WHERE command.command_ref = ${input.commandRef}
        `;
        if (
          bindings[0]?.reservation_ref !== input.reservationRef ||
          bindings[0]?.provider_execution_ref !== input.providerExecutionRef ||
          bindings[0]?.acknowledgement_event_ref !== input.acknowledgementEventRef ||
          bindings[0]?.acknowledgement_event_digest !== input.acknowledgementEventDigest ||
          Number(bindings[0]?.command_sequence) !== input.expectedCommandSequence ||
          Number(bindings[0]?.fence) !== input.expectedFence
        )
          throw new CloudComputerCommandStoreError("conflict", "acknowledgement bytes differ");
        return commandFrom(command, true);
      }
      const acceptedEvent = createCloudComputerCommandEvent({
        eventRef: input.acknowledgementEventRef,
        commandRef: input.commandRef,
        requestDigest: command.request_digest as `sha256:${string}`,
        providerExecutionRef: input.providerExecutionRef,
        sessionRef: input.sessionRef,
        runtimeRef: input.runtimeRef,
        runtimeGeneration: input.runtimeGeneration,
        sequence: input.expectedCommandSequence,
        fence: input.expectedFence,
        kind: "accepted",
        payload: {
          reservationRef: input.reservationRef,
          providerExecutionRef: input.providerExecutionRef,
        },
        observedAt: input.observedAt,
      });
      if (acceptedEvent.eventDigest !== input.acknowledgementEventDigest)
        throw new CloudComputerCommandStoreError("conflict", "acknowledgement digest differs");
      const attempts: ReadonlyArray<{ attempt_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_command_dispatch_attempts
        SET status = 'acknowledged', provider_command_ref = ${input.providerCommandRef},
            reservation_ref = ${input.reservationRef},
            provider_execution_ref = ${input.providerExecutionRef},
            acknowledgement_event_ref = ${input.acknowledgementEventRef},
            acknowledgement_event_digest = ${input.acknowledgementEventDigest},
            settled_at = ${input.observedAt}, revision = revision + 1
        WHERE attempt_ref = ${input.dispatchRef} AND command_ref = ${input.commandRef}
          AND status = 'reservation_recorded'
          AND reservation_ref = ${input.reservationRef}
          AND provider_execution_ref = ${input.providerExecutionRef}
          AND provider_command_ref = ${input.providerCommandRef}
        RETURNING attempt_ref
      `;
      if (attempts.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "dispatch acknowledgment differs");
      const sessionCounters: ReadonlyArray<{ session_sequence: string | number }> = await tx`
        UPDATE khala_sync_cloud_computer_command_sessions
        SET last_session_sequence = last_session_sequence + 1,
            updated_at = ${input.observedAt}
        WHERE session_ref = ${input.sessionRef} AND attachment_epoch = ${input.attachmentEpoch}
        RETURNING last_session_sequence AS session_sequence
      `;
      const commandCounters: ReadonlyArray<{ command_sequence: string | number }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET next_command_sequence = next_command_sequence + 1,
            updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef}
          AND fence = ${input.expectedFence}
          AND next_command_sequence = ${input.expectedCommandSequence}
        RETURNING next_command_sequence - 1 AS command_sequence
      `;
      if (sessionCounters[0] === undefined || commandCounters[0] === undefined)
        throw new CloudComputerCommandStoreError("conflict", "acknowledgement sequence differs");
      await tx`
        INSERT INTO khala_sync_cloud_computer_command_events
          (command_ref, session_ref, session_sequence, command_sequence,
           attachment_epoch, fence, event_ref, event_digest, kind,
           payload_json, artifact_refs_json, observed_at)
        VALUES (${input.commandRef}, ${input.sessionRef},
                ${Number(sessionCounters[0].session_sequence)},
                ${Number(commandCounters[0].command_sequence)}, ${input.attachmentEpoch},
                ${input.expectedFence}, ${input.acknowledgementEventRef},
                ${input.acknowledgementEventDigest}, 'accepted',
                ${{
                  reservationRef: input.reservationRef,
                  providerExecutionRef: input.providerExecutionRef,
                }}::jsonb,
                '[]'::jsonb, ${input.observedAt})
      `;
      const rows: ReadonlyArray<CommandRow> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'dispatched', provider_command_ref = ${input.providerCommandRef},
            reservation_ref = ${input.reservationRef},
            provider_execution_ref = ${input.providerExecutionRef},
            acknowledgement_event_ref = ${input.acknowledgementEventRef},
            acknowledgement_event_digest = ${input.acknowledgementEventDigest},
            updated_at = ${input.observedAt},
            revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND dispatch_ref = ${input.dispatchRef}
          AND status = 'may_have_started'
          AND reservation_ref = ${input.reservationRef}
          AND provider_execution_ref = ${input.providerExecutionRef}
          AND provider_command_ref = ${input.providerCommandRef}
        RETURNING command_ref, request_digest, computer_ref, workspace_ref, session_ref,
                  runtime_generation, runtime_ref, provider_lease_ref, status,
                  provider_command_ref, dispatch_ref, terminal_ref, terminal_digest,
                  terminal_reason, terminal_session_sequence, terminal_command_sequence, fence
      `;
      if (rows[0] === undefined)
        throw new CloudComputerCommandStoreError("conflict", "provider acknowledgment differs");
      const acknowledgementSequence = Number(sessionCounters[0].session_sequence);
      const removed: ReadonlyArray<{ session_sequence: string | number }> = await tx`
        DELETE FROM khala_sync_cloud_computer_command_events
        WHERE session_ref = ${input.sessionRef}
          AND session_sequence <= ${acknowledgementSequence - MAX_RETAINED_SESSION_EVENTS}
          AND NOT EXISTS (
            SELECT 1 FROM khala_sync_cloud_computer_commands AS terminal_command
            WHERE terminal_command.terminal_ref =
                  khala_sync_cloud_computer_command_events.event_ref
          )
        RETURNING session_sequence
      `;
      if (removed.length > 0) {
        const watermark = Math.max(...removed.map((event) => Number(event.session_sequence)));
        await tx`
          UPDATE khala_sync_cloud_computer_command_sessions
          SET retained_through_session_sequence =
                GREATEST(retained_through_session_sequence, ${watermark}),
              retention_epoch = retention_epoch + 1, updated_at = ${input.observedAt}
          WHERE session_ref = ${input.sessionRef}
        `;
      }
      return commandFrom(rows[0]);
    });
  }

  async recordRunning(
    input: Fence &
      Readonly<{
        dispatchRef: string;
        providerCommandRef: string;
        providerExecutionRef: string;
        requestDigest: string;
        expectedFence: number;
      }>,
  ): Promise<CloudComputerCommand> {
    this.validateFence(input);
    assertRef(input.dispatchRef, "dispatch ref");
    assertRef(input.providerCommandRef, "provider command ref");
    assertRef(input.providerExecutionRef, "provider execution ref");
    assertDigest(input.requestDigest, "request digest");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      const bindings: ReadonlyArray<{ provider_execution_ref: string | null }> = await tx`
        SELECT provider_execution_ref FROM khala_sync_cloud_computer_commands
        WHERE command_ref = ${input.commandRef}
      `;
      if (
        command.request_digest !== input.requestDigest ||
        Number(command.fence) !== input.expectedFence ||
        bindings[0]?.provider_execution_ref !== input.providerExecutionRef
      )
        throw new CloudComputerCommandStoreError("conflict", "runtime start binding differs");
      const deadlines: ReadonlyArray<{ allowed: boolean }> = await tx`
        SELECT (${input.observedAt} <= deadline_at) AS allowed
        FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
      `;
      if (deadlines[0]?.allowed !== true)
        throw new CloudComputerCommandStoreError("conflict", "runtime start exceeded deadline");
      if (
        command.status === "running" &&
        command.dispatch_ref === input.dispatchRef &&
        command.provider_command_ref === input.providerCommandRef
      )
        return commandFrom(command, true);
      const rows: ReadonlyArray<CommandRow> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'running', started_at = ${input.observedAt},
            updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND status = 'dispatched'
          AND dispatch_ref = ${input.dispatchRef}
          AND provider_command_ref = ${input.providerCommandRef}
        RETURNING command_ref, request_digest, computer_ref, workspace_ref, session_ref,
                  runtime_generation, runtime_ref, provider_lease_ref, status,
                  provider_command_ref, dispatch_ref, terminal_ref, terminal_digest,
                  terminal_reason, terminal_session_sequence, terminal_command_sequence, fence
      `;
      if (rows[0] === undefined)
        throw new CloudComputerCommandStoreError("conflict", "runtime start differs");
      return commandFrom(rows[0]);
    });
  }

  private async requestSettlement(
    input: Fence &
      Readonly<{
        settlementRef: string;
        settlementRequestDigest: string;
        expectedFence: number;
        reason: string;
      }>,
    kind: "cancel" | "timeout",
  ): Promise<Readonly<{ fence: number; replayed: boolean }>> {
    this.validateFence(input);
    assertRef(input.settlementRef, "settlement ref");
    assertDigest(input.settlementRequestDigest, "settlement request digest");
    if (
      !Number.isSafeInteger(input.expectedFence) ||
      input.expectedFence < 1 ||
      !SAFE_TERMINAL_REASON.test(input.reason)
    )
      throw new CloudComputerCommandStoreError("invalid", "settlement request is invalid");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      const prior: ReadonlyArray<{
        settlement_ref: string;
        command_ref: string;
        kind: "cancel" | "timeout";
        expected_fence: string | number;
        settled_fence: string | number;
        request_digest: string;
        reason: string;
      }> = await tx`
        SELECT settlement_ref, command_ref, kind, expected_fence, settled_fence, request_digest, reason
        FROM khala_sync_cloud_computer_command_settlements
        WHERE settlement_ref = ${input.settlementRef}
           OR (command_ref = ${input.commandRef} AND kind = ${kind})
        FOR UPDATE
      `;
      if (prior[0] !== undefined) {
        const settlement = prior[0];
        if (
          settlement.settlement_ref !== input.settlementRef ||
          settlement.command_ref !== input.commandRef ||
          settlement.kind !== kind ||
          Number(settlement.expected_fence) !== input.expectedFence ||
          settlement.request_digest !== input.settlementRequestDigest ||
          settlement.reason !== input.reason
        )
          throw new CloudComputerCommandStoreError("conflict", "settlement bytes differ");
        return { fence: Number(settlement.settled_fence), replayed: true };
      }
      if (Number(command.fence) !== input.expectedFence || command.terminal_ref !== null)
        throw new CloudComputerCommandStoreError("conflict", "settlement fence differs");
      const deadlineRows: ReadonlyArray<{ allowed: boolean }> = await tx`
        SELECT (${input.observedAt} >= deadline_at) AS allowed
        FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
      `;
      if (kind === "timeout" && deadlineRows[0]?.allowed !== true)
        throw new CloudComputerCommandStoreError("conflict", "timeout precedes deadline");
      const settledFence = input.expectedFence + 1;
      await tx`
        INSERT INTO khala_sync_cloud_computer_command_settlements
          (settlement_ref, command_ref, kind, expected_fence, settled_fence,
           request_digest, reason, requested_at)
        VALUES (${input.settlementRef}, ${input.commandRef}, ${kind}, ${input.expectedFence},
                ${settledFence}, ${input.settlementRequestDigest}, ${input.reason},
                ${input.observedAt})
      `;
      await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET fence = ${settledFence}, updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND fence = ${input.expectedFence}
      `;
      return { fence: settledFence, replayed: false };
    });
  }

  requestCancellation(
    input: Fence &
      Readonly<{
        settlementRef: string;
        settlementRequestDigest: string;
        expectedFence: number;
        reason: string;
      }>,
  ) {
    return this.requestSettlement(input, "cancel");
  }

  requestTimeout(
    input: Fence &
      Readonly<{
        settlementRef: string;
        settlementRequestDigest: string;
        expectedFence: number;
        reason: string;
      }>,
  ) {
    return this.requestSettlement(input, "timeout");
  }

  async recordTerminal(
    input: Fence &
      Readonly<{
        providerCommandRef: string;
        terminalRef: string;
        terminalDigest: string;
        eventDigest: string;
        status: "completed" | "failed" | "cancelled" | "timed_out";
        reason: string;
        exitCode: number | null;
        outputDigest: string | null;
        artifactRefs: ReadonlyArray<string>;
        maxRetainedEvents: number;
        requestDigest: string;
        providerExecutionRef: string;
        expectedFence: number;
        expectedCommandSequence: number;
      }>,
  ): Promise<CloudComputerCommand> {
    this.validateFence(input);
    assertRef(input.providerCommandRef, "provider command ref");
    assertRef(input.terminalRef, "terminal ref");
    assertDigest(input.terminalDigest, "terminal digest");
    assertDigest(input.eventDigest, "terminal event digest");
    assertDigest(input.requestDigest, "request digest");
    assertRef(input.providerExecutionRef, "provider execution ref");
    input.artifactRefs.forEach((ref) => assertRef(ref, "artifact ref"));
    if (!SAFE_TERMINAL_REASON.test(input.reason))
      throw new CloudComputerCommandStoreError("invalid", "terminal reason is invalid");
    if (input.exitCode !== null && (!Number.isSafeInteger(input.exitCode) || input.exitCode < 0))
      throw new CloudComputerCommandStoreError("invalid", "terminal exit code is invalid");
    if (input.status === "completed" && input.exitCode !== 0)
      throw new CloudComputerCommandStoreError(
        "invalid",
        "completed terminal requires exit code 0",
      );
    if (input.outputDigest !== null) assertDigest(input.outputDigest, "terminal output digest");
    if (
      Buffer.byteLength(
        canonicalJson({
          reason: input.reason,
          exitCode: input.exitCode,
          outputDigest: input.outputDigest,
        }),
      ) > MAX_INLINE_EVENT_BYTES
    )
      throw new CloudComputerCommandStoreError("invalid", "terminal payload exceeds inline limit");
    if (
      !Number.isSafeInteger(input.maxRetainedEvents) ||
      input.maxRetainedEvents < 1 ||
      input.maxRetainedEvents > 10_000
    )
      throw new CloudComputerCommandStoreError("invalid", "terminal retention is invalid");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      const bindings: ReadonlyArray<{
        provider_execution_ref: string | null;
        next_command_sequence: string | number;
      }> = await tx`
        SELECT provider_execution_ref, next_command_sequence
        FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
      `;
      if (
        command.request_digest !== input.requestDigest ||
        bindings[0]?.provider_execution_ref !== input.providerExecutionRef ||
        Number(command.fence) !== input.expectedFence
      )
        throw new CloudComputerCommandStoreError("conflict", "terminal command binding differs");
      if (command.terminal_ref !== null) {
        const terminalEvents: ReadonlyArray<{
          event_digest: string;
          payload_json: unknown;
          artifact_refs_json: unknown;
        }> = await tx`
          SELECT event_digest, payload_json, artifact_refs_json
          FROM khala_sync_cloud_computer_command_events
          WHERE event_ref = ${input.terminalRef} AND command_ref = ${input.commandRef}
        `;
        if (
          command.status !== input.status ||
          command.terminal_ref !== input.terminalRef ||
          command.terminal_digest !== input.terminalDigest ||
          command.terminal_reason !== input.reason ||
          (command.terminal_exit_code == null ? null : Number(command.terminal_exit_code)) !==
            input.exitCode ||
          command.terminal_output_digest !== input.outputDigest ||
          terminalEvents[0]?.event_digest !== input.eventDigest ||
          canonicalJson(terminalEvents[0]?.payload_json) !==
            canonicalJson({
              exitCode: input.exitCode,
              outputDigest: input.outputDigest,
              reason: input.reason,
            }) ||
          command.provider_command_ref !== input.providerCommandRef ||
          Number(command.terminal_command_sequence) !== input.expectedCommandSequence ||
          canonicalJson(terminalEvents[0]?.artifact_refs_json) !== canonicalJson(input.artifactRefs)
        )
          throw new CloudComputerCommandStoreError("conflict", "terminal result differs");
        return commandFrom(command, true);
      }
      if (Number(bindings[0]?.next_command_sequence) !== input.expectedCommandSequence)
        throw new CloudComputerCommandStoreError("conflict", "terminal sequence differs");
      const terminalEvent = createCloudComputerCommandEvent({
        eventRef: input.terminalRef,
        commandRef: input.commandRef,
        requestDigest: input.requestDigest as `sha256:${string}`,
        providerExecutionRef: input.providerExecutionRef,
        sessionRef: input.sessionRef,
        runtimeRef: input.runtimeRef,
        runtimeGeneration: input.runtimeGeneration,
        sequence: input.expectedCommandSequence,
        fence: input.expectedFence,
        kind: input.status,
        payload: {
          exitCode: input.exitCode,
          outputDigest: input.outputDigest,
          reason: input.reason,
        },
        observedAt: input.observedAt,
      });
      if (terminalEvent.eventDigest !== input.eventDigest)
        throw new CloudComputerCommandStoreError("conflict", "terminal event digest differs");
      const evidenceBytes = {
        schema: CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA,
        terminalRef: input.terminalRef,
        commandRef: input.commandRef,
        requestDigest: input.requestDigest,
        providerExecutionRef: input.providerExecutionRef,
        sessionRef: input.sessionRef,
        runtimeRef: input.runtimeRef,
        runtimeGeneration: input.runtimeGeneration,
        fence: input.expectedFence,
        sequence: input.expectedCommandSequence,
        outcome: input.status,
        exitCode: input.exitCode,
        outputDigest: input.outputDigest,
        reason: input.reason,
        observedAt: input.observedAt,
        eventDigest: input.eventDigest,
      };
      const evidenceDigest = `sha256:${createHash("sha256")
        .update(canonicalJson(evidenceBytes))
        .digest("hex")}`;
      if (evidenceDigest !== input.terminalDigest)
        throw new CloudComputerCommandStoreError("conflict", "terminal evidence digest differs");
      const allowed = ["running"];
      if (input.status === "cancelled" || input.status === "timed_out") {
        const kind = input.status === "cancelled" ? "cancel" : "timeout";
        const settlements: ReadonlyArray<{ settlement_ref: string }> = await tx`
          SELECT settlement_ref FROM khala_sync_cloud_computer_command_settlements
          WHERE command_ref = ${input.commandRef} AND kind = ${kind}
            AND settled_fence = ${command.fence}
            AND reason = ${input.reason}
        `;
        if (settlements.length !== 1)
          throw new CloudComputerCommandStoreError(
            "conflict",
            "terminal lacks settlement authority",
          );
      } else if (input.status === "completed" || input.status === "failed") {
        const settlements: ReadonlyArray<{ settlement_ref: string }> = await tx`
          SELECT settlement_ref FROM khala_sync_cloud_computer_command_settlements
          WHERE command_ref = ${input.commandRef}
        `;
        const deadlines: ReadonlyArray<{ allowed: boolean }> = await tx`
          SELECT (${input.observedAt} <= deadline_at) AS allowed
          FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
        `;
        if (settlements.length !== 0 || deadlines[0]?.allowed !== true)
          throw new CloudComputerCommandStoreError("conflict", "terminal authority was superseded");
      }
      if (input.artifactRefs.length > 0) {
        const artifacts: ReadonlyArray<{ artifact_ref: string }> = await tx`
          SELECT artifact_ref FROM khala_sync_cloud_computer_command_artifacts
          WHERE command_ref = ${input.commandRef}
            AND artifact_ref = ANY(${input.artifactRefs}::text[])
        `;
        if (artifacts.length !== new Set(input.artifactRefs).size)
          throw new CloudComputerCommandStoreError("conflict", "terminal artifact is unavailable");
      }
      const sessionCounters: ReadonlyArray<{ session_sequence: string | number }> = await tx`
        UPDATE khala_sync_cloud_computer_command_sessions
        SET last_session_sequence = last_session_sequence + 1,
            updated_at = ${input.observedAt}
        WHERE session_ref = ${input.sessionRef}
          AND attachment_epoch = ${input.attachmentEpoch}
        RETURNING last_session_sequence AS session_sequence
      `;
      const commandCounters: ReadonlyArray<{ command_sequence: string | number }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET next_command_sequence = next_command_sequence + 1,
            updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef}
          AND next_command_sequence = ${input.expectedCommandSequence}
        RETURNING next_command_sequence - 1 AS command_sequence
      `;
      if (sessionCounters[0] === undefined || commandCounters[0] === undefined)
        throw new CloudComputerCommandStoreError(
          "stale_generation",
          "terminal sequence fence differs",
        );
      const sessionSequence = Number(sessionCounters[0].session_sequence);
      const commandSequence = Number(commandCounters[0].command_sequence);
      await tx`
        INSERT INTO khala_sync_cloud_computer_command_events
          (command_ref, session_ref, session_sequence, command_sequence,
           attachment_epoch, fence, event_ref, event_digest, kind,
           payload_json, artifact_refs_json, observed_at)
        VALUES (${input.commandRef}, ${input.sessionRef}, ${sessionSequence}, ${commandSequence},
                ${input.attachmentEpoch}, ${command.fence}, ${input.terminalRef},
                ${input.eventDigest}, ${input.status},
                ${{
                  reason: input.reason,
                  exitCode: input.exitCode,
                  outputDigest: input.outputDigest,
                }}::jsonb,
                ${input.artifactRefs}::jsonb, ${input.observedAt})
      `;
      const rows: ReadonlyArray<CommandRow> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = ${input.status}, provider_command_ref = ${input.providerCommandRef},
            terminal_ref = ${input.terminalRef}, terminal_digest = ${input.terminalDigest},
            terminal_reason = ${input.reason}, terminal_exit_code = ${input.exitCode},
            terminal_output_digest = ${input.outputDigest},
            terminal_session_sequence = ${sessionSequence},
            terminal_command_sequence = ${commandSequence}, completed_at = ${input.observedAt},
            updated_at = ${input.observedAt}, revision = revision + 1
        WHERE command_ref = ${input.commandRef} AND status = ANY(${allowed}::text[])
          AND (provider_command_ref IS NULL OR provider_command_ref = ${input.providerCommandRef})
        RETURNING command_ref, request_digest, computer_ref, workspace_ref, session_ref,
                  runtime_generation, runtime_ref, provider_lease_ref, status,
                  provider_command_ref, dispatch_ref, terminal_ref, terminal_digest,
                  terminal_reason, terminal_exit_code, terminal_output_digest,
                  terminal_session_sequence, terminal_command_sequence, fence
      `;
      if (rows[0] === undefined)
        throw new CloudComputerCommandStoreError("conflict", "terminal transition differs");
      const removed: ReadonlyArray<{ session_sequence: string | number }> = await tx`
        DELETE FROM khala_sync_cloud_computer_command_events
        WHERE session_ref = ${input.sessionRef}
          AND session_sequence <= ${sessionSequence - input.maxRetainedEvents}
          AND NOT EXISTS (
            SELECT 1 FROM khala_sync_cloud_computer_commands AS terminal_command
            WHERE terminal_command.terminal_ref =
                  khala_sync_cloud_computer_command_events.event_ref
          )
        RETURNING session_sequence
      `;
      if (removed.length > 0) {
        const watermark = Math.max(...removed.map((event) => Number(event.session_sequence)));
        await tx`
          UPDATE khala_sync_cloud_computer_command_sessions
          SET retained_through_session_sequence =
                GREATEST(retained_through_session_sequence, ${watermark}),
              retention_epoch = retention_epoch + 1, updated_at = ${input.observedAt}
          WHERE session_ref = ${input.sessionRef}
        `;
      }
      return commandFrom(rows[0]);
    });
  }

  async appendEvent(
    input: Fence &
      Readonly<{
        eventRef: string;
        eventDigest: string;
        requestDigest: string;
        providerExecutionRef: string;
        providerCommandRef: string;
        expectedFence: number;
        expectedCommandSequence: number;
        kind: "stdout" | "stderr" | "progress" | "tool" | "lifecycle" | "checkpoint";
        payload: Readonly<Record<string, unknown>>;
        artifactRefs: ReadonlyArray<string>;
        maxRetainedEvents: number;
      }>,
  ): Promise<Readonly<{ sessionSequence: number; commandSequence: number }>> {
    this.validateFence(input);
    assertRef(input.eventRef, "event ref");
    assertDigest(input.eventDigest, "event digest");
    assertDigest(input.requestDigest, "request digest");
    assertRef(input.providerExecutionRef, "provider execution ref");
    assertRef(input.providerCommandRef, "provider command ref");
    input.artifactRefs.forEach((ref) => assertRef(ref, "artifact ref"));
    if (
      input.kind.length === 0 ||
      input.kind.length > 128 ||
      !Number.isSafeInteger(input.maxRetainedEvents) ||
      input.maxRetainedEvents < 1 ||
      input.maxRetainedEvents > 10_000
    )
      throw new CloudComputerCommandStoreError("invalid", "event bounds are invalid");
    if (Buffer.byteLength(canonicalJson(input.payload)) > MAX_INLINE_EVENT_BYTES)
      throw new CloudComputerCommandStoreError("invalid", "event payload exceeds inline limit");
    return this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      if (command.terminal_ref !== null)
        throw new CloudComputerCommandStoreError("conflict", "command is terminal");
      if (
        command.request_digest !== input.requestDigest ||
        command.provider_command_ref !== input.providerCommandRef ||
        Number(command.fence) !== input.expectedFence ||
        !Number.isSafeInteger(input.expectedCommandSequence) ||
        input.expectedCommandSequence < 1
      )
        throw new CloudComputerCommandStoreError("conflict", "event command binding differs");
      const deadlines: ReadonlyArray<{ allowed: boolean }> = await tx`
        SELECT (${input.observedAt} <= deadline_at) AS allowed
        FROM khala_sync_cloud_computer_commands WHERE command_ref = ${input.commandRef}
      `;
      if (deadlines[0]?.allowed !== true)
        throw new CloudComputerCommandStoreError("conflict", "event exceeded deadline");
      const prior: ReadonlyArray<{
        command_ref: string;
        session_sequence: string | number;
        command_sequence: string | number;
        event_digest: string;
        fence: string | number;
        kind: string;
        payload_json: unknown;
        artifact_refs_json: unknown;
      }> = await tx`
        SELECT command_ref, session_sequence, command_sequence, event_digest,
               fence, kind, payload_json, artifact_refs_json
        FROM khala_sync_cloud_computer_command_events
        WHERE event_ref = ${input.eventRef} FOR UPDATE
      `;
      if (prior[0] !== undefined) {
        if (
          prior[0].command_ref !== input.commandRef ||
          prior[0].event_digest !== input.eventDigest ||
          Number(prior[0].command_sequence) !== input.expectedCommandSequence ||
          Number(prior[0].fence) !== input.expectedFence ||
          prior[0].kind !== input.kind ||
          canonicalJson(prior[0].payload_json) !== canonicalJson(input.payload) ||
          canonicalJson(prior[0].artifact_refs_json) !== canonicalJson(input.artifactRefs)
        )
          throw new CloudComputerCommandStoreError("conflict", "event bytes differ");
        return {
          sessionSequence: Number(prior[0].session_sequence),
          commandSequence: Number(prior[0].command_sequence),
        };
      }
      const execution: ReadonlyArray<{
        provider_execution_ref: string | null;
        next_command_sequence: string | number;
      }> = await tx`
        SELECT provider_execution_ref, next_command_sequence
        FROM khala_sync_cloud_computer_commands
        WHERE command_ref = ${input.commandRef}
      `;
      if (
        execution[0]?.provider_execution_ref !== input.providerExecutionRef ||
        Number(execution[0]?.next_command_sequence) !== input.expectedCommandSequence
      )
        throw new CloudComputerCommandStoreError("conflict", "event sequence or execution differs");
      const runtimeEvent = createCloudComputerCommandEvent({
        eventRef: input.eventRef,
        commandRef: input.commandRef,
        requestDigest: input.requestDigest as `sha256:${string}`,
        providerExecutionRef: input.providerExecutionRef,
        sessionRef: input.sessionRef,
        runtimeRef: input.runtimeRef,
        runtimeGeneration: input.runtimeGeneration,
        sequence: input.expectedCommandSequence,
        fence: input.expectedFence,
        kind: input.kind,
        payload: input.payload,
        observedAt: input.observedAt,
      });
      if (runtimeEvent.eventDigest !== input.eventDigest)
        throw new CloudComputerCommandStoreError("conflict", "event digest differs");
      if (input.artifactRefs.length > 0) {
        const artifacts: ReadonlyArray<{ artifact_ref: string }> = await tx`
          SELECT artifact_ref FROM khala_sync_cloud_computer_command_artifacts
          WHERE command_ref = ${input.commandRef}
            AND artifact_ref = ANY(${input.artifactRefs}::text[])
        `;
        if (artifacts.length !== new Set(input.artifactRefs).size)
          throw new CloudComputerCommandStoreError("conflict", "event artifact is unavailable");
      }
      const sessionCounters: ReadonlyArray<{ session_sequence: string | number }> = await tx`
        UPDATE khala_sync_cloud_computer_command_sessions
        SET last_session_sequence = last_session_sequence + 1,
            updated_at = ${input.observedAt}
        WHERE session_ref = ${input.sessionRef}
          AND attachment_epoch = ${input.attachmentEpoch}
        RETURNING last_session_sequence AS session_sequence
      `;
      if (sessionCounters[0] === undefined)
        throw new CloudComputerCommandStoreError("stale_generation", "attachment epoch differs");
      const commandCounters: ReadonlyArray<{ command_sequence: string | number }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET next_command_sequence = next_command_sequence + 1, revision = revision + 1,
            updated_at = ${input.observedAt}
        WHERE command_ref = ${input.commandRef}
          AND next_command_sequence = ${input.expectedCommandSequence}
        RETURNING next_command_sequence - 1 AS command_sequence
      `;
      const sessionSequence = Number(sessionCounters[0].session_sequence);
      const commandSequence = Number(commandCounters[0]!.command_sequence);
      await tx`
        INSERT INTO khala_sync_cloud_computer_command_events
          (command_ref, session_ref, session_sequence, command_sequence,
           attachment_epoch, fence, event_ref,
           event_digest, kind, payload_json, artifact_refs_json, observed_at)
        VALUES (${input.commandRef}, ${input.sessionRef}, ${sessionSequence}, ${commandSequence},
                ${input.attachmentEpoch},
                ${input.expectedFence},
                ${input.eventRef}, ${input.eventDigest}, ${input.kind},
                ${input.payload}::jsonb, ${input.artifactRefs}::jsonb, ${input.observedAt})
      `;
      const removed: ReadonlyArray<{ session_sequence: string | number }> = await tx`
        DELETE FROM khala_sync_cloud_computer_command_events
        WHERE session_ref = ${input.sessionRef}
          AND session_sequence <= ${sessionSequence - input.maxRetainedEvents}
          AND NOT EXISTS (
            SELECT 1 FROM khala_sync_cloud_computer_commands AS terminal_command
            WHERE terminal_command.terminal_ref =
                  khala_sync_cloud_computer_command_events.event_ref
          )
        RETURNING session_sequence
      `;
      if (removed.length > 0) {
        const watermark = Math.max(...removed.map((event) => Number(event.session_sequence)));
        await tx`
          UPDATE khala_sync_cloud_computer_command_sessions
          SET retained_through_session_sequence =
                GREATEST(retained_through_session_sequence, ${watermark}),
              retention_epoch = retention_epoch + 1, updated_at = ${input.observedAt}
          WHERE session_ref = ${input.sessionRef}
        `;
      }
      return { sessionSequence, commandSequence };
    });
  }

  async recordArtifact(
    input: Fence &
      Readonly<{
        kind: "stdout" | "stderr" | "result" | "diagnostic";
        artifact: VerifiedCloudComputerCommandArtifact;
        requestDigest: string;
        providerExecutionRef: string;
        providerCommandRef: string;
        expectedFence: number;
      }>,
  ): Promise<void> {
    this.validateFence(input);
    assertVerifiedCloudComputerCommandArtifact(input.artifact);
    assertRef(input.artifact.artifactRef, "artifact ref");
    assertRef(input.artifact.object.objectRef, "object ref");
    assertDigest(input.artifact.contentDigest, "content digest");
    assertDigest(input.requestDigest, "request digest");
    assertRef(input.providerExecutionRef, "provider execution ref");
    assertRef(input.providerCommandRef, "provider command ref");
    if (
      !Number.isFinite(Date.parse(input.artifact.retainUntil)) ||
      Date.parse(input.artifact.retainUntil) < Date.parse(input.observedAt)
    )
      throw new CloudComputerCommandStoreError("invalid", "artifact retention is invalid");
    await this.serializable(async (tx) => {
      await this.lock(tx, input.commandRef);
      const command = await this.fencedCommand(tx, input);
      const execution: ReadonlyArray<{ provider_execution_ref: string | null }> = await tx`
        SELECT provider_execution_ref FROM khala_sync_cloud_computer_commands
        WHERE command_ref = ${input.commandRef}
      `;
      if (
        command.status !== "running" ||
        command.request_digest !== input.requestDigest ||
        command.provider_command_ref !== input.providerCommandRef ||
        Number(command.fence) !== input.expectedFence ||
        execution[0]?.provider_execution_ref !== input.providerExecutionRef
      )
        throw new CloudComputerCommandStoreError("conflict", "artifact command binding differs");
      const budgets: ReadonlyArray<{
        output_limit: string | number;
        prior_bytes: string | number;
        owner_ref: string;
        tenant_ref: string;
      }> = await tx`
        SELECT (command.budget_limits_json->>'outputBytes')::bigint AS output_limit,
               command.owner_ref, command.tenant_ref,
               COALESCE(SUM(artifact.byte_count) FILTER (
                 WHERE artifact.artifact_ref <> ${input.artifact.artifactRef}
               ), 0) AS prior_bytes
        FROM khala_sync_cloud_computer_commands AS command
        LEFT JOIN khala_sync_cloud_computer_command_artifacts AS artifact
          ON artifact.command_ref = command.command_ref
        WHERE command.command_ref = ${input.commandRef}
        GROUP BY command.budget_limits_json, command.owner_ref, command.tenant_ref
      `;
      if (
        budgets[0] === undefined ||
        Number(budgets[0].prior_bytes) + input.artifact.byteCount > Number(budgets[0].output_limit)
      )
        throw new CloudComputerCommandStoreError("conflict", "artifact output budget exceeded");
      const expectedObjectRef = cloudComputerCommandArtifactRef({
        ownerRef: budgets[0].owner_ref,
        tenantRef: budgets[0].tenant_ref,
        commandRef: input.commandRef,
        runtimeGeneration: input.runtimeGeneration,
        kind: input.kind,
        contentDigest: input.artifact.contentDigest,
      });
      if (input.artifact.object.objectRef !== expectedObjectRef)
        throw new CloudComputerCommandStoreError("conflict", "artifact scope differs");
      const rows: ReadonlyArray<{ artifact_ref: string }> = await tx`
        INSERT INTO khala_sync_cloud_computer_command_artifacts
          (artifact_ref, command_ref, runtime_generation, kind, object_ref, object_generation,
           content_digest, byte_count, created_at, retain_until)
        VALUES (${input.artifact.artifactRef}, ${input.commandRef}, ${input.runtimeGeneration},
                ${input.kind}, ${input.artifact.object.objectRef},
                ${input.artifact.object.generation}, ${input.artifact.contentDigest},
                ${input.artifact.byteCount}, ${input.observedAt}, ${input.artifact.retainUntil})
        ON CONFLICT (artifact_ref) DO UPDATE SET artifact_ref = EXCLUDED.artifact_ref
        WHERE khala_sync_cloud_computer_command_artifacts.command_ref = EXCLUDED.command_ref
          AND khala_sync_cloud_computer_command_artifacts.runtime_generation = EXCLUDED.runtime_generation
          AND khala_sync_cloud_computer_command_artifacts.kind = EXCLUDED.kind
          AND khala_sync_cloud_computer_command_artifacts.object_ref = EXCLUDED.object_ref
          AND khala_sync_cloud_computer_command_artifacts.object_generation = EXCLUDED.object_generation
          AND khala_sync_cloud_computer_command_artifacts.content_digest = EXCLUDED.content_digest
          AND khala_sync_cloud_computer_command_artifacts.byte_count = EXCLUDED.byte_count
          AND khala_sync_cloud_computer_command_artifacts.retain_until = EXCLUDED.retain_until
        RETURNING artifact_ref
      `;
      if (rows.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "artifact bytes differ");
    });
  }

  async recordRecoveryEvidence(
    evidence: CloudComputerCommandRecoveryEvidence,
  ): Promise<Readonly<{ lost: number; replayed: boolean }>> {
    assertCloudComputerCommandRecoveryEvidence(evidence);
    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`openagents.cloud-computer.lifecycle|${evidence.computerRef}`}, 3106))`;
      const prior: ReadonlyArray<{
        evidence_ref: string;
        kind: string;
        evidence_digest: string;
        computer_ref: string;
        workspace_ref: string;
        runtime_generation: string | number;
        runtime_ref: string;
        provider_lease_ref: string;
        observed_at: string | Date;
        affected_command_count: string | number;
      }> = await tx`
        SELECT evidence_ref, kind, evidence_digest, computer_ref, workspace_ref, runtime_generation,
               runtime_ref, provider_lease_ref, observed_at, affected_command_count
        FROM khala_sync_cloud_computer_command_recovery_evidence
        WHERE evidence_ref = ${evidence.evidenceRef}
           OR (kind = ${evidence.kind} AND computer_ref = ${evidence.computerRef}
               AND workspace_ref = ${evidence.workspaceRef}
               AND runtime_generation = ${evidence.runtimeGeneration}
               AND runtime_ref = ${evidence.runtimeRef}
               AND provider_lease_ref = ${evidence.providerLeaseRef}
               AND evidence_digest = ${evidence.evidenceDigest})
        FOR UPDATE
      `;
      if (prior[0] !== undefined) {
        const row = prior[0];
        if (
          row.evidence_ref !== evidence.evidenceRef ||
          row.kind !== evidence.kind ||
          row.evidence_digest !== evidence.evidenceDigest ||
          row.computer_ref !== evidence.computerRef ||
          row.workspace_ref !== evidence.workspaceRef ||
          Number(row.runtime_generation) !== evidence.runtimeGeneration ||
          row.runtime_ref !== evidence.runtimeRef ||
          row.provider_lease_ref !== evidence.providerLeaseRef ||
          new Date(row.observed_at).toISOString() !== new Date(evidence.observedAt).toISOString()
        )
          throw new CloudComputerCommandStoreError("conflict", "recovery evidence differs");
        return { lost: Number(row.affected_command_count), replayed: true };
      }
      await tx`
        INSERT INTO khala_sync_cloud_computer_command_recovery_evidence
          (evidence_ref, kind, evidence_digest, computer_ref, workspace_ref,
           runtime_generation, runtime_ref, provider_lease_ref, observed_at)
        VALUES (${evidence.evidenceRef}, ${evidence.kind}, ${evidence.evidenceDigest},
                ${evidence.computerRef}, ${evidence.workspaceRef},
                ${evidence.runtimeGeneration}, ${evidence.runtimeRef},
                ${evidence.providerLeaseRef}, ${evidence.observedAt})
      `;
      if (evidence.kind !== "runtime_lost") return { lost: 0, replayed: false };
      const lost: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'lost', terminal_ref = ${evidence.evidenceRef} || '.' || command_ref,
            terminal_digest = ${evidence.evidenceDigest}, terminal_reason = 'runtime_lost',
            completed_at = ${evidence.observedAt}, updated_at = ${evidence.observedAt},
            revision = revision + 1
        WHERE computer_ref = ${evidence.computerRef}
          AND workspace_ref = ${evidence.workspaceRef}
          AND runtime_generation = ${evidence.runtimeGeneration}
          AND runtime_ref = ${evidence.runtimeRef}
          AND provider_lease_ref = ${evidence.providerLeaseRef}
          AND status IN ('dispatched', 'may_have_started', 'running')
        RETURNING command_ref
      `;
      await tx`
        UPDATE khala_sync_cloud_computer_command_recovery_evidence
        SET affected_command_count = ${lost.length}
        WHERE evidence_ref = ${evidence.evidenceRef}
      `;
      return { lost: lost.length, replayed: false };
    });
  }

  async settleHostLoss(
    input: Readonly<{
      evidenceRef: string;
      evidenceDigest: string;
      computerRef: string;
      workspaceRef: string;
      runtimeGeneration: number;
      runtimeRef: string;
      providerLeaseRef: string;
      observedAt: string;
    }>,
  ): Promise<Readonly<{ lost: number; notDispatched: number }>> {
    for (const [field, value] of Object.entries({
      evidenceRef: input.evidenceRef,
      computerRef: input.computerRef,
      workspaceRef: input.workspaceRef,
      runtimeRef: input.runtimeRef,
      providerLeaseRef: input.providerLeaseRef,
    }))
      assertRef(value, field);
    assertDigest(input.evidenceDigest, "evidence digest");
    assertGeneration(input.runtimeGeneration);
    assertTimestamp(input.observedAt);
    return this.serializable(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`openagents.cloud-computer.lifecycle|${input.computerRef}`}, 3106))`;
      const evidence: ReadonlyArray<{ evidence_ref: string }> = await tx`
        SELECT evidence_ref FROM khala_sync_cloud_computer_host_loss_evidence
        WHERE evidence_ref = ${input.evidenceRef} AND evidence_digest = ${input.evidenceDigest}
          AND computer_ref = ${input.computerRef} AND workspace_ref = ${input.workspaceRef}
          AND runtime_generation = ${input.runtimeGeneration}
          AND provider_lease_ref = ${input.providerLeaseRef}
        FOR UPDATE
      `;
      if (evidence.length !== 1)
        throw new CloudComputerCommandStoreError("conflict", "host-loss evidence differs");
      const unexposed: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_command_dispatch_attempts AS attempt
        SET status = 'not_exposed', settled_at = ${input.observedAt},
            revision = attempt.revision + 1
        FROM khala_sync_cloud_computer_commands AS command
        WHERE command.computer_ref = ${input.computerRef}
          AND command.workspace_ref = ${input.workspaceRef}
          AND command.runtime_generation = ${input.runtimeGeneration}
          AND command.runtime_ref = ${input.runtimeRef}
          AND command.provider_lease_ref = ${input.providerLeaseRef}
          AND attempt.command_ref = command.command_ref AND attempt.status = 'prepared'
        RETURNING attempt.command_ref
      `;
      const safe: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'not_dispatched', updated_at = ${input.observedAt}, revision = revision + 1
        WHERE computer_ref = ${input.computerRef} AND workspace_ref = ${input.workspaceRef}
          AND runtime_generation = ${input.runtimeGeneration} AND runtime_ref = ${input.runtimeRef}
          AND provider_lease_ref = ${input.providerLeaseRef}
          AND status IN ('admitted', 'not_dispatched')
        RETURNING command_ref
      `;
      await tx`
        UPDATE khala_sync_cloud_computer_command_dispatch_attempts AS attempt
        SET status = 'may_have_started', settled_at = ${input.observedAt},
            revision = attempt.revision + 1
        FROM khala_sync_cloud_computer_commands AS command
        WHERE command.computer_ref = ${input.computerRef}
          AND command.workspace_ref = ${input.workspaceRef}
          AND command.runtime_generation = ${input.runtimeGeneration}
          AND command.runtime_ref = ${input.runtimeRef}
          AND command.provider_lease_ref = ${input.providerLeaseRef}
          AND attempt.command_ref = command.command_ref AND attempt.status = 'write_exposed'
      `;
      const lost: ReadonlyArray<{ command_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computer_commands
        SET status = 'lost', terminal_ref = ${input.evidenceRef} || '.' || command_ref,
            terminal_digest = ${input.evidenceDigest}, terminal_reason = 'host_lost',
            completed_at = ${input.observedAt}, updated_at = ${input.observedAt},
            revision = revision + 1
        WHERE computer_ref = ${input.computerRef} AND workspace_ref = ${input.workspaceRef}
          AND runtime_generation = ${input.runtimeGeneration} AND runtime_ref = ${input.runtimeRef}
          AND provider_lease_ref = ${input.providerLeaseRef}
          AND status IN ('dispatched', 'may_have_started', 'running')
        RETURNING command_ref
      `;
      return {
        lost: lost.length,
        notDispatched: new Set([...safe, ...unexposed].map((row) => row.command_ref)).size,
      };
    });
  }

  async recoverUnsettled(limit = 100): Promise<ReadonlyArray<CloudComputerCommand>> {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1_000)
      throw new CloudComputerCommandStoreError("invalid", "recovery limit is invalid");
    const rows: ReadonlyArray<CommandRow> = await this.sql`
      SELECT command_ref, request_digest, computer_ref, workspace_ref, session_ref, runtime_generation,
             runtime_ref, provider_lease_ref, status, provider_command_ref, dispatch_ref,
             terminal_ref, terminal_digest, terminal_reason, terminal_exit_code,
             terminal_output_digest,
             terminal_session_sequence, terminal_command_sequence, fence
      FROM khala_sync_cloud_computer_commands
      WHERE status IN ('dispatched', 'may_have_started', 'running')
      ORDER BY updated_at, command_ref
      LIMIT ${limit}
    `;
    return rows.map((row) => commandFrom(row));
  }

  async recoverSafeToDispatch(limit = 100): Promise<ReadonlyArray<CloudComputerCommand>> {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1_000)
      throw new CloudComputerCommandStoreError("invalid", "recovery limit is invalid");
    const rows: ReadonlyArray<CommandRow> = await this.sql`
      SELECT command_ref, request_digest, computer_ref, workspace_ref, session_ref,
             runtime_generation, runtime_ref, provider_lease_ref, status,
             provider_command_ref,
             COALESCE(dispatch_ref, (
               SELECT attempt.attempt_ref
               FROM khala_sync_cloud_computer_command_dispatch_attempts AS attempt
               WHERE attempt.command_ref = khala_sync_cloud_computer_commands.command_ref
                 AND attempt.status = 'prepared'
               ORDER BY attempt.prepared_at DESC, attempt.attempt_ref DESC LIMIT 1
             )) AS dispatch_ref,
             terminal_ref, terminal_digest,
             terminal_reason, terminal_session_sequence, terminal_command_sequence, fence
      FROM khala_sync_cloud_computer_commands
      WHERE status IN ('admitted', 'not_dispatched')
        AND NOT EXISTS (
          SELECT 1 FROM khala_sync_cloud_computer_command_dispatch_attempts AS attempt
          WHERE attempt.command_ref = khala_sync_cloud_computer_commands.command_ref
            AND attempt.status IN ('write_exposed', 'may_have_started', 'acknowledged')
        )
      ORDER BY updated_at, command_ref
      LIMIT ${limit}
    `;
    return rows.map((row) => commandFrom(row));
  }

  async getDispatchAttempt(commandRef: string): Promise<Readonly<{
    dispatchRef: string;
    status:
      | "prepared"
      | "not_exposed"
      | "write_exposed"
      | "may_have_started"
      | "reservation_recorded"
      | "acknowledged";
    runtimeGeneration: number;
    runtimeRef: string;
    providerLeaseRef: string;
    reservationRef: string | null;
    providerExecutionRef: string | null;
    providerCommandRef: string | null;
    acknowledgementEventRef: string | null;
    acknowledgementEventDigest: string | null;
  }> | null> {
    assertRef(commandRef, "command ref");
    const rows: ReadonlyArray<{
      attempt_ref: string;
      status:
        | "prepared"
        | "not_exposed"
        | "write_exposed"
        | "may_have_started"
        | "reservation_recorded"
        | "acknowledged";
      runtime_generation: string | number;
      runtime_ref: string;
      provider_lease_ref: string;
      reservation_ref: string | null;
      provider_execution_ref: string | null;
      provider_command_ref: string | null;
      acknowledgement_event_ref: string | null;
      acknowledgement_event_digest: string | null;
    }> = await this.sql`
      SELECT attempt_ref, status, runtime_generation, runtime_ref, provider_lease_ref,
             reservation_ref, provider_execution_ref, provider_command_ref,
             acknowledgement_event_ref, acknowledgement_event_digest
      FROM khala_sync_cloud_computer_command_dispatch_attempts
      WHERE command_ref = ${commandRef}
      ORDER BY prepared_at DESC, attempt_ref DESC
      LIMIT 1
    `;
    const row = rows[0];
    return row === undefined
      ? null
      : {
          dispatchRef: row.attempt_ref,
          status: row.status,
          runtimeGeneration: Number(row.runtime_generation),
          runtimeRef: row.runtime_ref,
          providerLeaseRef: row.provider_lease_ref,
          reservationRef: row.reservation_ref,
          providerExecutionRef: row.provider_execution_ref,
          providerCommandRef: row.provider_command_ref,
          acknowledgementEventRef: row.acknowledgement_event_ref,
          acknowledgementEventDigest: row.acknowledgement_event_digest,
        };
  }

  async loadCommandForDispatch(commandRef: string): Promise<Readonly<{
    command: CoreCloudComputerCommand;
    requestDigest: string;
    dispatchRef: string | null;
  }> | null> {
    assertRef(commandRef, "command ref");
    const rows: ReadonlyArray<{
      request_json: CoreCloudComputerCommand;
      request_digest: string;
      dispatch_ref: string | null;
      prepared_dispatch_ref: string | null;
    }> = await this.sql`
      SELECT command.request_json, command.request_digest, command.dispatch_ref,
             (
               SELECT attempt.attempt_ref
               FROM khala_sync_cloud_computer_command_dispatch_attempts AS attempt
               WHERE attempt.command_ref = command.command_ref AND attempt.status = 'prepared'
               ORDER BY attempt.prepared_at DESC, attempt.attempt_ref DESC LIMIT 1
             ) AS prepared_dispatch_ref
      FROM khala_sync_cloud_computer_commands AS command
      JOIN khala_sync_cloud_computers AS computer
        ON computer.computer_ref = command.computer_ref
      JOIN khala_sync_cloud_computer_workspaces AS workspace
        ON workspace.workspace_ref = command.workspace_ref
      WHERE command.command_ref = ${commandRef}
        AND command.status IN ('admitted', 'not_dispatched')
        AND computer.generation = command.runtime_generation
        AND computer.active_lease_ref = command.provider_lease_ref
        AND computer.state = 'active'
        AND workspace.runtime_generation = command.runtime_generation
        AND workspace.state = 'active'
    `;
    const row = rows[0];
    if (row === undefined) return null;
    assertCloudComputerCommand(row.request_json);
    const reconstructed = createCloudComputerCommand(row.request_json);
    if (
      reconstructed.requestDigest !== row.request_digest ||
      row.request_json.requestDigest !== row.request_digest
    )
      throw new CloudComputerCommandStoreError("conflict", "persisted request digest differs");
    return {
      command: reconstructed,
      requestDigest: row.request_digest,
      dispatchRef: row.dispatch_ref ?? row.prepared_dispatch_ref,
    };
  }

  async readEvents(
    input: Readonly<{
      sessionRef: string;
      attachmentEpoch: number;
      afterSessionSequence: number;
      limit: number;
    }>,
  ): Promise<
    Readonly<{
      retainedThroughSessionSequence: number;
      retentionEpoch: number;
      events: ReadonlyArray<
        Readonly<{
          commandRef: string;
          sessionSequence: number;
          commandSequence: number;
          eventRef: string;
          eventDigest: string;
          kind: string;
          payload: unknown;
          artifactRefs: unknown;
        }>
      >;
    }>
  > {
    assertRef(input.sessionRef, "session ref");
    if (
      !Number.isSafeInteger(input.attachmentEpoch) ||
      input.attachmentEpoch < 1 ||
      !Number.isSafeInteger(input.afterSessionSequence) ||
      input.afterSessionSequence < 0 ||
      !Number.isSafeInteger(input.limit) ||
      input.limit < 1 ||
      input.limit > 1_000
    )
      throw new CloudComputerCommandStoreError("invalid", "event cursor is invalid");
    const sessions: ReadonlyArray<{
      retained_through_session_sequence: string | number;
      last_session_sequence: string | number;
      retention_epoch: string | number;
    }> = await this.sql`
      SELECT retained_through_session_sequence, last_session_sequence, retention_epoch
      FROM khala_sync_cloud_computer_command_sessions
      WHERE session_ref = ${input.sessionRef} AND attachment_epoch = ${input.attachmentEpoch}
    `;
    const session = sessions[0];
    if (session === undefined)
      throw new CloudComputerCommandStoreError("stale_generation", "attachment epoch differs");
    if (input.afterSessionSequence < Number(session.retained_through_session_sequence))
      throw new CloudComputerCommandStoreError("cursor_expired", "event cursor was compacted");
    if (input.afterSessionSequence > Number(session.last_session_sequence))
      throw new CloudComputerCommandStoreError("conflict", "event cursor is ahead");
    const rows: ReadonlyArray<{
      command_ref: string;
      session_sequence: string | number;
      command_sequence: string | number;
      event_ref: string;
      event_digest: string;
      kind: string;
      payload_json: unknown;
      artifact_refs_json: unknown;
    }> = await this.sql`
      SELECT command_ref, session_sequence, command_sequence, event_ref, event_digest,
             kind, payload_json, artifact_refs_json
      FROM khala_sync_cloud_computer_command_events
      WHERE session_ref = ${input.sessionRef}
        AND session_sequence > ${input.afterSessionSequence}
      ORDER BY session_sequence
      LIMIT ${input.limit}
    `;
    return {
      retainedThroughSessionSequence: Number(session.retained_through_session_sequence),
      retentionEpoch: Number(session.retention_epoch),
      events: rows.map((row) => ({
        commandRef: row.command_ref,
        sessionSequence: Number(row.session_sequence),
        commandSequence: Number(row.command_sequence),
        eventRef: row.event_ref,
        eventDigest: row.event_digest,
        kind: row.kind,
        payload: row.payload_json,
        artifactRefs: row.artifact_refs_json,
      })),
    };
  }

  async get(commandRef: string): Promise<CloudComputerCommand> {
    assertRef(commandRef, "command ref");
    const rows: ReadonlyArray<CommandRow> = await this.sql`
      SELECT command_ref, request_digest, computer_ref, workspace_ref, session_ref, runtime_generation,
             runtime_ref, provider_lease_ref, status, provider_command_ref, dispatch_ref,
             terminal_ref, terminal_digest, terminal_reason, terminal_exit_code,
             terminal_output_digest,
             terminal_session_sequence, terminal_command_sequence, fence
      FROM khala_sync_cloud_computer_commands WHERE command_ref = ${commandRef}
    `;
    if (rows[0] === undefined)
      throw new CloudComputerCommandStoreError("not_found", "command missing");
    return commandFrom(rows[0]);
  }
}

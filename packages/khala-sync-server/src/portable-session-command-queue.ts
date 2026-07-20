import { createHash } from "node:crypto";

import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  PortableCommandExecutionClaimRequestSchema,
  PortableCommandExecutionClaimSchema,
  PortableCommandExecutionPendingReconcileRequestSchema,
  PortableCommandExecutionRenewRequestSchema,
  PortableCommandExecutionTerminalRequestSchema,
  PortableSessionCommandSchema,
  type PortableCommandExecutionClaim,
  type PortableCommandExecutionClaimResult,
  type PortableCommandExecutionPendingReconcileResult,
  type PortableCommandExecutionRenewResult,
  type PortableCommandExecutionTerminalResult,
  type PortableSessionCommand,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import type { SyncSql, SyncTransactionSql } from "./sql.js";

type CommandRow = {
  command_ref: string;
  owner_user_id: string;
  session_ref: string;
  kind: string;
  expected_attachment_ref: string;
  expected_generation: string | number;
  destination_target_ref: string | null;
  expires_at: Date | string;
  command_json: unknown;
  status: string;
  current_attachment_ref: string | null;
  current_attachment_generation: string | number;
  source_target_ref: string | null;
  destination_health: string | null;
};

type ExecutionRow = {
  command_ref: string;
  claim_ref: string;
  owner_user_id: string;
  session_ref: string;
  command_kind: "attach" | "move" | "failback";
  command_fingerprint: string;
  claim_fingerprint: string;
  source_attachment_ref: string;
  source_generation: string | number;
  destination_target_ref: string;
  executor_environment_ref: string;
  worker_instance_ref: string;
  claim_generation: string | number;
  lease_revision: string | number;
  state: "claimed" | "pending_reconcile" | "terminal" | "expired";
  claimed_at: Date | string;
  lease_expires_at: Date | string;
  terminal_status: "completed" | "failed" | "rejected" | "expired" | null;
  pending_reconcile_ref: string | null;
  outcome_ref: string | null;
  evidence_refs_json: unknown;
  updated_at: Date | string;
};

export class PortableSessionCommandQueueError extends Error {
  readonly _tag = "PortableSessionCommandQueueError";
  override readonly name = "PortableSessionCommandQueueError";

  constructor(
    readonly code:
      | "invalid"
      | "not_found"
      | "not_claimable"
      | "claim_conflict"
      | "claim_expired"
      | "stale_generation"
      | "stale_revision"
      | "unsafe_material",
    message: string,
  ) {
    super(message);
  }
}

const decodeClaimRequest = Schema.decodeUnknownSync(PortableCommandExecutionClaimRequestSchema);
const decodeRenewRequest = Schema.decodeUnknownSync(PortableCommandExecutionRenewRequestSchema);
const decodePendingRequest = Schema.decodeUnknownSync(
  PortableCommandExecutionPendingReconcileRequestSchema,
);
const decodeTerminalRequest = Schema.decodeUnknownSync(
  PortableCommandExecutionTerminalRequestSchema,
);
const decodeCommand = Schema.decodeUnknownSync(PortableSessionCommandSchema);
const decodeClaim = Schema.decodeUnknownSync(PortableCommandExecutionClaimSchema);

const forbiddenPrivateMaterial =
  /"(?:token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)/i;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      // The array is new. Sorting it cannot mutate caller state.
      // eslint-disable-next-line unicorn/no-array-sort
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const assertPublicSafe = (value: unknown): void => {
  if (forbiddenPrivateMaterial.test(canonical(value))) {
    throw new PortableSessionCommandQueueError(
      "unsafe_material",
      "portable command execution contains forbidden private material",
    );
  }
};

const fingerprint = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`;

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;

const iso = (value: Date | string): string => new Date(value).toISOString();

const positive = (value: string | number, field: string): number => {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new PortableSessionCommandQueueError("invalid", `${field} is invalid`);
  }
  return number;
};

const executionFromRow = (row: ExecutionRow): PortableCommandExecutionClaim =>
  decodeClaim({
    schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
    claimRef: row.claim_ref,
    commandRef: row.command_ref,
    ownerRef: row.owner_user_id,
    sessionRef: row.session_ref,
    commandKind: row.command_kind,
    commandFingerprint: row.command_fingerprint,
    claimFingerprint: row.claim_fingerprint,
    sourceAttachmentRef: row.source_attachment_ref,
    sourceGeneration: positive(row.source_generation, "source generation"),
    destinationTargetRef: row.destination_target_ref,
    executorEnvironmentRef: row.executor_environment_ref,
    workerInstanceRef: row.worker_instance_ref,
    claimGeneration: positive(row.claim_generation, "claim generation"),
    leaseRevision: positive(row.lease_revision, "lease revision"),
    state: row.state,
    claimedAt: iso(row.claimed_at),
    leaseExpiresAt: iso(row.lease_expires_at),
    updatedAt: iso(row.updated_at),
    terminalStatus: row.terminal_status,
    pendingReconcileRef: row.pending_reconcile_ref,
    outcomeRef: row.outcome_ref,
    evidenceRefs: parseJson(row.evidence_refs_json),
  });

const selectExecution = async (
  sql: SyncTransactionSql,
  claimRef: string,
): Promise<ExecutionRow | undefined> => {
  const rows: ExecutionRow[] = await sql`
    SELECT * FROM khala_sync_portable_command_executions
    WHERE claim_ref = ${claimRef}
    FOR UPDATE
  `;
  return rows[0];
};

const sameInstant = (left: Date | string, right: string): boolean =>
  iso(left) === new Date(right).toISOString();

/**
 * Durable worker serialization for accepted portable movement commands.
 * The command and session rows remain authority. This queue only records which
 * exact worker can invoke the canonical runtime for that authority fact.
 */
export class PostgresPortableSessionCommandQueue {
  constructor(
    private readonly sql: SyncSql,
    private readonly now: () => string = () => new Date().toISOString(),
  ) {}

  async claim(input: unknown): Promise<PortableCommandExecutionClaimResult> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeClaimRequest>;
    try {
      request = decodeClaimRequest(input);
    } catch {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "portable command claim request is invalid",
      );
    }
    const now = new Date(this.now());
    const requestedExpiry = new Date(request.leaseExpiresAt);
    if (!(requestedExpiry > now)) {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "portable command claim lease is not in the future",
      );
    }

    return this.sql.begin(async (tx) => {
      const commands: CommandRow[] = await tx`
        SELECT c.command_ref, c.owner_user_id, c.session_ref, c.kind,
               c.expected_attachment_ref, c.expected_generation,
               c.destination_target_ref, c.expires_at, c.command_json, c.status,
               s.current_attachment_ref, s.current_attachment_generation,
               source.target_ref AS source_target_ref,
               destination.health AS destination_health
        FROM khala_sync_portable_commands c
        JOIN khala_sync_portable_sessions s ON s.session_ref = c.session_ref
        LEFT JOIN khala_sync_portable_attachments source
          ON source.attachment_ref = s.current_attachment_ref
        LEFT JOIN khala_sync_portable_session_targets authorized_destination
          ON authorized_destination.session_ref = c.session_ref
         AND authorized_destination.target_ref = c.destination_target_ref
        LEFT JOIN khala_sync_portable_targets destination
          ON destination.target_ref = authorized_destination.target_ref
        WHERE c.command_ref = ${request.commandRef}
        FOR UPDATE OF c
      `;
      const row = commands[0];
      if (!row)
        throw new PortableSessionCommandQueueError("not_found", "portable command does not exist");
      const rawCommand = parseJson(row.command_json);
      assertPublicSafe(rawCommand);
      let command: PortableSessionCommand;
      try {
        command = decodeCommand(rawCommand);
      } catch {
        throw new PortableSessionCommandQueueError("invalid", "stored portable command is invalid");
      }
      this.assertCommandBytes(row, command);

      const commandDigest = fingerprint(command);
      const claimDigest = fingerprint(request);
      const existingRows: ExecutionRow[] = await tx`
        SELECT * FROM khala_sync_portable_command_executions
        WHERE command_ref = ${request.commandRef}
        FOR UPDATE
      `;
      const existing = existingRows[0];
      if (existing) {
        if (
          existing.claim_fingerprint === claimDigest &&
          existing.command_fingerprint === commandDigest &&
          existing.claim_ref === request.claimRef &&
          existing.executor_environment_ref === request.executorEnvironmentRef &&
          existing.worker_instance_ref === request.workerInstanceRef &&
          existing.state !== "terminal" &&
          existing.state !== "expired" &&
          new Date(existing.lease_expires_at) > now
        ) {
          return { status: "replayed", claim: executionFromRow(existing) };
        }
        throw new PortableSessionCommandQueueError(
          existing.state === "expired" || new Date(existing.lease_expires_at) <= now
            ? "claim_expired"
            : "claim_conflict",
          "portable command already has a different execution claim",
        );
      }

      this.assertCommandClaimable(
        row,
        command,
        request.executorEnvironmentRef,
        now,
        requestedExpiry,
      );

      const inserted: ExecutionRow[] = await tx`
        INSERT INTO khala_sync_portable_command_executions
          (command_ref, claim_ref, owner_user_id, session_ref, command_kind,
           command_fingerprint, claim_fingerprint, source_attachment_ref,
           source_generation, destination_target_ref, executor_environment_ref,
           worker_instance_ref, claim_generation, lease_revision, state,
           claimed_at, lease_expires_at, updated_at)
        VALUES
          (${command.commandRef}, ${request.claimRef}, ${command.ownerRef},
           ${command.sessionRef}, ${command.kind}, ${commandDigest}, ${claimDigest},
           ${command.expectedAttachmentRef}, ${command.expectedGeneration},
           ${command.destinationTargetRef!}, ${request.executorEnvironmentRef},
           ${request.workerInstanceRef}, 1, 1, 'claimed', ${now.toISOString()},
           ${request.leaseExpiresAt}, ${now.toISOString()})
        RETURNING *
      `;
      return { status: "claimed", claim: executionFromRow(inserted[0]!) };
    });
  }

  async renew(input: unknown): Promise<PortableCommandExecutionRenewResult> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeRenewRequest>;
    try {
      request = decodeRenewRequest(input);
    } catch {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "portable command renew request is invalid",
      );
    }
    const now = new Date(this.now());
    return this.sql.begin(async (tx) => {
      const row = await selectExecution(tx, request.claimRef);
      if (!row)
        throw new PortableSessionCommandQueueError(
          "not_found",
          "portable execution claim does not exist",
        );
      this.assertClaimIdentity(row, request);
      const revision = positive(row.lease_revision, "lease revision");
      if (row.state === "expired" || new Date(row.lease_expires_at) <= now) {
        throw new PortableSessionCommandQueueError(
          "claim_expired",
          "portable execution claim expired",
        );
      }
      if (row.state === "terminal") {
        throw new PortableSessionCommandQueueError(
          "stale_revision",
          "portable execution claim is terminal",
        );
      }
      if (
        revision === request.expectedLeaseRevision + 1 &&
        sameInstant(row.lease_expires_at, request.leaseExpiresAt)
      ) {
        return { status: "replayed", claim: executionFromRow(row) };
      }
      if (revision !== request.expectedLeaseRevision) {
        throw new PortableSessionCommandQueueError(
          "stale_revision",
          "portable execution lease revision is stale",
        );
      }
      const nextExpiry = new Date(request.leaseExpiresAt);
      const commandExpiry: Array<{ expires_at: Date | string }> = await tx`
        SELECT expires_at FROM khala_sync_portable_commands WHERE command_ref = ${row.command_ref}
      `;
      if (
        !(nextExpiry > new Date(row.lease_expires_at)) ||
        !(nextExpiry <= new Date(commandExpiry[0]!.expires_at))
      ) {
        throw new PortableSessionCommandQueueError(
          "invalid",
          "portable execution renewal is outside command bounds",
        );
      }
      const updated: ExecutionRow[] = await tx`
        UPDATE khala_sync_portable_command_executions
        SET lease_revision = lease_revision + 1,
            lease_expires_at = ${request.leaseExpiresAt}, updated_at = ${now.toISOString()}
        WHERE claim_ref = ${request.claimRef} AND lease_revision = ${revision}
        RETURNING *
      `;
      return { status: "renewed", claim: executionFromRow(updated[0]!) };
    });
  }

  async terminal(input: unknown): Promise<PortableCommandExecutionTerminalResult> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodeTerminalRequest>;
    try {
      request = decodeTerminalRequest(input);
    } catch {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "portable command terminal request is invalid",
      );
    }
    const now = new Date(this.now());
    return this.sql.begin(async (tx) => {
      const row = await selectExecution(tx, request.claimRef);
      if (!row)
        throw new PortableSessionCommandQueueError(
          "not_found",
          "portable execution claim does not exist",
        );
      this.assertClaimIdentity(row, request);
      const revision = positive(row.lease_revision, "lease revision");
      const sameTerminal =
        row.state === "terminal" &&
        revision === request.expectedLeaseRevision + 1 &&
        row.terminal_status === request.terminalStatus &&
        row.outcome_ref === request.outcomeRef &&
        canonical(parseJson(row.evidence_refs_json)) === canonical(request.evidenceRefs);
      if (sameTerminal) return { status: "replayed", claim: executionFromRow(row) };
      if (row.state === "expired" || new Date(row.lease_expires_at) <= now) {
        throw new PortableSessionCommandQueueError(
          "claim_expired",
          "portable execution claim expired",
        );
      }
      if (row.state === "terminal" || revision !== request.expectedLeaseRevision) {
        throw new PortableSessionCommandQueueError(
          "stale_revision",
          "portable execution lease revision is stale",
        );
      }
      if (new Date(request.completedAt) > now) {
        throw new PortableSessionCommandQueueError(
          "invalid",
          "portable execution completion is in the future",
        );
      }
      const updated: ExecutionRow[] = await tx`
        UPDATE khala_sync_portable_command_executions
        SET lease_revision = lease_revision + 1, state = 'terminal',
            terminal_status = ${request.terminalStatus}, pending_reconcile_ref = NULL,
            outcome_ref = ${request.outcomeRef},
            evidence_refs_json = ${JSON.stringify(request.evidenceRefs)}::jsonb,
            updated_at = ${request.completedAt}
        WHERE claim_ref = ${request.claimRef} AND lease_revision = ${revision}
          AND state IN ('claimed', 'pending_reconcile')
        RETURNING *
      `;
      return { status: "terminal", claim: executionFromRow(updated[0]!) };
    });
  }

  async markPendingReconcile(
    input: unknown,
  ): Promise<PortableCommandExecutionPendingReconcileResult> {
    assertPublicSafe(input);
    let request: ReturnType<typeof decodePendingRequest>;
    try {
      request = decodePendingRequest(input);
    } catch {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "portable pending-reconcile request is invalid",
      );
    }
    const now = new Date(this.now());
    return this.sql.begin(async (tx) => {
      const row = await selectExecution(tx, request.claimRef);
      if (!row)
        throw new PortableSessionCommandQueueError(
          "not_found",
          "portable execution claim does not exist",
        );
      this.assertClaimIdentity(row, request);
      const revision = positive(row.lease_revision, "lease revision");
      const samePending =
        row.state === "pending_reconcile" &&
        revision === request.expectedLeaseRevision + 1 &&
        row.pending_reconcile_ref === request.pendingReconcileRef &&
        canonical(parseJson(row.evidence_refs_json)) === canonical(request.evidenceRefs);
      if (samePending) return { status: "replayed", claim: executionFromRow(row) };
      if (row.state === "expired" || new Date(row.lease_expires_at) <= now) {
        throw new PortableSessionCommandQueueError(
          "claim_expired",
          "portable execution claim expired",
        );
      }
      if (row.state !== "claimed" || revision !== request.expectedLeaseRevision) {
        throw new PortableSessionCommandQueueError(
          "stale_revision",
          "portable execution lease revision is stale",
        );
      }
      if (new Date(request.observedAt) > now) {
        throw new PortableSessionCommandQueueError(
          "invalid",
          "pending-reconcile observation is in the future",
        );
      }
      const updated: ExecutionRow[] = await tx`
        UPDATE khala_sync_portable_command_executions
        SET lease_revision = lease_revision + 1, state = 'pending_reconcile',
            pending_reconcile_ref = ${request.pendingReconcileRef},
            evidence_refs_json = ${JSON.stringify(request.evidenceRefs)}::jsonb,
            updated_at = ${request.observedAt}
        WHERE claim_ref = ${request.claimRef} AND lease_revision = ${revision}
          AND state = 'claimed'
        RETURNING *
      `;
      return { status: "pending_reconcile", claim: executionFromRow(updated[0]!) };
    });
  }

  async expire(at: string = this.now()): Promise<number> {
    const instant = new Date(at);
    if (Number.isNaN(instant.valueOf())) {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "portable execution expiry instant is invalid",
      );
    }
    return this.sql.begin(async (tx) => {
      const expired: Array<{ command_ref: string }> = await tx`
        UPDATE khala_sync_portable_command_executions
        SET lease_revision = lease_revision + 1, state = 'expired',
            terminal_status = 'expired', pending_reconcile_ref = NULL,
            outcome_ref = 'outcome.portable-command-expired.' || substring(claim_fingerprint FROM 8),
            updated_at = ${instant.toISOString()}
        WHERE state IN ('claimed', 'pending_reconcile')
          AND lease_expires_at <= ${instant.toISOString()}
        RETURNING command_ref
      `;
      return expired.length;
    });
  }

  private assertCommandBytes(row: CommandRow, command: PortableSessionCommand): void {
    if (
      command.commandRef !== row.command_ref ||
      command.ownerRef !== row.owner_user_id ||
      command.sessionRef !== row.session_ref ||
      command.kind !== row.kind ||
      command.expectedAttachmentRef !== row.expected_attachment_ref ||
      command.expectedGeneration !== Number(row.expected_generation) ||
      command.destinationTargetRef !== row.destination_target_ref
    ) {
      throw new PortableSessionCommandQueueError(
        "invalid",
        "stored portable command columns do not match command bytes",
      );
    }
  }

  private assertCommandClaimable(
    row: CommandRow,
    command: PortableSessionCommand,
    executorEnvironmentRef: string,
    now: Date,
    requestedExpiry: Date,
  ): asserts command is PortableSessionCommand & {
    readonly kind: "attach" | "move" | "failback";
    readonly destinationTargetRef: string;
  } {
    const generation = Number(row.current_attachment_generation);
    if (row.status !== "accepted" || !["attach", "move", "failback"].includes(command.kind)) {
      throw new PortableSessionCommandQueueError(
        "not_claimable",
        "portable command is not accepted executable work",
      );
    }
    if (new Date(row.expires_at) <= now || requestedExpiry > new Date(row.expires_at)) {
      throw new PortableSessionCommandQueueError(
        "claim_expired",
        "portable command execution is outside command expiry",
      );
    }
    if (
      row.current_attachment_ref !== command.expectedAttachmentRef ||
      generation !== command.expectedGeneration ||
      row.source_target_ref !== executorEnvironmentRef
    ) {
      throw new PortableSessionCommandQueueError(
        "stale_generation",
        "portable command source generation is stale",
      );
    }
    if (
      !command.destinationTargetRef ||
      row.destination_health !== "ready" ||
      command.destinationTargetRef === executorEnvironmentRef ||
      !command.checkpointRef
    ) {
      throw new PortableSessionCommandQueueError(
        "not_claimable",
        "portable command destination or checkpoint is not executable",
      );
    }
  }

  private assertClaimIdentity(
    row: ExecutionRow,
    request: {
      readonly executorEnvironmentRef: string;
      readonly workerInstanceRef: string;
      readonly claimGeneration: number;
    },
  ): void {
    if (
      row.executor_environment_ref !== request.executorEnvironmentRef ||
      row.worker_instance_ref !== request.workerInstanceRef
    ) {
      throw new PortableSessionCommandQueueError(
        "claim_conflict",
        "portable execution claim belongs to another executor",
      );
    }
    if (positive(row.claim_generation, "claim generation") !== request.claimGeneration) {
      throw new PortableSessionCommandQueueError(
        "stale_generation",
        "portable execution claim generation is stale",
      );
    }
  }
}

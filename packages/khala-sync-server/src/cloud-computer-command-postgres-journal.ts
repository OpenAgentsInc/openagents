import type { SyncSql, SyncTransactionSql } from "./sql.js";
import {
  CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA,
  CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA,
  CloudComputerCommandStreamError,
  type CloudComputerCommandJournalPort,
  type CloudComputerCommandPublicStatus,
  type CloudComputerCommandStreamArtifact,
  type CloudComputerCommandStreamCursor,
  type CloudComputerCommandStreamEvent,
  type CloudComputerCommandStreamRead,
  type CloudComputerCommandStreamScope,
  type CloudComputerCommandTerminalSummary,
} from "./cloud-computer-command-stream.js";

type CommandStreamRow = Readonly<{
  owner_ref: string;
  tenant_ref: string;
  computer_ref: string;
  workspace_ref: string;
  session_ref: string;
  command_ref: string;
  runtime_generation: string | number;
  retention_epoch: string | number;
  next_command_sequence: string | number;
  first_retained_command_sequence: string | number | null;
  status: string;
  terminal_ref: string | null;
  terminal_digest: string | null;
  terminal_reason: string | null;
  terminal_exit_code: string | number | null;
  terminal_output_digest: string | null;
  terminal_payload_json: unknown;
  terminal_command_sequence: string | number | null;
  completed_at: Date | string | null;
}>;

type EventRow = Readonly<{
  event_ref: string;
  command_sequence: string | number;
  event_digest: string;
  kind: string;
  payload_json: unknown;
  artifact_refs_json: unknown;
  observed_at: Date | string;
}>;

type ArtifactRow = Readonly<{
  artifact_ref: string;
  kind: string;
  content_digest: string;
  byte_count: string | number;
  retain_until: Date | string;
}>;

const commandStatuses = new Set([
  "admitted",
  "not_dispatched",
  "dispatched",
  "may_have_started",
  "running",
  "completed",
  "failed",
  "cancelled",
  "timed_out",
  "lost",
]);
const terminalOutcomes = new Set(["completed", "failed", "cancelled", "timed_out", "lost"]);
const eventKinds = new Set([
  "accepted",
  "stdout",
  "stderr",
  "progress",
  "tool",
  "lifecycle",
  "checkpoint",
  ...terminalOutcomes,
]);
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const PUBLIC_REASON = /^[a-z][a-z0-9._-]{0,127}$/u;

const invalidJournal = (message: string): never => {
  throw new CloudComputerCommandStreamError("invalid_journal", message);
};

const integer = (value: string | number, field: string): number => {
  const result = Number(value);
  if (!Number.isSafeInteger(result) || result < 0) invalidJournal(`${field} is invalid`);
  return result;
};

const timestamp = (value: Date | string, field: string): string => {
  const result = value instanceof Date ? value.toISOString() : value;
  if (!Number.isFinite(Date.parse(result))) invalidJournal(`${field} is invalid`);
  return new Date(result).toISOString();
};

const record = (value: unknown, field: string): Readonly<Record<string, unknown>> => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    invalidJournal(`${field} is invalid`);
  }
  return value as Readonly<Record<string, unknown>>;
};

const refs = (value: unknown): ReadonlyArray<string> => {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    invalidJournal("event artifact refs are invalid");
  }
  return value as ReadonlyArray<string>;
};

const publicPayload = (kind: string, value: unknown): Readonly<Record<string, unknown>> => {
  const payload = record(value, "event payload");
  if (kind === "stdout" || kind === "stderr") {
    if (typeof payload.text === "string") return { text: payload.text };
    return typeof payload.chunk === "string" ? { chunk: payload.chunk } : {};
  }
  if (["completed", "failed", "cancelled", "timed_out", "lost"].includes(kind)) {
    const result: Record<string, unknown> = {};
    if (
      payload.exitCode === null ||
      (Number.isSafeInteger(payload.exitCode) && Number(payload.exitCode) >= 0)
    ) {
      result.exitCode = payload.exitCode;
    }
    if (
      payload.outputDigest === null ||
      (typeof payload.outputDigest === "string" && DIGEST.test(payload.outputDigest))
    ) {
      result.outputDigest = payload.outputDigest;
    }
    if (
      payload.reason === null ||
      (typeof payload.reason === "string" && PUBLIC_REASON.test(payload.reason))
    ) {
      result.reason = payload.reason;
    }
    return result;
  }
  // Until each event kind has a versioned public payload schema, expose only
  // its durable envelope. Stored runtime and provider fields remain private.
  return {};
};

const cursor = (
  scope: CloudComputerCommandStreamScope,
  sequence: number,
  retentionEpoch: number,
): CloudComputerCommandStreamCursor => ({
  schema: CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  sessionRef: scope.sessionRef,
  commandRef: scope.commandRef,
  runtimeGeneration: scope.runtimeGeneration,
  sequence,
  retentionEpoch,
});

const commandStatusFrom = (value: string): CloudComputerCommandPublicStatus => {
  if (!commandStatuses.has(value)) return invalidJournal("command status is invalid");
  return value as CloudComputerCommandPublicStatus;
};

const terminalFrom = (row: CommandStreamRow): CloudComputerCommandTerminalSummary | null => {
  if (!terminalOutcomes.has(row.status)) return null;
  if (
    row.terminal_ref === null ||
    row.terminal_digest === null ||
    row.terminal_reason === null ||
    row.completed_at === null
  ) {
    return invalidJournal("terminal evidence is incomplete");
  }
  if (!DIGEST.test(row.terminal_digest) || !PUBLIC_REASON.test(row.terminal_reason)) {
    return invalidJournal("terminal evidence is not safe for public projection");
  }
  const exitCode =
    row.terminal_exit_code === null ? null : integer(row.terminal_exit_code, "terminal exit code");
  if (row.status === "completed" && exitCode !== 0) {
    return invalidJournal("completed terminal exit code is invalid");
  }
  const terminalPayload =
    row.terminal_payload_json === null ? {} : record(row.terminal_payload_json, "terminal payload");
  const eventOutputDigest = terminalPayload.outputDigest;
  if (
    !(
      eventOutputDigest === undefined ||
      eventOutputDigest === null ||
      typeof eventOutputDigest === "string"
    )
  ) {
    return invalidJournal("terminal output digest is invalid");
  }
  if (typeof eventOutputDigest === "string" && !DIGEST.test(eventOutputDigest)) {
    return invalidJournal("terminal output digest is invalid");
  }
  if (row.terminal_output_digest !== null && !DIGEST.test(row.terminal_output_digest)) {
    return invalidJournal("stored terminal output digest is invalid");
  }
  if (
    eventOutputDigest !== undefined &&
    (eventOutputDigest ?? null) !== row.terminal_output_digest
  ) {
    return invalidJournal("terminal output digest evidence differs");
  }
  return {
    terminalRef: row.terminal_ref,
    terminalDigest: row.terminal_digest,
    outcome: row.status as CloudComputerCommandTerminalSummary["outcome"],
    reason: row.terminal_reason,
    exitCode,
    outputDigest: row.terminal_output_digest,
    completedAt: timestamp(row.completed_at, "terminal completion"),
  };
};

const eventFrom = (row: EventRow): CloudComputerCommandStreamEvent => {
  if (!eventKinds.has(row.kind) || !DIGEST.test(row.event_digest)) {
    return invalidJournal("event kind or digest is invalid");
  }
  return {
    eventRef: row.event_ref,
    sequence: integer(row.command_sequence, "event sequence"),
    eventDigest: row.event_digest,
    kind: row.kind,
    payload: publicPayload(row.kind, row.payload_json),
    artifactRefs: refs(row.artifact_refs_json),
    observedAt: timestamp(row.observed_at, "event timestamp"),
  };
};

const artifactFrom = (row: ArtifactRow): CloudComputerCommandStreamArtifact => {
  if (!(["stdout", "stderr", "result", "diagnostic"] as ReadonlyArray<string>).includes(row.kind)) {
    return invalidJournal("artifact kind is invalid");
  }
  return {
    artifactRef: row.artifact_ref,
    kind: row.kind as CloudComputerCommandStreamArtifact["kind"],
    contentDigest: row.content_digest,
    byteCount: integer(row.byte_count, "artifact byte count"),
    retainUntil: timestamp(row.retain_until, "artifact retention"),
  };
};

const readSnapshot = async (
  tx: SyncTransactionSql,
  scope: CloudComputerCommandStreamScope,
  requested: CloudComputerCommandStreamCursor | null,
  limit: number,
): Promise<CloudComputerCommandStreamRead> => {
  const commands: ReadonlyArray<CommandStreamRow> = await tx`
    SELECT command.owner_ref, command.tenant_ref, command.computer_ref, command.workspace_ref,
           command.session_ref, command.command_ref, command.runtime_generation,
           session.retention_epoch, command.next_command_sequence,
           (SELECT MIN(event.command_sequence)
              FROM khala_sync_cloud_computer_command_events AS event
             WHERE event.command_ref = command.command_ref) AS first_retained_command_sequence,
           command.status, command.terminal_ref, command.terminal_digest,
           command.terminal_reason, command.terminal_exit_code, command.terminal_output_digest,
           (SELECT terminal_event.payload_json
              FROM khala_sync_cloud_computer_command_events AS terminal_event
             WHERE terminal_event.event_ref = command.terminal_ref
               AND terminal_event.command_ref = command.command_ref) AS terminal_payload_json,
           command.terminal_command_sequence, command.completed_at
    FROM khala_sync_cloud_computer_commands AS command
    JOIN khala_sync_cloud_computer_command_sessions AS session
      ON session.session_ref = command.session_ref
    WHERE command.owner_ref = ${scope.ownerRef}
      AND command.tenant_ref = ${scope.tenantRef}
      AND command.computer_ref = ${scope.computerRef}
      AND command.workspace_ref = ${scope.workspaceRef}
      AND command.session_ref = ${scope.sessionRef}
      AND command.command_ref = ${scope.commandRef}
      AND command.runtime_generation = ${scope.runtimeGeneration}
      AND session.owner_ref = command.owner_ref
      AND session.tenant_ref = command.tenant_ref
      AND session.computer_ref = command.computer_ref
      AND session.workspace_ref = command.workspace_ref
      AND session.runtime_generation = command.runtime_generation
  `;
  const command = commands[0];
  if (command === undefined) {
    throw new CloudComputerCommandStreamError(
      "invalid_input",
      "command stream scope is unavailable",
    );
  }

  const retentionEpoch = integer(command.retention_epoch, "retention epoch");
  const commandStatus = commandStatusFrom(command.status);
  const nextCommandSequence = integer(command.next_command_sequence, "next command sequence");
  if (nextCommandSequence < 1) invalidJournal("next command sequence is invalid");
  const lastCommandSequence = nextCommandSequence - 1;
  const firstRetained =
    command.first_retained_command_sequence === null
      ? null
      : integer(command.first_retained_command_sequence, "first retained command sequence");
  const retentionWatermark = firstRetained === null ? lastCommandSequence : firstRetained - 1;
  if (retentionWatermark < 0 || retentionWatermark > lastCommandSequence) {
    invalidJournal("command retention watermark is invalid");
  }

  if (requested !== null && requested.sequence > lastCommandSequence) {
    throw new CloudComputerCommandStreamError("invalid_input", "command cursor is ahead");
  }

  if (
    requested !== null &&
    (requested.retentionEpoch !== retentionEpoch || requested.sequence < retentionWatermark)
  ) {
    const artifacts = await readArtifacts(tx, scope.commandRef);
    return {
      schema: CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA,
      kind: "stale_cursor",
      scope,
      commandStatus,
      reason:
        requested.retentionEpoch !== retentionEpoch
          ? "retention_epoch_changed"
          : "cursor_behind_retention",
      resetCursor: cursor(scope, retentionWatermark, retentionEpoch),
      retentionWatermark,
      retentionEpoch,
      terminal: terminalFrom(command),
      artifacts,
    };
  }

  const after = requested?.sequence ?? retentionWatermark;
  const rows: ReadonlyArray<EventRow> = await tx`
    SELECT event_ref, command_sequence, event_digest, kind, payload_json,
           artifact_refs_json, observed_at
    FROM khala_sync_cloud_computer_command_events
    WHERE command_ref = ${scope.commandRef}
      AND command_sequence > ${after}
    ORDER BY command_sequence
    LIMIT ${limit + 1}
  `;
  const hasMore = rows.length > limit;
  const events = rows.slice(0, limit).map(eventFrom);
  events.forEach((event, index) => {
    if (event.sequence !== after + index + 1 || event.sequence > lastCommandSequence) {
      invalidJournal("command event sequence is not dense");
    }
  });
  if (events.length === 0 && after < lastCommandSequence) {
    invalidJournal("retained command events are missing");
  }
  const pageSequence = events.at(-1)?.sequence ?? after;
  const terminalSequence =
    command.terminal_command_sequence === null
      ? null
      : integer(command.terminal_command_sequence, "terminal command sequence");
  const terminal =
    terminalSequence === null
      ? hasMore
        ? null
        : terminalFrom(command)
      : terminalSequence <= pageSequence
        ? terminalFrom(command)
        : null;

  return {
    schema: CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA,
    kind: "page",
    scope,
    commandStatus,
    cursor: cursor(scope, pageSequence, retentionEpoch),
    events,
    hasMore,
    retentionWatermark,
    retentionEpoch,
    terminal,
    artifacts: await readArtifacts(tx, scope.commandRef),
  };
};

const readArtifacts = async (
  tx: SyncTransactionSql,
  commandRef: string,
): Promise<ReadonlyArray<CloudComputerCommandStreamArtifact>> => {
  const rows: ReadonlyArray<ArtifactRow> = await tx`
    SELECT artifact_ref, kind, content_digest, byte_count, retain_until
    FROM khala_sync_cloud_computer_command_artifacts
    WHERE command_ref = ${commandRef}
    ORDER BY created_at, artifact_ref
  `;
  return rows.map(artifactFrom);
};

/** Reads each stream page from one repeatable Postgres snapshot. */
export class PostgresCloudComputerCommandJournal implements CloudComputerCommandJournalPort {
  constructor(private readonly sql: SyncSql) {}

  async read(input: {
    scope: CloudComputerCommandStreamScope;
    cursor: CloudComputerCommandStreamCursor | null;
    limit: number;
  }): Promise<CloudComputerCommandStreamRead> {
    if (!Number.isSafeInteger(input.limit) || input.limit < 1 || input.limit > 1_000) {
      throw new CloudComputerCommandStreamError("invalid_input", "page size is invalid");
    }
    if (
      input.cursor !== null &&
      (input.cursor.schema !== CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA ||
        input.cursor.sessionRef !== input.scope.sessionRef ||
        input.cursor.commandRef !== input.scope.commandRef ||
        input.cursor.runtimeGeneration !== input.scope.runtimeGeneration ||
        !Number.isSafeInteger(input.cursor.sequence) ||
        input.cursor.sequence < 0 ||
        !Number.isSafeInteger(input.cursor.retentionEpoch) ||
        input.cursor.retentionEpoch < 0)
    ) {
      throw new CloudComputerCommandStreamError("invalid_input", "cursor scope differs");
    }
    return this.sql.begin("isolation level repeatable read read only", (tx) =>
      readSnapshot(tx, input.scope, input.cursor, input.limit),
    );
  }
}

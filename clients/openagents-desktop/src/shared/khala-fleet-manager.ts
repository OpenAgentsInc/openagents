import { Database } from "bun:sqlite"
import { Schema } from "effect"
import { mkdirSync } from "node:fs"
import { dirname } from "node:path"

export const KhalaFleetRowState = Schema.Literals([
  "planned",
  "dispatched",
  "accepted",
  "executing",
  "completed",
  "rejected",
  "retryable",
  "blocked",
])
export type KhalaFleetRowState = typeof KhalaFleetRowState.Type

export const KhalaFleetReasonKind = Schema.Literals([
  "none",
  "capacity_unavailable",
  "credentials_missing",
  "execution_refused",
  "github_closed",
  "pylon_unavailable",
  "token_reconciliation_failed",
  "verifier_failed",
  "worker_stale",
  "unknown",
])
export type KhalaFleetReasonKind = typeof KhalaFleetReasonKind.Type

export type KhalaFleetTaskRef = {
  readonly issueRef: string | null
  readonly prRef: string | null
}

export type KhalaFleetPlanInput = KhalaFleetTaskRef & {
  readonly accountRef: string
  readonly claimRef?: string | null
  readonly originMainCommit: string
  readonly queueDecision?: string | null
  readonly queueLane?: string | null
  readonly verifier: string
  readonly reasonKind?: KhalaFleetReasonKind
  readonly reasonDetail?: string | null
}

export type KhalaFleetLifecycleInput = {
  readonly assignmentRef?: string | null
  readonly pid?: number | null
  readonly reasonKind?: KhalaFleetReasonKind
  readonly reasonDetail?: string | null
}

export type KhalaFleetLogInput = {
  readonly assignmentRef?: string | null
  readonly eventType: string
  readonly message?: string | null
  readonly payload?: unknown
}

export type KhalaFleetRow = KhalaFleetTaskRef & {
  readonly id: number
  readonly accountRef: string
  readonly assignmentRef: string | null
  readonly claimRef: string | null
  readonly verifier: string
  readonly originMainCommit: string
  readonly queueDecision: string | null
  readonly queueLane: string | null
  readonly pid: number | null
  readonly state: KhalaFleetRowState
  readonly reasonKind: KhalaFleetReasonKind
  readonly reasonDetail: string | null
  readonly createdAt: string
  readonly updatedAt: string
  readonly plannedAt: string | null
  readonly dispatchedAt: string | null
  readonly acceptedAt: string | null
  readonly executingAt: string | null
  readonly completedAt: string | null
  readonly rejectedAt: string | null
  readonly retryableAt: string | null
  readonly blockedAt: string | null
  readonly tokenFailureCount: number
  readonly tokenInputTokens: number | null
  readonly tokenOutputTokens: number | null
  readonly tokenReasoningTokens: number | null
  readonly tokenReconciledAt: string | null
  readonly tokenTotalTokens: number | null
}

export type KhalaFleetLogRow = {
  readonly id: number
  readonly rowId: number
  readonly assignmentRef: string | null
  readonly eventType: string
  readonly message: string | null
  readonly payloadJson: string | null
  readonly createdAt: string
}

export type KhalaFleetRestartSnapshot = {
  readonly activeRows: readonly KhalaFleetRow[]
  readonly blockedRows: readonly KhalaFleetRow[]
  readonly completedRows: readonly KhalaFleetRow[]
  readonly controller: {
    readonly singletonActive: boolean
    readonly storePath: string
  }
  readonly observedAt: string
  readonly retryableRows: readonly KhalaFleetRow[]
}

export type KhalaFleetSnapshotResult =
  | {
      readonly ok: true
      readonly snapshot: KhalaFleetRestartSnapshot
    }
  | {
      readonly ok: false
      readonly error: string
      readonly observedAt: string
    }

type FleetStoreOptions = {
  readonly path: string
}

const activeStates = new Set<KhalaFleetRowState>([
  "planned",
  "dispatched",
  "accepted",
  "executing",
])

const timestampColumns: Partial<Record<KhalaFleetRowState, keyof KhalaFleetRow>> = {
  accepted: "acceptedAt",
  blocked: "blockedAt",
  completed: "completedAt",
  dispatched: "dispatchedAt",
  executing: "executingAt",
  planned: "plannedAt",
  rejected: "rejectedAt",
  retryable: "retryableAt",
}

const stringValue = (value: unknown, fallback = ""): string =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : fallback

const nullableString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const nullableNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const stateValue = (value: unknown): KhalaFleetRowState =>
  Schema.decodeUnknownSync(KhalaFleetRowState)(value)

const reasonValue = (value: unknown): KhalaFleetReasonKind =>
  Schema.decodeUnknownSync(KhalaFleetReasonKind)(value)

export const redactKhalaFleetLogText = (value: string): string => {
  return value
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gi, "Bearer [REDACTED]")
    .replace(
      /\b(OPENAGENTS_AGENT_TOKEN|GITHUB_TOKEN|NPM_TOKEN|ANTHROPIC_API_KEY|OPENAI_API_KEY)=([^\s"'`]+)/gi,
      "$1=[REDACTED]",
    )
    .replace(
      /("(?:access_token|refresh_token|id_token|api_key|authorization|token)"\s*:\s*)"[^"]*"/gi,
      "$1\"[REDACTED]\"",
    )
    .replace(/\/Users\/[^/\s]+\/\.codex\/auth\.json/g, "[REDACTED_CODEX_AUTH]")
    .replace(
      /\/Users\/[^/\s]+\/\.pylon-fable\/accounts\/codex\/[^/\s]+\/auth\.json/g,
      "[REDACTED_CODEX_AUTH]",
    )
}

const redactedPayloadJson = (payload: unknown): string | null => {
  if (payload === undefined || payload === null) return null
  try {
    return redactKhalaFleetLogText(JSON.stringify(payload))
  } catch {
    return redactKhalaFleetLogText(String(payload))
  }
}

const rowFromSql = (row: Record<string, unknown>): KhalaFleetRow => ({
  id: Number(row.id),
  accountRef: stringValue(row.account_ref),
  assignmentRef: nullableString(row.assignment_ref),
  claimRef: nullableString(row.claim_ref),
  prRef: nullableString(row.pr_ref),
  issueRef: nullableString(row.issue_ref),
  verifier: stringValue(row.verifier),
  originMainCommit: stringValue(row.origin_main_commit),
  queueDecision: nullableString(row.queue_decision),
  queueLane: nullableString(row.queue_lane),
  pid: nullableNumber(row.pid),
  state: stateValue(row.state),
  reasonKind: reasonValue(row.reason_kind),
  reasonDetail: nullableString(row.reason_detail),
  createdAt: stringValue(row.created_at),
  updatedAt: stringValue(row.updated_at),
  plannedAt: nullableString(row.planned_at),
  dispatchedAt: nullableString(row.dispatched_at),
  acceptedAt: nullableString(row.accepted_at),
  executingAt: nullableString(row.executing_at),
  completedAt: nullableString(row.completed_at),
  rejectedAt: nullableString(row.rejected_at),
  retryableAt: nullableString(row.retryable_at),
  blockedAt: nullableString(row.blocked_at),
  tokenFailureCount: Number(row.token_failure_count ?? 0),
  tokenInputTokens: nullableNumber(row.token_input_tokens),
  tokenOutputTokens: nullableNumber(row.token_output_tokens),
  tokenReasoningTokens: nullableNumber(row.token_reasoning_tokens),
  tokenReconciledAt: nullableString(row.token_reconciled_at),
  tokenTotalTokens: nullableNumber(row.token_total_tokens),
})

const logFromSql = (row: Record<string, unknown>): KhalaFleetLogRow => ({
  id: Number(row.id),
  rowId: Number(row.row_id),
  assignmentRef: nullableString(row.assignment_ref),
  eventType: stringValue(row.event_type),
  message: nullableString(row.message),
  payloadJson: nullableString(row.payload_json),
  createdAt: stringValue(row.created_at),
})

export class KhalaFleetEventStore {
  readonly path: string
  private readonly db: Database

  constructor(options: FleetStoreOptions) {
    this.path = options.path
    if (this.path !== ":memory:") {
      mkdirSync(dirname(this.path), { recursive: true })
    }
    this.db = new Database(this.path, { create: true, strict: true })
    this.db.exec("PRAGMA journal_mode = WAL")
    this.db.exec("PRAGMA foreign_keys = ON")
    this.migrate()
  }

  close(): void {
    this.db.close()
  }

  plan(input: KhalaFleetPlanInput, now = new Date()): KhalaFleetRow {
    const observedAt = now.toISOString()
    const insert = this.db.prepare(`
      INSERT INTO khala_fleet_rows (
        account_ref, assignment_ref, claim_ref, pr_ref, issue_ref, verifier,
        origin_main_commit, queue_lane, queue_decision, pid, state,
        reason_kind, reason_detail,
        created_at, updated_at, planned_at
      )
      VALUES ($accountRef, NULL, $claimRef, $prRef, $issueRef, $verifier,
        $originMainCommit, $queueLane, $queueDecision, NULL, 'planned',
        $reasonKind, $reasonDetail, $observedAt, $observedAt, $observedAt)
      RETURNING *
    `)

    return rowFromSql(insert.get({
      accountRef: input.accountRef,
      claimRef: input.claimRef ?? null,
      issueRef: input.issueRef,
      originMainCommit: input.originMainCommit,
      prRef: input.prRef,
      queueDecision: input.queueDecision ?? null,
      queueLane: input.queueLane ?? null,
      reasonDetail: input.reasonDetail ?? null,
      reasonKind: input.reasonKind ?? "none",
      observedAt: observedAt,
      verifier: input.verifier,
    }) as Record<string, unknown>)
  }

  transition(
    rowId: number,
    state: KhalaFleetRowState,
    input: KhalaFleetLifecycleInput = {},
    now = new Date(),
  ): KhalaFleetRow {
    const observedAt = now.toISOString()
    const timestampColumn = timestampColumns[state]
    if (timestampColumn === undefined) {
      throw new Error(`No timestamp column for fleet state ${state}`)
    }
    const sqlColumn = this.sqlTimestampColumn(timestampColumn)
    const existing = this.requireRow(rowId)
    const assignmentRef = input.assignmentRef ?? existing.assignmentRef
    const pid = input.pid ?? existing.pid
    const reasonKind = input.reasonKind ?? existing.reasonKind
    const reasonDetail =
      input.reasonDetail === undefined ? existing.reasonDetail : input.reasonDetail

    const update = this.db.prepare(`
      UPDATE khala_fleet_rows
         SET assignment_ref = $assignmentRef,
             pid = $pid,
             state = $state,
             reason_kind = $reasonKind,
             reason_detail = $reasonDetail,
             updated_at = $observedAt,
             ${sqlColumn} = $observedAt
       WHERE id = $rowId
      RETURNING *
    `)
    return rowFromSql(update.get({
      assignmentRef: assignmentRef,
      observedAt: observedAt,
      pid: pid,
      reasonDetail: reasonDetail,
      reasonKind: reasonKind,
      rowId: rowId,
      state: state,
    }) as Record<string, unknown>)
  }

  appendLog(
    rowId: number,
    input: KhalaFleetLogInput,
    now = new Date(),
  ): KhalaFleetLogRow {
    this.requireRow(rowId)
    const observedAt = now.toISOString()
    const insert = this.db.prepare(`
      INSERT INTO khala_fleet_events (
        row_id, assignment_ref, event_type, message, payload_json, created_at
      )
      VALUES ($rowId, $assignmentRef, $eventType, $message, $payloadJson, $observedAt)
      RETURNING *
    `)

    return logFromSql(insert.get({
      assignmentRef: input.assignmentRef ?? null,
      eventType: input.eventType,
      message:
        input.message === undefined || input.message === null
          ? null
          : redactKhalaFleetLogText(input.message),
      observedAt: observedAt,
      payloadJson: redactedPayloadJson(input.payload),
      rowId: rowId,
    }) as Record<string, unknown>)
  }

  reconcileTokens(
    rowId: number,
    input: {
      readonly inputTokens: number
      readonly outputTokens: number
      readonly reasoningTokens?: number | null
      readonly totalTokens: number
    },
    now = new Date(),
  ): KhalaFleetRow {
    this.requireRow(rowId)
    const observedAt = now.toISOString()
    const update = this.db.prepare(`
      UPDATE khala_fleet_rows
         SET token_input_tokens = $inputTokens,
             token_output_tokens = $outputTokens,
             token_reasoning_tokens = $reasoningTokens,
             token_total_tokens = $totalTokens,
             token_reconciled_at = $observedAt,
             updated_at = $observedAt
       WHERE id = $rowId
      RETURNING *
    `)

    return rowFromSql(update.get({
      inputTokens: input.inputTokens,
      observedAt,
      outputTokens: input.outputTokens,
      reasoningTokens: input.reasoningTokens ?? null,
      rowId,
      totalTokens: input.totalTokens,
    }) as Record<string, unknown>)
  }

  recordTokenFailure(rowId: number, now = new Date()): KhalaFleetRow {
    this.requireRow(rowId)
    const observedAt = now.toISOString()
    const update = this.db.prepare(`
      UPDATE khala_fleet_rows
         SET token_failure_count = token_failure_count + 1,
             updated_at = $observedAt
       WHERE id = $rowId
      RETURNING *
    `)

    return rowFromSql(update.get({ observedAt, rowId }) as Record<string, unknown>)
  }

  getRow(rowId: number): KhalaFleetRow | null {
    const row = this.db
      .prepare("SELECT * FROM khala_fleet_rows WHERE id = $rowId")
      .get({ rowId: rowId }) as Record<string, unknown> | null
    return row === null ? null : rowFromSql(row)
  }

  requireRow(rowId: number): KhalaFleetRow {
    const row = this.getRow(rowId)
    if (row === null) throw new Error(`Unknown Khala fleet row ${rowId}`)
    return row
  }

  logsForRow(rowId: number): readonly KhalaFleetLogRow[] {
    return (this.db
      .prepare(`
        SELECT * FROM khala_fleet_events
         WHERE row_id = $rowId
         ORDER BY id ASC
      `)
      .all({ rowId: rowId }) as Record<string, unknown>[]).map(logFromSql)
  }

  reconstructActiveState(now = new Date()): KhalaFleetRestartSnapshot {
    const rows = this.listRows()
    return {
      activeRows: rows.filter(row => activeStates.has(row.state)),
      blockedRows: rows.filter(row => row.state === "blocked"),
      completedRows: rows.filter(
        row => row.state === "completed" || row.state === "rejected",
      ),
      controller: {
        singletonActive: OpenAgentsDesktopFleetManager.hasSingleton(this.path),
        storePath: this.path,
      },
      observedAt: now.toISOString(),
      retryableRows: rows.filter(row => row.state === "retryable"),
    }
  }

  private listRows(): readonly KhalaFleetRow[] {
    return (this.db
      .prepare("SELECT * FROM khala_fleet_rows ORDER BY id ASC")
      .all() as Record<string, unknown>[]).map(rowFromSql)
  }

  private migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS khala_fleet_rows (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        account_ref TEXT NOT NULL,
        assignment_ref TEXT,
        claim_ref TEXT,
        pr_ref TEXT,
        issue_ref TEXT,
        verifier TEXT NOT NULL,
        origin_main_commit TEXT NOT NULL,
        queue_lane TEXT,
        queue_decision TEXT,
        pid INTEGER,
        state TEXT NOT NULL CHECK (state IN (
          'planned', 'dispatched', 'accepted', 'executing',
          'completed', 'rejected', 'retryable', 'blocked'
        )),
        reason_kind TEXT NOT NULL DEFAULT 'none' CHECK (reason_kind IN (
          'none', 'capacity_unavailable', 'credentials_missing',
          'execution_refused', 'github_closed', 'pylon_unavailable',
          'token_reconciliation_failed', 'verifier_failed', 'worker_stale',
          'unknown'
        )),
        reason_detail TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        planned_at TEXT,
        dispatched_at TEXT,
        accepted_at TEXT,
        executing_at TEXT,
        completed_at TEXT,
        rejected_at TEXT,
        retryable_at TEXT,
        blocked_at TEXT,
        token_input_tokens INTEGER,
        token_output_tokens INTEGER,
        token_reasoning_tokens INTEGER,
        token_total_tokens INTEGER,
        token_reconciled_at TEXT,
        token_failure_count INTEGER NOT NULL DEFAULT 0,
        CHECK (pr_ref IS NOT NULL OR issue_ref IS NOT NULL)
      );

      CREATE INDEX IF NOT EXISTS khala_fleet_rows_state_idx
        ON khala_fleet_rows(state, updated_at);
      CREATE INDEX IF NOT EXISTS khala_fleet_rows_assignment_idx
        ON khala_fleet_rows(assignment_ref);
      CREATE INDEX IF NOT EXISTS khala_fleet_rows_task_idx
        ON khala_fleet_rows(pr_ref, issue_ref);

      CREATE TABLE IF NOT EXISTS khala_fleet_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        row_id INTEGER NOT NULL REFERENCES khala_fleet_rows(id) ON DELETE CASCADE,
        assignment_ref TEXT,
        event_type TEXT NOT NULL,
        message TEXT,
        payload_json TEXT,
        created_at TEXT NOT NULL
      );

      CREATE INDEX IF NOT EXISTS khala_fleet_events_row_idx
        ON khala_fleet_events(row_id, id);
    `)
    this.addMissingFleetColumns()
  }

  private addMissingFleetColumns(): void {
    const existingColumns = new Set(
      (this.db
        .prepare("PRAGMA table_info(khala_fleet_rows)")
        .all() as Array<{ name: string }>).map(column => column.name),
    )
    const additions: ReadonlyArray<readonly [string, string]> = [
      ["claim_ref", "TEXT"],
      ["queue_lane", "TEXT"],
      ["queue_decision", "TEXT"],
      ["token_input_tokens", "INTEGER"],
      ["token_output_tokens", "INTEGER"],
      ["token_reasoning_tokens", "INTEGER"],
      ["token_total_tokens", "INTEGER"],
      ["token_reconciled_at", "TEXT"],
      ["token_failure_count", "INTEGER NOT NULL DEFAULT 0"],
    ]

    for (const [column, definition] of additions) {
      if (existingColumns.has(column)) continue
      this.db.exec(`ALTER TABLE khala_fleet_rows ADD COLUMN ${column} ${definition}`)
    }
  }

  private sqlTimestampColumn(column: keyof KhalaFleetRow): string {
    switch (column) {
      case "acceptedAt":
        return "accepted_at"
      case "blockedAt":
        return "blocked_at"
      case "completedAt":
        return "completed_at"
      case "dispatchedAt":
        return "dispatched_at"
      case "executingAt":
        return "executing_at"
      case "plannedAt":
        return "planned_at"
      case "rejectedAt":
        return "rejected_at"
      case "retryableAt":
        return "retryable_at"
      default:
        throw new Error(`Unsupported timestamp column ${String(column)}`)
    }
  }
}

export class OpenAgentsDesktopFleetManager {
  private static readonly singletons = new Map<string, OpenAgentsDesktopFleetManager>()

  readonly store: KhalaFleetEventStore

  private constructor(store: KhalaFleetEventStore) {
    this.store = store
  }

  static acquire(options: FleetStoreOptions): OpenAgentsDesktopFleetManager {
    const existing = this.singletons.get(options.path)
    if (existing !== undefined) return existing

    const manager = new OpenAgentsDesktopFleetManager(
      new KhalaFleetEventStore(options),
    )
    this.singletons.set(options.path, manager)
    return manager
  }

  static hasSingleton(path: string): boolean {
    return this.singletons.has(path)
  }

  static release(path: string): void {
    const manager = this.singletons.get(path)
    if (manager === undefined) return
    manager.store.close()
    this.singletons.delete(path)
  }

  plan(input: KhalaFleetPlanInput, now?: Date): KhalaFleetRow {
    return this.store.plan(input, now)
  }

  dispatch(
    rowId: number,
    input: KhalaFleetLifecycleInput,
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "dispatched", input, now)
  }

  accept(
    rowId: number,
    input: KhalaFleetLifecycleInput = {},
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "accepted", input, now)
  }

  execute(
    rowId: number,
    input: KhalaFleetLifecycleInput,
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "executing", input, now)
  }

  complete(
    rowId: number,
    input: KhalaFleetLifecycleInput = {},
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "completed", input, now)
  }

  reject(
    rowId: number,
    input: KhalaFleetLifecycleInput,
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "rejected", input, now)
  }

  retry(
    rowId: number,
    input: KhalaFleetLifecycleInput,
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "retryable", input, now)
  }

  block(
    rowId: number,
    input: KhalaFleetLifecycleInput,
    now?: Date,
  ): KhalaFleetRow {
    return this.store.transition(rowId, "blocked", input, now)
  }

  appendLog(rowId: number, input: KhalaFleetLogInput, now?: Date): KhalaFleetLogRow {
    return this.store.appendLog(rowId, input, now)
  }

  reconcileTokens(
    rowId: number,
    input: {
      readonly inputTokens: number
      readonly outputTokens: number
      readonly reasoningTokens?: number | null
      readonly totalTokens: number
    },
    now?: Date,
  ): KhalaFleetRow {
    return this.store.reconcileTokens(rowId, input, now)
  }

  recordTokenFailure(rowId: number, now?: Date): KhalaFleetRow {
    return this.store.recordTokenFailure(rowId, now)
  }

  reconstructActiveState(now?: Date): KhalaFleetRestartSnapshot {
    return this.store.reconstructActiveState(now)
  }
}

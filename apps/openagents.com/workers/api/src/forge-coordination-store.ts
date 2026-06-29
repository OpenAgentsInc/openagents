import {
  decodeForgeCoordinationIssueRow,
  decodeForgeCoordinationPrRow,
  decodeForgeCoordinationStatusRow,
  decodeForgeDispatchLeaseRow,
  decodeForgeGitHubMirrorReceipt,
  decodeForgeMergeQueueLedgerRow,
  decodeForgePromotionDecisionReceipt,
  decodeForgeVerificationReceipt,
  forgeNip34StatusKindForState,
  type ForgeCoordinationChangeState,
  type ForgeCoordinationIssueRow,
  type ForgeCoordinationIssueState,
  type ForgeCoordinationPrRow,
  type ForgeCoordinationStatusRow,
  type ForgeCoordinationStatusState,
  type ForgeDispatchLeaseRow,
  type ForgeGitHubMirrorReceipt,
  type ForgeMergeQueueLedgerRow,
  type ForgeMergeQueueLedgerState,
  type ForgePromotionDecisionReceipt,
  type ForgeVerificationReceipt,
} from '@openagentsinc/forge-protocol'
import { Schema as S } from 'effect'

import { parseJsonWithSchema } from './json-boundary'

export type ForgeCoordinationIssueInput = Readonly<{
  tenantRef: string
  issueRef: string
  githubIssueNumber?: number | null
  title: string
  state: ForgeCoordinationIssueState
  priorityRef?: string | null
  sourceRefs: ReadonlyArray<string>
  nowIso: string
}>

export type ForgeCoordinationChangeInput = Readonly<{
  tenantRef: string
  prRef: string
  issueRef: string
  changeRef: string
  state: ForgeCoordinationChangeState
  baseHead: string
  patchHead: string
  verificationRef?: string | null
  blockerRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  nowIso: string
}>

export type ForgeCoordinationStatusInput = Readonly<{
  tenantRef: string
  statusRef: string
  subjectRef: string
  state: ForgeCoordinationStatusState
  actorRef: string
  sourceRefs: ReadonlyArray<string>
  createdAt: string
}>

export type ForgeDispatchLeaseInput = Readonly<{
  tenantRef: string
  leaseRef: string
  workRef: string
  ownerAgentRef: string
  idempotencyKeyHash?: string | null
  acquiredAt: string
  expiresAt: string
  sourceRefs: ReadonlyArray<string>
}>

export type ForgeMergeQueueLedgerInput = Readonly<{
  tenantRef: string
  queueRef: string
  baseHead: string
  actualHead: string
  virtualHead: string
  state: ForgeMergeQueueLedgerState
  nextPromotionRef?: string | null
  ready: unknown
  blocked: unknown
  sourceRefs: ReadonlyArray<string>
  nowIso: string
}>

export type ForgeDispatchLeaseAcquireResult =
  | Readonly<{ acquired: true; lease: ForgeDispatchLeaseRow }>
  | Readonly<{ acquired: false; activeLease: ForgeDispatchLeaseRow | undefined }>

export type ForgeCoordinationStore = Readonly<{
  upsertIssue: (input: ForgeCoordinationIssueInput) => Promise<ForgeCoordinationIssueRow>
  listIssues: (tenantRef: string, limit: number) => Promise<ReadonlyArray<ForgeCoordinationIssueRow>>
  upsertChange: (input: ForgeCoordinationChangeInput) => Promise<ForgeCoordinationPrRow>
  listChanges: (
    tenantRef: string,
    limit: number,
    issueRef?: string,
  ) => Promise<ReadonlyArray<ForgeCoordinationPrRow>>
  recordStatus: (input: ForgeCoordinationStatusInput) => Promise<ForgeCoordinationStatusRow>
  listStatuses: (
    tenantRef: string,
    limit: number,
    subjectRef?: string,
  ) => Promise<ReadonlyArray<ForgeCoordinationStatusRow>>
  acquireDispatchLease: (input: ForgeDispatchLeaseInput) => Promise<ForgeDispatchLeaseAcquireResult>
  listDispatchLeases: (
    tenantRef: string,
    limit: number,
    workRef?: string,
  ) => Promise<ReadonlyArray<ForgeDispatchLeaseRow>>
  readActiveDispatchLease: (
    tenantRef: string,
    workRef: string,
  ) => Promise<ForgeDispatchLeaseRow | undefined>
  recordMergeQueueLedger: (input: ForgeMergeQueueLedgerInput) => Promise<ForgeMergeQueueLedgerRow>
  listMergeQueueLedgers: (
    tenantRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<ForgeMergeQueueLedgerRow>>
  readLatestMergeQueueLedger: (
    tenantRef: string,
  ) => Promise<ForgeMergeQueueLedgerRow | undefined>
  recordVerificationReceipt: (
    receipt: ForgeVerificationReceipt,
    createdAt: string,
  ) => Promise<ForgeVerificationReceipt>
  listVerificationReceipts: (
    tenantRef: string,
    limit: number,
    changeRef?: string,
  ) => Promise<ReadonlyArray<ForgeVerificationReceipt>>
  recordPromotionDecisionReceipt: (
    receipt: ForgePromotionDecisionReceipt,
    createdAt: string,
  ) => Promise<ForgePromotionDecisionReceipt>
  readPromotionDecisionReceipt: (
    tenantRef: string,
    promotionRef: string,
  ) => Promise<ForgePromotionDecisionReceipt | undefined>
  listPromotionDecisionReceipts: (
    tenantRef: string,
    limit: number,
    changeRef?: string,
  ) => Promise<ReadonlyArray<ForgePromotionDecisionReceipt>>
  recordGitHubMirrorReceipt: (
    receipt: ForgeGitHubMirrorReceipt,
    createdAt: string,
  ) => Promise<ForgeGitHubMirrorReceipt>
  listGitHubMirrorReceipts: (
    tenantRef: string,
    limit: number,
    promotionRef?: string,
  ) => Promise<ReadonlyArray<ForgeGitHubMirrorReceipt>>
}>

const StringArray = S.Array(S.String)

const jsonArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...values])

const limitRows = (limit: number): number =>
  Math.min(Math.max(Math.trunc(limit), 1), 100)

type ForgeVerificationReceiptRow = Readonly<{
  tenant_ref: string
  verification_ref: string
  change_ref: string
  repository_ref: string
  base_ref: string
  base_head: string
  head_ref: string
  head_head: string
  packfile_ref: string
  packfile_sha256: string
  executor_identity_ref: string
  command_ref: string
  command_args_json: string
  exit_code: number | null
  verdict: string
  started_at: string
  completed_at: string
  artifact_refs_json: string
  log_sha256: string
  source_refs_json: string
  redacted: number | boolean
}>

type ForgePromotionDecisionReceiptRow = Readonly<{
  tenant_ref: string
  promotion_ref: string
  queue_ref: string
  change_ref: string
  decision: string
  base_head: string
  candidate_head: string
  promoted_head: string | null
  verification_ref: string | null
  gate_refs_json: string
  blocker_refs_json: string
  decided_by_ref: string
  decided_at: string
  source_refs_json: string
  redacted: number | boolean
}>

type ForgeGitHubMirrorReceiptRow = Readonly<{
  tenant_ref: string
  mirror_ref: string
  promotion_ref: string
  source_canonical_ref: string
  destination_github_ref: string
  repository_ref: string
  github_repository: string
  commit_id: string
  status: string
  attempted_at: string
  mirrored_at: string | null
  refusal_reason: string | null
  error_reason: string | null
  source_refs_json: string
  redacted: number | boolean
}>

const stringArrayFromJson = (value: string): ReadonlyArray<string> =>
  parseJsonWithSchema(StringArray, value)

const verificationReceiptFromRow = (
  row: ForgeVerificationReceiptRow,
): ForgeVerificationReceipt =>
  decodeForgeVerificationReceipt({
    schema: 'openagents.forge.verification.receipt.v0.1',
    tenant_ref: row.tenant_ref,
    verification_ref: row.verification_ref,
    change_ref: row.change_ref,
    repository_ref: row.repository_ref,
    base_ref: row.base_ref,
    base_head: row.base_head,
    head_ref: row.head_ref,
    head_head: row.head_head,
    packfile_ref: row.packfile_ref,
    packfile_sha256: row.packfile_sha256,
    executor_identity_ref: row.executor_identity_ref,
    command_ref: row.command_ref,
    command_args: stringArrayFromJson(row.command_args_json),
    exit_code: row.exit_code,
    verdict: row.verdict,
    started_at: row.started_at,
    completed_at: row.completed_at,
    artifact_refs: stringArrayFromJson(row.artifact_refs_json),
    log_sha256: row.log_sha256,
    source_refs: stringArrayFromJson(row.source_refs_json),
    redacted: row.redacted === true || row.redacted === 1,
  })

const promotionDecisionReceiptFromRow = (
  row: ForgePromotionDecisionReceiptRow,
): ForgePromotionDecisionReceipt =>
  decodeForgePromotionDecisionReceipt({
    schema: 'openagents.forge.promotion.decision.v0.1',
    tenant_ref: row.tenant_ref,
    promotion_ref: row.promotion_ref,
    queue_ref: row.queue_ref,
    change_ref: row.change_ref,
    decision: row.decision,
    base_head: row.base_head,
    candidate_head: row.candidate_head,
    promoted_head: row.promoted_head,
    verification_ref: row.verification_ref,
    gate_refs: stringArrayFromJson(row.gate_refs_json),
    blocker_refs: stringArrayFromJson(row.blocker_refs_json),
    decided_by_ref: row.decided_by_ref,
    decided_at: row.decided_at,
    source_refs: stringArrayFromJson(row.source_refs_json),
    redacted: row.redacted === true || row.redacted === 1,
  })

const githubMirrorReceiptFromRow = (
  row: ForgeGitHubMirrorReceiptRow,
): ForgeGitHubMirrorReceipt =>
  decodeForgeGitHubMirrorReceipt({
    schema: 'openagents.forge.github_mirror.receipt.v0.1',
    tenant_ref: row.tenant_ref,
    mirror_ref: row.mirror_ref,
    promotion_ref: row.promotion_ref,
    source_canonical_ref: row.source_canonical_ref,
    destination_github_ref: row.destination_github_ref,
    repository_ref: row.repository_ref,
    github_repository: row.github_repository,
    commit_id: row.commit_id,
    status: row.status,
    attempted_at: row.attempted_at,
    mirrored_at: row.mirrored_at,
    refusal_reason: row.refusal_reason,
    error_reason: row.error_reason,
    source_refs: stringArrayFromJson(row.source_refs_json),
    redacted: row.redacted === true || row.redacted === 1,
  })

class ForgeCoordinationStoreInvariantError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeCoordinationStoreInvariantError'
  }
}

const rowOrFail = <T>(row: T | null, label: string): T => {
  if (row === null) {
    throw new ForgeCoordinationStoreInvariantError(`${label} was not persisted`)
  }
  return row
}

const firstIssue = async (
  db: D1Database,
  tenantRef: string,
  issueRef: string,
): Promise<ForgeCoordinationIssueRow> =>
  decodeForgeCoordinationIssueRow(
    rowOrFail(
      await db
        .prepare(
          `
            SELECT *
            FROM forge_coordination_issues
            WHERE tenant_ref = ? AND issue_ref = ?
          `,
        )
        .bind(tenantRef, issueRef)
        .first(),
      'forge coordination issue',
    ),
  )

const firstChange = async (
  db: D1Database,
  tenantRef: string,
  prRef: string,
): Promise<ForgeCoordinationPrRow> =>
  decodeForgeCoordinationPrRow(
    rowOrFail(
      await db
        .prepare(
          `
            SELECT *
            FROM forge_coordination_prs
            WHERE tenant_ref = ? AND pr_ref = ?
          `,
        )
        .bind(tenantRef, prRef)
        .first(),
      'forge coordination change',
    ),
  )

const firstStatus = async (
  db: D1Database,
  tenantRef: string,
  statusRef: string,
): Promise<ForgeCoordinationStatusRow> =>
  decodeForgeCoordinationStatusRow(
    rowOrFail(
      await db
        .prepare(
          `
            SELECT *
            FROM forge_coordination_status
            WHERE tenant_ref = ? AND status_ref = ?
          `,
        )
        .bind(tenantRef, statusRef)
        .first(),
      'forge coordination status',
    ),
  )

const readActiveDispatchLease = async (
  db: D1Database,
  tenantRef: string,
  workRef: string,
): Promise<ForgeDispatchLeaseRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_dispatch_leases
        WHERE tenant_ref = ? AND work_ref = ? AND state = 'active'
        ORDER BY acquired_at DESC
        LIMIT 1
      `,
    )
    .bind(tenantRef, workRef)
    .first()

  return row === null ? undefined : decodeForgeDispatchLeaseRow(row)
}

export const makeD1ForgeCoordinationStore = (
  db: D1Database,
): ForgeCoordinationStore => ({
  async upsertIssue(input) {
    await db
      .prepare(
        `
          INSERT INTO forge_coordination_issues (
            tenant_ref,
            issue_ref,
            github_issue_number,
            title,
            state,
            priority_ref,
            source_refs_json,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT (tenant_ref, issue_ref) DO UPDATE SET
            github_issue_number = excluded.github_issue_number,
            title = excluded.title,
            state = excluded.state,
            priority_ref = excluded.priority_ref,
            source_refs_json = excluded.source_refs_json,
            updated_at = excluded.updated_at
        `,
      )
      .bind(
        input.tenantRef,
        input.issueRef,
        input.githubIssueNumber ?? null,
        input.title,
        input.state,
        input.priorityRef ?? null,
        jsonArray(input.sourceRefs),
        input.nowIso,
        input.nowIso,
      )
      .run()

    return firstIssue(db, input.tenantRef, input.issueRef)
  },

  async listIssues(tenantRef, limit) {
    const rows = await db
      .prepare(
        `
          SELECT *
          FROM forge_coordination_issues
          WHERE tenant_ref = ?
          ORDER BY updated_at DESC, issue_ref DESC
          LIMIT ?
        `,
      )
      .bind(tenantRef, limitRows(limit))
      .all<ForgeCoordinationIssueRow>()

    return rows.results.map(row => decodeForgeCoordinationIssueRow(row))
  },

  async upsertChange(input) {
    await db
      .prepare(
        `
          INSERT INTO forge_coordination_prs (
            tenant_ref,
            pr_ref,
            issue_ref,
            change_ref,
            state,
            base_head,
            patch_head,
            verification_ref,
            blocker_refs_json,
            source_refs_json,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT (tenant_ref, pr_ref) DO UPDATE SET
            issue_ref = excluded.issue_ref,
            change_ref = excluded.change_ref,
            state = excluded.state,
            base_head = excluded.base_head,
            patch_head = excluded.patch_head,
            verification_ref = excluded.verification_ref,
            blocker_refs_json = excluded.blocker_refs_json,
            source_refs_json = excluded.source_refs_json,
            updated_at = excluded.updated_at
        `,
      )
      .bind(
        input.tenantRef,
        input.prRef,
        input.issueRef,
        input.changeRef,
        input.state,
        input.baseHead,
        input.patchHead,
        input.verificationRef ?? null,
        jsonArray(input.blockerRefs),
        jsonArray(input.sourceRefs),
        input.nowIso,
        input.nowIso,
      )
      .run()

    return firstChange(db, input.tenantRef, input.prRef)
  },

  async listChanges(tenantRef, limit, issueRef) {
    const rows =
      issueRef === undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_coordination_prs
                WHERE tenant_ref = ?
                ORDER BY updated_at DESC, pr_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, limitRows(limit))
            .all<ForgeCoordinationPrRow>()
        : await db
            .prepare(
              `
                SELECT *
                FROM forge_coordination_prs
                WHERE tenant_ref = ? AND issue_ref = ?
                ORDER BY updated_at DESC, pr_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, issueRef, limitRows(limit))
            .all<ForgeCoordinationPrRow>()

    return rows.results.map(row => decodeForgeCoordinationPrRow(row))
  },

  async recordStatus(input) {
    await db
      .prepare(
        `
          INSERT INTO forge_coordination_status (
            tenant_ref,
            status_ref,
            subject_ref,
            nip34_kind,
            state,
            actor_ref,
            source_refs_json,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        input.tenantRef,
        input.statusRef,
        input.subjectRef,
        forgeNip34StatusKindForState(input.state),
        input.state,
        input.actorRef,
        jsonArray(input.sourceRefs),
        input.createdAt,
      )
      .run()

    return firstStatus(db, input.tenantRef, input.statusRef)
  },

  async listStatuses(tenantRef, limit, subjectRef) {
    const rows =
      subjectRef === undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_coordination_status
                WHERE tenant_ref = ?
                ORDER BY created_at DESC, status_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, limitRows(limit))
            .all<ForgeCoordinationStatusRow>()
        : await db
            .prepare(
              `
                SELECT *
                FROM forge_coordination_status
                WHERE tenant_ref = ? AND subject_ref = ?
                ORDER BY created_at DESC, status_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, subjectRef, limitRows(limit))
            .all<ForgeCoordinationStatusRow>()

    return rows.results.map(row => decodeForgeCoordinationStatusRow(row))
  },

  async acquireDispatchLease(input) {
    await db
      .prepare(
        `
          UPDATE forge_dispatch_leases
          SET state = 'expired', released_at = ?
          WHERE tenant_ref = ?
            AND work_ref = ?
            AND state = 'active'
            AND expires_at <= ?
        `,
      )
      .bind(input.acquiredAt, input.tenantRef, input.workRef, input.acquiredAt)
      .run()

    try {
      await db
        .prepare(
          `
            INSERT INTO forge_dispatch_leases (
              tenant_ref,
              lease_ref,
              work_ref,
              owner_agent_ref,
              state,
              idempotency_key_hash,
              acquired_at,
              heartbeat_at,
              expires_at,
              released_at,
              source_refs_json
            ) VALUES (?, ?, ?, ?, 'active', ?, ?, ?, ?, NULL, ?)
          `,
        )
        .bind(
          input.tenantRef,
          input.leaseRef,
          input.workRef,
          input.ownerAgentRef,
          input.idempotencyKeyHash ?? null,
          input.acquiredAt,
          input.acquiredAt,
          input.expiresAt,
          jsonArray(input.sourceRefs),
        )
        .run()
    } catch {
      return {
        acquired: false,
        activeLease: await readActiveDispatchLease(db, input.tenantRef, input.workRef),
      }
    }

    const activeLease = await readActiveDispatchLease(db, input.tenantRef, input.workRef)
    if (activeLease === undefined) {
      throw new ForgeCoordinationStoreInvariantError(
        'forge dispatch lease was not persisted',
      )
    }
    return { acquired: true, lease: activeLease }
  },

  async listDispatchLeases(tenantRef, limit, workRef) {
    const rows =
      workRef === undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_dispatch_leases
                WHERE tenant_ref = ?
                ORDER BY acquired_at DESC, lease_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, limitRows(limit))
            .all<ForgeDispatchLeaseRow>()
        : await db
            .prepare(
              `
                SELECT *
                FROM forge_dispatch_leases
                WHERE tenant_ref = ? AND work_ref = ?
                ORDER BY acquired_at DESC, lease_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, workRef, limitRows(limit))
            .all<ForgeDispatchLeaseRow>()

    return rows.results.map(row => decodeForgeDispatchLeaseRow(row))
  },

  readActiveDispatchLease: (tenantRef, workRef) =>
    readActiveDispatchLease(db, tenantRef, workRef),

  async recordMergeQueueLedger(input) {
    await db
      .prepare(
        `
          INSERT INTO forge_merge_queue_ledger (
            tenant_ref,
            queue_ref,
            base_head,
            actual_head,
            virtual_head,
            state,
            next_promotion_ref,
            ready_json,
            blocked_json,
            source_refs_json,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT (tenant_ref, queue_ref) DO UPDATE SET
            base_head = excluded.base_head,
            actual_head = excluded.actual_head,
            virtual_head = excluded.virtual_head,
            state = excluded.state,
            next_promotion_ref = excluded.next_promotion_ref,
            ready_json = excluded.ready_json,
            blocked_json = excluded.blocked_json,
            source_refs_json = excluded.source_refs_json,
            updated_at = excluded.updated_at
        `,
      )
      .bind(
        input.tenantRef,
        input.queueRef,
        input.baseHead,
        input.actualHead,
        input.virtualHead,
        input.state,
        input.nextPromotionRef ?? null,
        JSON.stringify(input.ready),
        JSON.stringify(input.blocked),
        jsonArray(input.sourceRefs),
        input.nowIso,
        input.nowIso,
      )
      .run()

    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_merge_queue_ledger
          WHERE tenant_ref = ? AND queue_ref = ?
        `,
      )
      .bind(input.tenantRef, input.queueRef)
      .first()
    return decodeForgeMergeQueueLedgerRow(
      rowOrFail(row, 'forge merge queue ledger'),
    )
  },

  async listMergeQueueLedgers(tenantRef, limit) {
    const rows = await db
      .prepare(
        `
          SELECT *
          FROM forge_merge_queue_ledger
          WHERE tenant_ref = ?
          ORDER BY updated_at DESC, queue_ref DESC
          LIMIT ?
        `,
      )
      .bind(tenantRef, limitRows(limit))
      .all<ForgeMergeQueueLedgerRow>()

    return rows.results.map(row => decodeForgeMergeQueueLedgerRow(row))
  },

  async readLatestMergeQueueLedger(tenantRef) {
    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_merge_queue_ledger
          WHERE tenant_ref = ?
          ORDER BY updated_at DESC, queue_ref DESC
          LIMIT 1
        `,
      )
      .bind(tenantRef)
      .first()

    return row === null ? undefined : decodeForgeMergeQueueLedgerRow(row)
  },

  async recordVerificationReceipt(receipt, createdAt) {
    const decoded = decodeForgeVerificationReceipt(receipt)
    await db
      .prepare(
        `
          INSERT INTO forge_verification_receipts (
            tenant_ref,
            verification_ref,
            change_ref,
            repository_ref,
            base_ref,
            base_head,
            head_ref,
            head_head,
            packfile_ref,
            packfile_sha256,
            executor_identity_ref,
            command_ref,
            command_args_json,
            exit_code,
            verdict,
            started_at,
            completed_at,
            artifact_refs_json,
            log_sha256,
            source_refs_json,
            redacted,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
          ON CONFLICT (tenant_ref, verification_ref) DO UPDATE SET
            change_ref = excluded.change_ref,
            repository_ref = excluded.repository_ref,
            base_ref = excluded.base_ref,
            base_head = excluded.base_head,
            head_ref = excluded.head_ref,
            head_head = excluded.head_head,
            packfile_ref = excluded.packfile_ref,
            packfile_sha256 = excluded.packfile_sha256,
            executor_identity_ref = excluded.executor_identity_ref,
            command_ref = excluded.command_ref,
            command_args_json = excluded.command_args_json,
            exit_code = excluded.exit_code,
            verdict = excluded.verdict,
            started_at = excluded.started_at,
            completed_at = excluded.completed_at,
            artifact_refs_json = excluded.artifact_refs_json,
            log_sha256 = excluded.log_sha256,
            source_refs_json = excluded.source_refs_json
        `,
      )
      .bind(
        decoded.tenant_ref,
        decoded.verification_ref,
        decoded.change_ref,
        decoded.repository_ref,
        decoded.base_ref,
        decoded.base_head,
        decoded.head_ref,
        decoded.head_head,
        decoded.packfile_ref,
        decoded.packfile_sha256,
        decoded.executor_identity_ref,
        decoded.command_ref,
        jsonArray(decoded.command_args),
        decoded.exit_code,
        decoded.verdict,
        decoded.started_at,
        decoded.completed_at,
        jsonArray(decoded.artifact_refs),
        decoded.log_sha256,
        jsonArray(decoded.source_refs),
        createdAt,
      )
      .run()

    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_verification_receipts
          WHERE tenant_ref = ? AND verification_ref = ?
        `,
      )
      .bind(decoded.tenant_ref, decoded.verification_ref)
      .first<ForgeVerificationReceiptRow>()

    return verificationReceiptFromRow(
      rowOrFail(row, 'forge verification receipt'),
    )
  },

  async listVerificationReceipts(tenantRef, limit, changeRef) {
    const rows =
      changeRef === undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_verification_receipts
                WHERE tenant_ref = ?
                ORDER BY completed_at DESC, verification_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, limitRows(limit))
            .all<ForgeVerificationReceiptRow>()
        : await db
            .prepare(
              `
                SELECT *
                FROM forge_verification_receipts
                WHERE tenant_ref = ? AND change_ref = ?
                ORDER BY completed_at DESC, verification_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, changeRef, limitRows(limit))
            .all<ForgeVerificationReceiptRow>()

    return rows.results.map(verificationReceiptFromRow)
  },

  async recordPromotionDecisionReceipt(receipt, createdAt) {
    const decoded = decodeForgePromotionDecisionReceipt(receipt)
    await db
      .prepare(
        `
          INSERT INTO forge_promotion_decisions (
            tenant_ref,
            promotion_ref,
            queue_ref,
            change_ref,
            decision,
            base_head,
            candidate_head,
            promoted_head,
            verification_ref,
            gate_refs_json,
            blocker_refs_json,
            decided_by_ref,
            decided_at,
            source_refs_json,
            redacted,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
          ON CONFLICT (tenant_ref, promotion_ref) DO UPDATE SET
            queue_ref = excluded.queue_ref,
            change_ref = excluded.change_ref,
            decision = excluded.decision,
            base_head = excluded.base_head,
            candidate_head = excluded.candidate_head,
            promoted_head = excluded.promoted_head,
            verification_ref = excluded.verification_ref,
            gate_refs_json = excluded.gate_refs_json,
            blocker_refs_json = excluded.blocker_refs_json,
            decided_by_ref = excluded.decided_by_ref,
            decided_at = excluded.decided_at,
            source_refs_json = excluded.source_refs_json
        `,
      )
      .bind(
        decoded.tenant_ref,
        decoded.promotion_ref,
        decoded.queue_ref,
        decoded.change_ref,
        decoded.decision,
        decoded.base_head,
        decoded.candidate_head,
        decoded.promoted_head,
        decoded.verification_ref,
        jsonArray(decoded.gate_refs),
        jsonArray(decoded.blocker_refs),
        decoded.decided_by_ref,
        decoded.decided_at,
        jsonArray(decoded.source_refs),
        createdAt,
      )
      .run()

    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_promotion_decisions
          WHERE tenant_ref = ? AND promotion_ref = ?
        `,
      )
      .bind(decoded.tenant_ref, decoded.promotion_ref)
      .first<ForgePromotionDecisionReceiptRow>()

    return promotionDecisionReceiptFromRow(
      rowOrFail(row, 'forge promotion decision receipt'),
    )
  },

  async listPromotionDecisionReceipts(tenantRef, limit, changeRef) {
    const rows =
      changeRef === undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_promotion_decisions
                WHERE tenant_ref = ?
                ORDER BY decided_at DESC, promotion_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, limitRows(limit))
            .all<ForgePromotionDecisionReceiptRow>()
        : await db
            .prepare(
              `
                SELECT *
                FROM forge_promotion_decisions
                WHERE tenant_ref = ? AND change_ref = ?
                ORDER BY decided_at DESC, promotion_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, changeRef, limitRows(limit))
            .all<ForgePromotionDecisionReceiptRow>()

    return rows.results.map(promotionDecisionReceiptFromRow)
  },

  async readPromotionDecisionReceipt(tenantRef, promotionRef) {
    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_promotion_decisions
          WHERE tenant_ref = ? AND promotion_ref = ?
        `,
      )
      .bind(tenantRef, promotionRef)
      .first<ForgePromotionDecisionReceiptRow>()

    return row === null ? undefined : promotionDecisionReceiptFromRow(row)
  },

  async recordGitHubMirrorReceipt(receipt, createdAt) {
    const decoded = decodeForgeGitHubMirrorReceipt(receipt)
    await db
      .prepare(
        `
          INSERT INTO forge_github_mirror_receipts (
            tenant_ref,
            mirror_ref,
            promotion_ref,
            source_canonical_ref,
            destination_github_ref,
            repository_ref,
            github_repository,
            commit_id,
            status,
            attempted_at,
            mirrored_at,
            refusal_reason,
            error_reason,
            source_refs_json,
            redacted,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
          ON CONFLICT (
            tenant_ref,
            promotion_ref,
            source_canonical_ref,
            destination_github_ref,
            commit_id
          ) DO UPDATE SET
            mirror_ref = excluded.mirror_ref,
            repository_ref = excluded.repository_ref,
            github_repository = excluded.github_repository,
            status = excluded.status,
            attempted_at = excluded.attempted_at,
            mirrored_at = excluded.mirrored_at,
            refusal_reason = excluded.refusal_reason,
            error_reason = excluded.error_reason,
            source_refs_json = excluded.source_refs_json,
            updated_at = excluded.updated_at
        `,
      )
      .bind(
        decoded.tenant_ref,
        decoded.mirror_ref,
        decoded.promotion_ref,
        decoded.source_canonical_ref,
        decoded.destination_github_ref,
        decoded.repository_ref,
        decoded.github_repository,
        decoded.commit_id,
        decoded.status,
        decoded.attempted_at,
        decoded.mirrored_at,
        decoded.refusal_reason,
        decoded.error_reason,
        jsonArray(decoded.source_refs),
        createdAt,
        createdAt,
      )
      .run()

    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_github_mirror_receipts
          WHERE tenant_ref = ?
            AND promotion_ref = ?
            AND source_canonical_ref = ?
            AND destination_github_ref = ?
            AND commit_id = ?
        `,
      )
      .bind(
        decoded.tenant_ref,
        decoded.promotion_ref,
        decoded.source_canonical_ref,
        decoded.destination_github_ref,
        decoded.commit_id,
      )
      .first<ForgeGitHubMirrorReceiptRow>()

    return githubMirrorReceiptFromRow(
      rowOrFail(row, 'forge GitHub mirror receipt'),
    )
  },

  async listGitHubMirrorReceipts(tenantRef, limit, promotionRef) {
    const rows =
      promotionRef === undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_github_mirror_receipts
                WHERE tenant_ref = ?
                ORDER BY attempted_at DESC, mirror_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, limitRows(limit))
            .all<ForgeGitHubMirrorReceiptRow>()
        : await db
            .prepare(
              `
                SELECT *
                FROM forge_github_mirror_receipts
                WHERE tenant_ref = ? AND promotion_ref = ?
                ORDER BY attempted_at DESC, mirror_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, promotionRef, limitRows(limit))
            .all<ForgeGitHubMirrorReceiptRow>()

    return rows.results.map(githubMirrorReceiptFromRow)
  },
})

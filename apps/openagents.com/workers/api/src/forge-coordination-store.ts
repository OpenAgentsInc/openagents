import {
  decodeForgeCoordinationIssueRow,
  decodeForgeCoordinationPrRow,
  decodeForgeCoordinationStatusRow,
  decodeForgeDispatchLeaseRow,
  decodeForgeMergeQueueLedgerRow,
  forgeNip34StatusKindForState,
  type ForgeCoordinationChangeState,
  type ForgeCoordinationIssueRow,
  type ForgeCoordinationIssueState,
  type ForgeCoordinationPrRow,
  type ForgeCoordinationStatusRow,
  type ForgeCoordinationStatusState,
  type ForgeDispatchLeaseRow,
  type ForgeMergeQueueLedgerRow,
  type ForgeMergeQueueLedgerState,
} from '@openagentsinc/forge-protocol'

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
  upsertChange: (input: ForgeCoordinationChangeInput) => Promise<ForgeCoordinationPrRow>
  recordStatus: (input: ForgeCoordinationStatusInput) => Promise<ForgeCoordinationStatusRow>
  acquireDispatchLease: (input: ForgeDispatchLeaseInput) => Promise<ForgeDispatchLeaseAcquireResult>
  readActiveDispatchLease: (
    tenantRef: string,
    workRef: string,
  ) => Promise<ForgeDispatchLeaseRow | undefined>
  recordMergeQueueLedger: (input: ForgeMergeQueueLedgerInput) => Promise<ForgeMergeQueueLedgerRow>
  readLatestMergeQueueLedger: (
    tenantRef: string,
  ) => Promise<ForgeMergeQueueLedgerRow | undefined>
}>

const jsonArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...values])

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
})

import {
  decodeForgeGitHubMirrorReceipt,
  type ForgeGitHubMirrorReceipt,
} from '@openagentsinc/forge-protocol'

import { parseJsonStringArray } from './json-boundary'

export type ForgeGitHubMirrorReceiptInput = Omit<
  ForgeGitHubMirrorReceipt,
  'attempt_count' | 'schema'
>

export type ForgeGitHubMirrorListInput = Readonly<{
  limit: number
  promotionRef?: string | undefined
  status?: ForgeGitHubMirrorReceipt['status'] | undefined
}>

export type ForgeGitHubMirrorStore = Readonly<{
  listReceipts: (
    tenantRef: string,
    input: ForgeGitHubMirrorListInput,
  ) => Promise<ReadonlyArray<ForgeGitHubMirrorReceipt>>
  readReceiptForPromotion: (
    tenantRef: string,
    promotionRef: string,
    destinationGithubRepository: string,
    destinationGithubRef: string,
  ) => Promise<ForgeGitHubMirrorReceipt | undefined>
  recordReceipt: (
    input: ForgeGitHubMirrorReceiptInput,
  ) => Promise<ForgeGitHubMirrorReceipt>
}>

type ForgeGitHubMirrorReceiptRow = Readonly<{
  tenant_ref: string
  mirror_ref: string
  promotion_ref: string
  change_ref: string
  repository_ref: string
  source_canonical_ref: string
  destination_github_repository: string
  destination_github_ref: string
  commit_id: string
  status: string
  attempt_count: number
  first_attempted_at: string
  last_attempted_at: string
  completed_at: string | null
  refusal_reason: string | null
  error_reason: string | null
  source_refs_json: string
  redacted: number | boolean
}>

class ForgeGitHubMirrorStoreInvariantError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeGitHubMirrorStoreInvariantError'
  }
}

const limitRows = (limit: number): number =>
  Math.min(Math.max(Math.trunc(limit), 1), 100)

const jsonArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...values])

const receiptFromRow = (
  row: ForgeGitHubMirrorReceiptRow,
): ForgeGitHubMirrorReceipt =>
  decodeForgeGitHubMirrorReceipt({
    schema: 'openagents.forge.github_mirror.receipt.v0.1',
    tenant_ref: row.tenant_ref,
    mirror_ref: row.mirror_ref,
    promotion_ref: row.promotion_ref,
    change_ref: row.change_ref,
    repository_ref: row.repository_ref,
    source_canonical_ref: row.source_canonical_ref,
    destination_github_repository: row.destination_github_repository,
    destination_github_ref: row.destination_github_ref,
    commit_id: row.commit_id,
    status: row.status,
    attempt_count: row.attempt_count,
    first_attempted_at: row.first_attempted_at,
    last_attempted_at: row.last_attempted_at,
    completed_at: row.completed_at,
    refusal_reason: row.refusal_reason,
    error_reason: row.error_reason,
    source_refs: parseJsonStringArray(row.source_refs_json),
    redacted: row.redacted === true || row.redacted === 1,
  })

const readByMirrorRef = async (
  db: D1Database,
  tenantRef: string,
  mirrorRef: string,
): Promise<ForgeGitHubMirrorReceipt> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_github_mirror_receipts
        WHERE tenant_ref = ? AND mirror_ref = ?
      `,
    )
    .bind(tenantRef, mirrorRef)
    .first<ForgeGitHubMirrorReceiptRow>()

  if (row === null) {
    throw new ForgeGitHubMirrorStoreInvariantError(
      'forge GitHub mirror receipt was not persisted',
    )
  }

  return receiptFromRow(row)
}

export const makeD1ForgeGitHubMirrorStore = (
  db: D1Database,
): ForgeGitHubMirrorStore => ({
  async listReceipts(tenantRef, input) {
    const limit = limitRows(input.limit)
    const rows =
      input.promotionRef !== undefined
        ? await db
            .prepare(
              `
                SELECT *
                FROM forge_github_mirror_receipts
                WHERE tenant_ref = ? AND promotion_ref = ?
                ORDER BY updated_at DESC, mirror_ref DESC
                LIMIT ?
              `,
            )
            .bind(tenantRef, input.promotionRef, limit)
            .all<ForgeGitHubMirrorReceiptRow>()
        : input.status !== undefined
          ? await db
              .prepare(
                `
                  SELECT *
                  FROM forge_github_mirror_receipts
                  WHERE tenant_ref = ? AND status = ?
                  ORDER BY updated_at DESC, mirror_ref DESC
                  LIMIT ?
                `,
              )
              .bind(tenantRef, input.status, limit)
              .all<ForgeGitHubMirrorReceiptRow>()
          : await db
              .prepare(
                `
                  SELECT *
                  FROM forge_github_mirror_receipts
                  WHERE tenant_ref = ?
                  ORDER BY updated_at DESC, mirror_ref DESC
                  LIMIT ?
                `,
              )
              .bind(tenantRef, limit)
              .all<ForgeGitHubMirrorReceiptRow>()

    return rows.results.map(receiptFromRow)
  },

  async readReceiptForPromotion(
    tenantRef,
    promotionRef,
    destinationGithubRepository,
    destinationGithubRef,
  ) {
    const row = await db
      .prepare(
        `
          SELECT *
          FROM forge_github_mirror_receipts
          WHERE tenant_ref = ?
            AND promotion_ref = ?
            AND destination_github_repository = ?
            AND destination_github_ref = ?
          ORDER BY updated_at DESC, mirror_ref DESC
          LIMIT 1
        `,
      )
      .bind(
        tenantRef,
        promotionRef,
        destinationGithubRepository,
        destinationGithubRef,
      )
      .first<ForgeGitHubMirrorReceiptRow>()

    return row === null ? undefined : receiptFromRow(row)
  },

  async recordReceipt(input) {
    const decoded = decodeForgeGitHubMirrorReceipt({
      schema: 'openagents.forge.github_mirror.receipt.v0.1',
      ...input,
      attempt_count: 1,
    })

    await db
      .prepare(
        `
          INSERT INTO forge_github_mirror_receipts (
            tenant_ref,
            mirror_ref,
            promotion_ref,
            change_ref,
            repository_ref,
            source_canonical_ref,
            destination_github_repository,
            destination_github_ref,
            commit_id,
            status,
            attempt_count,
            first_attempted_at,
            last_attempted_at,
            completed_at,
            refusal_reason,
            error_reason,
            source_refs_json,
            redacted,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, 1, ?, ?)
          ON CONFLICT (tenant_ref, mirror_ref) DO UPDATE SET
            change_ref = excluded.change_ref,
            repository_ref = excluded.repository_ref,
            source_canonical_ref = excluded.source_canonical_ref,
            destination_github_repository = excluded.destination_github_repository,
            destination_github_ref = excluded.destination_github_ref,
            commit_id = excluded.commit_id,
            status = excluded.status,
            attempt_count = forge_github_mirror_receipts.attempt_count + 1,
            last_attempted_at = excluded.last_attempted_at,
            completed_at = excluded.completed_at,
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
        decoded.change_ref,
        decoded.repository_ref,
        decoded.source_canonical_ref,
        decoded.destination_github_repository,
        decoded.destination_github_ref,
        decoded.commit_id,
        decoded.status,
        decoded.first_attempted_at,
        decoded.last_attempted_at,
        decoded.completed_at,
        decoded.refusal_reason,
        decoded.error_reason,
        jsonArray(decoded.source_refs),
        decoded.first_attempted_at,
        decoded.last_attempted_at,
      )
      .run()

    return readByMirrorRef(db, decoded.tenant_ref, decoded.mirror_ref)
  },
})

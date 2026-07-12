import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountAuthGrantRow,
  ProviderAccountEventRecord,
  ProviderAccountRepository,
} from './provider-account-domain'
import { toGrantRecord } from './provider-account-domain'
import { ProviderGrantNotIssued } from './provider-account-errors'

export type ProviderGrantPostgresQuery = (
  text: string,
  params: ReadonlyArray<unknown>,
) => Promise<ReadonlyArray<Record<string, unknown>>>

/**
 * Replace only the grant-consumption operations of the normal repository.
 * The UPDATE and audit INSERT share one Postgres statement/transaction, so a
 * grant can cross issued -> used exactly once in the same database that the
 * managed-runtime scheduler and usage receipt gate read.
 */
export const makeAuthoritativePostgresProviderGrantRepository = (
  base: ProviderAccountRepository,
  query: ProviderGrantPostgresQuery,
): ProviderAccountRepository => ({
  ...base,
  findGrantByRef: async grantRef => {
    const rows = await query(
      `SELECT * FROM provider_account_auth_grants WHERE grant_ref = $1 LIMIT 1`,
      [grantRef],
    )
    const row = rows[0]
    return row === undefined
      ? undefined
      : toGrantRecord(row as ProviderAccountAuthGrantRow)
  },
  markGrantUsed: async (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ) => {
    const rows = await query(
      `WITH claimed AS (
         UPDATE provider_account_auth_grants
            SET status = 'used', used_at = $1, updated_at = $2
          WHERE id = $3 AND status = 'issued'
          RETURNING *
       ), audited AS (
         INSERT INTO provider_account_events
           (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
            workroom_id, runner_session_id, kind, summary, source_refs_json,
            evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
         SELECT $4, $5, $3, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
           FROM claimed
         RETURNING id
       )
       SELECT claimed.* FROM claimed JOIN audited ON TRUE`,
      [
        grant.usedAt,
        grant.updatedAt,
        grant.id,
        event.id,
        event.providerAccountId,
        event.userId,
        event.teamId,
        event.threadId,
        event.workroomId,
        event.runnerSessionId,
        event.kind,
        event.summary,
        event.sourceRefsJson,
        event.evidenceRefsJson,
        event.targetRef,
        event.metadataJson,
        event.actorId,
        event.createdAt,
      ],
    )
    const row = rows[0]
    if (row === undefined) {
      throw new ProviderGrantNotIssued({ message: 'Grant is not issued.' })
    }
    return toGrantRecord(row as ProviderAccountAuthGrantRow)
  },
})

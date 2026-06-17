import { Effect } from 'effect'

import {
  type CustomerOneCohortPrivateRow,
  type CustomerOneCohortState,
} from './customer-one-cohort-projection'
import { parseJsonStringArray } from './json-boundary'

export type CustomerOneCohortSourceStore = Readonly<{
  listRows: () => Effect.Effect<ReadonlyArray<CustomerOneCohortPrivateRow>>
}>

export type CustomerOneCohortRowStore = CustomerOneCohortSourceStore &
  Readonly<{
    upsertRow: (row: CustomerOneCohortPrivateRow) => Effect.Effect<void>
  }>

type CustomerOneCohortD1Row = Readonly<{
  artifact_ref: string | null
  blocker_refs_json: string
  candidate_ref: string | null
  caveat_refs_json: string
  completion_bundle_ref: string | null
  invite_ref: string | null
  privacy_review_ref: string | null
  review_ref: string | null
  routing_ref: string | null
  run_ref: string | null
  state: string
  team_cohort_ref: string
  template_ref: string | null
  updated_at: string
  verification_ref: string | null
  vertical_ref: string | null
  workspace_ref: string | null
}>

const nullable = (value: string | undefined): string | null => value ?? null

const rowFromD1 = (
  row: CustomerOneCohortD1Row,
): CustomerOneCohortPrivateRow => ({
  ...(row.artifact_ref === null ? {} : { artifactRef: row.artifact_ref }),
  blockerRefs: parseJsonStringArray(row.blocker_refs_json),
  ...(row.candidate_ref === null ? {} : { candidateRef: row.candidate_ref }),
  caveatRefs: parseJsonStringArray(row.caveat_refs_json),
  ...(row.completion_bundle_ref === null
    ? {}
    : { completionBundleRef: row.completion_bundle_ref }),
  ...(row.invite_ref === null ? {} : { inviteRef: row.invite_ref }),
  ...(row.privacy_review_ref === null
    ? {}
    : { privacyReviewRef: row.privacy_review_ref }),
  ...(row.review_ref === null ? {} : { reviewRef: row.review_ref }),
  ...(row.routing_ref === null ? {} : { routingRef: row.routing_ref }),
  ...(row.run_ref === null ? {} : { runRef: row.run_ref }),
  state: row.state as CustomerOneCohortState,
  teamCohortRef: row.team_cohort_ref,
  ...(row.template_ref === null ? {} : { templateRef: row.template_ref }),
  updatedAt: row.updated_at,
  ...(row.verification_ref === null
    ? {}
    : { verificationRef: row.verification_ref }),
  ...(row.vertical_ref === null ? {} : { verticalRef: row.vertical_ref }),
  ...(row.workspace_ref === null ? {} : { workspaceRef: row.workspace_ref }),
})

export const makeD1CustomerOneCohortRowStore = (
  db: D1Database,
): CustomerOneCohortRowStore => ({
  listRows: () =>
    Effect.promise(async () => {
      const rows = await db
        .prepare(
          `SELECT
            artifact_ref,
            blocker_refs_json,
            candidate_ref,
            caveat_refs_json,
            completion_bundle_ref,
            invite_ref,
            privacy_review_ref,
            review_ref,
            routing_ref,
            run_ref,
            state,
            team_cohort_ref,
            template_ref,
            updated_at,
            verification_ref,
            vertical_ref,
            workspace_ref
          FROM customer_one_cohort_rows
          ORDER BY team_cohort_ref ASC`,
        )
        .all<CustomerOneCohortD1Row>()

      return (rows.results ?? []).map(rowFromD1)
    }),
  upsertRow: row =>
    Effect.promise(async () => {
      await db
        .prepare(
          `INSERT INTO customer_one_cohort_rows (
            team_cohort_ref,
            state,
            candidate_ref,
            invite_ref,
            vertical_ref,
            template_ref,
            workspace_ref,
            routing_ref,
            run_ref,
            artifact_ref,
            review_ref,
            verification_ref,
            completion_bundle_ref,
            privacy_review_ref,
            blocker_refs_json,
            caveat_refs_json,
            updated_at,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT(team_cohort_ref) DO UPDATE SET
            state = excluded.state,
            candidate_ref = excluded.candidate_ref,
            invite_ref = excluded.invite_ref,
            vertical_ref = excluded.vertical_ref,
            template_ref = excluded.template_ref,
            workspace_ref = excluded.workspace_ref,
            routing_ref = excluded.routing_ref,
            run_ref = excluded.run_ref,
            artifact_ref = excluded.artifact_ref,
            review_ref = excluded.review_ref,
            verification_ref = excluded.verification_ref,
            completion_bundle_ref = excluded.completion_bundle_ref,
            privacy_review_ref = excluded.privacy_review_ref,
            blocker_refs_json = excluded.blocker_refs_json,
            caveat_refs_json = excluded.caveat_refs_json,
            updated_at = excluded.updated_at`,
        )
        .bind(
          row.teamCohortRef,
          row.state,
          nullable(row.candidateRef),
          nullable(row.inviteRef),
          nullable(row.verticalRef),
          nullable(row.templateRef),
          nullable(row.workspaceRef),
          nullable(row.routingRef),
          nullable(row.runRef),
          nullable(row.artifactRef),
          nullable(row.reviewRef),
          nullable(row.verificationRef),
          nullable(row.completionBundleRef),
          nullable(row.privacyReviewRef),
          JSON.stringify(row.blockerRefs ?? []),
          JSON.stringify(row.caveatRefs ?? []),
          row.updatedAt,
          row.updatedAt,
        )
        .run()
    }),
})

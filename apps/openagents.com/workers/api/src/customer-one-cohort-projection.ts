import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const CUSTOMER_ONE_COHORT_PROJECTION_VERSION =
  'customer-one-cohort-projection:v1' as const

export const CustomerOneCohortEndpoint = '/api/public/customer-one-cohort'

const CUSTOMER_ONE_COHORT_PUBLIC_COLLECTION =
  'customer_one_cohort_public_projection'

const CUSTOMER_ONE_COHORT_TARGET = {
  maximumTargetTeams: 5,
  minimumCompletedTeams: 3,
} as const

const CUSTOMER_ONE_COHORT_REBUILDS_ON = [
  'cohort_row_written',
  'privacy_review_recorded',
] as const

const CUSTOMER_ONE_COHORT_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /raw[_ -]prompt/i,
  /raw[_ -]shell/i,
  /shell[_ -]log/i,
  /stack[_ -]trace/i,
  /private[_ -]repo/i,
  /private[_ -]content/i,
  /provider[_ -]payload/i,
  /invoice/i,
  /payment[_ -]hash/i,
  /preimage/i,
  /mnemonic/i,
  /bearer/i,
  /oauth/i,
  /api[_ -]?key/i,
  /(?:^|[\s"':])\/Users\//,
  /(?:^|[\s"':])(?:git|ssh|https?):\/\//i,
  /git@/i,
  /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/i,
]

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const TEAM_COHORT_REF_PATTERN = /^cohort\.team\.[a-z0-9_-]+\.v[0-9]+$/

class CustomerOneCohortProjectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'CustomerOneCohortProjectionError'
  }
}

export const CustomerOneCohortState = S.Literals([
  'candidate',
  'invited',
  'workspace_seeded',
  'first_run_started',
  'delivery_reviewed',
  'loop_completed',
  'blocked',
  'deferred',
])
export type CustomerOneCohortState = typeof CustomerOneCohortState.Type

export const CustomerOneCohortCounts = S.Struct({
  blocked: S.Number,
  candidate: S.Number,
  deferred: S.Number,
  delivery_reviewed: S.Number,
  first_run_started: S.Number,
  invited: S.Number,
  loop_completed: S.Number,
  workspace_seeded: S.Number,
})
export type CustomerOneCohortCounts = typeof CustomerOneCohortCounts.Type

export const CustomerOneCohortPrivateRow = S.Struct({
  artifactRef: S.optionalKey(S.String),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidateRef: S.optionalKey(S.String),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  completionBundleRef: S.optionalKey(S.String),
  inviteRef: S.optionalKey(S.String),
  privacyReviewRef: S.optionalKey(S.String),
  reviewRef: S.optionalKey(S.String),
  routingRef: S.optionalKey(S.String),
  runRef: S.optionalKey(S.String),
  state: CustomerOneCohortState,
  teamCohortRef: S.String,
  templateRef: S.optionalKey(S.String),
  updatedAt: S.String,
  verificationRef: S.optionalKey(S.String),
  verticalRef: S.optionalKey(S.String),
  workspaceRef: S.optionalKey(S.String),
})
export type CustomerOneCohortPrivateRow =
  typeof CustomerOneCohortPrivateRow.Type

export const CustomerOneCohortProjectionRow = S.Struct({
  artifactRef: S.optionalKey(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  completionBundleRef: S.optionalKey(S.String),
  countsTowardD3Completion: S.Boolean,
  displayLabel: S.String,
  privacyReviewRef: S.optionalKey(S.String),
  reviewRef: S.optionalKey(S.String),
  routingRef: S.optionalKey(S.String),
  runRef: S.optionalKey(S.String),
  state: CustomerOneCohortState,
  teamCohortRef: S.String,
  templateRef: S.optionalKey(S.String),
  verificationRef: S.optionalKey(S.String),
  verticalRef: S.optionalKey(S.String),
  workspaceRef: S.optionalKey(S.String),
})
export type CustomerOneCohortProjectionRow =
  typeof CustomerOneCohortProjectionRow.Type

export const CustomerOneCohortGate = S.Struct({
  reasonRefs: S.Array(S.String),
  state: S.Literals(['blocked', 'ready']),
})
export type CustomerOneCohortGate = typeof CustomerOneCohortGate.Type

export const CustomerOneCohortProjection = S.Struct({
  authority: S.Literal('evidence_only'),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  cohortProjectionVersion: S.Literal(CUSTOMER_ONE_COHORT_PROJECTION_VERSION),
  counts: CustomerOneCohortCounts,
  gate: CustomerOneCohortGate,
  generatedAt: S.String,
  rows: S.Array(CustomerOneCohortProjectionRow),
  staleness: PublicProjectionStalenessContract,
  target: S.Struct({
    maximumTargetTeams: S.Number,
    minimumCompletedTeams: S.Number,
  }),
})
export type CustomerOneCohortProjection =
  typeof CustomerOneCohortProjection.Type

export type CustomerOneCohortProjectionInput = Readonly<{
  generatedAt: string
  rows: ReadonlyArray<CustomerOneCohortPrivateRow>
}>

export const decodeCustomerOneCohortPrivateRow = (
  value: unknown,
): CustomerOneCohortPrivateRow => {
  assertNoPrivateCohortMaterial(value, 'customer-one-cohort.row-input')

  const row = S.decodeUnknownSync(CustomerOneCohortPrivateRow)(value)

  projectCustomerOneCohort({
    generatedAt: row.updatedAt,
    rows: [row],
  })

  return row
}

const emptyCounts: CustomerOneCohortCounts = {
  blocked: 0,
  candidate: 0,
  deferred: 0,
  delivery_reviewed: 0,
  first_run_started: 0,
  invited: 0,
  loop_completed: 0,
  workspace_seeded: 0,
}

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(values),
]

const assertNoPrivateCohortMaterial = (
  value: unknown,
  context: string,
): void => {
  assertNoProviderSecretMaterial(value, context)

  const text = typeof value === 'string' ? value : (JSON.stringify(value) ?? '')

  if (CUSTOMER_ONE_COHORT_PRIVATE_MARKERS.some(marker => marker.test(text))) {
    throw new CustomerOneCohortProjectionError(
      `${context} contains private cohort material.`,
    )
  }
}

const safeRef = (field: string, value: string): string => {
  const trimmed = value.trim()
  assertNoPrivateCohortMaterial(trimmed, field)

  if (!SAFE_REF_PATTERN.test(trimmed)) {
    throw new CustomerOneCohortProjectionError(
      `${field} must be a public-safe cohort ref.`,
    )
  }

  return trimmed
}

const safeTeamCohortRef = (value: string): string => {
  const trimmed = safeRef('customer-one-cohort.teamCohortRef', value)

  if (!TEAM_COHORT_REF_PATTERN.test(trimmed)) {
    throw new CustomerOneCohortProjectionError(
      'teamCohortRef must be an opaque cohort.team.*.vN ref.',
    )
  }

  return trimmed
}

const safeOptionalRef = (
  field: string,
  value: string | undefined,
): string | undefined =>
  value === undefined ? undefined : safeRef(field, value)

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const completionBlockerRefs = (
  row: Readonly<{
    completionBundleRef: string | undefined
    privacyReviewRef: string | undefined
    state: CustomerOneCohortState
    teamCohortRef: string
  }>,
): ReadonlyArray<string> =>
  row.state === 'loop_completed'
    ? [
        ...(row.completionBundleRef === undefined
          ? [
              `customer-one-cohort-blocker:${row.teamCohortRef}:missing-completion-bundle`,
            ]
          : []),
        ...(row.privacyReviewRef === undefined
          ? [
              `customer-one-cohort-blocker:${row.teamCohortRef}:missing-privacy-review`,
            ]
          : []),
      ]
    : []

const projectRow = (
  row: CustomerOneCohortPrivateRow,
  index: number,
): CustomerOneCohortProjectionRow => {
  assertNoPrivateCohortMaterial(row, 'customer-one-cohort.row')

  const teamCohortRef = safeTeamCohortRef(row.teamCohortRef)
  const completionBundleRef = safeOptionalRef(
    'customer-one-cohort.completionBundleRef',
    row.completionBundleRef,
  )
  const privacyReviewRef = safeOptionalRef(
    'customer-one-cohort.privacyReviewRef',
    row.privacyReviewRef,
  )
  const blockerRefs = unique([
    ...safeRefs('customer-one-cohort.blockerRefs', row.blockerRefs),
    ...completionBlockerRefs({
      completionBundleRef,
      privacyReviewRef,
      state: row.state,
      teamCohortRef,
    }),
  ])
  const artifactRef = safeOptionalRef(
    'customer-one-cohort.artifactRef',
    row.artifactRef,
  )
  const reviewRef = safeOptionalRef(
    'customer-one-cohort.reviewRef',
    row.reviewRef,
  )
  const routingRef = safeOptionalRef(
    'customer-one-cohort.routingRef',
    row.routingRef,
  )
  const runRef = safeOptionalRef('customer-one-cohort.runRef', row.runRef)
  const templateRef = safeOptionalRef(
    'customer-one-cohort.templateRef',
    row.templateRef,
  )
  const verificationRef = safeOptionalRef(
    'customer-one-cohort.verificationRef',
    row.verificationRef,
  )
  const verticalRef = safeOptionalRef(
    'customer-one-cohort.verticalRef',
    row.verticalRef,
  )
  const workspaceRef = safeOptionalRef(
    'customer-one-cohort.workspaceRef',
    row.workspaceRef,
  )
  const countsTowardD3Completion =
    row.state === 'loop_completed' &&
    completionBundleRef !== undefined &&
    privacyReviewRef !== undefined

  return {
    ...(artifactRef === undefined ? {} : { artifactRef }),
    blockerRefs,
    caveatRefs: safeRefs('customer-one-cohort.caveatRefs', row.caveatRefs),
    ...(completionBundleRef === undefined ? {} : { completionBundleRef }),
    countsTowardD3Completion,
    displayLabel: `Team ${index + 1}`,
    ...(privacyReviewRef === undefined ? {} : { privacyReviewRef }),
    ...(reviewRef === undefined ? {} : { reviewRef }),
    ...(routingRef === undefined ? {} : { routingRef }),
    ...(runRef === undefined ? {} : { runRef }),
    state: row.state,
    teamCohortRef,
    ...(templateRef === undefined ? {} : { templateRef }),
    ...(verificationRef === undefined ? {} : { verificationRef }),
    ...(verticalRef === undefined ? {} : { verticalRef }),
    ...(workspaceRef === undefined ? {} : { workspaceRef }),
  }
}

const incrementCounts = (
  counts: CustomerOneCohortCounts,
  row: CustomerOneCohortProjectionRow,
): CustomerOneCohortCounts => ({
  blocked: counts.blocked + (row.state === 'blocked' ? 1 : 0),
  candidate: counts.candidate + (row.state === 'candidate' ? 1 : 0),
  deferred: counts.deferred + (row.state === 'deferred' ? 1 : 0),
  delivery_reviewed:
    counts.delivery_reviewed + (row.state === 'delivery_reviewed' ? 1 : 0),
  first_run_started:
    counts.first_run_started + (row.state === 'first_run_started' ? 1 : 0),
  invited: counts.invited + (row.state === 'invited' ? 1 : 0),
  loop_completed:
    counts.loop_completed + (row.countsTowardD3Completion ? 1 : 0),
  workspace_seeded:
    counts.workspace_seeded + (row.state === 'workspace_seeded' ? 1 : 0),
})

const countRows = (
  rows: ReadonlyArray<CustomerOneCohortProjectionRow>,
): CustomerOneCohortCounts =>
  rows.reduce((counts, row) => incrementCounts(counts, row), emptyCounts)

const gateForCounts = (
  counts: CustomerOneCohortCounts,
): CustomerOneCohortGate =>
  counts.loop_completed >= CUSTOMER_ONE_COHORT_TARGET.minimumCompletedTeams
    ? {
        reasonRefs: [],
        state: 'ready',
      }
    : {
        reasonRefs: ['reason.customer_one.cohort_completion_bundles_missing'],
        state: 'blocked',
      }

export const projectCustomerOneCohort = (
  input: CustomerOneCohortProjectionInput,
): CustomerOneCohortProjection => {
  assertNoPrivateCohortMaterial(input, 'customer-one-cohort.input')

  const rows = input.rows.map(projectRow)
  const counts = countRows(rows)
  const gate = gateForCounts(counts)
  const blockerRefs = unique([
    ...gate.reasonRefs,
    ...rows.flatMap(row => row.blockerRefs),
  ])
  const projection: CustomerOneCohortProjection = {
    authority: 'evidence_only',
    blockerRefs,
    caveatRefs: unique(rows.flatMap(row => row.caveatRefs)),
    cohortProjectionVersion: CUSTOMER_ONE_COHORT_PROJECTION_VERSION,
    counts,
    gate,
    generatedAt: input.generatedAt,
    rows,
    staleness: liveAtReadStaleness(CUSTOMER_ONE_COHORT_REBUILDS_ON),
    target: CUSTOMER_ONE_COHORT_TARGET,
  }

  assertNoPrivateCohortMaterial(
    projection,
    CUSTOMER_ONE_COHORT_PUBLIC_COLLECTION,
  )

  return projection
}

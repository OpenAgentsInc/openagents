import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  PublicClaimStateProjection,
  publicClaimStateProjection,
} from './public-claim-state'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const PUBLIC_ADJUTANT_ACTIVITY_STALENESS = liveAtReadStaleness([
  'adjutant_assignment_changed',
  'software_order_changed',
  'site_project_changed',
  'site_deployment_changed',
])

export class PublicAdjutantActivityMilestone extends S.Class<PublicAdjutantActivityMilestone>(
  'PublicAdjutantActivityMilestone',
)({
  id: S.String,
  kind: S.Literals(['order', 'site']),
  stage: S.Literals([
    'queued',
    'running',
    'reviewing',
    'deployed',
    'waiting_for_input',
    'unavailable',
  ]),
  label: S.String,
  summary: S.String,
  status: S.String,
  publicRef: S.String,
  siteSlug: S.NullOr(S.String),
  siteTitle: S.NullOr(S.String),
  siteUrl: S.NullOr(S.String),
  claimState: PublicClaimStateProjection,
  updatedAt: S.String,
}) {}

export class PublicAdjutantDeployedSite extends S.Class<PublicAdjutantDeployedSite>(
  'PublicAdjutantDeployedSite',
)({
  slug: S.String,
  title: S.String,
  url: S.String,
  status: S.String,
  publicRef: S.String,
  claimState: PublicClaimStateProjection,
  updatedAt: S.String,
}) {}

export class PublicAdjutantActivity extends S.Class<PublicAdjutantActivity>(
  'PublicAdjutantActivity',
)({
  generatedAt: S.String,
  milestones: S.Array(PublicAdjutantActivityMilestone),
  deployedSites: S.Array(PublicAdjutantDeployedSite),
  staleness: PublicProjectionStalenessContract,
}) {}

export class PublicAdjutantActivityStorageError extends S.TaggedErrorClass<PublicAdjutantActivityStorageError>()(
  'PublicAdjutantActivityStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class PublicAdjutantActivityUnsafe extends S.TaggedErrorClass<PublicAdjutantActivityUnsafe>()(
  'PublicAdjutantActivityUnsafe',
  {},
) {}

type PublicAdjutantActivityRow = Readonly<{
  active_deployment_status: string | null
  active_deployment_updated_at: string | null
  active_deployment_url: string | null
  assignment_id: string
  assignment_kind: string
  assignment_status: string
  assignment_updated_at: string
  order_status: string | null
  site_slug: string | null
  site_status: string | null
  site_title: string | null
  software_order_id: string | null
}>

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, PublicAdjutantActivityStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new PublicAdjutantActivityStorageError({ operation, error }),
  })

const rowStage = (
  row: PublicAdjutantActivityRow,
): PublicAdjutantActivityMilestone['stage'] => {
  if (
    row.active_deployment_status === 'active' &&
    row.active_deployment_url !== null
  ) {
    return 'deployed'
  }

  if (row.order_status === 'needs_customer_input') {
    return 'waiting_for_input'
  }

  if (row.assignment_status === 'running') {
    return 'running'
  }

  if (
    row.assignment_status === 'queued' ||
    row.assignment_status === 'preflight_pending' ||
    row.assignment_status === 'draft'
  ) {
    return 'queued'
  }

  if (
    row.assignment_status === 'blocked' ||
    row.assignment_status === 'canceled' ||
    row.order_status === 'declined' ||
    row.order_status === 'unavailable'
  ) {
    return 'unavailable'
  }

  return 'reviewing'
}

const rowPublicRef = (row: PublicAdjutantActivityRow): string =>
  row.site_slug === null
    ? row.software_order_id === null
      ? `assignment:${row.assignment_id}`
      : `order:${row.software_order_id}`
    : `site:${row.site_slug}`

const milestoneLabel = (
  stage: PublicAdjutantActivityMilestone['stage'],
): string =>
  stage === 'deployed'
    ? 'Public Site deployed'
    : stage === 'running'
      ? 'Autopilot running'
      : stage === 'queued'
        ? 'Autopilot queued'
        : stage === 'waiting_for_input'
          ? 'Input requested'
          : stage === 'unavailable'
            ? 'Autopilot unavailable'
            : 'Review in progress'

const milestoneSummary = (
  row: PublicAdjutantActivityRow,
  stage: PublicAdjutantActivityMilestone['stage'],
): string => {
  const siteName = row.site_title ?? row.site_slug ?? 'public Site'

  if (stage === 'deployed') {
    return `${siteName} is live.`
  }

  if (stage === 'running') {
    return 'Autopilot is building a public Site version.'
  }

  if (stage === 'queued') {
    return 'Autopilot accepted a public Site order.'
  }

  if (stage === 'waiting_for_input') {
    return 'Autopilot is waiting for customer input.'
  }

  if (stage === 'unavailable') {
    return 'Autopilot cannot continue this public order right now.'
  }

  return 'OpenAgents is reviewing public Site progress.'
}

const milestoneClaimState = (
  row: PublicAdjutantActivityRow,
  stage: PublicAdjutantActivityMilestone['stage'],
  publicRef: string,
) =>
  publicClaimStateProjection({
    desiredState:
      stage === 'deployed'
        ? 'verified'
        : stage === 'queued'
          ? 'planned'
          : 'measured',
    evidenceRefs: [
      publicRef,
      ...(row.active_deployment_status === 'active' &&
      row.active_deployment_url !== null
        ? [row.active_deployment_url]
        : []),
    ],
    kind: stage === 'deployed' ? 'site_url' : 'deployment',
  })

const milestoneFromRow = (
  row: PublicAdjutantActivityRow,
): PublicAdjutantActivityMilestone => {
  const stage = rowStage(row)
  const publicRef = rowPublicRef(row)

  return new PublicAdjutantActivityMilestone({
    id: row.assignment_id,
    kind: row.site_slug === null ? 'order' : 'site',
    stage,
    label: milestoneLabel(stage),
    summary: milestoneSummary(row, stage),
    status: row.site_status ?? row.order_status ?? row.assignment_status,
    publicRef,
    siteSlug: row.site_slug,
    siteTitle: row.site_title,
    siteUrl:
      row.active_deployment_status === 'active'
        ? row.active_deployment_url
        : null,
    claimState: milestoneClaimState(row, stage, publicRef),
    updatedAt: row.active_deployment_updated_at ?? row.assignment_updated_at,
  })
}

const deployedSitesFromMilestones = (
  milestones: ReadonlyArray<PublicAdjutantActivityMilestone>,
): ReadonlyArray<PublicAdjutantDeployedSite> => {
  const sites = new Map<string, PublicAdjutantDeployedSite>()

  for (const milestone of milestones) {
    if (
      milestone.stage !== 'deployed' ||
      milestone.siteSlug === null ||
      milestone.siteTitle === null ||
      milestone.siteUrl === null
    ) {
      continue
    }

    if (!sites.has(milestone.siteSlug)) {
      sites.set(
        milestone.siteSlug,
        new PublicAdjutantDeployedSite({
          slug: milestone.siteSlug,
          title: milestone.siteTitle,
          url: milestone.siteUrl,
          status: milestone.status,
          publicRef: milestone.publicRef,
          claimState: milestone.claimState,
          updatedAt: milestone.updatedAt,
        }),
      )
    }
  }

  return [...sites.values()]
}

export const publicAdjutantActivityFromRows = (
  rows: ReadonlyArray<PublicAdjutantActivityRow>,
  generatedAt: string,
): Effect.Effect<PublicAdjutantActivity, PublicAdjutantActivityUnsafe> => {
  const milestones = rows.map(milestoneFromRow)
  const activity = new PublicAdjutantActivity({
    generatedAt,
    milestones,
    deployedSites: deployedSitesFromMilestones(milestones),
    staleness: PUBLIC_ADJUTANT_ACTIVITY_STALENESS,
  })

  return containsProviderSecretMaterial(JSON.stringify(activity))
    ? Effect.fail(new PublicAdjutantActivityUnsafe())
    : Effect.succeed(activity)
}

export const publicAdjutantActivity = (
  db: D1Database,
  nowIso: () => string = currentIsoTimestamp,
): Effect.Effect<
  PublicAdjutantActivity,
  PublicAdjutantActivityStorageError | PublicAdjutantActivityUnsafe
> =>
  d1Effect('publicAdjutantActivity.list', () =>
    db
      .prepare(
        `SELECT adjutant_assignments.id AS assignment_id,
                adjutant_assignments.assignment_kind AS assignment_kind,
                adjutant_assignments.status AS assignment_status,
                adjutant_assignments.updated_at AS assignment_updated_at,
                software_orders.id AS software_order_id,
                software_orders.status AS order_status,
                site_projects.slug AS site_slug,
                site_projects.title AS site_title,
                site_projects.status AS site_status,
                active_deployments.url AS active_deployment_url,
                active_deployments.status AS active_deployment_status,
                active_deployments.updated_at AS active_deployment_updated_at
           FROM adjutant_assignments
           LEFT JOIN software_orders
             ON software_orders.id = adjutant_assignments.software_order_id
            AND software_orders.archived_at IS NULL
           LEFT JOIN site_projects
             ON site_projects.id = adjutant_assignments.site_id
            AND site_projects.archived_at IS NULL
           LEFT JOIN site_deployments AS active_deployments
             ON active_deployments.id = site_projects.active_deployment_id
            AND active_deployments.status = 'active'
          WHERE adjutant_assignments.agent_id = 'agent_adjutant'
            AND adjutant_assignments.visibility = 'public'
            AND adjutant_assignments.archived_at IS NULL
            AND (
              adjutant_assignments.software_order_id IS NULL
              OR software_orders.visibility = 'public'
            )
            AND (
              adjutant_assignments.site_id IS NULL
              OR (
                site_projects.visibility = 'public'
                AND site_projects.access_mode = 'public'
              )
            )
          ORDER BY COALESCE(active_deployments.updated_at, adjutant_assignments.updated_at) DESC
          LIMIT 20`,
      )
      .all<PublicAdjutantActivityRow>(),
  ).pipe(
    Effect.flatMap(result =>
      publicAdjutantActivityFromRows(result.results, nowIso()),
    ),
  )

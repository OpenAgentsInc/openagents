import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import type { AdjutantAssignment } from './adjutant-assignments'
import type { ExaSearchCategory } from './exa'

export const PublicSourceRefKind = S.Literals([
  'github_repository',
  'github_profile',
  'personal_site',
  'linkedin_profile',
  'x_profile',
  'generic_url',
])
export type PublicSourceRefKind = typeof PublicSourceRefKind.Type

export const PublicSourceRefStatus = S.Literals([
  'proposed',
  'approved',
  'rejected',
  'internal_only',
  'public_safe',
])
export type PublicSourceRefStatus = typeof PublicSourceRefStatus.Type

export const ExplicitPublicSourceRef = S.Struct({
  id: S.optionalKey(S.String),
  kind: PublicSourceRefKind,
  label: S.optionalKey(S.String),
  status: PublicSourceRefStatus,
  url: S.String,
})
export type ExplicitPublicSourceRef = typeof ExplicitPublicSourceRef.Type

export type AdjutantEnrichmentOrderContext = Readonly<{
  id: string
  repositoryDefaultBranch: string | null
  repositoryFullName: string | null
  repositoryHtmlUrl: string | null
  repositoryName: string | null
  repositoryOwner: string | null
  repositoryPrivate: boolean | null
  request: string
}>

export type AdjutantEnrichmentSiteContext = Readonly<{
  id: string
  slug: string
  sourceRepositoryName: string | null
  sourceRepositoryOwner: string | null
  sourceRepositoryProvider: 'github' | null
  sourceRepositoryRef: string | null
  title: string
}>

export const ExaEnrichmentTaskKind = S.Literals(['search', 'contents'])
export type ExaEnrichmentTaskKind = typeof ExaEnrichmentTaskKind.Type

export const ExaEnrichmentPlanTask = S.Struct({
  id: S.String,
  category: S.optionalKey(S.NullOr(S.String)),
  contentsMaxAgeHours: S.Number,
  includeDomains: S.Array(S.String),
  kind: ExaEnrichmentTaskKind,
  numResults: S.Number,
  query: S.String,
  reason: S.String,
  searchType: S.String,
  sourceCategory: S.String,
  sourceRefId: S.NullOr(S.String),
  urls: S.Array(S.String),
})
export type ExaEnrichmentPlanTask = typeof ExaEnrichmentPlanTask.Type

export const ExaEnrichmentBlockedSource = S.Struct({
  reason: S.String,
  sourceRefId: S.NullOr(S.String),
  sourceRefKind: S.NullOr(S.String),
  url: S.NullOr(S.String),
})
export type ExaEnrichmentBlockedSource = typeof ExaEnrichmentBlockedSource.Type

export const ExaEnrichmentPolicyDecision = S.Struct({
  decision: S.String,
  reason: S.String,
})
export type ExaEnrichmentPolicyDecision =
  typeof ExaEnrichmentPolicyDecision.Type

export const ExaEnrichmentPlan = S.Struct({
  assignmentId: S.String,
  blockedSources: S.Array(ExaEnrichmentBlockedSource),
  contentsTasks: S.Array(ExaEnrichmentPlanTask),
  expectedSourceCategories: S.Array(S.String),
  planId: S.String,
  policyDecisions: S.Array(ExaEnrichmentPolicyDecision),
  searchTasks: S.Array(ExaEnrichmentPlanTask),
  subjectSummary: S.String,
})
export type ExaEnrichmentPlan = typeof ExaEnrichmentPlan.Type

export type BuildAdjutantEnrichmentPlanInput = Readonly<{
  assignment: AdjutantAssignment
  explicitSourceRefs?: ReadonlyArray<ExplicitPublicSourceRef> | undefined
  freshnessMaxAgeHours?: number | undefined
  numResults?: number | undefined
  operatorNotes?: string | undefined
  order?: AdjutantEnrichmentOrderContext | null | undefined
  site?: AdjutantEnrichmentSiteContext | null | undefined
}>

export class AdjutantEnrichmentPlannerValidationError extends S.TaggedErrorClass<AdjutantEnrichmentPlannerValidationError>()(
  'AdjutantEnrichmentPlannerValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantEnrichmentPlannerError =
  AdjutantEnrichmentPlannerValidationError

export type AdjutantEnrichmentPlannerShape = Readonly<{
  buildPlan: (
    input: BuildAdjutantEnrichmentPlanInput,
  ) => Effect.Effect<ExaEnrichmentPlan, AdjutantEnrichmentPlannerError>
}>

const MAX_QUERY_CHARS = 420
const SOURCE_REF_DOMAIN_LIMIT = 8

const normalizedText = (value: string): string =>
  value.trim().replace(/\s+/g, ' ')

const boundedQuery = (value: string): string =>
  normalizedText(value).slice(0, MAX_QUERY_CHARS)

const hasExplicitAssignmentContext = (
  assignment: AdjutantAssignment,
): boolean =>
  assignment.softwareOrderId !== null ||
  assignment.siteId !== null ||
  assignment.taskSpecPath !== null

const stableToken = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 80)

const taskId = (
  planId: string,
  sourceCategory: string,
  value: string,
): string => `${planId}_${sourceCategory}_${stableToken(value)}`

const urlDomain = (url: string): string | undefined => {
  try {
    return new URL(url).hostname
  } catch {
    return undefined
  }
}

const sourceRefAllowedForPlan = (sourceRef: ExplicitPublicSourceRef): boolean =>
  sourceRef.status === 'approved' || sourceRef.status === 'public_safe'

const sourceRefBlockedReason = (
  sourceRef: ExplicitPublicSourceRef,
): string | undefined => {
  if (sourceRefAllowedForPlan(sourceRef)) {
    return undefined
  }

  if (sourceRef.status === 'internal_only') {
    return 'Source ref is internal-only and cannot be used for public evidence enrichment.'
  }

  if (sourceRef.status === 'rejected') {
    return 'Source ref was rejected by an operator.'
  }

  return 'Source ref has not been approved for enrichment.'
}

const sourceRefIdentitySearchCategory = (
  sourceRef: ExplicitPublicSourceRef,
): ExaSearchCategory | undefined => {
  if (
    sourceRef.kind === 'github_profile' ||
    sourceRef.kind === 'linkedin_profile' ||
    sourceRef.kind === 'x_profile' ||
    sourceRef.kind === 'personal_site'
  ) {
    return 'people'
  }

  return undefined
}

const sourceRefSourceCategory = (
  sourceRef: ExplicitPublicSourceRef,
): string => {
  if (sourceRef.kind === 'github_repository') {
    return 'repository'
  }

  if (sourceRef.kind === 'github_profile') {
    return 'github_profile'
  }

  if (sourceRef.kind === 'linkedin_profile') {
    return 'linkedin_profile'
  }

  if (sourceRef.kind === 'x_profile') {
    return 'x_profile'
  }

  if (sourceRef.kind === 'personal_site') {
    return 'personal_site'
  }

  return 'generic_url'
}

const planSourceRefTasks = (
  planId: string,
  sourceRefs: ReadonlyArray<ExplicitPublicSourceRef>,
  freshnessMaxAgeHours: number,
  numResults: number,
): Readonly<{
  blockedSources: ReadonlyArray<ExaEnrichmentBlockedSource>
  contentsTasks: ReadonlyArray<ExaEnrichmentPlanTask>
  searchTasks: ReadonlyArray<ExaEnrichmentPlanTask>
}> => {
  const allowedRefs = sourceRefs.filter(sourceRefAllowedForPlan)
  const blockedSources = sourceRefs
    .filter(sourceRef => !sourceRefAllowedForPlan(sourceRef))
    .map(sourceRef => ({
      reason: sourceRefBlockedReason(sourceRef) ?? 'Source ref is blocked.',
      sourceRefId: sourceRef.id ?? null,
      sourceRefKind: sourceRef.kind,
      url: sourceRef.url,
    }))
  const contentsTasks = allowedRefs.map(sourceRef => ({
    id: taskId(planId, 'contents', sourceRef.url),
    category: null,
    contentsMaxAgeHours: freshnessMaxAgeHours,
    includeDomains: [],
    kind: 'contents' as const,
    numResults: 1,
    query: boundedQuery(sourceRef.label ?? sourceRef.url),
    reason: 'Retrieve explicitly approved public source ref content.',
    searchType: 'auto',
    sourceCategory: sourceRefSourceCategory(sourceRef),
    sourceRefId: sourceRef.id ?? null,
    urls: [sourceRef.url],
  }))
  const searchTasks = allowedRefs.flatMap(sourceRef => {
    const domain = urlDomain(sourceRef.url)
    const peopleCategory = sourceRefIdentitySearchCategory(sourceRef)
    const includeDomains =
      domain === undefined ? [] : [domain].slice(0, SOURCE_REF_DOMAIN_LIMIT)
    const baseTask = {
      id: taskId(planId, 'source_ref', sourceRef.url),
      category: peopleCategory ?? null,
      contentsMaxAgeHours: freshnessMaxAgeHours,
      includeDomains,
      kind: 'search' as const,
      numResults,
      query: boundedQuery(
        peopleCategory === 'people'
          ? `Public professional context for ${sourceRef.label ?? sourceRef.url}`
          : `${sourceRef.label ?? sourceRef.url} public context`,
      ),
      reason:
        peopleCategory === 'people'
          ? 'Explicit public identity/source ref approved for people search.'
          : 'Explicit public source ref approved for contextual search.',
      searchType: 'auto',
      sourceCategory: sourceRefSourceCategory(sourceRef),
      sourceRefId: sourceRef.id ?? null,
      urls: [],
    }

    return [baseTask]
  })

  return { blockedSources, contentsTasks, searchTasks }
}

const repositoryFromOrder = (
  order: AdjutantEnrichmentOrderContext | null | undefined,
): Readonly<{ fullName: string; htmlUrl: string | null }> | undefined => {
  if (order?.repositoryPrivate === true) {
    return undefined
  }

  if (
    order?.repositoryFullName !== null &&
    order?.repositoryFullName !== undefined
  ) {
    return {
      fullName: order.repositoryFullName,
      htmlUrl: order.repositoryHtmlUrl,
    }
  }

  if (
    order !== null &&
    order !== undefined &&
    order.repositoryOwner !== null &&
    order.repositoryName !== null
  ) {
    return {
      fullName: `${order.repositoryOwner}/${order.repositoryName}`,
      htmlUrl: order.repositoryHtmlUrl,
    }
  }

  return undefined
}

const repositoryFromSite = (
  site: AdjutantEnrichmentSiteContext | null | undefined,
): Readonly<{ fullName: string; htmlUrl: string | null }> | undefined =>
  site?.sourceRepositoryProvider === 'github' &&
  site.sourceRepositoryOwner !== null &&
  site.sourceRepositoryName !== null
    ? {
        fullName: `${site.sourceRepositoryOwner}/${site.sourceRepositoryName}`,
        htmlUrl: null,
      }
    : undefined

const buildPlan = (
  input: BuildAdjutantEnrichmentPlanInput,
): Effect.Effect<ExaEnrichmentPlan, AdjutantEnrichmentPlannerError> =>
  Effect.gen(function* () {
    if (!hasExplicitAssignmentContext(input.assignment)) {
      return yield* new AdjutantEnrichmentPlannerValidationError({
        reason:
          'Autopilot enrichment requires explicit softwareOrderId, siteId, or taskSpecPath context.',
      })
    }

    const freshnessMaxAgeHours = Math.max(
      0,
      Math.trunc(input.freshnessMaxAgeHours ?? 24),
    )
    const numResults = Math.max(
      1,
      Math.min(10, Math.trunc(input.numResults ?? 8)),
    )
    const planId = `exa_plan_${input.assignment.id}`
    const orderRequest = normalizedText(input.order?.request ?? '')
    const subjectSummary = boundedQuery(
      [
        input.site?.title ?? '',
        orderRequest,
        input.assignment.objective,
        input.operatorNotes ?? '',
      ]
        .filter(text => normalizedText(text) !== '')
        .join(' '),
    )
    const repository =
      repositoryFromOrder(input.order) ?? repositoryFromSite(input.site)
    const topicQuery = boundedQuery(
      orderRequest === ''
        ? input.assignment.objective
        : `${orderRequest} public evidence current context`,
    )
    const topicSearchTask: ExaEnrichmentPlanTask = {
      id: taskId(planId, 'topic_web', topicQuery),
      category: null,
      contentsMaxAgeHours: freshnessMaxAgeHours,
      includeDomains: [],
      kind: 'search',
      numResults,
      query: topicQuery,
      reason:
        'Explicit assignment/order context selected; gather public topic evidence.',
      searchType: 'auto',
      sourceCategory: 'topic_web',
      sourceRefId: null,
      urls: [],
    }
    const repositorySearchTask =
      repository === undefined
        ? []
        : [
            {
              id: taskId(planId, 'repository', repository.fullName),
              category: 'github',
              contentsMaxAgeHours: freshnessMaxAgeHours,
              includeDomains: ['github.com'],
              kind: 'search' as const,
              numResults: Math.min(4, numResults),
              query: boundedQuery(
                `${repository.fullName} repository project public context`,
              ),
              reason:
                'Explicit public repository ref is attached to the order or Site.',
              searchType: 'auto',
              sourceCategory: 'repository',
              sourceRefId: null,
              urls: repository.htmlUrl === null ? [] : [repository.htmlUrl],
            },
          ]
    const sourceRefTasks = planSourceRefTasks(
      planId,
      input.explicitSourceRefs ?? [],
      freshnessMaxAgeHours,
      numResults,
    )
    const searchTasks = [
      topicSearchTask,
      ...repositorySearchTask,
      ...sourceRefTasks.searchTasks,
    ]
    const contentsTasks = sourceRefTasks.contentsTasks
    const expectedSourceCategories = [
      ...new Set(
        [...searchTasks, ...contentsTasks].map(task => task.sourceCategory),
      ),
    ]

    return {
      assignmentId: input.assignment.id,
      blockedSources: sourceRefTasks.blockedSources,
      contentsTasks,
      expectedSourceCategories,
      planId,
      policyDecisions: [
        {
          decision: 'explicit_assignment_required',
          reason:
            'Exa enrichment runs only after Autopilot assignment/order/Site context is explicit.',
        },
        {
          decision: 'no_private_identity_inference',
          reason:
            'Prompt names and topic keywords do not select customer identities, private repos, or private social context.',
        },
      ],
      searchTasks,
      subjectSummary,
    }
  })

export const makeAdjutantEnrichmentPlanner =
  (): AdjutantEnrichmentPlannerShape => ({
    buildPlan: Effect.fn('AdjutantEnrichmentPlanner.buildPlan')(buildPlan),
  })

export class AdjutantEnrichmentPlanner extends Context.Service<
  AdjutantEnrichmentPlanner,
  AdjutantEnrichmentPlannerShape
>()('@openagentsinc/autopilot-omega/AdjutantEnrichmentPlanner') {
  static readonly layer = Layer.succeed(
    AdjutantEnrichmentPlanner,
    makeAdjutantEnrichmentPlanner(),
  )
}

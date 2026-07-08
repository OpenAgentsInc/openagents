// OB-3 (#8560): audit-first personalization at fleet scale.
//
// Two routes:
//
//   POST /api/operator/agent-readiness/reports
//     Operator/fleet-only (admin bearer token). Takes a real
//     `AgentReadinessReport` (LG-1 scan output) plus a `pipelineRef`,
//     renders it through the OB-3 15-step rubric, stores it under a fresh
//     unguessable public token, and appends the resulting report receipt
//     ref onto the pipeline row (BF-9.2 `business_pipeline_rows`).
//
//   GET /api/public/agent-readiness/reports/:token
//     Public, unauthenticated, tokenized. Returns only the public-safe
//     15-step assessment for that one prospect's own domain — never the
//     internal pipelineRef/sourceRef, never any Apollo enrichment field.
//     Every successful read is a real "prospect opened their report" click
//     and is wired into the existing LG-6 funnel counters
//     (`business_funnel_events`, stage `visit`) via the row's stored
//     `sourceRef`, best-effort: a funnel-write failure never breaks the
//     report response.

import { Effect, Schema as S } from 'effect'

import {
  decodeAgentReadinessReport,
  renderAgentReadinessFifteenStepAssessment,
} from '@openagentsinc/agent-readiness'

import {
  type AgentReadinessPublicReportStore,
  AgentReadinessPublicReportValidationError,
  makeD1AgentReadinessPublicReportStore,
} from './agent-readiness-public-report-store'
import {
  type BusinessPipelineStore,
  BusinessPipelineStoreError,
  systemBusinessPipelineRuntime,
} from './business-pipeline-queue'
import { recordBusinessFunnelEvent } from './business-funnel-dashboard'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalString, readJsonObject } from './json-boundary'
import { logWorkerRouteWarning } from './observability'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export class AgentReadinessRouteUnexpectedError extends S.TaggedErrorClass<AgentReadinessRouteUnexpectedError>()(
  'AgentReadinessRouteUnexpectedError',
  { reason: S.String },
) {}

const asUnexpectedError = (error: unknown): AgentReadinessRouteUnexpectedError =>
  new AgentReadinessRouteUnexpectedError({
    reason: error instanceof Error ? error.message : String(error),
  })

// ---------------------------------------------------------------------------
// Operator create route
// ---------------------------------------------------------------------------

type OperatorAgentReadinessReportDependencies<Bindings> = Readonly<{
  makePipelineStore: (env: Bindings) => BusinessPipelineStore
  makeReportStore: (env: Bindings) => AgentReadinessPublicReportStore
  nowIso?: () => string
  publicBaseUrl?: string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const operatorErrorResponse = (
  error:
    | BusinessPipelineStoreError
    | AgentReadinessPublicReportValidationError
    | AgentReadinessRouteUnexpectedError,
): HttpResponse => {
  if (error instanceof BusinessPipelineStoreError) {
    const status =
      error.kind === 'not_found' ? 404 : error.kind === 'validation_error' ? 400 : 500
    return noStoreJsonResponse(
      { error: `business_pipeline_${error.kind}`, reason: error.reason },
      { status },
    )
  }
  if (error instanceof AgentReadinessPublicReportValidationError) {
    return noStoreJsonResponse(
      { error: 'agent_readiness_public_report_validation_error', reason: error.reason },
      { status: 400 },
    )
  }
  return noStoreJsonResponse(
    {
      error: 'agent_readiness_public_report_create_failed',
      reason: error.reason,
    },
    { status: 400 },
  )
}

const routeCreateReport = <Bindings>(
  dependencies: OperatorAgentReadinessReportDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError ||
      error instanceof AgentReadinessPublicReportValidationError
        ? error
        : asUnexpectedError(error),
    try: async () => {
      if (!(await dependencies.requireAdminApiToken(request, env))) {
        return unauthorized()
      }

      const body = await readJsonObject(request)
      const pipelineRef = optionalString(body.pipelineRef)
      if (pipelineRef === undefined) {
        return noStoreJsonResponse(
          {
            error: 'agent_readiness_public_report_validation_error',
            reason: 'pipelineRef is required',
          },
          { status: 400 },
        )
      }

      const report = decodeAgentReadinessReport(body.report)
      const nowIso = dependencies.nowIso ?? currentIsoTimestamp

      const pipelineStore = dependencies.makePipelineStore(env)
      const pipelineRow = await pipelineStore.readPipelineRow(pipelineRef)
      if (pipelineRow === null) {
        throw new BusinessPipelineStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }

      const assessment = renderAgentReadinessFifteenStepAssessment(report, {
        generatedAt: nowIso(),
      })

      const reportStore = dependencies.makeReportStore(env)
      const created = await reportStore.createPublicReport({
        pipelineRef,
        sourceRef: pipelineRow.sourceRef,
        domain: report.domain,
        assessment,
      })

      await pipelineStore.appendPipelineReceiptRefs(
        pipelineRef,
        [created.receiptRef],
        {
          ...systemBusinessPipelineRuntime,
          nowIso: dependencies.nowIso ?? systemBusinessPipelineRuntime.nowIso,
        },
      )

      const baseUrl = dependencies.publicBaseUrl ?? 'https://openagents.com'
      return noStoreJsonResponse(
        {
          report: {
            createdAt: created.createdAt,
            domain: created.domain,
            grade: created.grade,
            receiptRef: created.receiptRef,
            reportToken: created.reportToken,
            score: created.score,
            url: `${baseUrl}/api/public/agent-readiness/reports/${created.reportToken}`,
          },
        },
        { status: 201 },
      )
    },
  }).pipe(Effect.catch(error => Effect.succeed(operatorErrorResponse(error))))

export const makeOperatorAgentReadinessReportRoutes = <Bindings>(
  dependencies: OperatorAgentReadinessReportDependencies<Bindings>,
) => ({
  routeOperatorAgentReadinessReportRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    if (url.pathname !== '/api/operator/agent-readiness/reports') return undefined
    if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))
    return routeCreateReport(dependencies, request, env)
  },
})

// ---------------------------------------------------------------------------
// Public tokenized read route
// ---------------------------------------------------------------------------

export type PublicAgentReadinessReportRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  store?: AgentReadinessPublicReportStore
  recordClick?: (
    reportToken: string,
    store: AgentReadinessPublicReportStore,
  ) => Promise<void>
}>

const PUBLIC_REPORT_PATH_PATTERN =
  /^\/api\/public\/agent-readiness\/reports\/([^/]+)$/

const defaultRecordClick = async (
  reportToken: string,
  store: AgentReadinessPublicReportStore,
  db: D1Database | undefined,
  nowIso: () => string,
): Promise<void> => {
  const clickResult = await store.recordReportClick(reportToken)
  if (clickResult === null || db === undefined) return
  await recordBusinessFunnelEvent(db, {
    eventRef: `agent_readiness_report_click_${reportToken}_${clickResult.clickCount}`,
    occurredAt: nowIso(),
    sourceKind: 'unknown', // recordBusinessFunnelEvent re-derives from sourceRef
    sourceRef: clickResult.sourceRef,
    stage: 'visit',
  })
}

export const handlePublicAgentReadinessReportApi = (
  request: Request,
  input: PublicAgentReadinessReportRouteInput,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const url = new URL(request.url)
  const match = PUBLIC_REPORT_PATH_PATTERN.exec(url.pathname)
  if (match === null) {
    return Effect.succeed(
      noStoreJsonResponse({ error: 'agent_readiness_public_report_not_found' }, {
        status: 404,
      }),
    )
  }
  const reportToken = decodeURIComponent(match[1] ?? '')

  return Effect.tryPromise({
    catch: asUnexpectedError,
    try: async () => {
      const store =
        input.store ??
        makeD1AgentReadinessPublicReportStore(input.OPENAGENTS_DB as D1Database)
      const projection = await store.readPublicReportByToken(reportToken)
      if (projection === null) {
        return noStoreJsonResponse(
          { error: 'agent_readiness_public_report_not_found' },
          { status: 404 },
        )
      }

      // Every successful GET is a real report-click: best-effort, never
      // blocks or fails the response the prospect is looking at.
      try {
        if (input.recordClick !== undefined) {
          await input.recordClick(reportToken, store)
        } else {
          await defaultRecordClick(
            reportToken,
            store,
            input.OPENAGENTS_DB,
            currentIsoTimestamp,
          )
        }
      } catch (error) {
        logWorkerRouteWarning('agent_readiness_public_report_click_failed', {
          error: error instanceof Error ? error.message : String(error),
          reportToken,
        })
      }

      return noStoreJsonResponse(projection)
    },
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        noStoreJsonResponse(
          {
            error: 'agent_readiness_public_report_read_failed',
            reason: error.reason,
          },
          { status: 500 },
        ),
      ),
    ),
  )
}

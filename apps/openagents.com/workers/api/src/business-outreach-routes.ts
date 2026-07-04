import { Effect, Match as M } from 'effect'

import {
  type BusinessOutreachDraftInput,
  type BusinessOutreachSendInput,
  BusinessOutreachStoreError,
  type BusinessOutreachStore,
  type BusinessOutreachSuppressionInput,
  type BusinessOutreachTemplateApprovalInput,
} from './business-outreach'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import {
  optionalInteger,
  optionalString,
  readJsonObject,
  stringArrayFromUnknown,
} from './json-boundary'

type HttpResponse = globalThis.Response

type OperatorBusinessOutreachDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => BusinessOutreachStore
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const routeErrorResponse = (error: BusinessOutreachStoreError): HttpResponse =>
  M.value(error.kind).pipe(
    M.when('conflict', () =>
      noStoreJsonResponse(
        { error: 'business_outreach_conflict', reason: error.reason },
        { status: 409 },
      ),
    ),
    M.when('not_found', () =>
      noStoreJsonResponse(
        { error: 'business_outreach_not_found', reason: error.reason },
        { status: 404 },
      ),
    ),
    M.when('validation_error', () =>
      noStoreJsonResponse(
        { error: 'business_outreach_validation_error', reason: error.reason },
        { status: 400 },
      ),
    ),
    M.orElse(() =>
      noStoreJsonResponse(
        { error: 'business_outreach_storage_error', reason: error.reason },
        { status: 500 },
      ),
    ),
  )

const requireOperator = async <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse | undefined> =>
  (await dependencies.requireAdminApiToken(request, env)) ? undefined : unauthorized()

const approvalInputFromBody = (
  body: Record<string, unknown>,
): BusinessOutreachTemplateApprovalInput => {
  const sourceRef = optionalString(body.sourceRef)
  return {
    approvalReceiptRef: optionalString(body.approvalReceiptRef) ?? '',
    approvedByRef: optionalString(body.approvedByRef) ?? '',
    ...(sourceRef === undefined ? {} : { sourceRef }),
    templateVersionRef: optionalString(body.templateVersionRef) ?? '',
  }
}

const suppressionInputFromBody = (
  body: Record<string, unknown>,
): BusinessOutreachSuppressionInput => {
  const suppressionRef = optionalString(body.suppressionRef)
  return {
    reason: (optionalString(body.reason) ?? 'active_intake') as BusinessOutreachSuppressionInput['reason'],
    sourceRef: optionalString(body.sourceRef) ?? '',
    subjectRef: optionalString(body.subjectRef) ?? '',
    ...(suppressionRef === undefined ? {} : { suppressionRef }),
  }
}

const draftInputFromBody = (
  body: Record<string, unknown>,
): BusinessOutreachDraftInput => {
  const draftRef = optionalString(body.draftRef)
  const observedFact = optionalString(body.observedFact)
  const sourceRef = optionalString(body.sourceRef)
  const templateVersionRef = optionalString(body.templateVersionRef)
  return {
    auditReportRef: optionalString(body.auditReportRef) ?? '',
    ...(draftRef === undefined ? {} : { draftRef }),
    findingRefs: stringArrayFromUnknown(body.findingRefs),
    ...(observedFact === undefined ? {} : { observedFact }),
    ...(sourceRef === undefined ? {} : { sourceRef }),
    subjectRef: optionalString(body.subjectRef) ?? '',
    ...(templateVersionRef === undefined ? {} : { templateVersionRef }),
  }
}

const sendInputFromBody = (
  body: Record<string, unknown>,
): BusinessOutreachSendInput => {
  const approvalReceiptRef = optionalString(body.approvalReceiptRef)
  const channel = optionalString(body.channel) as
    | BusinessOutreachSendInput['channel']
    | undefined
  const dailyMailboxSendCap = optionalInteger(body.dailyMailboxSendCap)
  const sendRef = optionalString(body.sendRef)
  const sentAt = optionalString(body.sentAt)
  return {
    ...(approvalReceiptRef === undefined ? {} : { approvalReceiptRef }),
    ...(channel === undefined ? {} : { channel }),
    ...(dailyMailboxSendCap === undefined ? {} : { dailyMailboxSendCap }),
    draftRef: optionalString(body.draftRef) ?? '',
    mailboxRef: optionalString(body.mailboxRef) ?? '',
    ...(sendRef === undefined ? {} : { sendRef }),
    ...(sentAt === undefined ? {} : { sentAt }),
    sourceRef: optionalString(body.sourceRef) ?? '',
  }
}

const refusalStatus = (reason: string): number =>
  reason === 'claim_lint_failed' ||
  reason === 'template_mismatch' ||
  reason === 'template_not_found' ||
  reason === 'draft_not_found'
    ? 400
    : 409

const routeTemplates = <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessOutreachStoreError
        ? error
        : new BusinessOutreachStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      return noStoreJsonResponse({
        templates: dependencies.makeStore(env).listTemplates(),
      })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeApproveTemplate = <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessOutreachStoreError
        ? error
        : new BusinessOutreachStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const approval = await dependencies
        .makeStore(env)
        .approveTemplate(approvalInputFromBody(await readJsonObject(request)))
      return noStoreJsonResponse({ approval }, { status: 201 })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeCreateSuppression = <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessOutreachStoreError
        ? error
        : new BusinessOutreachStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const suppression = await dependencies
        .makeStore(env)
        .createSuppression(suppressionInputFromBody(await readJsonObject(request)))
      return noStoreJsonResponse({ suppression }, { status: 201 })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeRenderDraft = <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessOutreachStoreError
        ? error
        : new BusinessOutreachStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const outcome = await dependencies
        .makeStore(env)
        .renderDraft(pipelineRef, draftInputFromBody(await readJsonObject(request)))

      if (!outcome.ok) {
        return noStoreJsonResponse(
          {
            claimLintRefs: outcome.claimLintRefs ?? [],
            error: 'business_outreach_refused',
            message: outcome.message,
            ok: false,
            reason: outcome.reason,
            suppression: outcome.suppression ?? null,
          },
          { status: refusalStatus(outcome.reason) },
        )
      }

      return noStoreJsonResponse({ draft: outcome.draft }, { status: 201 })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeRecordSend = <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessOutreachStoreError
        ? error
        : new BusinessOutreachStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const outcome = await dependencies
        .makeStore(env)
        .recordSend(pipelineRef, sendInputFromBody(await readJsonObject(request)))

      if (!outcome.ok) {
        return noStoreJsonResponse(
          {
            error: 'business_outreach_refused',
            message: outcome.message,
            ok: false,
            reason: outcome.reason,
          },
          { status: refusalStatus(outcome.reason) },
        )
      }

      return noStoreJsonResponse(
        {
          pipelineReceiptRefs: outcome.pipelineReceiptRefs,
          send: outcome.send,
        },
        { status: 201 },
      )
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

export const makeOperatorBusinessOutreachRoutes = <Bindings>(
  dependencies: OperatorBusinessOutreachDependencies<Bindings>,
) => ({
  routeOperatorBusinessOutreachRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/business/outreach/templates') {
      if (request.method === 'GET') return routeTemplates(dependencies, request, env)
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (url.pathname === '/api/operator/business/outreach/template-approvals') {
      if (request.method === 'POST') {
        return routeApproveTemplate(dependencies, request, env)
      }
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/operator/business/outreach/suppressions') {
      if (request.method === 'POST') {
        return routeCreateSuppression(dependencies, request, env)
      }
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    const match =
      /^\/api\/operator\/business\/pipeline\/([^/]+)\/(outreach-drafts|outreach-sends)$/.exec(
        url.pathname,
      )
    if (match === null) return undefined
    if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))

    const pipelineRef = decodeURIComponent(match[1] ?? '')
    return match[2] === 'outreach-drafts'
      ? routeRenderDraft(dependencies, request, env, pipelineRef)
      : routeRecordSend(dependencies, request, env, pipelineRef)
  },
})

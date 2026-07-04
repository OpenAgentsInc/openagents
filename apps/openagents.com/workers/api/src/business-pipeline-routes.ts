import { Effect, Match as M } from 'effect'

import {
  type BusinessPipelineAdvanceInput,
  type BusinessPipelineCommitmentInput,
  type BusinessPipelineCreateInput,
  type BusinessPipelinePartnerRouteInput,
  BusinessPipelineStoreError,
  type BusinessPipelineStore,
  systemBusinessPipelineRuntime,
} from './business-pipeline-queue'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import {
  optionalBoolean,
  optionalInteger,
  optionalString,
  readJsonObject,
  stringArrayFromUnknown,
} from './json-boundary'

type HttpResponse = globalThis.Response

type OperatorBusinessPipelineDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => BusinessPipelineStore
  nowIso?: () => string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const routeErrorResponse = (error: BusinessPipelineStoreError): HttpResponse =>
  M.value(error.kind).pipe(
    M.when('conflict', () =>
      noStoreJsonResponse(
        { error: 'business_pipeline_conflict', reason: error.reason },
        { status: 409 },
      ),
    ),
    M.when('not_found', () =>
      noStoreJsonResponse(
        { error: 'business_pipeline_not_found', reason: error.reason },
        { status: 404 },
      ),
    ),
    M.when('validation_error', () =>
      noStoreJsonResponse(
        { error: 'business_pipeline_validation_error', reason: error.reason },
        { status: 400 },
      ),
    ),
    M.orElse(() =>
      noStoreJsonResponse(
        { error: 'business_pipeline_storage_error', reason: error.reason },
        { status: 500 },
      ),
    ),
  )

const requireOperator = async <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse | undefined> =>
  (await dependencies.requireAdminApiToken(request, env)) ? undefined : unauthorized()

const numberOrUndefined = (value: unknown): number | undefined => {
  if (typeof value === 'number') return value
  if (typeof value === 'string' && value.trim() !== '') return Number(value)
  return undefined
}

const objectOrEmpty = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const optionalPartnerRouteValue = (
  body: Record<string, unknown>,
  nested: Record<string, unknown>,
  key: string,
): string | undefined => optionalString(body[key]) ?? optionalString(nested[key])

const partnerRouteInputFromBody = (
  body: Record<string, unknown>,
  requireState: boolean,
): BusinessPipelinePartnerRouteInput | null => {
  const nested = objectOrEmpty(body.partnerRoute)
  const state =
    optionalString(body.partnerRouteState) ??
    optionalString(nested.state) ??
    (requireState ? optionalString(body.state) : undefined)
  const hasRouteFields = [
    'approvalReceiptRef',
    'budgetRangeRef',
    'dueWindowRef',
    'offerRef',
    'peerRef',
    'privacyTierRef',
    'scopeSummaryRef',
  ].some(key => body[key] !== undefined || nested[key] !== undefined)

  if (state === undefined && !hasRouteFields && !requireState) {
    return null
  }

  const input: BusinessPipelinePartnerRouteInput = {
    state: (state ?? '') as BusinessPipelinePartnerRouteInput['state'],
  }
  const optionalFields = [
    'approvalReceiptRef',
    'budgetRangeRef',
    'dueWindowRef',
    'offerRef',
    'peerRef',
    'privacyTierRef',
    'scopeSummaryRef',
  ] as const
  const writableInput = input as Partial<
    Record<(typeof optionalFields)[number], string | null>
  >

  for (const field of optionalFields) {
    const value = optionalPartnerRouteValue(body, nested, field)
    if (value !== undefined) {
      writableInput[field] = value
    }
  }

  return input
}

const createInputFromBody = (
  body: Record<string, unknown>,
): BusinessPipelineCreateInput => {
  const stage = optionalString(body.stage) as
    | BusinessPipelineCreateInput['stage']
    | undefined

  return {
    blockerRef: optionalString(body.blockerRef) ?? null,
    businessSignupRequestId: optionalString(body.businessSignupRequestId) ?? null,
    nextActionDueAt: optionalString(body.nextActionDueAt) ?? null,
    ownerRole: (optionalString(body.ownerRole) ?? 'operator') as BusinessPipelineCreateInput['ownerRole'],
    partnerRoute: partnerRouteInputFromBody(body, false),
    partnerRouteFlag: optionalBoolean(body.partnerRouteFlag) ?? false,
    pipelineRef: optionalString(body.pipelineRef) ?? '',
    quotedBand: {
      label: optionalString(body.quotedBandLabel) ?? optionalString(body.quotedBand) ?? 'unquoted',
      maxUsdCents: optionalInteger(body.quotedMaxUsdCents) ?? numberOrUndefined(body.quotedMaxUsdCents) ?? 0,
      minUsdCents: optionalInteger(body.quotedMinUsdCents) ?? numberOrUndefined(body.quotedMinUsdCents) ?? 0,
    },
    receiptRefs: stringArrayFromUnknown(body.receiptRefs),
    sourceRef: optionalString(body.sourceRef) ?? '',
    ...(stage === undefined ? {} : { stage }),
    vertical: optionalString(body.vertical) ?? '',
  }
}

const advanceInputFromBody = (
  body: Record<string, unknown>,
): BusinessPipelineAdvanceInput => {
  const ownerRole = optionalString(body.ownerRole) as
    | BusinessPipelineAdvanceInput['ownerRole']
    | undefined

  return {
    blockerRef: optionalString(body.blockerRef) ?? null,
    nextActionDueAt: optionalString(body.nextActionDueAt) ?? null,
    ...(ownerRole === undefined ? {} : { ownerRole }),
    quotedBand:
    body.quotedMinUsdCents === undefined &&
    body.quotedMaxUsdCents === undefined &&
    body.quotedBandLabel === undefined &&
    body.quotedBand === undefined
      ? null
      : {
          label: optionalString(body.quotedBandLabel) ?? optionalString(body.quotedBand) ?? 'quoted',
          maxUsdCents: optionalInteger(body.quotedMaxUsdCents) ?? numberOrUndefined(body.quotedMaxUsdCents) ?? 0,
          minUsdCents: optionalInteger(body.quotedMinUsdCents) ?? numberOrUndefined(body.quotedMinUsdCents) ?? 0,
        },
    receiptRef: optionalString(body.receiptRef) ?? '',
    stage: optionalString(body.stage) as BusinessPipelineAdvanceInput['stage'],
  }
}

const commitmentInputFromBody = (
  pipelineRef: string,
  body: Record<string, unknown>,
): BusinessPipelineCommitmentInput => {
  const engagementRef = optionalString(body.engagementRef)
  const dueState = (optionalString(body.dueState) ?? 'due') as NonNullable<
    BusinessPipelineCommitmentInput['dueState']
  >

  return {
    blockerRefs: stringArrayFromUnknown(body.blockerRefs),
    commitmentKind: (optionalString(body.commitmentKind) ?? 'deliverable') as BusinessPipelineCommitmentInput['commitmentKind'],
    commitmentRef: optionalString(body.commitmentRef) ?? '',
    dueAt: optionalString(body.dueAt) ?? '',
    dueState,
    ...(engagementRef === undefined ? {} : { engagementRef }),
    evidenceRefs: stringArrayFromUnknown(body.evidenceRefs),
    ownerRef: optionalString(body.ownerRef) ?? '',
    pipelineRef,
    promisedObjectRef: optionalString(body.promisedObjectRef) ?? '',
    shippedAt: optionalString(body.shippedAt) ?? null,
    sourceRefs: stringArrayFromUnknown(body.sourceRefs),
  }
}

const routeList = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError
        ? error
        : new BusinessPipelineStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      return noStoreJsonResponse({
        rows: await dependencies.makeStore(env).listPipelineRows(),
      })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeMetrics = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError
        ? error
        : new BusinessPipelineStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const nowIso = dependencies.nowIso ?? systemBusinessPipelineRuntime.nowIso
      return noStoreJsonResponse(await dependencies.makeStore(env).readMetrics(nowIso()))
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeCreate = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError
        ? error
        : new BusinessPipelineStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const body = await readJsonObject(request)
      const row = await dependencies.makeStore(env).createPipelineRow(
        createInputFromBody(body),
        {
          ...systemBusinessPipelineRuntime,
          nowIso: dependencies.nowIso ?? systemBusinessPipelineRuntime.nowIso,
        },
      )
      return noStoreJsonResponse({ row }, { status: 201 })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeAdvance = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError
        ? error
        : new BusinessPipelineStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const body = await readJsonObject(request)
      const row = await dependencies.makeStore(env).advancePipelineRow(
        pipelineRef,
        advanceInputFromBody(body),
        {
          ...systemBusinessPipelineRuntime,
          nowIso: dependencies.nowIso ?? systemBusinessPipelineRuntime.nowIso,
        },
      )
      return noStoreJsonResponse({ row })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routePartnerRoute = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError
        ? error
        : new BusinessPipelineStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const body = await readJsonObject(request)
      const partnerRouteInput = partnerRouteInputFromBody(body, true)
      if (partnerRouteInput === null) {
        throw new BusinessPipelineStoreError({
          kind: 'validation_error',
          reason: 'partnerRouteState is required',
        })
      }
      const row = await dependencies.makeStore(env).setPartnerRoute(
        pipelineRef,
        partnerRouteInput,
        {
          ...systemBusinessPipelineRuntime,
          nowIso: dependencies.nowIso ?? systemBusinessPipelineRuntime.nowIso,
        },
      )
      return noStoreJsonResponse({ row })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeCreateCommitment = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessPipelineStoreError
        ? error
        : new BusinessPipelineStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const body = await readJsonObject(request)
      const commitment = await dependencies.makeStore(env).createCommitment(
        commitmentInputFromBody(pipelineRef, body),
        {
          ...systemBusinessPipelineRuntime,
          nowIso: dependencies.nowIso ?? systemBusinessPipelineRuntime.nowIso,
        },
      )
      return noStoreJsonResponse({ commitment }, { status: 201 })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

export const makeOperatorBusinessPipelineRoutes = <Bindings>(
  dependencies: OperatorBusinessPipelineDependencies<Bindings>,
) => ({
  routeOperatorBusinessPipelineRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/business/pipeline') {
      if (request.method === 'GET') return routeList(dependencies, request, env)
      if (request.method === 'POST') return routeCreate(dependencies, request, env)
      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    if (url.pathname === '/api/operator/business/pipeline/metrics') {
      if (request.method === 'GET') return routeMetrics(dependencies, request, env)
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const match =
      /^\/api\/operator\/business\/pipeline\/([^/]+)\/(advance|commitments|partner-route)$/.exec(
        url.pathname,
      )

    if (match === null) return undefined

    const pipelineRef = decodeURIComponent(match[1] ?? '')
    const action = match[2]

    if (action === 'advance') {
      if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))
      return routeAdvance(dependencies, request, env, pipelineRef)
    }

    if (action === 'partner-route') {
      if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))
      return routePartnerRoute(dependencies, request, env, pipelineRef)
    }

    if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))
    return routeCreateCommitment(dependencies, request, env, pipelineRef)
  },
})

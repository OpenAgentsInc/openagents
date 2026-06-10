import { Effect } from 'effect'

import { sha256Hex as agentSha256Hex } from './agent-registration'
import {
  DefaultBuyModeRelayUrl,
  type BuyModeDispatcherStore,
  type BuyModePaymentBridge,
  type BuyModeRelayPublisher,
  dispatchBuyModeJob,
  settleBuyModeResult,
  sha256Hex,
  startBuyModeCampaign,
  stopBuyModeCampaign,
} from './buy-mode-dispatcher'
import { badRequest } from '@openagents/sync-worker'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { optionalString, readJsonObject, stringArrayFromUnknown } from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

type HttpResponse = globalThis.Response

type OperatorBuyModeSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

export type OperatorBuyModeRouteDependencies<Bindings> = Readonly<{
  currentIsoTimestamp?: () => string
  makePaymentBridge?: (env: Bindings) => BuyModePaymentBridge
  makeRelayPublisher?: (env: Bindings) => BuyModeRelayPublisher
  makeStore: (env: Bindings) => BuyModeDispatcherStore
  makeUuid?: () => string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const numberFromBody = (
  body: Record<string, unknown>,
  key: string,
): number | undefined => {
  const value = body[key]

  if (typeof value === 'number') {
    return value
  }

  if (typeof value === 'string' && value.trim() !== '') {
    return Number(value)
  }

  return undefined
}

const booleanFromBody = (
  body: Record<string, unknown>,
  key: string,
  fallback: boolean,
): boolean => {
  const value = body[key]

  if (typeof value === 'boolean') {
    return value
  }

  if (typeof value === 'string') {
    if (value === 'true') return true
    if (value === 'false') return false
  }

  return fallback
}

const requireIdempotencyKey = (request: Request): string | HttpResponse => {
  const value = request.headers.get('idempotency-key')?.trim()

  if (value === undefined || value.length < 8 || value.length > 200) {
    return badRequest('Idempotency-Key header must be 8-200 characters.')
  }

  return value
}

const requireOperator = async <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<OperatorBuyModeSession | HttpResponse> => {
  if (!(await dependencies.requireAdminApiToken(request, env))) {
    return unauthorized()
  }

  return {
    user: {
      email: 'operator@openagents.com',
      userId: 'operator:admin_token',
    },
  }
}

const defaultRelayPublisher = <Bindings>(_env: Bindings): BuyModeRelayPublisher => ({
  publishJobRequest: async input => ({
    accepted: false,
    relayRef: `relay.public.unconfigured.${await sha256Hex(input.relayUrl)}`,
    requestEventId: `event.unconfigured.${await sha256Hex(JSON.stringify(input.requestEvent))}`,
  }),
})

const resultResponse = (result: unknown, status = 200): HttpResponse =>
  noStoreJsonResponse({ result }, { status })

const statusResponse = async <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const operator = await requireOperator(dependencies, request, env)

  if (operator instanceof Response) {
    return operator
  }

  const campaign = await dependencies.makeStore(env).latestCampaign()

  return noStoreJsonResponse({
    authority: {
      liveSpendEnabledByDefault: false,
      operatorApprovalRequiredForSpend: true,
      spendMutationAllowed: campaign?.spendEnabled === true,
    },
    campaign,
    disabledByDefault: campaign === null || campaign.state !== 'enabled',
  })
}

const startResponse = async <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const operator = await requireOperator(dependencies, request, env)

  if (operator instanceof Response) {
    return operator
  }

  const idempotencyKey = requireIdempotencyKey(request)

  if (idempotencyKey instanceof Response) {
    return idempotencyKey
  }

  const body = await readJsonObject(request)
  const nowIso = (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)()
  const perJobCapMsats = numberFromBody(body, 'perJobCapMsats')
  const dailyCapMsats = numberFromBody(body, 'dailyCapMsats')

  if (perJobCapMsats === undefined || dailyCapMsats === undefined) {
    return badRequest('perJobCapMsats and dailyCapMsats are required.')
  }

  const result = await startBuyModeCampaign(dependencies.makeStore(env), {
    campaignId: optionalString(body.campaignId) ??
      `buy_mode_campaign_${(dependencies.makeUuid ?? randomUuid)()}`,
    dailyCapMsats,
    idempotencyKeyHash: await agentSha256Hex(idempotencyKey),
    nowIso,
    operatorUserId: operator.user.userId,
    perJobCapMsats,
    relayUrl: optionalString(body.relayUrl) ?? DefaultBuyModeRelayUrl,
    spendEnabled: booleanFromBody(body, 'spendEnabled', false),
  })

  return resultResponse(result, result.kind === 'started' ? 201 : 409)
}

const stopResponse = async <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const operator = await requireOperator(dependencies, request, env)

  if (operator instanceof Response) {
    return operator
  }

  const result = await stopBuyModeCampaign(
    dependencies.makeStore(env),
    (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)(),
  )

  return resultResponse(result, result.kind === 'stopped' ? 200 : 409)
}

const dispatchResponse = async <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const operator = await requireOperator(dependencies, request, env)

  if (operator instanceof Response) {
    return operator
  }

  const idempotencyKey = requireIdempotencyKey(request)

  if (idempotencyKey instanceof Response) {
    return idempotencyKey
  }

  const body = await readJsonObject(request)
  const amountMsats = numberFromBody(body, 'amountMsats')
  const content = optionalString(body.content)

  if (amountMsats === undefined || content === undefined) {
    return badRequest('amountMsats and content are required.')
  }

  const result = await dispatchBuyModeJob(
    dependencies.makeStore(env),
    (dependencies.makeRelayPublisher ?? defaultRelayPublisher)(env),
    {
      amountMsats,
      campaignId: optionalString(body.campaignId),
      content,
      idempotencyKeyHash: await agentSha256Hex(idempotencyKey),
      jobId: optionalString(body.jobId) ??
        `buy_mode_job_${(dependencies.makeUuid ?? randomUuid)()}`,
      nowIso: (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)(),
      providerPubkeys: stringArrayFromUnknown(body.providerPubkeys),
    },
  )

  return resultResponse(
    result,
    result.kind === 'dispatched'
      ? 201
      : result.kind === 'idempotent_replay'
        ? 200
        : 409,
  )
}

const settleResponse = async <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const operator = await requireOperator(dependencies, request, env)

  if (operator instanceof Response) {
    return operator
  }

  const idempotencyKey = requireIdempotencyKey(request)

  if (idempotencyKey instanceof Response) {
    return idempotencyKey
  }

  const body = await readJsonObject(request)
  const amountMsats = numberFromBody(body, 'amountMsats')
  const bolt11 = optionalString(body.bolt11)
  const content = optionalString(body.content)
  const providerPubkey = optionalString(body.providerPubkey)
  const requestEventId = optionalString(body.requestEventId)
  const resultEventId = optionalString(body.resultEventId)

  if (
    amountMsats === undefined ||
    bolt11 === undefined ||
    content === undefined ||
    providerPubkey === undefined ||
    requestEventId === undefined ||
    resultEventId === undefined
  ) {
    return badRequest(
      'amountMsats, bolt11, content, providerPubkey, requestEventId, and resultEventId are required.',
    )
  }

  const paymentBridge = dependencies.makePaymentBridge?.(env)

  if (paymentBridge === undefined) {
    return resultResponse({
      kind: 'blocked',
      reasonRef: 'blocker.buy_mode.payment_bridge_unconfigured',
    }, 409)
  }

  const result = await settleBuyModeResult(
    dependencies.makeStore(env),
    paymentBridge,
    {
      amountMsats,
      bolt11,
      content,
      idempotencyKeyHash: await agentSha256Hex(idempotencyKey),
      nowIso: (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)(),
      providerPubkey,
      requestEventId,
      resultEventId,
    },
  )

  return resultResponse(
    result,
    result.kind === 'settled'
      ? 201
      : result.kind === 'idempotent_replay'
        ? 200
        : 409,
  )
}

export const makeOperatorBuyModeRoutes = <Bindings>(
  dependencies: OperatorBuyModeRouteDependencies<Bindings>,
) => ({
  handleOperatorBuyModeDispatchApi: (request: Request, env: Bindings) =>
    Effect.promise(() => dispatchResponse(dependencies, request, env)),
  handleOperatorBuyModeSettleApi: (request: Request, env: Bindings) =>
    Effect.promise(() => settleResponse(dependencies, request, env)),
  handleOperatorBuyModeStartApi: (request: Request, env: Bindings) =>
    Effect.promise(() => startResponse(dependencies, request, env)),
  handleOperatorBuyModeStatusApi: (request: Request, env: Bindings) =>
    Effect.promise(() => statusResponse(dependencies, request, env)),
  handleOperatorBuyModeStopApi: (request: Request, env: Bindings) =>
    Effect.promise(() => stopResponse(dependencies, request, env)),
})

import { Effect } from 'effect'

import { sha256Hex as agentSha256Hex } from './agent-registration'
import {
  DefaultBuyModeRelayUrl,
  type BuyModeDispatcherResult,
  type BuyModeDispatcherStore,
  type BuyModeJobRecord,
  type BuyModePaymentBridge,
  type BuyModeRelayPublisher,
  dispatchBuyModeJob,
  settleBuyModeResult,
  sha256Hex,
  startBuyModeCampaign,
  stopBuyModeCampaign,
} from './buy-mode-dispatcher'
import { badRequest } from '@openagentsinc/sync-worker'
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

type VerificationClass = 'exact_trace_replay' | 'command_check'

type VerificationClassVerdict = Readonly<{
  class: VerificationClass
  passed: boolean
}>

type BuyModeEvalJob = Readonly<{
  amountMsats: number
  roleIndex: number
  sampleId: string
  workerId: string
}>

type BuyModeEvalResult = Readonly<{
  settledMsats: number
  verdict: VerificationClassVerdict
}>

export type BuyModeEvalBridge = Readonly<{
  dispatchEval: (
    input: Readonly<{
      job: BuyModeEvalJob
      dispatchedJob: BuyModeJobRecord
      requestEventId: string
    }>,
  ) => Promise<BuyModeEvalResult>
}>

export type OperatorBuyModeRouteDependencies<Bindings> = Readonly<{
  currentIsoTimestamp?: () => string
  makeEvalBridge?: (env: Bindings) => BuyModeEvalBridge | undefined
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

const psionicEvalResponse = (
  result: BuyModeEvalResult,
  status = 200,
): HttpResponse =>
  noStoreJsonResponse({
    settled_msats: result.settledMsats,
    verdict: result.verdict,
  }, { status })

const psionicBlockedResponse = (reasonRef: string): HttpResponse =>
  resultResponse({ kind: 'blocked', reasonRef }, 409)

const textFromBody = (
  body: Record<string, unknown>,
  key: string,
): string | undefined => optionalString(body[key])

const parsePsionicEvalJob = (
  body: Record<string, unknown>,
): BuyModeEvalJob | HttpResponse => {
  const workerId = textFromBody(body, 'worker_id')
  const sampleId = textFromBody(body, 'sample_id')
  const roleIndex = numberFromBody(body, 'role_index')
  const amountMsats = numberFromBody(body, 'amount_msats')

  if (
    workerId === undefined ||
    sampleId === undefined ||
    roleIndex === undefined ||
    amountMsats === undefined
  ) {
    return badRequest(
      'worker_id, role_index, sample_id, and amount_msats are required.',
    )
  }

  if (!Number.isInteger(roleIndex) || roleIndex < 0) {
    return badRequest('role_index must be a non-negative integer.')
  }

  return {
    amountMsats,
    roleIndex,
    sampleId,
    workerId,
  }
}

const psionicEvalContent = (job: BuyModeEvalJob): string =>
  JSON.stringify({
    amount_msats: job.amountMsats,
    job_kind: 'psionic_buy_mode_eval',
    role_index: job.roleIndex,
    sample_id: job.sampleId,
    schema_version: 'psionic.buy_mode_eval_job.v1',
    worker_id: job.workerId,
  })

const psionicEvalIdempotencyKey = async (
  job: BuyModeEvalJob,
): Promise<string> =>
  `psionic.eval.${(await agentSha256Hex(psionicEvalContent(job))).slice(0, 48)}`

const resultJob = (
  result: BuyModeDispatcherResult,
): BuyModeJobRecord | undefined =>
  result.kind === 'dispatched' || result.kind === 'idempotent_replay'
    ? result.job
    : undefined

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

const evalResponse = async <Bindings>(
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

  const body = await readJsonObject(request)
  const parsedJob = parsePsionicEvalJob(body)

  if (parsedJob instanceof Response) {
    return parsedJob
  }

  const idempotencyKey = await psionicEvalIdempotencyKey(parsedJob)
  const dispatchResult = await dispatchBuyModeJob(
    dependencies.makeStore(env),
    (dependencies.makeRelayPublisher ?? defaultRelayPublisher)(env),
    {
      amountMsats: parsedJob.amountMsats,
      content: psionicEvalContent(parsedJob),
      idempotencyKeyHash: await agentSha256Hex(idempotencyKey),
      jobId: `buy_mode_eval_${(await agentSha256Hex(idempotencyKey)).slice(0, 32)}`,
      nowIso: (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)(),
      providerPubkeys: [parsedJob.workerId],
    },
  )

  const dispatchedJob = resultJob(dispatchResult)

  if (dispatchedJob === undefined) {
    return resultResponse(dispatchResult, 409)
  }

  const evalBridge = dependencies.makeEvalBridge?.(env)

  if (evalBridge === undefined) {
    return psionicBlockedResponse('blocker.buy_mode.eval_bridge_unconfigured')
  }

  const evalResult = await evalBridge.dispatchEval({
    dispatchedJob,
    job: parsedJob,
    requestEventId: dispatchedJob.requestEventId,
  })

  return psionicEvalResponse(evalResult)
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
  handleOperatorBuyModeEvalApi: (request: Request, env: Bindings) =>
    Effect.promise(() => evalResponse(dependencies, request, env)),
  handleOperatorBuyModeSettleApi: (request: Request, env: Bindings) =>
    Effect.promise(() => settleResponse(dependencies, request, env)),
  handleOperatorBuyModeStartApi: (request: Request, env: Bindings) =>
    Effect.promise(() => startResponse(dependencies, request, env)),
  handleOperatorBuyModeStatusApi: (request: Request, env: Bindings) =>
    Effect.promise(() => statusResponse(dependencies, request, env)),
  handleOperatorBuyModeStopApi: (request: Request, env: Bindings) =>
    Effect.promise(() => stopResponse(dependencies, request, env)),
})

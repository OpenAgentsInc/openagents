import { Effect, Match as M } from 'effect'

import {
  BusinessStarterCreditStoreError,
  type BusinessStarterCreditGrantInput,
  type BusinessStarterCreditRedemptionInput,
  type BusinessStarterCreditStore,
} from './business-starter-credit'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalInteger, optionalString, readJsonObject, stringArrayFromUnknown } from './json-boundary'

type HttpResponse = globalThis.Response

type OperatorBusinessStarterCreditDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => BusinessStarterCreditStore
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const routeErrorResponse = (
  error: BusinessStarterCreditStoreError,
): HttpResponse =>
  M.value(error.kind).pipe(
    M.when('conflict', () =>
      noStoreJsonResponse(
        { error: 'business_starter_credit_conflict', reason: error.reason },
        { status: 409 },
      ),
    ),
    M.when('not_found', () =>
      noStoreJsonResponse(
        { error: 'business_starter_credit_not_found', reason: error.reason },
        { status: 404 },
      ),
    ),
    M.when('validation_error', () =>
      noStoreJsonResponse(
        { error: 'business_starter_credit_validation_error', reason: error.reason },
        { status: 400 },
      ),
    ),
    M.orElse(() =>
      noStoreJsonResponse(
        { error: 'business_starter_credit_storage_error', reason: error.reason },
        { status: 500 },
      ),
    ),
  )

const requireOperator = async <Bindings>(
  dependencies: OperatorBusinessStarterCreditDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<HttpResponse | undefined> =>
  (await dependencies.requireAdminApiToken(request, env)) ? undefined : unauthorized()

const grantInputFromBody = (
  body: Record<string, unknown>,
): BusinessStarterCreditGrantInput => {
  const amountCapUsdCents = optionalInteger(body.amountCapUsdCents)
  const amountUsdCents = optionalInteger(body.amountUsdCents)
  const engagementRef = optionalString(body.engagementRef)
  const grantRef = optionalString(body.grantRef)
  const windowGrantCap = optionalInteger(body.windowGrantCap)
  const windowRef = optionalString(body.windowRef)

  return {
    accountRef: optionalString(body.accountRef) ?? '',
    ...(amountCapUsdCents === undefined ? {} : { amountCapUsdCents }),
    ...(amountUsdCents === undefined ? {} : { amountUsdCents }),
    ...(engagementRef === undefined ? {} : { engagementRef }),
    ...(grantRef === undefined ? {} : { grantRef }),
    sourceRefs: stringArrayFromUnknown(body.sourceRefs),
    ...(windowGrantCap === undefined ? {} : { windowGrantCap }),
    ...(windowRef === undefined ? {} : { windowRef }),
  }
}

const redemptionInputFromBody = (
  body: Record<string, unknown>,
): BusinessStarterCreditRedemptionInput => ({
  grantRef: optionalString(body.grantRef) ?? '',
  redemptionReceiptRef: optionalString(body.redemptionReceiptRef) ?? '',
})

const grantRefusalStatus = (reason: string): number =>
  reason === 'amount_cap_exceeded' || reason === 'window_cap_exceeded' ? 409 : 400

const routeCreateGrant = <Bindings>(
  dependencies: OperatorBusinessStarterCreditDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessStarterCreditStoreError
        ? error
        : new BusinessStarterCreditStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const body = await readJsonObject(request)
      const outcome = await dependencies
        .makeStore(env)
        .createGrant(pipelineRef, grantInputFromBody(body))

      if (!outcome.ok) {
        return noStoreJsonResponse(
          {
            error: 'business_starter_credit_refused',
            message: outcome.message,
            ok: false,
            reason: outcome.reason,
          },
          { status: grantRefusalStatus(outcome.reason) },
        )
      }

      return noStoreJsonResponse(
        {
          grant: outcome.grant,
          ok: true,
          pipelineReceiptRefs: outcome.pipelineReceiptRefs,
        },
        { status: 201 },
      )
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const routeLinkRedemption = <Bindings>(
  dependencies: OperatorBusinessStarterCreditDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pipelineRef: string,
) =>
  Effect.tryPromise({
    catch: error =>
      error instanceof BusinessStarterCreditStoreError
        ? error
        : new BusinessStarterCreditStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
    try: async () => {
      const denial = await requireOperator(dependencies, request, env)
      if (denial !== undefined) return denial
      const body = await readJsonObject(request)
      const grant = await dependencies
        .makeStore(env)
        .linkRedemption(pipelineRef, redemptionInputFromBody(body))
      return noStoreJsonResponse({ grant, ok: true })
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

export const makeOperatorBusinessStarterCreditRoutes = <Bindings>(
  dependencies: OperatorBusinessStarterCreditDependencies<Bindings>,
) => ({
  routeOperatorBusinessStarterCreditRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const match =
      /^\/api\/operator\/business\/pipeline\/([^/]+)\/(starter-credit-grants|starter-credit-redemptions)$/.exec(
        url.pathname,
      )

    if (match === null) return undefined

    if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))

    const pipelineRef = decodeURIComponent(match[1] ?? '')
    const action = match[2]

    return action === 'starter-credit-grants'
      ? routeCreateGrant(dependencies, request, env, pipelineRef)
      : routeLinkRedemption(dependencies, request, env, pipelineRef)
  },
})

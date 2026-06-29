import { Effect, Match as M } from 'effect'

import type { AgentRegistrationStore } from './agent-registration'
import type { AutopilotContinuationStore } from './autopilot-continuation-policy'
import {
  AUTOPILOT_MORNING_REPORT_DEFAULT_SINCE_HOURS,
  AUTOPILOT_MORNING_REPORT_MAX_SINCE_HOURS,
  autopilotMorningReportForOwner,
} from './autopilot-morning-report'
import type { AutopilotWorkStore } from './autopilot-work-routes'
import {
  authenticateCustomerOrderAgentRequest,
  CustomerOrderAgentAuthFailure,
} from './customer-order-agent-auth'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

type AutopilotMorningReportRouteEnv = Readonly<Record<string, unknown>>

type AutopilotMorningReportRoutesDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  makeContinuationStore: (env: Bindings) => AutopilotContinuationStore
  makeWorkStore: (env: Bindings) => AutopilotWorkStore
  nowIso?: () => string
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
}>

const hasBearerAuthorization = (request: Request): boolean =>
  request.headers.get('authorization')?.trim().toLowerCase().startsWith(
    'bearer ',
  ) === true

const sinceHoursFromRequest = (request: Request): number => {
  const raw = new URL(request.url).searchParams.get('sinceHours')
  const parsed = raw === null ? Number.NaN : Number.parseInt(raw, 10)

  return Number.isInteger(parsed) &&
    parsed >= 1 &&
    parsed <= AUTOPILOT_MORNING_REPORT_MAX_SINCE_HOURS
    ? parsed
    : AUTOPILOT_MORNING_REPORT_DEFAULT_SINCE_HOURS
}

const readMorningReport = <Bindings extends AutopilotMorningReportRouteEnv>(
  dependencies: AutopilotMorningReportRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const ownerUserId = yield* hasBearerAuthorization(request) ||
      dependencies.requireBrowserSession === undefined
      ? authenticateCustomerOrderAgentRequest(
          request,
          dependencies.agentStore(env),
          {
            nowIso: () => nowIso,
            requiredScope: 'customer_orders.read',
          },
        ).pipe(Effect.map(auth => auth.ownerUserId))
      : Effect.gen(function* () {
          const session = yield* Effect.tryPromise({
            catch: () => 'morning_report_storage_error' as const,
            try: () =>
              dependencies.requireBrowserSession?.(request, env, ctx) ??
              Promise.resolve(undefined),
          })

          if (session === undefined) {
            return yield* new CustomerOrderAgentAuthFailure({
              failureKind: 'missing_credentials',
              reason:
                'Autopilot morning report requires a browser session or agent token.',
            })
          }

          return session.user.userId
        })
    const sinceHours = sinceHoursFromRequest(request)
    const sinceIso = isoTimestampAfterIso(nowIso, -sinceHours * 60 * 60_000)
    const [workOrders, continuationEvents] = yield* Effect.tryPromise({
      catch: () => 'morning_report_storage_error' as const,
      try: () =>
        Promise.all([
          dependencies.makeWorkStore(env).listWorkOrdersForOwner({
            limit: 200,
            ownerUserId,
          }),
          dependencies
            .makeContinuationStore(env)
            .listEventsForUserSince(ownerUserId, sinceIso, 100),
        ]),
    })

    return noStoreJsonResponse({
      report: autopilotMorningReportForOwner({
        continuationEvents,
        nowIso,
        sinceIso,
        workOrders,
      }),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          {
            error: 'autopilot_morning_report_storage_error',
            reason: 'Autopilot morning report could not be composed.',
          },
          { status: 500 },
        ),
      )
    ),
  )

export const makeAutopilotMorningReportRoutes = <
  Bindings extends AutopilotMorningReportRouteEnv,
>(
  dependencies: AutopilotMorningReportRoutesDependencies<Bindings>,
) => ({
  routeAutopilotMorningReportRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> =>
    M.value(request.method).pipe(
      M.when('GET', () => readMorningReport(dependencies, request, env, ctx)),
      M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
    ),
})

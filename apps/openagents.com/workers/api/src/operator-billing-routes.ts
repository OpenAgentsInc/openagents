import { applyManualBillingCredit } from './billing'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  optionalInteger,
  optionalString,
  readRequestSelector,
} from './json-boundary'
import type { OperatorTargetUser } from './operator-targets'
import { openAgentsDatabase } from './runtime'

type OperatorBillingEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type OperatorBillingDependencies<Env extends OperatorBillingEnv> = Readonly<{
  readSelectedOperatorTargetUser: (
    db: D1Database,
    selector: Record<string, unknown>,
  ) => Promise<OperatorTargetUser | undefined>
  requireAdminApiToken: (request: Request, env: Env) => Promise<boolean>
}>

export const makeOperatorBillingHandlers = <Env extends OperatorBillingEnv>(
  dependencies: OperatorBillingDependencies<Env>,
) => ({
  handleOmniOperatorBillingCreditsApi: async (
    request: Request,
    env: Env,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      openAgentsDatabase(env),
      selector,
    )

    if (targetUser === undefined) {
      return noStoreJsonResponse(
        { error: 'target_user_not_found' },
        { status: 404 },
      )
    }

    const amountCents = optionalInteger(selector.amountCents)

    if (amountCents === undefined || amountCents <= 0) {
      return noStoreJsonResponse(
        {
          error: 'bad_request',
          reason: 'amountCents must be a positive integer',
          targetUser,
        },
        { status: 400 },
      )
    }

    const reason =
      optionalString(selector.reason) ?? 'Operator Autopilot credit adjustment'
    const idempotencyKey =
      optionalString(selector.idempotencyKey) ??
      `billing:operator-credit:${targetUser.userId}:${amountCents}:${reason}`
    const billing = await applyManualBillingCredit(openAgentsDatabase(env), {
      amountCents,
      idempotencyKey,
      reason,
      userId: targetUser.userId,
    })

    return noStoreJsonResponse({
      billing,
      targetUser,
    })
  },
})

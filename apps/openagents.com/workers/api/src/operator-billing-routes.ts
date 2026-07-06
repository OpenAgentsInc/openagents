import { Effect } from 'effect'

import { applyManualBillingCredit } from './billing'
import {
  billingRuntimeForEnv,
  type BillingSyncEnv,
} from './billing-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { fundInferenceFromCredit } from './inference/usd-credit-bridge'
import { identityDbForEnv, type IdentityDb } from './identity-db'
import { paymentsLedgerDbForEnv, type PaymentsLedgerDb, type PaymentsLedgerEnv } from './payments-ledger-db'
import {
  optionalInteger,
  optionalString,
  readRequestSelector,
} from './json-boundary'
import { compactRandomId } from './runtime-primitives'
import type { OperatorTargetUser } from './operator-targets'
import { openAgentsDatabase } from './runtime'

type OperatorBillingEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}> &
  BillingSyncEnv &
  PaymentsLedgerEnv

type OperatorBillingDependencies<RouteEnv extends OperatorBillingEnv> = Readonly<{
  readSelectedOperatorTargetUser: (
    identityDb: IdentityDb,
    selector: Record<string, unknown>,
  ) => Promise<OperatorTargetUser | undefined>
  // Target resolver for the inference-credit grant. Kind-agnostic on a direct
  // userId (human OR agent) so an agent account under test is a valid target —
  // the bridge funds `agent:<userId>` for either. Defaults to the human-only
  // `readSelectedOperatorTargetUser` when omitted (so existing callers/tests are
  // unchanged), but the Worker wires the agent-inclusive resolver.
  readSelectedInferenceCreditTargetUser?: (
    identityDb: IdentityDb,
    selector: Record<string, unknown>,
  ) => Promise<OperatorTargetUser | undefined>
  requireAdminApiToken: (request: Request, env: RouteEnv) => Promise<boolean>
  /** CFG-4 (#8519): injectable credits-ledger accessor (tests). Default:
   * `paymentsLedgerDbForEnv` — the Postgres-only production path. */
  ledgerDb?: (env: RouteEnv) => PaymentsLedgerDb
  /** CFG-4 Domain 2 (#8519): injectable identity accessor (tests). Default:
   * `identityDbForEnv` — the Postgres-only `users`/`auth_identities` path. */
  identityDb?: (env: RouteEnv) => IdentityDb
}>

export const makeOperatorBillingHandlers = <RouteEnv extends OperatorBillingEnv>(
  dependencies: OperatorBillingDependencies<RouteEnv>,
) => ({
  handleOmniOperatorBillingCreditsApi: async (
    request: Request,
    env: RouteEnv,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const targetUser = await dependencies.readSelectedOperatorTargetUser(
      (dependencies.identityDb ?? identityDbForEnv)(env),
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
    const billing = await applyManualBillingCredit(
      openAgentsDatabase(env),
      {
        amountCents,
        idempotencyKey,
        reason,
        userId: targetUser.userId,
      },
      billingRuntimeForEnv(env),
    )

    return noStoreJsonResponse({
      billing,
      targetUser,
    })
  },

  // POST /api/omni/operator/billing/inference-credit
  //
  // Admin-token-gated path that grants a SPENDABLE inference balance onto an
  // arbitrary target agent in ONE call, without a browser session or a Stripe
  // purchase. This is the programmatic mirror of the browser-only, self-scoped
  // `POST /api/billing/inference-credit` (#5497): the public bridge can only
  // fund the LOGGED-IN user's own balance, so there was no way to put a
  // purchased/granted (non-free) balance onto an agent under test on staging.
  //
  // It performs both #5497 halves atomically-enough for an operator: (1) grant
  // `amountCents` of USD credit to the target user's `billing_ledger_entries`
  // (idempotent manual adjustment), then (2) bridge that USD into the target's
  // agent balance as USD-origin `usd_credit_msat` (inference-spendable, NOT
  // Bitcoin-withdrawable — RL-3 asset boundary honored by the bridge). Same
  // admin-token auth and target-selector shape as the credits route above.
  handleOmniOperatorInferenceCreditApi: async (
    request: Request,
    env: RouteEnv,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (!(await dependencies.requireAdminApiToken(request, env))) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const selector = await readRequestSelector(request)
    const db = openAgentsDatabase(env)
    const resolveTarget =
      dependencies.readSelectedInferenceCreditTargetUser ??
      dependencies.readSelectedOperatorTargetUser
    const targetUser = await resolveTarget(
      (dependencies.identityDb ?? identityDbForEnv)(env),
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

    // One stable grantRef ties the USD grant and the msat bridge so a retry is
    // idempotent end-to-end (one ref = one USD adjustment = one msat grant).
    const grantRef =
      optionalString(selector.grantRef) ??
      `operator-inference-credit:${compactRandomId('grant')}`
    const reason =
      optionalString(selector.reason) ??
      'Operator inference credit (staging test grant)'

    // (1) USD credit (idempotent on the same grantRef).
    await applyManualBillingCredit(
      db,
      {
        amountCents,
        idempotencyKey: `billing:operator-inference-credit:${grantRef}`,
        reason,
        userId: targetUser.userId,
      },
      billingRuntimeForEnv(env),
    )

    // (2) Bridge USD -> spendable usd_credit_msat (idempotent on the grantRef).
    const outcome = await Effect.runPromise(
      fundInferenceFromCredit(
        { amountCents, grantRef, userId: targetUser.userId },
        { billingRuntime: billingRuntimeForEnv(env), db, ledgerDb: (dependencies.ledgerDb ?? paymentsLedgerDbForEnv)(env) },
      ),
    )

    if (!outcome.ok) {
      return noStoreJsonResponse(
        {
          error: outcome.reason,
          message: outcome.message,
          targetUser,
        },
        { status: 400 },
      )
    }

    return noStoreJsonResponse({
      grantedCents: outcome.grantedCents,
      grantedMsat: outcome.grantedMsat,
      grantRef: outcome.grantRef,
      receiptRef: outcome.receiptRef,
      remainingCreditCents: outcome.remainingCreditCents,
      status: 'inference_credit_granted',
      targetUser,
    })
  },
})

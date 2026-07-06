import { Effect, Schema as S } from 'effect'
import { noStoreJsonResponse } from './http/responses'
import {
  buildSelfServeLaborPayoutPlan,
  dispatchSelfServeLaborPayout,
  LaborSelfServePayoutInput,
} from './labor-self-serve-earning-payout'
import { readAgentBalance } from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { currentIsoTimestamp } from './runtime-primitives'

export type LaborSelfServePayoutAuth = (
  request: Request,
) => Promise<Readonly<{ actorRef: string }> | undefined>

export type LaborSelfServePayoutDeps = Readonly<{
  // CFG-4 (#8519): `agent_balances` is Postgres-authoritative; the balance
  // read goes through the credits-domain PaymentsLedgerDb (the old D1 `db`
  // dep had no other use here and is gone).
  ledgerDb: PaymentsLedgerDb
  authenticate: LaborSelfServePayoutAuth
  enabled: boolean
}>

export const handleSelfServeLaborPayoutApi = (
  request: Request,
  deps: LaborSelfServePayoutDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = yield* Effect.promise(async () => {
      try {
        const json = await request.json()
        return S.decodeUnknownSync(LaborSelfServePayoutInput)(json)
      } catch {
        return undefined
      }
    })

    if (body === undefined) {
      return noStoreJsonResponse({ error: 'bad_request' }, { status: 400 })
    }

    if (body.providerRef !== session.actorRef) {
      return noStoreJsonResponse({ error: 'forbidden' }, { status: 403 })
    }

    const balance = yield* Effect.promise(() => readAgentBalance(deps.ledgerDb, session.actorRef))
    const bitcoinWithdrawableMsat = balance?.bitcoinWithdrawableMsat ?? 0

    const planResult = buildSelfServeLaborPayoutPlan(body, {
      bitcoinWithdrawableMsat,
    }, currentIsoTimestamp())

    if (!planResult.ok) {
      return noStoreJsonResponse(
        { error: 'validation_failed', reason: planResult.error.reason },
        { status: 400 },
      )
    }

    const dispatchResult = yield* dispatchSelfServeLaborPayout(
      { enabled: deps.enabled },
      { plan: planResult.plan },
    )

    // INERT seam: we do not execute the payout or debit the ledger here yet.
    // The response reveals the typed plan + the flag-gated dispatch decision.
    return noStoreJsonResponse({
      plan: planResult.plan,
      dispatch: dispatchResult,
    })
  })

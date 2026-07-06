import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { readLaborEarnings } from './labor-earnings'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { currentIsoTimestamp } from './runtime-primitives'

// This route returns a public-projection-staleness compliant payload
export const handlePublicLaborEarningsApi = (
  request: Request,
  input: Readonly<{
    // CFG-4 (#8519): `labor_escrows` is Postgres-authoritative; the earnings
    // read goes through the credits-domain PaymentsLedgerDb (the old D1 `db`
    // dep had no other use here and is gone).
    ledgerDb: PaymentsLedgerDb
    nowIso?: () => string
  }>,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const url = new URL(request.url)
  const providerActorRef = url.searchParams.get('providerRef')
  if (providerActorRef === null || providerActorRef.trim() === '') {
    return Effect.succeed(
      noStoreJsonResponse({ error: 'providerRef is required' }, { status: 400 })
    )
  }

  const limitParam = url.searchParams.get('limit')
  const limit = limitParam !== null ? Math.min(100, Math.max(1, Number(limitParam) || 50)) : 50

  return Effect.promise(async () => {
    const generatedAt = input.nowIso?.() ?? currentIsoTimestamp()
    const projection = await readLaborEarnings(input.ledgerDb, providerActorRef, generatedAt, limit)
    return noStoreJsonResponse(projection)
  })
}

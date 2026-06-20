import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makePartnerAgreementRoutes } from './partner-agreement-routes'

// ---------------------------------------------------------------------------
// Combined in-memory D1 fake for the partner_agreements writer + reader paths.
//   - INSERT OR IGNORE keyed on agreement_ref            (recordPartnerAgreement)
//   - SELECT ... WHERE agreement_ref = ?  -> .first()    (idempotent read-back)
//   - SELECT ... WHERE customer_user_id = ? -> .all()    (reader / list route)
// ---------------------------------------------------------------------------

type StoredRow = {
  agreement_ref: string
  customer_user_id: string
  effective_from: string
  effective_until: string | null
  partner_ref: string
  partner_user_id: string
  role: 'affiliate' | 'design_partner' | 'referral'
}

class AgreementStore {
  rows = new Map<string, StoredRow>()
}

const agreementDb = (store: AgreementStore): D1Database => {
  const statement = (
    query: string,
    bound: ReadonlyArray<unknown> = [],
  ): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        const ref = String(bound[0] ?? '')

        return Promise.resolve((store.rows.get(ref) ?? null) as T | null)
      },
      all: <T,>() => {
        const customer = String(bound[0] ?? '')
        const results = [...store.rows.values()].filter(
          row => row.customer_user_id === customer,
        )

        return Promise.resolve({
          meta: {} as D1Meta,
          results: results as unknown as Array<T>,
          success: true,
        } as D1Result<T>)
      },
      run: () => {
        const [
          ,
          agreementRef,
          partnerRef,
          partnerUserId,
          customerUserId,
          role,
          from,
          until,
        ] = bound

        if (!store.rows.has(String(agreementRef))) {
          store.rows.set(String(agreementRef), {
            agreement_ref: String(agreementRef),
            customer_user_id: String(customerUserId),
            effective_from: String(from),
            effective_until: until === null ? null : String(until),
            partner_ref: String(partnerRef),
            partner_user_id: String(partnerUserId),
            role: role as StoredRow['role'],
          })
        }

        return Promise.resolve({
          meta: {} as D1Meta,
          results: [],
          success: true,
        } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw should not be used')),
    }) as unknown as D1PreparedStatement

  return {
    batch: () => Promise.reject(new Error('batch should not be used')),
    dump: () => Promise.reject(new Error('dump should not be used')),
    exec: () => Promise.reject(new Error('exec should not be used')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session should not be used')
    },
  } as unknown as D1Database
}

// ---------------------------------------------------------------------------
// Route harness
// ---------------------------------------------------------------------------

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

const nowIso = (): string => '2026-06-10T10:00:00.000Z'

const makeRoutes = (admin = true) =>
  makePartnerAgreementRoutes<TestEnv>({
    nowIso,
    requireAdminApiToken: () => Promise.resolve(admin),
  })

const ctx = {} as ExecutionContext

const runRequest = (
  routes: ReturnType<typeof makePartnerAgreementRoutes<TestEnv>>,
  store: AgreementStore,
  request: Request,
): Promise<Response> => {
  const effect = routes.routePartnerAgreementRequest(
    request,
    { OPENAGENTS_DB: agreementDb(store) },
    ctx,
  )

  if (effect === undefined) {
    throw new Error(`Route did not match: ${request.url}`)
  }

  return Effect.runPromise(effect)
}

const COLLECTION_URL = 'https://openagents.com/api/operator/partners/agreements'

const validSeed = {
  agreementRef: 'partner_agreement_acme',
  customerUserId: 'github:client',
  effectiveFromIso: '2026-01-01T00:00:00.000Z',
  effectiveUntilIso: null,
  partnerRef: 'design_partner_acme',
  partnerUserId: 'github:acme_agency',
  role: 'design_partner',
}

const postSeed = (body: unknown): Request =>
  new Request(COLLECTION_URL, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('Partner agreement routes — create', () => {
  test('seeds a policy-conformant agreement and projects it', async () => {
    const store = new AgreementStore()
    const response = await runRequest(makeRoutes(), store, postSeed(validSeed))

    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      agreement: { agreementRef: string; partnerRole: string }
    }
    expect(json.agreement).toMatchObject({
      agreementRef: 'partner_agreement_acme',
      partnerRef: 'design_partner_acme',
      partnerRole: 'design_partner',
      partnerUserId: 'github:acme_agency',
    })
    // customerUserId is NOT echoed (PartnerAgreement omits it).
    expect(json.agreement).not.toHaveProperty('customerUserId')
    expect(store.rows.size).toBe(1)
  })

  test('is idempotent on agreementRef (replay seeds no second row)', async () => {
    const store = new AgreementStore()
    const routes = makeRoutes()

    await runRequest(routes, store, postSeed(validSeed))
    const repeat = await runRequest(routes, store, postSeed(validSeed))

    expect(repeat.status).toBe(200)
    expect(store.rows.size).toBe(1)
  })

  test('rejects a referral-role seed as a 422 (referral rail owns it)', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(),
      store,
      postSeed({ ...validSeed, role: 'referral' }),
    )

    expect(response.status).toBe(422)
    const json = (await response.json()) as { error: string; reason: string }
    expect(json.error).toBe('partner_agreement_rejected')
    expect(json.reason).toContain('referral')
    expect(store.rows.size).toBe(0)
  })

  test('rejects a self-agreement as a 422', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(),
      store,
      postSeed({ ...validSeed, partnerUserId: 'github:client' }),
    )

    expect(response.status).toBe(422)
    expect(store.rows.size).toBe(0)
  })

  test('rejects an inverted effective window as a 422', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(),
      store,
      postSeed({
        ...validSeed,
        effectiveFromIso: '2026-06-01T00:00:00.000Z',
        effectiveUntilIso: '2026-01-01T00:00:00.000Z',
      }),
    )

    expect(response.status).toBe(422)
    expect(store.rows.size).toBe(0)
  })

  test('malformed body is a 400 bad request', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(),
      store,
      postSeed({ agreementRef: 'x' }),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { error: string }
    expect(json.error).toBe('bad_request')
  })

  test('create is rejected with 401 when admin token is absent', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(false),
      store,
      postSeed(validSeed),
    )

    expect(response.status).toBe(401)
    expect(store.rows.size).toBe(0)
  })
})

describe('Partner agreement routes — list', () => {
  test('GET returns active agreements for the customer', async () => {
    const store = new AgreementStore()
    const routes = makeRoutes()
    await runRequest(routes, store, postSeed(validSeed))

    const response = await runRequest(
      routes,
      store,
      new Request(`${COLLECTION_URL}?customerUserId=github:client`, {
        method: 'GET',
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      agreements: ReadonlyArray<{ agreementRef: string }>
    }
    expect(json.agreements).toHaveLength(1)
    expect(json.agreements[0]?.agreementRef).toBe('partner_agreement_acme')
  })

  test('GET without customerUserId is a 400', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(),
      store,
      new Request(COLLECTION_URL, { method: 'GET' }),
    )

    expect(response.status).toBe(400)
  })

  test('GET is rejected with 401 when admin token is absent', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(false),
      store,
      new Request(`${COLLECTION_URL}?customerUserId=github:client`, {
        method: 'GET',
      }),
    )

    expect(response.status).toBe(401)
  })
})

describe('Partner agreement routes — matching', () => {
  test('non-matching paths return undefined (route passthrough)', () => {
    const routes = makeRoutes()
    const effect = routes.routePartnerAgreementRequest(
      new Request('https://openagents.com/api/operator/something-else', {
        method: 'GET',
      }),
      { OPENAGENTS_DB: agreementDb(new AgreementStore()) },
      ctx,
    )

    expect(effect).toBeUndefined()
  })

  test('unsupported method on the collection path is a 405', async () => {
    const store = new AgreementStore()
    const response = await runRequest(
      makeRoutes(),
      store,
      new Request(COLLECTION_URL, { method: 'DELETE' }),
    )

    expect(response.status).toBe(405)
  })
})

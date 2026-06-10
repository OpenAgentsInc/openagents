import { ErrorCorrectionLevel, QRCode } from '@liquid-js/qrcode-generator'
import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { isoTimestampAfterIso } from './runtime-primitives'

type HttpResponse = globalThis.Response

export type TreasuryTransactionRecord = Readonly<{
  id: string
  direction: 'in' | 'out'
  amountSat: number
  state: 'pending' | 'settled' | 'expired'
  bolt11: string | null
  paymentRef: string | null
  createdAt: string
  settledAt: string | null
  expiresAt: string | null
}>

export type TreasuryTransactionStore = Readonly<{
  insert: (record: TreasuryTransactionRecord) => Promise<void>
  listRecent: (limit: number) => Promise<ReadonlyArray<TreasuryTransactionRecord>>
  read: (id: string) => Promise<TreasuryTransactionRecord | undefined>
  settle: (input: {
    amountSat: number
    id: string
    settledAt: string
  }) => Promise<void>
  expire: (input: { expiredAt: string; id: string }) => Promise<void>
}>

type TreasuryTransactionRow = Readonly<{
  id: string
  direction: 'in' | 'out'
  amount_sat: number
  state: 'pending' | 'settled' | 'expired'
  bolt11: string | null
  payment_ref: string | null
  created_at: string
  settled_at: string | null
  expires_at: string | null
}>

const rowToRecord = (row: TreasuryTransactionRow): TreasuryTransactionRecord => ({
  amountSat: row.amount_sat,
  bolt11: row.bolt11,
  createdAt: row.created_at,
  direction: row.direction,
  expiresAt: row.expires_at,
  id: row.id,
  paymentRef: row.payment_ref,
  settledAt: row.settled_at,
  state: row.state,
})

export const makeD1TreasuryTransactionStore = (
  db: D1Database,
): TreasuryTransactionStore => ({
  expire: async input => {
    await db
      .prepare(
        `UPDATE treasury_transactions
         SET state = 'expired', settled_at = NULL
         WHERE id = ? AND state = 'pending'`,
      )
      .bind(input.id)
      .run()
  },
  insert: async record => {
    await db
      .prepare(
        `INSERT INTO treasury_transactions
           (id, direction, amount_sat, state, bolt11, payment_ref,
            created_at, settled_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.direction,
        record.amountSat,
        record.state,
        record.bolt11,
        record.paymentRef,
        record.createdAt,
        record.settledAt,
        record.expiresAt,
      )
      .run()
  },
  listRecent: async limit => {
    const result = await db
      .prepare(
        `SELECT * FROM treasury_transactions
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(limit)
      .all<TreasuryTransactionRow>()

    return (result.results ?? []).map(rowToRecord)
  },
  read: async id => {
    const row = await db
      .prepare(`SELECT * FROM treasury_transactions WHERE id = ?`)
      .bind(id)
      .first<TreasuryTransactionRow>()

    return row === null ? undefined : rowToRecord(row)
  },
  settle: async input => {
    await db
      .prepare(
        `UPDATE treasury_transactions
         SET state = 'settled', amount_sat = ?, settled_at = ?
         WHERE id = ? AND state = 'pending'`,
      )
      .bind(input.amountSat, input.settledAt, input.id)
      .run()
  },
})

export type TreasuryPageRouteDependencies = Readonly<{
  fetchTreasury?: ContainerPathFetch | undefined
  makeUuid: () => string
  nowIso: () => string
  store?: TreasuryTransactionStore | undefined
}>

const TRANSACTION_LIST_LIMIT = 20

// Public projection: amount, time, direction, state only. Never the
// recipient, destination, payment hash, or invoice of other people's rows.
const publicTransaction = (record: TreasuryTransactionRecord) => ({
  amountSat: record.state === 'settled' ? record.amountSat : null,
  createdAt: record.createdAt,
  direction: record.direction,
  settledAt: record.settledAt,
  state: record.state,
})

const treasuryBalance = (
  fetchTreasury: ContainerPathFetch,
): Effect.Effect<{ balanceSat: number; maxSendableSat: number | null } | null> =>
  Effect.tryPromise({
    catch: () => null,
    try: async () => {
      const response = await fetchTreasury('/balance')

      if (!response.ok) {
        return null
      }

      const payload = (await response.json()) as Record<string, unknown>

      return typeof payload.balanceSat === 'number'
        ? {
            balanceSat: payload.balanceSat,
            maxSendableSat:
              typeof payload.maxSendableSat === 'number'
                ? payload.maxSendableSat
                : null,
          }
        : null
    },
  }).pipe(Effect.catch(() => Effect.succeed(null)))

const invoiceQrSvg = (invoice: string): string => {
  try {
    const qr = new QRCode(0, ErrorCorrectionLevel.L)

    qr.addData(`LIGHTNING:${invoice.toUpperCase()}`, 'alphanumeric')
    qr.make()

    return qr.createSvgTag({ cellSize: 4, margin: 4, scalable: true })
  } catch {
    return ''
  }
}

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')

// Styled after the openagents.com homepage: black, white, Berkeley Mono.
const pageShell = (title: string, body: string, refresh: boolean): string =>
  `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
${refresh ? '<meta http-equiv="refresh" content="12" />' : ''}
<title>${escapeHtml(title)} | OpenAgents</title>
<style>
@font-face { font-family: 'Berkeley Mono'; src: url('/fonts/nMono-Regular.woff2') format('woff2'); font-display: swap; }
body { background:#000; color:#fff; font-family:'Berkeley Mono', ui-monospace, SFMono-Regular, Menlo, monospace; display:flex; justify-content:center; padding:48px 16px; }
main { max-width: 640px; width:100%; }
h1 { font-size: 18px; color:#fff; text-transform: uppercase; letter-spacing: 0.08em; }
p { line-height: 1.6; font-size: 14px; color:#a1a1aa; }
a { color:#fff; }
pre { background:#0a0a0a; border:1px solid #27272a; padding:12px; white-space:pre-wrap; word-break:break-all; font-size:12px; border-radius:6px; color:#e4e4e7; }
a.button { display:inline-block; background:#fff; color:#000; padding:10px 16px; border-radius:6px; text-decoration:none; font-weight:bold; margin-top:8px; }
.qr { background:#fff; border-radius:8px; padding:8px; width:fit-content; max-width:320px; margin:16px 0; }
.qr svg { display:block; width:100%; max-width:304px; height:auto; }
.balance { font-size:32px; color:#fff; margin:8px 0 0; }
.balance small { font-size:14px; color:#a1a1aa; }
table { width:100%; border-collapse:collapse; margin-top:16px; font-size:13px; }
th { text-align:left; color:#71717a; font-weight:normal; border-bottom:1px solid #27272a; padding:6px 8px; text-transform:uppercase; letter-spacing:0.06em; font-size:11px; }
td { border-bottom:1px solid #18181b; padding:8px; color:#e4e4e7; }
.in { color:#fff; }
.out { color:#a1a1aa; }
.ok { color:#fff; }
.muted { color:#71717a; font-size:12px; }
</style>
</head>
<body><main>${body}</main></body>
</html>`

const htmlPage = (
  title: string,
  body: string,
  options: Readonly<{ refresh?: boolean; status?: number }> = {},
): HttpResponse =>
  new Response(pageShell(title, body, options.refresh === true), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
    status: options.status ?? 200,
  })

const transactionRows = (
  transactions: ReadonlyArray<TreasuryTransactionRecord>,
): string =>
  transactions.length === 0
    ? '<p class="muted">No transactions yet.</p>'
    : `<table>
<tr><th>Time (UTC)</th><th>Direction</th><th>Amount</th><th>State</th></tr>
${transactions
  .map(record => {
    const projection = publicTransaction(record)
    const time = escapeHtml(
      (projection.settledAt ?? projection.createdAt).slice(0, 16).replace('T', ' '),
    )
    const direction =
      projection.direction === 'in'
        ? '<span class="in">+ in</span>'
        : '<span class="out">- out</span>'
    const amount =
      projection.amountSat === null
        ? '<span class="muted">-</span>'
        : `${projection.amountSat} sats`

    return `<tr><td>${time}</td><td>${direction}</td><td>${amount}</td><td>${escapeHtml(projection.state)}</td></tr>`
  })
  .join('\n')}
</table>`

const donationExpired = (
  record: TreasuryTransactionRecord,
  nowIso: string,
): boolean =>
  record.expiresAt !== null && record.state === 'pending' && record.expiresAt < nowIso

export const handlePublicTreasuryApi = (
  request: Request,
  dependencies: TreasuryPageRouteDependencies,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const fetchTreasury = dependencies.fetchTreasury
  const store = dependencies.store

  return Effect.flatMap(
    fetchTreasury === undefined
      ? Effect.succeed(null)
      : treasuryBalance(fetchTreasury),
    balance =>
      Effect.map(
        Effect.tryPromise({
          catch: () => [] as ReadonlyArray<TreasuryTransactionRecord>,
          try: () =>
            store === undefined
              ? Promise.resolve([] as ReadonlyArray<TreasuryTransactionRecord>)
              : store.listRecent(TRANSACTION_LIST_LIMIT),
        }).pipe(
          Effect.catch(() =>
            Effect.succeed([] as ReadonlyArray<TreasuryTransactionRecord>),
          ),
        ),
        transactions =>
          noStoreJsonResponse({
            balance,
            service: 'mdk_treasury',
            transactions: transactions.map(publicTransaction),
          }),
      ),
  )
}

const treasuryIndexPage = (
  dependencies: TreasuryPageRouteDependencies,
): Effect.Effect<HttpResponse> => {
  const fetchTreasury = dependencies.fetchTreasury
  const store = dependencies.store

  return Effect.flatMap(
    fetchTreasury === undefined
      ? Effect.succeed(null)
      : treasuryBalance(fetchTreasury),
    balance =>
      Effect.map(
        Effect.tryPromise({
          catch: () => [] as ReadonlyArray<TreasuryTransactionRecord>,
          try: () =>
            store === undefined
              ? Promise.resolve([] as ReadonlyArray<TreasuryTransactionRecord>)
              : store.listRecent(TRANSACTION_LIST_LIMIT),
        }).pipe(
          Effect.catch(() =>
            Effect.succeed([] as ReadonlyArray<TreasuryTransactionRecord>),
          ),
        ),
        transactions =>
          htmlPage(
            'Treasury',
            `<h1>OpenAgents Treasury</h1>
<p>The campaign treasury pays bounded rewards to agents and contributors.
Balance and recent activity are public; recipients are not.</p>
${
  balance === null
    ? '<p class="muted">Balance temporarily unavailable.</p>'
    : `<p class="balance">${balance.balanceSat} sats <small>(${balance.maxSendableSat ?? 0} spendable)</small></p>`
}
<a class="button" href="/treasury/donate">Donate</a>
<h1 style="margin-top:32px">Recent transactions</h1>
${transactionRows(transactions)}
<p class="muted">Updated live from the treasury node. JSON: <a href="/api/public/treasury">/api/public/treasury</a></p>`,
          ),
      ),
  )
}

const createDonation = (
  dependencies: TreasuryPageRouteDependencies,
): Effect.Effect<HttpResponse> => {
  const fetchTreasury = dependencies.fetchTreasury
  const store = dependencies.store

  if (fetchTreasury === undefined || store === undefined) {
    return Effect.succeed(
      htmlPage(
        'Donations unavailable',
        `<h1>Donations unavailable</h1>
<p>The treasury is not accepting donations right now. Try again later.</p>`,
        { status: 503 },
      ),
    )
  }

  return Effect.tryPromise({
    catch: () => null,
    try: async () => {
      const response = await fetchTreasury('/donation-invoice', {
        method: 'POST',
      })

      if (!response.ok) {
        return null
      }

      const payload = (await response.json()) as Record<string, unknown>

      if (
        typeof payload.bolt11 !== 'string' ||
        typeof payload.paymentHash !== 'string'
      ) {
        return null
      }

      const nowIso = dependencies.nowIso()
      const record: TreasuryTransactionRecord = {
        amountSat: 0,
        bolt11: payload.bolt11,
        createdAt: nowIso,
        direction: 'in',
        // Invoice expiry is 3600s from mint; derive from nowIso instead of
        // converting the epoch payload so no raw Date constructor is needed.
        expiresAt: isoTimestampAfterIso(nowIso, 3_600_000),
        id: `treasury_donation_${dependencies.makeUuid()}`,
        paymentRef: payload.paymentHash.toLowerCase(),
        settledAt: null,
        state: 'pending',
      }

      await store.insert(record)

      return record.id
    },
  }).pipe(
    Effect.catch(() => Effect.succeed(null)),
    Effect.map(donationId =>
      donationId === null
        ? htmlPage(
            'Donations unavailable',
            `<h1>Donations unavailable</h1>
<p>A donation invoice could not be created right now. Try again in a few
seconds.</p>`,
            { status: 503 },
          )
        : new Response(null, {
            headers: {
              'cache-control': 'no-store',
              location: `/treasury/donations/${donationId}`,
            },
            status: 303,
          }),
    ),
  )
}

const donationPage = (
  dependencies: TreasuryPageRouteDependencies,
  donationId: string,
): Effect.Effect<HttpResponse> => {
  const fetchTreasury = dependencies.fetchTreasury
  const store = dependencies.store

  if (store === undefined) {
    return Effect.succeed(
      htmlPage(
        'Donation not found',
        `<h1>Donation not found</h1><p>This donation does not exist.</p>`,
        { status: 404 },
      ),
    )
  }

  return Effect.tryPromise({
    catch: () => undefined,
    try: async () => {
      const record = await store.read(donationId)

      if (record === undefined || record.direction !== 'in') {
        return undefined
      }

      const nowIso = dependencies.nowIso()

      if (record.state === 'pending' && fetchTreasury !== undefined && record.paymentRef !== null) {
        const response = await fetchTreasury(`/received/${record.paymentRef}`)

        if (response.ok) {
          const payload = (await response.json()) as Record<string, unknown>

          if (payload.received === true) {
            const amountSat =
              typeof payload.amountSat === 'number' ? payload.amountSat : 0

            await store.settle({ amountSat, id: record.id, settledAt: nowIso })

            return { ...record, amountSat, settledAt: nowIso, state: 'settled' as const }
          }
        }

        if (donationExpired(record, nowIso)) {
          await store.expire({ expiredAt: nowIso, id: record.id })

          return { ...record, state: 'expired' as const }
        }
      }

      return record
    },
  }).pipe(
    Effect.catch(() => Effect.succeed(undefined)),
    Effect.map(record => {
      if (record === undefined) {
        return htmlPage(
          'Donation not found',
          `<h1>Donation not found</h1><p>This donation does not exist.</p>`,
          { status: 404 },
        )
      }

      if (record.state === 'settled') {
        return htmlPage(
          'Donation received',
          `<h1>Donation received</h1>
<p class="ok">Thank you. ${record.amountSat} sats are now in the treasury and
will fund agent rewards.</p>
<a class="button" href="/treasury">Back to treasury</a>`,
        )
      }

      if (record.state === 'expired') {
        return htmlPage(
          'Donation expired',
          `<h1>Donation expired</h1>
<p>This invoice expired before payment.</p>
<a class="button" href="/treasury/donate">Get a fresh invoice</a>`,
          { status: 410 },
        )
      }

      const invoice = record.bolt11 ?? ''
      const qrSvg = invoice === '' ? '' : invoiceQrSvg(invoice)

      return htmlPage(
        'Donate to the treasury',
        `<h1>Donate to the treasury</h1>
<p>Scan with a Lightning wallet and choose any amount, or pay the BOLT11
invoice below. This page refreshes automatically and confirms when your
donation arrives.</p>
${qrSvg === '' ? '' : `<div class="qr">${qrSvg}</div>`}
<pre>${escapeHtml(invoice)}</pre>
<a class="button" href="lightning:${escapeHtml(invoice)}">Open in wallet</a>
<p class="muted">Status: pending - waiting for payment.</p>`,
        { refresh: true },
      )
    }),
  )
}

export const makeTreasuryPageRoutes = (
  dependencies: TreasuryPageRouteDependencies,
) => ({
  routeTreasuryPageRequest: (
    request: Request,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/public/treasury') {
      return handlePublicTreasuryApi(request, dependencies)
    }

    if (url.pathname === '/treasury') {
      return request.method === 'GET'
        ? treasuryIndexPage(dependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    if (url.pathname === '/treasury/donate') {
      return request.method === 'GET'
        ? createDonation(dependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const donationMatch = /^\/treasury\/donations\/([A-Za-z0-9_-]{8,80})$/.exec(
      url.pathname,
    )

    if (donationMatch !== null) {
      return request.method === 'GET'
        ? donationPage(dependencies, donationMatch[1] ?? '')
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    return undefined
  },
})

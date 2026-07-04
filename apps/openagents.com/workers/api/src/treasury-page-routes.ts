import { ErrorCorrectionLevel, QRCode } from '@liquid-js/qrcode-generator'
import { Effect } from 'effect'

import type { ContainerPathFetch } from './http/container-fetch'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { isoTimestampAfterIso } from './runtime-primitives'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  treasuryRead,
  type TreasuryDatabase,
} from './treasury-domain-store'

type HttpResponse = globalThis.Response

export type TreasuryTransactionRecord = Readonly<{
  id: string
  direction: 'in' | 'out'
  amountSat: number
  state: 'pending' | 'settled' | 'expired' | 'failed'
  bolt11: string | null
  paymentRef: string | null
  failureReasonRef: string | null
  owedRef: string | null
  owedSat: number | null
  recipientConfirmationRef: string | null
  recipientConfirmationState: 'unconfirmed' | 'confirmed_received'
  recipientConfirmedAt: string | null
  recipientRef: string | null
  redactedDestinationRef: string | null
  createdAt: string
  settledAt: string | null
  expiresAt: string | null
}>

export type TreasuryTransactionStore = Readonly<{
  insert: (record: TreasuryTransactionRecord) => Promise<void>
  listRecent: (
    limit: number,
  ) => Promise<ReadonlyArray<TreasuryTransactionRecord>>
  listPendingOutbound: (
    limit: number,
  ) => Promise<ReadonlyArray<TreasuryTransactionRecord>>
  listByRecipient: (
    input: Readonly<{ limit: number; recipientRef: string }>,
  ) => Promise<ReadonlyArray<TreasuryTransactionRecord>>
  read: (id: string) => Promise<TreasuryTransactionRecord | undefined>
  settle: (input: {
    amountSat: number
    id: string
    settledAt: string
  }) => Promise<void>
  confirmReceived: (input: {
    confirmationRef: string
    id: string
    recipientConfirmedAt: string
  }) => Promise<void>
  expire: (input: { expiredAt: string; id: string }) => Promise<void>
  fail: (input: { id: string }) => Promise<void>
}>

type TreasuryTransactionRow = Readonly<{
  id: string
  direction: 'in' | 'out'
  amount_sat: number
  state: 'pending' | 'settled' | 'expired' | 'failed'
  bolt11: string | null
  payment_ref: string | null
  failure_reason_ref?: string | null
  owed_ref?: string | null
  owed_sat?: number | null
  recipient_confirmation_ref?: string | null
  recipient_confirmation_state?: 'unconfirmed' | 'confirmed_received' | null
  recipient_confirmed_at?: string | null
  recipient_ref?: string | null
  redacted_destination_ref?: string | null
  created_at: string
  settled_at: string | null
  expires_at: string | null
}>

const rowToRecord = (
  row: TreasuryTransactionRow,
): TreasuryTransactionRecord => ({
  amountSat: row.amount_sat,
  bolt11: row.bolt11,
  createdAt: row.created_at,
  direction: row.direction,
  expiresAt: row.expires_at,
  failureReasonRef: row.failure_reason_ref ?? null,
  id: row.id,
  owedRef: row.owed_ref ?? null,
  owedSat: row.owed_sat ?? null,
  paymentRef: row.payment_ref,
  recipientConfirmationRef: row.recipient_confirmation_ref ?? null,
  recipientConfirmationState: row.recipient_confirmation_state ?? 'unconfirmed',
  recipientConfirmedAt: row.recipient_confirmed_at ?? null,
  recipientRef: row.recipient_ref ?? null,
  redactedDestinationRef: row.redacted_destination_ref ?? null,
  settledAt: row.settled_at,
  state: row.state,
})

/**
 * KS-8.8 (#8319): D1 stays the sole authority for every method below; on a
 * `TreasuryDatabase` seam handle each WRITE additionally read-back-mirrors
 * the touched row into Postgres fail-soft, and `listRecent` (the public
 * treasury page projection) becomes flag-routable
 * (KHALA_SYNC_TREASURY_READS: d1 | compare | postgres). The
 * `listPendingOutbound` scan DRIVES the TreasuryTransactions.reconcilePending
 * cron's settlement side effects, so it reads exactly one store (D1) with
 * no Postgres twin until the epic-gated cutover. A bare D1Database behaves
 * exactly as before.
 */
export const makeD1TreasuryTransactionStore = (
  database: TreasuryDatabase,
): TreasuryTransactionStore => {
  const db = treasuryAuthorityDb(database)
  const mirror = (id: string) =>
    mirrorTreasuryRows(database, 'treasury_transactions', 'id', [id])
  return {
  expire: async input => {
    await db
      .prepare(
        `UPDATE treasury_transactions
         SET state = 'expired', settled_at = NULL
         WHERE id = ? AND state = 'pending'`,
      )
      .bind(input.id)
      .run()
    await mirror(input.id)
  },
  fail: async input => {
    await db
      .prepare(
        `UPDATE treasury_transactions
         SET state = 'failed', settled_at = NULL
         WHERE id = ? AND state = 'pending'`,
      )
      .bind(input.id)
      .run()
    await mirror(input.id)
  },
  insert: async record => {
    await db
      .prepare(
        `INSERT INTO treasury_transactions
           (id, direction, amount_sat, state, bolt11, payment_ref,
            failure_reason_ref, recipient_ref, redacted_destination_ref,
            owed_ref, owed_sat, recipient_confirmation_state,
            recipient_confirmation_ref, recipient_confirmed_at,
            created_at, settled_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.direction,
        record.amountSat,
        record.state,
        record.bolt11,
        record.paymentRef,
        record.failureReasonRef,
        record.recipientRef,
        record.redactedDestinationRef,
        record.owedRef,
        record.owedSat,
        record.recipientConfirmationState,
        record.recipientConfirmationRef,
        record.recipientConfirmedAt,
        record.createdAt,
        record.settledAt,
        record.expiresAt,
      )
      .run()
    await mirror(record.id)
  },
  listRecent: async limit =>
    treasuryRead(
      database,
      'treasury_transactions:list_recent',
      [],
      async () => {
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
      async postgres => {
        const rows = await postgres.selectLatestRows(
          'treasury_transactions',
          limit,
        )
        // bigint columns come back driver-typed; normalize to D1 numbers
        // so compare mode diffs semantics, not driver representations.
        return rows.map(row =>
          rowToRecord({
            ...(row as unknown as TreasuryTransactionRow),
            amount_sat: Number(row['amount_sat']),
            owed_sat:
              row['owed_sat'] === null || row['owed_sat'] === undefined
                ? null
                : Number(row['owed_sat']),
          }),
        )
      },
    ),
  listByRecipient: async input => {
    const result = await db
      .prepare(
        `SELECT * FROM treasury_transactions
         WHERE recipient_ref = ?
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(input.recipientRef, input.limit)
      .all<TreasuryTransactionRow>()

    return (result.results ?? []).map(rowToRecord)
  },
  listPendingOutbound: async limit => {
    const result = await db
      .prepare(
        `SELECT * FROM treasury_transactions
         WHERE direction = 'out'
           AND state = 'pending'
           AND payment_ref IS NOT NULL
         ORDER BY created_at ASC
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
    await mirror(input.id)
  },
  confirmReceived: async input => {
    await db
      .prepare(
        `UPDATE treasury_transactions
         SET recipient_confirmation_state = 'confirmed_received',
             recipient_confirmation_ref = ?,
             recipient_confirmed_at = ?
         WHERE id = ?
           AND direction = 'out'
           AND state = 'settled'`,
      )
      .bind(input.confirmationRef, input.recipientConfirmedAt, input.id)
      .run()
    await mirror(input.id)
  },
  }
}

export type TreasuryPageRouteDependencies = Readonly<{
  fetchTreasury?: ContainerPathFetch | undefined
  makeUuid: () => string
  nowIso: () => string
  store?: TreasuryTransactionStore | undefined
}>

const TRANSACTION_LIST_LIMIT = 20

// Public projection: amount, time, direction, state only. Never the
// recipient, destination, payment hash, or invoice of other people's rows.
// Unpaid inbound invoice mints are not transactions and never render -
// only settled rows and real outbound payouts (which always carry their
// amount) are listed.
const isPublicTransaction = (record: TreasuryTransactionRecord): boolean =>
  record.state === 'settled' ||
  (record.direction === 'out' && record.state !== 'expired')

const publicTransaction = (record: TreasuryTransactionRecord) => ({
  amountSat: record.amountSat,
  createdAt: record.createdAt,
  direction: record.direction,
  settledAt: record.settledAt,
  state: record.state,
})

// A flat "most recent N" list gets dominated by outbound payouts during busy
// periods, hiding settled inbound funding/donations entirely. Fetch a wider
// slice and keep the most-recent public rows of EACH direction so inbound
// transfers always surface alongside outbound.
const RECENT_FETCH_LIMIT = 250

const balancedRecentTransactions = (
  records: ReadonlyArray<TreasuryTransactionRecord>,
): ReadonlyArray<TreasuryTransactionRecord> => {
  const visible = records.filter(isPublicTransaction)
  const sortKey = (record: TreasuryTransactionRecord): string =>
    record.settledAt ?? record.createdAt
  const inbound = visible
    .filter(record => record.direction === 'in')
    .slice(0, TRANSACTION_LIST_LIMIT)
  const outbound = visible
    .filter(record => record.direction === 'out')
    .slice(0, TRANSACTION_LIST_LIMIT)
  return [...inbound, ...outbound].sort((a, b) =>
    sortKey(b).localeCompare(sortKey(a)),
  )
}

type TreasuryRailBalance = Readonly<{
  balanceSat: number | null
  maxSendableSat: number | null
  rail: 'mdk' | 'spark'
  state: 'ok' | 'unavailable'
}>

type TreasuryBalanceProjection = Readonly<{
  balanceSat: number
  maxSendableSat: number | null
  rails: ReadonlyArray<TreasuryRailBalance>
}>

const railBalancePayload = (
  rail: TreasuryRailBalance['rail'],
  payload: unknown,
): TreasuryRailBalance | null => {
  if (typeof payload !== 'object' || payload === null) {
    return null
  }

  const record = payload as Record<string, unknown>

  return typeof record.balanceSat === 'number'
    ? {
        balanceSat: record.balanceSat,
        maxSendableSat:
          typeof record.maxSendableSat === 'number'
            ? record.maxSendableSat
            : null,
        rail,
        state: 'ok',
      }
    : null
}

const unavailableRail = (
  rail: TreasuryRailBalance['rail'],
): TreasuryRailBalance => ({
  balanceSat: null,
  maxSendableSat: null,
  rail,
  state: 'unavailable',
})

const readRailBalance = (
  fetchTreasury: ContainerPathFetch,
  rail: TreasuryRailBalance['rail'],
  path: string,
): Promise<TreasuryRailBalance> =>
  fetchTreasury(path)
    .then(async response =>
      response.ok
        ? (railBalancePayload(rail, await response.json()) ??
          unavailableRail(rail))
        : unavailableRail(rail),
    )
    .catch(() => unavailableRail(rail))

const totalTreasuryBalance = (
  rails: ReadonlyArray<TreasuryRailBalance>,
): TreasuryBalanceProjection | null => {
  const available = rails.filter(
    (rail): rail is TreasuryRailBalance & { balanceSat: number } =>
      rail.state === 'ok' && typeof rail.balanceSat === 'number',
  )

  if (available.length === 0) {
    return null
  }

  const maxSendableValues = available
    .map(rail => rail.maxSendableSat)
    .filter(
      (maxSendableSat): maxSendableSat is number =>
        typeof maxSendableSat === 'number',
    )

  return {
    balanceSat: available.reduce((sum, rail) => sum + rail.balanceSat, 0),
    maxSendableSat:
      maxSendableValues.length === 0
        ? null
        : maxSendableValues.reduce(
            (sum, maxSendableSat) => sum + maxSendableSat,
            0,
          ),
    rails,
  }
}

const treasuryBalance = (
  fetchTreasury: ContainerPathFetch,
): Effect.Effect<TreasuryBalanceProjection | null> =>
  Effect.tryPromise({
    catch: () => null,
    try: async () => {
      const rails = await Promise.all([
        readRailBalance(fetchTreasury, 'mdk', '/balance'),
        readRailBalance(fetchTreasury, 'spark', '/spark/balance'),
      ])

      return totalTreasuryBalance(rails)
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
.rail-breakout { color:#71717a; font-size:12px; margin-top:4px; }
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
): string => {
  const visible = transactions.filter(isPublicTransaction)

  return visible.length === 0
    ? '<p class="muted">No transactions yet.</p>'
    : `<table>
<tr><th>Time (UTC)</th><th>Direction</th><th>Amount</th><th>State</th></tr>
${visible
  .map(record => {
    const projection = publicTransaction(record)
    const time = escapeHtml(
      (projection.settledAt ?? projection.createdAt)
        .slice(0, 16)
        .replace('T', ' '),
    )
    const direction =
      projection.direction === 'in'
        ? '<span class="in">+ in</span>'
        : '<span class="out">- out</span>'

    return `<tr><td>${time}</td><td>${direction}</td><td>${projection.amountSat} sats</td><td>${escapeHtml(projection.state)}</td></tr>`
  })
  .join('\n')}
</table>`
}

const balanceRailBreakout = (
  balance: TreasuryBalanceProjection | null,
): string => {
  if (balance === null) {
    return ''
  }

  const label = (rail: TreasuryRailBalance): string => {
    const name = rail.rail === 'mdk' ? 'MDK' : 'Spark'

    return rail.state === 'ok' && rail.balanceSat !== null
      ? `${name}: ${rail.balanceSat} sats`
      : `${name}: unavailable`
  }

  return `<p class="rail-breakout">${balance.rails.map(label).join(' · ')}</p>`
}

const donationExpired = (
  record: TreasuryTransactionRecord,
  nowIso: string,
): boolean =>
  record.expiresAt !== null &&
  record.state === 'pending' &&
  record.expiresAt < nowIso

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
              : store.listRecent(RECENT_FETCH_LIMIT),
        }).pipe(
          Effect.catch(() =>
            Effect.succeed([] as ReadonlyArray<TreasuryTransactionRecord>),
          ),
        ),
        transactions =>
          noStoreJsonResponse({
            balance,
            service: 'mdk_treasury',
            transactions: balancedRecentTransactions(transactions).map(
              publicTransaction,
            ),
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
              : store.listRecent(RECENT_FETCH_LIMIT),
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
    : `<p class="balance">${balance.balanceSat} sats <small>(${balance.maxSendableSat ?? 0} spendable)</small></p>${balanceRailBreakout(balance)}`
}
<a class="button" href="/treasury/donate">Donate</a>
<h1 style="margin-top:32px">Recent transactions</h1>
${transactionRows(balancedRecentTransactions(transactions))}
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
        failureReasonRef: null,
        id: `treasury_donation_${dependencies.makeUuid()}`,
        owedRef: null,
        owedSat: null,
        paymentRef: payload.paymentHash.toLowerCase(),
        recipientConfirmationRef: null,
        recipientConfirmationState: 'unconfirmed',
        recipientConfirmedAt: null,
        recipientRef: null,
        redactedDestinationRef: null,
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

      if (
        record.state === 'pending' &&
        fetchTreasury !== undefined &&
        record.paymentRef !== null
      ) {
        const response = await fetchTreasury(`/received/${record.paymentRef}`)

        if (response.ok) {
          const payload = (await response.json()) as Record<string, unknown>

          if (payload.received === true) {
            const amountSat =
              typeof payload.amountSat === 'number' ? payload.amountSat : 0

            await store.settle({ amountSat, id: record.id, settledAt: nowIso })

            return {
              ...record,
              amountSat,
              settledAt: nowIso,
              state: 'settled' as const,
            }
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
    Effect.catch(() => Effect.void),
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

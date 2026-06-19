import type { AgentRunRecord, OmniEventRecord } from './omni-runs'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import { sourceRefForTokenUsageEvent, tokenUsageFromEvent } from './token-usage'

export const BILLING_CURRENCY = 'USD'
export const INITIAL_TRIAL_CREDIT_CENTS = 1_000
export const CONTAINER_RATE_CENTS_PER_MINUTE = 5
export const CODEX_RATE_CENTS_PER_THOUSAND_TOKENS = 2
export const MINIMUM_RUN_CREDIT_CENTS = CONTAINER_RATE_CENTS_PER_MINUTE

export type BillingLedgerSource =
  | 'trial_grant'
  | 'coupon'
  | 'credit_card_placeholder'
  | 'stripe_checkout'
  | 'stripe_auto_top_up'
  | 'container_usage'
  | 'codex_usage'
  | 'manual_adjustment'

export type BillingLedgerEntry = Readonly<{
  id: string
  source: BillingLedgerSource
  description: string
  amountCents: number
  amountFormatted: string
  quantity: number | null
  unit: string | null
  createdAt: string
}>

export type BillingActiveRun = Readonly<{
  id: string
  title: string
  status: string
  accruedSeconds: number
  estimatedDebitCents: number
  estimatedDebitFormatted: string
  startedAt: string | null
}>

export type BillingSavedPaymentMethod = Readonly<{
  brand: string | null
  expMonth: number | null
  expYear: number | null
  last4: string | null
  status: 'active' | 'detached' | 'failed' | 'requires_action'
  stripePaymentMethodId: string
  updatedAt: string
}>

export type BillingAutoTopUpPolicy = Readonly<{
  amountCents: number
  amountFormatted: string
  enabled: boolean
  monthlyCapCents: number
  monthlyCapFormatted: string
  pauseReason: string | null
  spentThisMonthCents: number
  spentThisMonthFormatted: string
  status: 'active' | 'disabled' | 'paused'
  thresholdCents: number
  thresholdFormatted: string
  updatedAt: string
}>

export type BillingAutoTopUpEvent = Readonly<{
  amountCents: number
  amountFormatted: string
  createdAt: string
  id: string
  reason: string | null
  status:
    | 'cap_reached'
    | 'declined'
    | 'requires_payment_method'
    | 'skipped'
    | 'succeeded'
}>

export type BillingAutoTopUpState = Readonly<{
  events: ReadonlyArray<BillingAutoTopUpEvent>
  policy: BillingAutoTopUpPolicy
  savedPaymentMethod: BillingSavedPaymentMethod | null
}>

// The purchasable credit catalog, as the UI should render it. This is the
// projection of the server-configured Stripe catalog
// (`STRIPE_CREDIT_PACKAGES_JSON`) into display-ready fields so the billing page
// always offers exactly the package ids the checkout endpoint accepts. The
// price `id` here is the real catalog id POSTed back to `/api/billing/checkout`.
export type BillingCreditPackageDisplay = Readonly<{
  id: string
  label: string
  amountCents: number
  amountFormatted: string
  currency: 'USD'
}>

export type BillingSummary = Readonly<{
  currency: 'USD'
  status: 'active' | 'suspended'
  balanceCents: number
  balanceFormatted: string
  minimumRunCreditCents: number
  minimumRunCreditFormatted: string
  rates: Readonly<{
    containerCentsPerMinute: number
    codexCentsPerThousandTokens: number
  }>
  // The purchasable catalog the UI renders buy buttons from. Defaults to an
  // empty list; only browser-facing producers that can read the Stripe config
  // (the billing routes and the authenticated bootstrap) populate it. An empty
  // catalog means "card checkout is not configured here" and the UI shows no
  // purchasable packages rather than a stale hardcoded list.
  packages: ReadonlyArray<BillingCreditPackageDisplay>
  recentEntries: ReadonlyArray<BillingLedgerEntry>
  activeRuns: ReadonlyArray<BillingActiveRun>
  autoTopUp: BillingAutoTopUpState
}>

export type BillingCreditExhaustionResult =
  | Readonly<{
      balanceCents: number
      balanceFormatted: string
      exhausted: false
      newlySuspended: false
    }>
  | Readonly<{
      balanceCents: number
      balanceFormatted: string
      exhausted: true
      newlySuspended: boolean
    }>

export type OutOfCreditsNotificationReservation =
  | Readonly<{
      displayName: string
      email: string
      idempotencyKey: string
      ok: true
    }>
  | Readonly<{
      ok: false
      reason: 'already_sent' | 'missing_email'
    }>

type BalanceRow = Readonly<{ balance_cents: number | null }>
type AccountRow = Readonly<{ status: 'active' | 'suspended' }>
type BillingContactRow = Readonly<{
  display_name: string
  primary_email: string | null
}>
type NotificationRow = Readonly<{ status: 'pending' | 'sent' | 'failed' }>
type LedgerRow = Readonly<{
  id: string
  source: BillingLedgerSource
  description: string
  amount_cents: number
  quantity: number | null
  unit: string | null
  created_at: string
}>
type ActiveRunRow = Readonly<{
  id: string
  goal: string
  status: string
  started_at: string | null
  updated_at: string
  last_billed_at: string | null
}>
type UsageCursorRow = Readonly<{
  last_billed_at: string
  total_billed_quantity: number
}>
type SavedPaymentMethodRow = Readonly<{
  brand: string | null
  exp_month: number | null
  exp_year: number | null
  last4: string | null
  status: BillingSavedPaymentMethod['status']
  stripe_payment_method_id: string
  updated_at: string
}>
type AutoTopUpPolicyRow = Readonly<{
  amount_cents: number
  enabled: number
  monthly_cap_cents: number
  pause_reason: string | null
  spent_this_month_cents: number
  status: BillingAutoTopUpPolicy['status']
  threshold_cents: number
  updated_at: string
}>
type AutoTopUpEventRow = Readonly<{
  amount_cents: number
  created_at: string
  id: string
  reason: string | null
  status: BillingAutoTopUpEvent['status']
}>

export type BillingRuntime = Readonly<{
  nowIso: () => string
  randomId: (prefix: string) => string
}>

export const systemBillingRuntime: BillingRuntime = {
  nowIso: currentIsoTimestamp,
  randomId: compactRandomId,
}

const metadataJson = (value: unknown): string => JSON.stringify(value ?? {})

const compactText = (value: string, maxLength: number): string => {
  const compact = value.replace(/\s+/g, ' ').trim()

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, Math.max(0, maxLength - 3))}...`
}

export const formatUsdCents = (amountCents: number): string => {
  const amount = Math.trunc(amountCents)
  const sign = amount < 0 ? '-' : ''
  const absolute = Math.abs(amount)

  return `${sign}$${(absolute / 100).toFixed(2)}`
}

export const calculateContainerUsageDebitCents = (
  seconds: number,
  rateCentsPerMinute = CONTAINER_RATE_CENTS_PER_MINUTE,
): number => {
  const quantity = Math.max(0, Math.trunc(seconds))

  if (quantity === 0 || rateCentsPerMinute <= 0) {
    return 0
  }

  return Math.max(1, Math.ceil((quantity * rateCentsPerMinute) / 60))
}

export const calculateCodexUsageDebitCents = (
  totalTokens: number,
  rateCentsPerThousandTokens = CODEX_RATE_CENTS_PER_THOUSAND_TOKENS,
): number => {
  const quantity = Math.max(0, Math.trunc(totalTokens))

  if (quantity === 0 || rateCentsPerThousandTokens <= 0) {
    return 0
  }

  return Math.max(1, Math.ceil((quantity * rateCentsPerThousandTokens) / 1_000))
}

export const secondsBetweenIso = (startIso: string, endIso: string): number => {
  const startMs = Date.parse(startIso)
  const endMs = Date.parse(endIso)

  if (
    !Number.isFinite(startMs) ||
    !Number.isFinite(endMs) ||
    endMs <= startMs
  ) {
    return 0
  }

  return Math.max(0, Math.ceil((endMs - startMs) / 1_000))
}

export const normalizeCouponCode = (value: string): string =>
  value.trim().toUpperCase().replace(/\s+/g, '-')

export const shouldSuspendBillingBalance = (balanceCents: number): boolean =>
  Math.trunc(balanceCents) <= 0

const couponCreditCents = (couponCode: string): number | undefined => {
  switch (normalizeCouponCode(couponCode)) {
    case 'OPENAGENTS-TRIAL':
      return 2_500
    case 'FOUNDER-100':
      return 10_000
    case 'SHC-SMOKE':
      return 1_000
    default:
      return undefined
  }
}

export const ensureBillingAccount = async (
  db: D1Database,
  userId: string,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<void> => {
  const now = runtime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_accounts
          (user_id, currency, status, created_at, updated_at)
         VALUES (?, ?, 'active', ?, ?)`,
      )
      .bind(userId, BILLING_CURRENCY, now, now),
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES (?, ?, NULL, NULL, 'trial_grant', ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
      )
      .bind(
        runtime.randomId('bill'),
        userId,
        'OpenAgents launch credits',
        INITIAL_TRIAL_CREDIT_CENTS,
        BILLING_CURRENCY,
        INITIAL_TRIAL_CREDIT_CENTS,
        'credit_cents',
        metadataJson({ reason: 'initial_launch_credit' }),
        `billing:trial:${userId}`,
        now,
      ),
  ])
}

const readBalanceCents = async (
  db: D1Database,
  userId: string,
): Promise<number> => {
  const row = await db
    .prepare(
      `SELECT COALESCE(SUM(amount_cents), 0) AS balance_cents
       FROM billing_ledger_entries
       WHERE user_id = ?`,
    )
    .bind(userId)
    .first<BalanceRow>()

  return Math.trunc(Number(row?.balance_cents ?? 0))
}

export const readBillingBalanceCents = readBalanceCents

const readAccountStatus = async (
  db: D1Database,
  userId: string,
): Promise<'active' | 'suspended'> => {
  const row = await db
    .prepare(`SELECT status FROM billing_accounts WHERE user_id = ?`)
    .bind(userId)
    .first<AccountRow>()

  return row?.status ?? 'active'
}

const readRecentLedgerEntries = async (
  db: D1Database,
  userId: string,
): Promise<ReadonlyArray<BillingLedgerEntry>> => {
  const rows = await db
    .prepare(
      `SELECT id, source, description, amount_cents, quantity, unit, created_at
       FROM billing_ledger_entries
       WHERE user_id = ?
       ORDER BY created_at DESC
       LIMIT 12`,
    )
    .bind(userId)
    .all<LedgerRow>()

  return rows.results.map(row => ({
    id: row.id,
    source: row.source,
    description: row.description,
    amountCents: row.amount_cents,
    amountFormatted: formatUsdCents(row.amount_cents),
    quantity: row.quantity,
    unit: row.unit,
    createdAt: row.created_at,
  }))
}

const readActiveRuns = async (
  db: D1Database,
  userId: string,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<ReadonlyArray<BillingActiveRun>> => {
  const rows = await db
    .prepare(
      `SELECT r.id, r.goal, r.status, r.started_at, r.updated_at,
              c.last_billed_at
       FROM agent_runs r
       LEFT JOIN billing_usage_cursors c
         ON c.run_id = r.id AND c.meter = 'container_seconds'
       WHERE r.user_id = ?
         AND r.status IN ('queued', 'running', 'waiting_for_input')
       ORDER BY r.updated_at DESC
       LIMIT 20`,
    )
    .bind(userId)
    .all<ActiveRunRow>()
  const now = runtime.nowIso()

  return rows.results.map(row => {
    const start = row.last_billed_at ?? row.started_at
    const accruedSeconds =
      start === null
        ? 0
        : secondsBetweenIso(start, row.updated_at > now ? row.updated_at : now)
    const estimatedDebitCents =
      calculateContainerUsageDebitCents(accruedSeconds)

    return {
      id: row.id,
      title: compactText(row.goal, 72) || row.id,
      status: row.status,
      accruedSeconds,
      estimatedDebitCents,
      estimatedDebitFormatted: formatUsdCents(-estimatedDebitCents),
      startedAt: row.started_at,
    }
  })
}

const defaultAutoTopUpPolicy = (
  runtime: BillingRuntime = systemBillingRuntime,
): BillingAutoTopUpPolicy => ({
  amountCents: 2_500,
  amountFormatted: formatUsdCents(2_500),
  enabled: false,
  monthlyCapCents: 10_000,
  monthlyCapFormatted: formatUsdCents(10_000),
  pauseReason: null,
  spentThisMonthCents: 0,
  spentThisMonthFormatted: formatUsdCents(0),
  status: 'disabled',
  thresholdCents: 500,
  thresholdFormatted: formatUsdCents(500),
  updatedAt: runtime.nowIso(),
})

const policyProjection = (row: AutoTopUpPolicyRow): BillingAutoTopUpPolicy => ({
  amountCents: row.amount_cents,
  amountFormatted: formatUsdCents(row.amount_cents),
  enabled: row.enabled === 1,
  monthlyCapCents: row.monthly_cap_cents,
  monthlyCapFormatted: formatUsdCents(row.monthly_cap_cents),
  pauseReason: row.pause_reason,
  spentThisMonthCents: row.spent_this_month_cents,
  spentThisMonthFormatted: formatUsdCents(row.spent_this_month_cents),
  status: row.status,
  thresholdCents: row.threshold_cents,
  thresholdFormatted: formatUsdCents(row.threshold_cents),
  updatedAt: row.updated_at,
})

export const readBillingAutoTopUpState = async (
  db: D1Database,
  userId: string,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingAutoTopUpState> => {
  const [paymentMethod, policy, events] = await Promise.all([
    db
      .prepare(
        `SELECT stripe_payment_method_id, brand, last4, exp_month, exp_year,
                status, updated_at
         FROM stripe_saved_payment_methods
         WHERE user_id = ? AND currency = ? AND livemode = 0`,
      )
      .bind(userId, BILLING_CURRENCY)
      .first<SavedPaymentMethodRow>(),
    db
      .prepare(
        `SELECT enabled, threshold_cents, amount_cents, monthly_cap_cents,
                spent_this_month_cents, status, pause_reason, updated_at
         FROM billing_auto_top_up_policies
         WHERE user_id = ? AND currency = ?`,
      )
      .bind(userId, BILLING_CURRENCY)
      .first<AutoTopUpPolicyRow>(),
    db
      .prepare(
        `SELECT id, status, amount_cents, reason, created_at
         FROM billing_auto_top_up_events
         WHERE user_id = ?
         ORDER BY created_at DESC
         LIMIT 6`,
      )
      .bind(userId)
      .all<AutoTopUpEventRow>(),
  ])

  return {
    events: events.results.map(row => ({
      amountCents: row.amount_cents,
      amountFormatted: formatUsdCents(row.amount_cents),
      createdAt: row.created_at,
      id: row.id,
      reason: row.reason,
      status: row.status,
    })),
    policy:
      policy === null
        ? defaultAutoTopUpPolicy(runtime)
        : policyProjection(policy),
    savedPaymentMethod:
      paymentMethod === null
        ? null
        : {
            brand: paymentMethod.brand,
            expMonth: paymentMethod.exp_month,
            expYear: paymentMethod.exp_year,
            last4: paymentMethod.last4,
            status: paymentMethod.status,
            stripePaymentMethodId: paymentMethod.stripe_payment_method_id,
            updatedAt: paymentMethod.updated_at,
          },
  }
}

export const upsertBillingAutoTopUpPolicy = async (
  db: D1Database,
  input: Readonly<{
    amountCents: number
    enabled: boolean
    monthlyCapCents: number
    thresholdCents: number
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingSummary> => {
  await ensureBillingAccount(db, input.userId, runtime)

  const thresholdCents = Math.max(100, Math.trunc(input.thresholdCents))
  const amountCents = Math.max(500, Math.trunc(input.amountCents))
  const monthlyCapCents = Math.max(
    amountCents,
    Math.trunc(input.monthlyCapCents),
  )
  const now = runtime.nowIso()

  await db
    .prepare(
      `INSERT INTO billing_auto_top_up_policies
        (user_id, currency, enabled, threshold_cents, amount_cents,
         monthly_cap_cents, spent_this_month_cents, cap_period_yyyymm,
         status, pause_reason, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, 0, substr(?, 1, 7), ?, NULL, ?, ?)
       ON CONFLICT(user_id, currency) DO UPDATE SET
         enabled = excluded.enabled,
         threshold_cents = excluded.threshold_cents,
         amount_cents = excluded.amount_cents,
         monthly_cap_cents = excluded.monthly_cap_cents,
         status = excluded.status,
         pause_reason = NULL,
         updated_at = excluded.updated_at`,
    )
    .bind(
      input.userId,
      BILLING_CURRENCY,
      input.enabled ? 1 : 0,
      thresholdCents,
      amountCents,
      monthlyCapCents,
      now,
      input.enabled ? 'active' : 'disabled',
      now,
      now,
    )
    .run()

  return readBillingSummary(db, input.userId, runtime)
}

export const pauseBillingAutoTopUpPolicy = async (
  db: D1Database,
  input: Readonly<{ reason: string; userId: string }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<void> => {
  const now = runtime.nowIso()

  await db
    .prepare(
      `UPDATE billing_auto_top_up_policies
       SET enabled = 0,
           status = 'paused',
           pause_reason = ?,
           updated_at = ?
       WHERE user_id = ? AND currency = ?`,
    )
    .bind(compactText(input.reason, 240), now, input.userId, BILLING_CURRENCY)
    .run()
}

export const recordBillingAutoTopUpEvent = async (
  db: D1Database,
  input: Readonly<{
    amountCents: number
    balanceAfterCents?: number | undefined
    balanceBeforeCents?: number | undefined
    idempotencyKey: string
    ledgerEntryId?: string | null | undefined
    paymentIntentId?: string | null | undefined
    reason?: string | null | undefined
    status: BillingAutoTopUpEvent['status']
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<void> => {
  const now = runtime.nowIso()

  await db
    .prepare(
      `INSERT OR IGNORE INTO billing_auto_top_up_events
        (id, user_id, status, amount_cents, currency, balance_before_cents,
         balance_after_cents, stripe_payment_intent_id, ledger_entry_id,
         reason, idempotency_key, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      runtime.randomId('topup'),
      input.userId,
      input.status,
      Math.max(0, Math.trunc(input.amountCents)),
      BILLING_CURRENCY,
      input.balanceBeforeCents ?? null,
      input.balanceAfterCents ?? null,
      input.paymentIntentId ?? null,
      input.ledgerEntryId ?? null,
      input.reason === undefined || input.reason === null
        ? null
        : compactText(input.reason, 240),
      input.idempotencyKey,
      now,
    )
    .run()
}

export const readBillingSummary = async (
  db: D1Database,
  userId: string,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingSummary> => {
  await ensureBillingAccount(db, userId, runtime)

  const [status, balanceCents, recentEntries, activeRuns, autoTopUp] =
    await Promise.all([
      readAccountStatus(db, userId),
      readBalanceCents(db, userId),
      readRecentLedgerEntries(db, userId),
      readActiveRuns(db, userId, runtime),
      readBillingAutoTopUpState(db, userId, runtime),
    ])

  return {
    currency: BILLING_CURRENCY,
    status,
    balanceCents,
    balanceFormatted: formatUsdCents(balanceCents),
    minimumRunCreditCents: MINIMUM_RUN_CREDIT_CENTS,
    minimumRunCreditFormatted: formatUsdCents(MINIMUM_RUN_CREDIT_CENTS),
    rates: {
      containerCentsPerMinute: CONTAINER_RATE_CENTS_PER_MINUTE,
      codexCentsPerThousandTokens: CODEX_RATE_CENTS_PER_THOUSAND_TOKENS,
    },
    // The catalog is owned by the Stripe config, which is not available on this
    // pure D1 read path. Browser-facing producers attach the real catalog with
    // `withBillingCreditPackages` before returning to the client.
    packages: [],
    recentEntries,
    activeRuns,
    autoTopUp,
  }
}

// Attach the purchasable credit catalog to a billing summary for browser
// responses. Pure and total: it never reads I/O and simply replaces the
// (default empty) `packages` projection so the UI renders buy buttons that
// match exactly what `/api/billing/checkout` will accept.
export const withBillingCreditPackages = (
  summary: BillingSummary,
  packages: ReadonlyArray<BillingCreditPackageDisplay>,
): BillingSummary => ({ ...summary, packages })

export const requireMinimumRunCredits = async (
  db: D1Database,
  userId: string,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<
  | Readonly<{ ok: true; billing: BillingSummary }>
  | Readonly<{ ok: false; billing: BillingSummary; message: string }>
> => {
  const billing = await readBillingSummary(db, userId, runtime)

  if (
    billing.status !== 'active' ||
    billing.balanceCents < MINIMUM_RUN_CREDIT_CENTS
  ) {
    return {
      ok: false,
      billing,
      message: `Add credits before launching Autopilot. Minimum launch balance is ${billing.minimumRunCreditFormatted}.`,
    }
  }

  return { ok: true, billing }
}

export const applyManualBillingCredit = async (
  db: D1Database,
  input: Readonly<{
    amountCents: number
    idempotencyKey: string
    reason: string
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingSummary> => {
  await ensureBillingAccount(db, input.userId, runtime)

  const amountCents = Math.max(1, Math.trunc(input.amountCents))
  const now = runtime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES (?, ?, NULL, NULL, 'manual_adjustment', ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
      )
      .bind(
        runtime.randomId('bill'),
        input.userId,
        compactText(input.reason, 160) || 'Operator credit',
        amountCents,
        BILLING_CURRENCY,
        amountCents,
        'credit_cents',
        metadataJson({ reason: input.reason }),
        input.idempotencyKey,
        now,
      ),
    db
      .prepare(
        `UPDATE billing_accounts
         SET status = 'active',
             updated_at = ?
         WHERE user_id = ?`,
      )
      .bind(now, input.userId),
  ])

  return readBillingSummary(db, input.userId, runtime)
}

export const applyStripeCheckoutCredit = async (
  db: D1Database,
  input: Readonly<{
    amountCents: number
    packageId: string
    sessionId: string
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingSummary> => {
  await ensureBillingAccount(db, input.userId, runtime)

  const amountCents = Math.max(1, Math.trunc(input.amountCents))
  const now = runtime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES (?, ?, NULL, NULL, 'stripe_checkout', ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
      )
      .bind(
        runtime.randomId('bill'),
        input.userId,
        'Stripe credit purchase',
        amountCents,
        BILLING_CURRENCY,
        amountCents,
        'credit_cents',
        metadataJson({
          packageId: input.packageId,
          sessionId: input.sessionId,
        }),
        `billing:stripe-checkout:${input.sessionId}`,
        now,
      ),
    db
      .prepare(
        `UPDATE billing_accounts
         SET status = 'active',
             updated_at = ?
         WHERE user_id = ?`,
      )
      .bind(now, input.userId),
  ])

  return readBillingSummary(db, input.userId, runtime)
}

export const applyStripeAutoTopUpCredit = async (
  db: D1Database,
  input: Readonly<{
    amountCents: number
    balanceBeforeCents: number
    idempotencyKey: string
    paymentIntentId: string
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingSummary> => {
  await ensureBillingAccount(db, input.userId, runtime)

  const amountCents = Math.max(1, Math.trunc(input.amountCents))
  const now = runtime.nowIso()
  const period = now.slice(0, 7)

  await db.batch([
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES (?, ?, NULL, NULL, 'stripe_auto_top_up', ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
      )
      .bind(
        runtime.randomId('bill'),
        input.userId,
        'Stripe auto top-up',
        amountCents,
        BILLING_CURRENCY,
        amountCents,
        'credit_cents',
        metadataJson({ paymentIntentId: input.paymentIntentId }),
        input.idempotencyKey,
        now,
      ),
    db
      .prepare(
        `UPDATE billing_accounts
         SET status = 'active',
             updated_at = ?
         WHERE user_id = ?`,
      )
      .bind(now, input.userId),
    db
      .prepare(
        `UPDATE billing_auto_top_up_policies
         SET spent_this_month_cents =
               CASE
                 WHEN cap_period_yyyymm = ? THEN spent_this_month_cents + ?
                 ELSE ?
               END,
             cap_period_yyyymm = ?,
             updated_at = ?
         WHERE user_id = ? AND currency = ?`,
      )
      .bind(
        period,
        amountCents,
        amountCents,
        period,
        now,
        input.userId,
        BILLING_CURRENCY,
      ),
  ])

  const ledger = await db
    .prepare(`SELECT id FROM billing_ledger_entries WHERE idempotency_key = ?`)
    .bind(input.idempotencyKey)
    .first<Readonly<{ id: string }>>()
  const balanceAfterCents = await readBalanceCents(db, input.userId)

  await recordBillingAutoTopUpEvent(
    db,
    {
      amountCents,
      balanceAfterCents,
      balanceBeforeCents: input.balanceBeforeCents,
      idempotencyKey: `${input.idempotencyKey}:event`,
      ledgerEntryId: ledger?.id ?? null,
      paymentIntentId: input.paymentIntentId,
      status: 'succeeded',
      userId: input.userId,
    },
    runtime,
  )

  return readBillingSummary(db, input.userId, runtime)
}

export const suspendBillingAccountIfOutOfCredits = async (
  db: D1Database,
  userId: string,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<BillingCreditExhaustionResult> => {
  await ensureBillingAccount(db, userId, runtime)

  const [balanceCents, status] = await Promise.all([
    readBalanceCents(db, userId),
    readAccountStatus(db, userId),
  ])
  const balanceFormatted = formatUsdCents(balanceCents)

  if (!shouldSuspendBillingBalance(balanceCents)) {
    return {
      balanceCents,
      balanceFormatted,
      exhausted: false,
      newlySuspended: false,
    }
  }

  const now = runtime.nowIso()
  const newlySuspended = status !== 'suspended'

  if (newlySuspended) {
    await db
      .prepare(
        `UPDATE billing_accounts
         SET status = 'suspended',
             updated_at = ?
         WHERE user_id = ?`,
      )
      .bind(now, userId)
      .run()
  }

  return {
    balanceCents,
    balanceFormatted,
    exhausted: true,
    newlySuspended,
  }
}

export const reserveOutOfCreditsNotification = async (
  db: D1Database,
  input: Readonly<{ balanceCents: number; userId: string }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<OutOfCreditsNotificationReservation> => {
  const existing = await db
    .prepare(
      `SELECT status
       FROM billing_credit_notifications
       WHERE user_id = ? AND kind = 'out_of_credits'`,
    )
    .bind(input.userId)
    .first<NotificationRow>()

  if (existing?.status === 'sent') {
    return { ok: false, reason: 'already_sent' }
  }

  const contact = await db
    .prepare(
      `SELECT display_name, primary_email
       FROM users
       WHERE id = ?`,
    )
    .bind(input.userId)
    .first<BillingContactRow>()
  const email = contact?.primary_email?.trim()

  if (email === undefined || email === '') {
    return { ok: false, reason: 'missing_email' }
  }

  const displayName = contact?.display_name.trim() || 'there'
  const idempotencyKey = `billing:out-of-credits:${input.userId}`
  const now = runtime.nowIso()

  if (existing === null) {
    await db
      .prepare(
        `INSERT INTO billing_credit_notifications
          (user_id, kind, email, display_name, balance_cents, status,
           resend_email_id, error_message, idempotency_key, created_at,
           updated_at)
         VALUES (?, 'out_of_credits', ?, ?, ?, 'pending',
           NULL, NULL, ?, ?, ?)`,
      )
      .bind(
        input.userId,
        email,
        displayName,
        input.balanceCents,
        idempotencyKey,
        now,
        now,
      )
      .run()
  } else {
    await db
      .prepare(
        `UPDATE billing_credit_notifications
         SET email = ?,
             display_name = ?,
             balance_cents = ?,
             status = 'pending',
             resend_email_id = NULL,
             error_message = NULL,
             idempotency_key = ?,
             updated_at = ?
         WHERE user_id = ? AND kind = 'out_of_credits'`,
      )
      .bind(
        email,
        displayName,
        input.balanceCents,
        idempotencyKey,
        now,
        input.userId,
      )
      .run()
  }

  return {
    displayName,
    email,
    idempotencyKey,
    ok: true,
  }
}

export const markOutOfCreditsNotificationSent = async (
  db: D1Database,
  input: Readonly<{
    resendEmailId: string | null
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<void> => {
  const now = runtime.nowIso()

  await db
    .prepare(
      `UPDATE billing_credit_notifications
       SET status = 'sent',
           resend_email_id = ?,
           error_message = NULL,
           updated_at = ?
       WHERE user_id = ? AND kind = 'out_of_credits'`,
    )
    .bind(input.resendEmailId, now, input.userId)
    .run()
}

export const markOutOfCreditsNotificationFailed = async (
  db: D1Database,
  input: Readonly<{
    errorMessage: string
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<void> => {
  const now = runtime.nowIso()

  await db
    .prepare(
      `UPDATE billing_credit_notifications
       SET status = 'failed',
           error_message = ?,
           updated_at = ?
       WHERE user_id = ? AND kind = 'out_of_credits'`,
    )
    .bind(compactText(input.errorMessage, 500), now, input.userId)
    .run()
}

export const redeemBillingCoupon = async (
  db: D1Database,
  input: Readonly<{ couponCode: string; userId: string }>,
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<
  | Readonly<{ ok: true; billing: BillingSummary; message: string }>
  | Readonly<{
      ok: false
      billing: BillingSummary
      error: string
      message: string
    }>
> => {
  const couponCode = normalizeCouponCode(input.couponCode)
  await ensureBillingAccount(db, input.userId, runtime)
  const existing = await db
    .prepare(
      `SELECT ledger_entry_id
       FROM billing_coupon_redemptions
       WHERE user_id = ? AND coupon_code = ?`,
    )
    .bind(input.userId, couponCode)
    .first<Readonly<{ ledger_entry_id: string }>>()

  if (existing !== null) {
    return {
      ok: false,
      billing: await readBillingSummary(db, input.userId, runtime),
      error: 'coupon_already_redeemed',
      message: 'That coupon has already been redeemed on this account.',
    }
  }

  const amountCents = couponCreditCents(couponCode)

  if (amountCents === undefined) {
    return {
      ok: false,
      billing: await readBillingSummary(db, input.userId, runtime),
      error: 'invalid_coupon',
      message: 'Coupon code not recognized.',
    }
  }

  const now = runtime.nowIso()
  const ledgerEntryId = runtime.randomId('bill')

  await db.batch([
    db
      .prepare(
        `INSERT INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES (?, ?, NULL, NULL, 'coupon', ?, ?, ?, ?, ?, NULL, ?, ?, ?)`,
      )
      .bind(
        ledgerEntryId,
        input.userId,
        `Coupon ${couponCode}`,
        amountCents,
        BILLING_CURRENCY,
        amountCents,
        'credit_cents',
        metadataJson({ couponCode }),
        `billing:coupon:${input.userId}:${couponCode}`,
        now,
      ),
    db
      .prepare(
        `INSERT INTO billing_coupon_redemptions
          (user_id, coupon_code, ledger_entry_id, redeemed_at)
         VALUES (?, ?, ?, ?)`,
      )
      .bind(input.userId, couponCode, ledgerEntryId, now),
    db
      .prepare(
        `UPDATE billing_accounts
         SET status = 'active',
             updated_at = ?
         WHERE user_id = ?`,
      )
      .bind(now, input.userId),
  ])

  return {
    ok: true,
    billing: await readBillingSummary(db, input.userId, runtime),
    message: `${formatUsdCents(amountCents)} credit applied.`,
  }
}

export const codexUsageDebitInsert = (
  db: D1Database,
  input: Readonly<{
    event: OmniEventRecord
    teamId: string | null
    userId: string
  }>,
  runtime: BillingRuntime = systemBillingRuntime,
): D1PreparedStatement | undefined => {
  const usage = tokenUsageFromEvent(input.event)

  if (usage === undefined) {
    return undefined
  }

  const amountCents = calculateCodexUsageDebitCents(usage.totalTokens)

  if (amountCents === 0) {
    return undefined
  }

  const sourceRef = sourceRefForTokenUsageEvent(input.event)

  return db
    .prepare(
      `INSERT OR IGNORE INTO billing_ledger_entries
        (id, user_id, team_id, run_id, source, description, amount_cents,
         currency, quantity, unit, unit_rate_cents, metadata_json,
         idempotency_key, created_at)
       VALUES (?, ?, ?, ?, 'codex_usage', ?, ?, ?, ?, 'tokens', ?, ?, ?, ?)`,
    )
    .bind(
      runtime.randomId('bill'),
      input.userId,
      input.teamId,
      input.event.parentId,
      `Codex usage: ${usage.totalTokens.toLocaleString('en-US')} tokens`,
      -amountCents,
      BILLING_CURRENCY,
      usage.totalTokens,
      CODEX_RATE_CENTS_PER_THOUSAND_TOKENS,
      metadataJson({
        eventId: input.event.id,
        model: usage.model,
        provider: usage.provider,
        source: input.event.source,
        sourceRef,
      }),
      `billing:codex:${input.event.parentId}:${sourceRef}`,
      input.event.createdAt,
    )
}

const billUntilForRun = (run: AgentRunRecord): string | undefined => {
  if (run.completedAt !== null) {
    return run.completedAt
  }

  if (run.failedAt !== null) {
    return run.failedAt
  }

  if (run.canceledAt !== null) {
    return run.canceledAt
  }

  return run.status === 'running' || run.status === 'waiting_for_input'
    ? run.updatedAt
    : undefined
}

export const recordContainerUsageDebitForRun = async (
  db: D1Database,
  run: AgentRunRecord,
  input: Readonly<{ billUntil?: string | undefined }> = {},
  runtime: BillingRuntime = systemBillingRuntime,
): Promise<void> => {
  if (run.startedAt === null) {
    return
  }

  const billUntil = input.billUntil ?? billUntilForRun(run)

  if (billUntil === undefined) {
    return
  }

  await ensureBillingAccount(db, run.userId, runtime)

  const cursor = await db
    .prepare(
      `SELECT last_billed_at, total_billed_quantity
       FROM billing_usage_cursors
       WHERE run_id = ? AND meter = 'container_seconds'`,
    )
    .bind(run.id)
    .first<UsageCursorRow>()
  const lastBilledAt = cursor?.last_billed_at ?? run.startedAt
  const seconds = secondsBetweenIso(lastBilledAt, billUntil)
  const amountCents = calculateContainerUsageDebitCents(seconds)

  if (seconds === 0 || amountCents === 0) {
    return
  }

  const now = runtime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_usage_cursors
          (run_id, meter, user_id, team_id, last_billed_at,
           total_billed_quantity, updated_at)
         VALUES (?, 'container_seconds', ?, ?, ?, 0, ?)`,
      )
      .bind(run.id, run.userId, run.teamId, lastBilledAt, now),
    db
      .prepare(
        `INSERT OR IGNORE INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES (?, ?, ?, ?, 'container_usage', ?, ?, ?, ?, 'seconds', ?, ?, ?, ?)`,
      )
      .bind(
        runtime.randomId('bill'),
        run.userId,
        run.teamId,
        run.id,
        `Computer usage: ${seconds} seconds`,
        -amountCents,
        BILLING_CURRENCY,
        seconds,
        CONTAINER_RATE_CENTS_PER_MINUTE,
        metadataJson({
          backend: run.backend,
          billedFrom: lastBilledAt,
          billedUntil: billUntil,
          runnerId: run.runnerId,
        }),
        `billing:container:${run.id}:${lastBilledAt}:${billUntil}`,
        billUntil,
      ),
    db
      .prepare(
        `UPDATE billing_usage_cursors
         SET last_billed_at = ?,
             total_billed_quantity = total_billed_quantity + ?,
             updated_at = ?
         WHERE run_id = ? AND meter = 'container_seconds'`,
      )
      .bind(billUntil, seconds, now, run.id),
  ])
}

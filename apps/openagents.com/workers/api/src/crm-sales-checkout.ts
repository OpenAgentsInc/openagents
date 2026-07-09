// OB-5 (#8562): close via Stripe — pack-priced checkout link issuance +
// settled-webhook confirmation reflected into the CRM.
//
// Scope (see docs/fable/MASTER_ROADMAP.md P1 Track C, OB-5): reply -> Sarah
// conversation -> checkout link (PACK-PRICED ONLY, reusing the same
// `STRIPE_CREDIT_PACKAGES_JSON` catalog `stripe-billing.ts` already serves
// for authenticated-user billing — "no improvised pricing" per the issue;
// deal-rules/arbitrary-amount checkout is Sarah repo SR-2 scope, not this
// module) -> settled Stripe receipt -> `crm_opportunities` stage transition
// (sourced -> replied -> conversed -> quoted -> closed_won/lost).
//
// Deliberately does NOT touch the authenticated-user billing tables
// (`stripe_customers` / `stripe_checkout_sessions`, migration 0031): those FK
// `user_id` to `users(id)`, and a CRM prospect is not a `users` row until they
// actually sign up. This module creates the Stripe Checkout Session with
// `customer_email` + `customer_creation: 'always'` instead of a pre-vended
// `stripe_customers` row, and records its own `crm_sales_checkout_sessions`
// row (migration 0312) keyed by `contact_id`.
import {
  BUSINESS_SOURCE_REF_UNKNOWN,
  coerceStoredBusinessSourceRef,
} from './business-source-attribution'
import type {
  BusinessStarterCreditGrantInput,
  BusinessStarterCreditGrantOutcome,
  BusinessStarterCreditStore,
} from './business-starter-credit'
import {
  createCrmOpportunity,
  DEFAULT_CRM_TENANT_REF,
  getCrmContactById,
  getCrmOpportunityById,
  recordCrmActivity,
  updateCrmOpportunityStage,
  upsertCrmOpportunityContactRole,
} from './crm-store'
import {
  crmEmailAuthorityDb,
  type CrmEmailDatabase,
} from './crm-email-domain-store'
// Type-only: `stripe-billing.ts` imports VALUES from this module (to route
// the webhook), so this side of the dependency must stay type-only or the
// two modules would form a runtime import cycle.
import type {
  BillingCreditPackageId,
  StripeClientShape,
  StripeConfigShape,
  StripeCreditPackage,
} from './stripe-billing'
import { currentIsoTimestamp } from './runtime-primitives'
import { Schema as S } from 'effect'

export class CrmSalesCheckoutError extends S.TaggedErrorClass<CrmSalesCheckoutError>()(
  'CrmSalesCheckoutError',
  { reason: S.String },
) {}

/** Pack-priced only: resolves against the SAME `config.packages` catalog
 * `stripe-billing.ts`'s `packageForId` reads (STRIPE_CREDIT_PACKAGES_JSON) —
 * duplicated here (rather than imported) solely to keep this module's
 * dependency on `stripe-billing.ts` type-only; see the import comment above. */
const resolveCrmSalesPackage = (
  config: StripeConfigShape,
  packageId: string,
): StripeCreditPackage => {
  const pack = config.packages.get(packageId as BillingCreditPackageId)
  if (pack === undefined) {
    throw new CrmSalesCheckoutError({ reason: 'Unknown credit package.' })
  }
  return pack
}

/** Distinguishes a CRM/Sarah-issued checkout from the authenticated-user
 * credit-purchase and Khala Code paid-plan checkouts in the SAME Stripe
 * webhook stream (`processStripeWebhook` in stripe-billing.ts routes on this
 * `session.metadata.product` value). */
export const CRM_SALES_CHECKOUT_STRIPE_PRODUCT =
  'openagents_crm_sales_checkout' as const

export type CrmSalesCheckoutLink = Readonly<{
  sessionId: string
  checkoutUrl: string
  opportunityId: string
  packageId: string
  amountCents: number
  currency: 'USD'
  sourceRef: string
}>

export type CreateCrmSalesCheckoutLinkInput = Readonly<{
  db: CrmEmailDatabase
  tenantRef?: string
  contactId: string
  packageId: string
  sourceRef?: string | null
  /** Reuse an existing opportunity (e.g. one already created when the reply
   * landed) instead of minting a new one. */
  opportunityId?: string | null
  idempotencyKey?: string
}>

/**
 * Create a pack-priced Stripe Checkout Session for a CRM contact and advance
 * (or create) their `crm_opportunities` row to the `quoted` stage. Pack-priced
 * only: `packageForId` resolves against the SAME `STRIPE_CREDIT_PACKAGES_JSON`
 * catalog authenticated-user billing uses — there is no arbitrary-amount path
 * here.
 */
export const createCrmSalesCheckoutLink = async (
  config: StripeConfigShape,
  stripeClient: StripeClientShape,
  input: CreateCrmSalesCheckoutLinkInput,
): Promise<CrmSalesCheckoutLink> => {
  const tenantRef = input.tenantRef ?? DEFAULT_CRM_TENANT_REF
  const pack = resolveCrmSalesPackage(config, input.packageId)
  const sourceRef = coerceStoredBusinessSourceRef(input.sourceRef ?? null)

  const contact = await getCrmContactById(input.db, tenantRef, input.contactId)
  if (contact === null) {
    throw new CrmSalesCheckoutError({ reason: `CRM contact not found: ${input.contactId}` })
  }

  const opportunity =
    input.opportunityId === undefined || input.opportunityId === null
      ? null
      : await getCrmOpportunityById(input.db, tenantRef, input.opportunityId)

  const resolvedOpportunity =
    opportunity ??
    (await createCrmOpportunity(input.db, {
      tenantRef,
      name: `${contact.fullName ?? contact.primaryEmail} — ${pack.label}`,
      stage: 'quoted',
      expectedAmountCents: pack.amountCents,
      metadata: { sourceRef, contactId: contact.id },
    }))

  if (resolvedOpportunity.stage !== 'quoted') {
    await updateCrmOpportunityStage(input.db, {
      tenantRef,
      id: resolvedOpportunity.id,
      stage: 'quoted',
      metadata: { sourceRef },
    })
  }

  await upsertCrmOpportunityContactRole(input.db, {
    tenantRef,
    opportunityId: resolvedOpportunity.id,
    contactId: contact.id,
    roleType: 'primary',
  })

  const stripe = stripeClient.unsafeClient()
  const idempotencyKey =
    input.idempotencyKey ??
    `crm_sales_checkout:${resolvedOpportunity.id}:${pack.id}:${currentIsoTimestamp()}`
  const session = await stripe.checkout.sessions.create(
    {
      cancel_url: config.cancelUrl,
      client_reference_id: contact.id,
      customer_creation: 'always',
      customer_email: contact.primaryEmail,
      line_items: [{ price: pack.priceId, quantity: 1 }],
      metadata: {
        amount_cents: String(pack.amountCents),
        crm_contact_id: contact.id,
        crm_opportunity_id: resolvedOpportunity.id,
        crm_tenant_ref: tenantRef,
        currency: pack.currency,
        package_id: pack.id,
        product: CRM_SALES_CHECKOUT_STRIPE_PRODUCT,
        source_ref: sourceRef,
      },
      mode: 'payment',
      success_url: config.successUrl,
    },
    { idempotencyKey },
  )

  if (session.url === null) {
    throw new CrmSalesCheckoutError({ reason: 'Stripe did not return a Checkout URL.' })
  }

  const nowIso = currentIsoTimestamp()
  await crmEmailAuthorityDb(input.db)
    .prepare(
      `INSERT OR IGNORE INTO crm_sales_checkout_sessions (
         session_id, tenant_ref, contact_id, opportunity_id, package_id,
         amount_cents, currency, source_ref, payment_status,
         fulfillment_status, stripe_customer_id, checkout_url, created_at,
         updated_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?)`,
    )
    .bind(
      session.id,
      tenantRef,
      contact.id,
      resolvedOpportunity.id,
      pack.id,
      pack.amountCents,
      pack.currency,
      sourceRef,
      session.payment_status ?? 'unpaid',
      typeof session.customer === 'string' ? session.customer : null,
      session.url,
      nowIso,
      nowIso,
    )
    .run()

  await recordCrmActivity(
    input.db,
    {
      tenantRef,
      contactId: contact.id,
      activityType: 'checkout_link_issued',
      subject: `Checkout link — ${pack.label}`,
      summary: `Pack-priced checkout (${pack.id}) issued for ${(pack.amountCents / 100).toFixed(2)} ${pack.currency}.`,
      sourceRecordType: 'stripe_checkout_session',
      // Suffixed (not the bare session id): `crm_activities` dedupes on
      // (sourceRecordType, sourceRecordId), and `fulfillCrmSalesCheckoutSession`
      // below records a SECOND, distinct activity against the SAME Stripe
      // session once it settles/fails — sharing the bare session id would
      // silently `INSERT OR IGNORE` that second row away.
      sourceRecordId: `${session.id}:checkout_link_issued`,
      sourceSystem: 'crm_sales_checkout',
    },
  )

  return {
    sessionId: session.id,
    checkoutUrl: session.url,
    opportunityId: resolvedOpportunity.id,
    packageId: pack.id,
    amountCents: pack.amountCents,
    currency: 'USD',
    sourceRef,
  }
}

export type CrmSalesCheckoutSessionRow = Readonly<{
  session_id: string
  tenant_ref: string
  contact_id: string
  opportunity_id: string
  package_id: string
  amount_cents: number
  currency: string
  source_ref: string
  payment_status: string
  fulfillment_status: string
}>

export type FulfillCrmSalesCheckoutSessionResult = Readonly<{
  status: 'fulfilled' | 'unpaid' | 'expired' | 'failed'
  opportunityId: string
  contactId: string
  sourceRef: string
  stage: 'closed_won' | 'closed_lost' | 'quoted'
}>

/**
 * Reflect a settled (or failed/expired) Stripe Checkout Session for a CRM
 * sales checkout back into the CRM: `crm_opportunities.stage` transitions to
 * `closed_won` (paid) or `closed_lost` (failed/expired), and a
 * `crm_activities` row records the receipt. Called from
 * `processStripeWebhook` in stripe-billing.ts when
 * `session.metadata.product === CRM_SALES_CHECKOUT_STRIPE_PRODUCT`.
 */
export const fulfillCrmSalesCheckoutSession = async (
  stripeClient: StripeClientShape,
  input: Readonly<{
    db: CrmEmailDatabase
    sessionId: string
    eventType?: string | undefined
  }>,
): Promise<FulfillCrmSalesCheckoutSessionResult> => {
  const stripe = stripeClient.unsafeClient()
  const session = await stripe.checkout.sessions.retrieve(input.sessionId)

  if (session.metadata?.product !== CRM_SALES_CHECKOUT_STRIPE_PRODUCT) {
    throw new CrmSalesCheckoutError({ reason: 'Checkout Session is not a CRM sales checkout.' })
  }

  const row = await crmEmailAuthorityDb(input.db)
    .prepare(
      `SELECT session_id, tenant_ref, contact_id, opportunity_id, package_id,
              amount_cents, currency, source_ref, payment_status, fulfillment_status
         FROM crm_sales_checkout_sessions WHERE session_id = ?`,
    )
    .bind(input.sessionId)
    .first<CrmSalesCheckoutSessionRow>()

  if (row === null) {
    throw new CrmSalesCheckoutError({ reason: `crm_sales_checkout_sessions row not found for session ${input.sessionId}` })
  }

  const nowIso = currentIsoTimestamp()
  const sourceRef = coerceStoredBusinessSourceRef(row.source_ref)

  if (session.payment_status !== 'paid') {
    const fulfillmentStatus =
      input.eventType === 'checkout.session.async_payment_failed'
        ? 'failed'
        : session.status === 'expired'
          ? 'expired'
          : 'unpaid'

    await crmEmailAuthorityDb(input.db)
      .prepare(
        `UPDATE crm_sales_checkout_sessions
            SET payment_status = ?, fulfillment_status = ?, updated_at = ?
          WHERE session_id = ?`,
      )
      .bind(
        session.payment_status ?? 'unpaid',
        fulfillmentStatus,
        nowIso,
        input.sessionId,
      )
      .run()

    const shouldCloseLost = fulfillmentStatus !== 'unpaid'
    if (shouldCloseLost) {
      await updateCrmOpportunityStage(input.db, {
        tenantRef: row.tenant_ref,
        id: row.opportunity_id,
        stage: 'closed_lost',
        metadata: { sourceRef, stripeCheckoutSessionId: input.sessionId },
      })
      await recordCrmActivity(input.db, {
        tenantRef: row.tenant_ref,
        contactId: row.contact_id,
        activityType: 'checkout_failed',
        subject: `Checkout ${fulfillmentStatus}`,
        summary: `Stripe checkout ${input.sessionId} did not complete (${fulfillmentStatus}).`,
        sourceRecordType: 'stripe_checkout_session',
        sourceRecordId: `${input.sessionId}:checkout_failed`,
        sourceSystem: 'crm_sales_checkout',
      })
    }

    return {
      status: fulfillmentStatus,
      opportunityId: row.opportunity_id,
      contactId: row.contact_id,
      sourceRef,
      stage: shouldCloseLost ? 'closed_lost' : 'quoted',
    }
  }

  await crmEmailAuthorityDb(input.db)
    .prepare(
      `UPDATE crm_sales_checkout_sessions
          SET payment_status = 'paid', fulfillment_status = 'fulfilled', updated_at = ?
        WHERE session_id = ?`,
    )
    .bind(nowIso, input.sessionId)
    .run()

  await updateCrmOpportunityStage(input.db, {
    tenantRef: row.tenant_ref,
    id: row.opportunity_id,
    stage: 'closed_won',
    metadata: { sourceRef, stripeCheckoutSessionId: input.sessionId },
  })

  await recordCrmActivity(input.db, {
    tenantRef: row.tenant_ref,
    contactId: row.contact_id,
    activityType: 'checkout_settled',
    subject: 'Checkout settled',
    summary: `Stripe checkout ${input.sessionId} settled (${(row.amount_cents / 100).toFixed(2)} ${row.currency}); receipt evidence.stripe_crm_checkout_paid.${input.sessionId}.`,
    sourceRecordType: 'stripe_checkout_session',
    sourceRecordId: `${input.sessionId}:checkout_settled`,
    sourceSystem: 'crm_sales_checkout',
  })

  return {
    status: 'fulfilled',
    opportunityId: row.opportunity_id,
    contactId: row.contact_id,
    sourceRef,
    stage: 'closed_won',
  }
}

// ---------------------------------------------------------------------------
// LG-3 starter-credit hook
// ---------------------------------------------------------------------------
//
// The roadmap names starter-credit grants (LG-3, `business-starter-credit.ts`)
// as "usable as the hook where the playbook calls for it." `createGrant`
// there requires a `business_pipeline_rows` `pipelineRef` and an OpenAgents
// `accountRef` — neither of which a bare CRM reply carries on its own (a cold
// prospect who replies to outbound has a `crm_contacts` row, not necessarily
// a signed-up OpenAgents account yet). Full automatic identity resolution
// (deriving an accountRef + pipelineRef from an inbound reply email alone) is
// out of scope here; this function is the explicit, best-effort HOOK that a
// caller who already has both refs (e.g. an operator/Sarah flow that already
// resolved the prospect's pipeline row and account) can invoke before or
// alongside checkout-link issuance. Never blocks checkout-link issuance:
// callers should treat a `false` outcome as informational, matching the
// other best-effort feeds in `stripe-billing.ts`.
export const maybeGrantCrmSalesStarterCredit = async (
  store: BusinessStarterCreditStore,
  input: Readonly<{
    pipelineRef: string
    grant: BusinessStarterCreditGrantInput
  }>,
): Promise<BusinessStarterCreditGrantOutcome | Readonly<{ ok: false; reason: 'error'; message: string }>> => {
  try {
    return await store.createGrant(input.pipelineRef, input.grant)
  } catch (error) {
    return {
      ok: false,
      reason: 'error',
      message: error instanceof Error ? error.message : String(error),
    }
  }
}

export const CRM_SALES_CHECKOUT_UNKNOWN_SOURCE_REF = BUSINESS_SOURCE_REF_UNKNOWN

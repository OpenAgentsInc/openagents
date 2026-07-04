import { Effect, Schema as S } from 'effect'

import { parseCookies } from './auth-cookies'
import { recordBusinessAffiliateSignupAttribution } from './business-affiliate-attribution'
import { recordBusinessFunnelEvent } from './business-funnel-dashboard'
import {
  BUSINESS_SOURCE_REF_DIRECT,
  businessSourceKindForSourceRef,
  businessSourceRefForReferralCode,
  decodeBusinessSourceRef,
} from './business-source-attribution'
import type { ResendEmailConfig } from './config'
import type { PrivateWorkspaceInviteEmailInput } from './email'
import type { EmailLedgerSendResult, EmailServiceError } from './email'
import {
  type BusinessSignupFulfillmentOptions,
  fulfillBusinessSignup,
} from './business-signup-fulfillment'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  isRecord,
  optionalString,
  parseJsonUnknown,
  readJsonObject,
} from './json-boundary'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  capturePendingReferralBySourceRef,
  isSafeReferralSourceRef,
} from './referral-source-capture'
import {
  compactRandomId,
  currentDate,
  currentIsoTimestamp,
  isoTimestampAfter,
} from './runtime-primitives'
import { linkPendingReferralToBusinessSignup } from './site-referral-attribution-consumption'
import { PENDING_REFERRAL_MAX_AGE_SECONDS } from './site-referrals'

const maxSignupBodyBytes = 16_384
const emailPattern = /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/

export const SlackConnectStatus = S.Literals([
  'not_requested',
  'manual_invite_pending',
  'invite_sent',
  'accepted',
  'declined',
])
export type SlackConnectStatus = typeof SlackConnectStatus.Type

class BusinessSignupIntakeFailure extends S.TaggedErrorClass<BusinessSignupIntakeFailure>()(
  'BusinessSignupIntakeFailure',
  {
    cause: S.Unknown,
  },
) {}

export type BusinessSignupRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
  expiresAtFromNow: () => string
}>

export const systemBusinessSignupRuntime: BusinessSignupRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
  expiresAtFromNow: () =>
    isoTimestampAfter(currentDate(), PENDING_REFERRAL_MAX_AGE_SECONDS * 1000),
}

export type BusinessSignupInput = Readonly<{
  businessName: string
  contactEmail: string
  website: string | null
  phone: string
  helpWith: string | null
  requestSlackChannel: boolean
  sourceRoute?: string | undefined
  // Inbound referral code (a site_referral_sources.public_source_ref). Captured
  // from the /business ?ref= query param or a `referralCode` form field; null
  // when no code was present.
  referralCode: string | null
  // Public-safe acquisition token, never a raw UTM, URL, email, or identity.
  sourceRef: string
}>

export type BusinessSignupRecord = BusinessSignupInput &
  Readonly<{
    id: string
    slackConnectStatus: SlackConnectStatus
    sourceRoute: string
    // The pending referral_attributions.id bound to this signup once a valid
    // active referral source was resolved (consume-once via the spine); null
    // when there was no captured/resolvable attribution.
    referralAttributionId: string | null
    fulfillmentStatus: 'pending' | 'invited' | 'operator_parked'
    fulfillmentRef: string | null
    fulfillmentReason: string | null
    createdAt: string
    updatedAt: string
  }>

type BusinessSignupRow = Readonly<{
  id: string
  business_name: string
  contact_email: string
  website: string | null
  phone: string
  help_with: string | null
  request_slack_channel: number
  slack_connect_status: SlackConnectStatus
  source_route: string
  referral_code: string | null
  referral_attribution_id: string | null
  source_ref: string | null
  linked_pipeline_ref: string | null
  fulfillment_status: 'pending' | 'invited' | 'operator_parked'
  fulfillment_ref: string | null
  fulfillment_reason: string | null
  created_at: string
  updated_at: string
}>

export type BusinessSignupApiOptions = BusinessSignupFulfillmentOptions &
  Readonly<{
    appOrigin?: string | undefined
    getResendEmailConfig?: (() => ResendEmailConfig | undefined) | undefined
    sendInviteEmailWithLedger?:
      | ((
          config: ResendEmailConfig,
          input: PrivateWorkspaceInviteEmailInput,
        ) => Effect.Effect<EmailLedgerSendResult, EmailServiceError>)
      | undefined
  }>

const wantsJsonResponse = (request: Request): boolean => {
  const accept = request.headers.get('accept') ?? ''
  const contentType = request.headers.get('content-type') ?? ''

  return (
    accept.toLowerCase().includes('application/json') ||
    contentType.toLowerCase().includes('application/json')
  )
}

const textResponse = (body: string, init: ResponseInit = {}) => {
  const headers = new Headers(init.headers)
  headers.set('cache-control', 'no-store')
  headers.set('content-type', 'text/html; charset=utf-8')

  return new Response(body, { ...init, headers })
}

const validationError = (reason: string, request: Request) => {
  if (wantsJsonResponse(request)) {
    return noStoreJsonResponse(
      { error: 'business_signup_validation_error', reason },
      { status: 400 },
    )
  }

  return textResponse(
    `<!doctype html><html><head><meta charset="utf-8"><title>Check the form</title></head><body><main><h1>Check the form</h1><p>${reason}</p><p><a href="/business">Return to the business page</a></p></main></body></html>`,
    { status: 400 },
  )
}

const payloadTooLarge = (request: Request) => {
  if (wantsJsonResponse(request)) {
    return noStoreJsonResponse(
      {
        error: 'business_signup_payload_too_large',
        reason: 'request body is too large',
      },
      { status: 413 },
    )
  }

  return textResponse(
    '<!doctype html><html><head><meta charset="utf-8"><title>Request too large</title></head><body><main><h1>Request too large</h1><p>The signup request is too large.</p><p><a href="/business">Return to the business page</a></p></main></body></html>',
    { status: 413 },
  )
}

const normalizeText = (value: unknown, maxLength: number): string | undefined =>
  optionalString(value)?.replace(/\s+/g, ' ').slice(0, maxLength)

const normalizeMultiline = (
  value: unknown,
  maxLength: number,
): string | undefined =>
  optionalString(value)?.replace(/\s+\n/g, '\n').trim().slice(0, maxLength)

const normalizeEmail = (value: unknown): string | undefined => {
  const email = normalizeText(value, 320)?.toLowerCase()

  return email !== undefined && emailPattern.test(email) ? email : undefined
}

const normalizeWebsite = (
  value: unknown,
): { website: string | null; reason?: string } => {
  const raw = normalizeText(value, 500)

  if (raw === undefined) {
    return { website: null }
  }

  try {
    const parsed = new URL(raw)

    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
      ? { website: parsed.toString() }
      : { website: null, reason: 'website must use http or https' }
  } catch {
    return { website: null, reason: 'website must be a valid URL' }
  }
}

const booleanFromUnknown = (value: unknown): boolean => {
  if (typeof value === 'boolean') {
    return value
  }

  if (typeof value !== 'string') {
    return false
  }

  return ['1', 'on', 'true', 'yes'].includes(value.trim().toLowerCase())
}

const formDataToRecord = (formData: FormData): Record<string, unknown> => {
  const fields: Record<string, unknown> = {}

  for (const [key, value] of formData.entries()) {
    fields[key] = typeof value === 'string' ? value : value.name
  }

  return fields
}

const readSignupFields = async (
  request: Request,
): Promise<Record<string, unknown>> => {
  const contentType = request.headers.get('content-type')?.toLowerCase() ?? ''

  if (contentType.includes('application/json')) {
    return readJsonObject(request)
  }

  if (
    contentType.includes('application/x-www-form-urlencoded') ||
    contentType.includes('multipart/form-data')
  ) {
    return formDataToRecord(await request.formData())
  }

  const text = await request.text()

  if (text.trim() === '') {
    return {}
  }

  if (contentType.includes('text/plain')) {
    return Object.fromEntries(new URLSearchParams(text).entries())
  }

  try {
    const parsed = parseJsonUnknown(text)

    return isRecord(parsed) ? parsed : {}
  } catch {
    return Object.fromEntries(new URLSearchParams(text).entries())
  }
}

const sourceCandidateFromFields = (
  fields: Record<string, unknown>,
  url: URL,
): unknown =>
  fields.sourceRef ??
  fields.source_ref ??
  fields.source ??
  url.searchParams.get('sourceRef') ??
  url.searchParams.get('source_ref') ??
  url.searchParams.get('source')

const decodeSignupInput = (
  fields: Record<string, unknown>,
  url: URL,
): { input: BusinessSignupInput } | { reason: string } => {
  const businessName = normalizeText(fields.businessName, 200)

  if (businessName === undefined) {
    return { reason: 'businessName is required' }
  }

  const contactEmail = normalizeEmail(fields.contactEmail)

  if (contactEmail === undefined) {
    return { reason: 'contactEmail is required and must be a valid email' }
  }

  const phone = normalizeText(fields.phone, 80)

  if (phone === undefined) {
    return { reason: 'phone is required' }
  }

  const websiteResult = normalizeWebsite(fields.website)

  if (websiteResult.reason !== undefined) {
    return { reason: websiteResult.reason }
  }

  const referralCode =
    normalizeReferralCode(fields.referralCode ?? fields.ref) ??
    normalizeReferralCode(url.searchParams.get('ref'))
  const decodedSourceRef = decodeBusinessSourceRef(
    sourceCandidateFromFields(fields, url),
  )

  if ('reason' in decodedSourceRef) {
    return { reason: decodedSourceRef.reason }
  }

  const sourceRef =
    decodedSourceRef.sourceRef === BUSINESS_SOURCE_REF_DIRECT &&
    referralCode !== null
      ? businessSourceRefForReferralCode(referralCode)
      : decodedSourceRef.sourceRef

  return {
    input: {
      businessName,
      contactEmail,
      website: websiteResult.website,
      phone,
      helpWith: normalizeMultiline(fields.helpWith, 2_000) ?? null,
      requestSlackChannel: booleanFromUnknown(fields.requestSlackChannel),
      referralCode,
      sourceRef,
    },
  }
}

// A referral code is a site_referral_sources.public_source_ref. We only keep it
// when it matches the bounded safe shape; anything else is dropped (null) so a
// hostile value can never reach the referral query.
const normalizeReferralCode = (value: unknown): string | null => {
  const raw = normalizeText(value, 190)

  if (raw === undefined || !isSafeReferralSourceRef(raw)) {
    return null
  }

  return raw
}

export const makeBusinessSignupRecord = (
  input: BusinessSignupInput,
  runtime: BusinessSignupRuntime = systemBusinessSignupRuntime,
): BusinessSignupRecord => {
  const now = runtime.nowIso()

  return {
    ...input,
    id: runtime.makeId('business_signup'),
    slackConnectStatus: input.requestSlackChannel
      ? 'manual_invite_pending'
      : 'not_requested',
    sourceRoute: input.sourceRoute ?? '/business',
    referralAttributionId: null,
    fulfillmentStatus: 'pending',
    fulfillmentRef: null,
    fulfillmentReason: null,
    createdAt: now,
    updatedAt: now,
  }
}

const rowToRecord = (row: BusinessSignupRow): BusinessSignupRecord => ({
  id: row.id,
  businessName: row.business_name,
  contactEmail: row.contact_email,
  website: row.website,
  phone: row.phone,
  helpWith: row.help_with,
  requestSlackChannel: row.request_slack_channel === 1,
  slackConnectStatus: row.slack_connect_status,
  sourceRoute: row.source_route,
  referralCode: row.referral_code,
  referralAttributionId: row.referral_attribution_id,
  sourceRef: row.source_ref ?? BUSINESS_SOURCE_REF_DIRECT,
  fulfillmentStatus: row.fulfillment_status,
  fulfillmentRef: row.fulfillment_ref,
  fulfillmentReason: row.fulfillment_reason,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

export const insertBusinessSignupRequest = async (
  db: D1Database,
  input: BusinessSignupInput,
  runtime: BusinessSignupRuntime = systemBusinessSignupRuntime,
): Promise<BusinessSignupRecord> => {
  const record = makeBusinessSignupRecord(input, runtime)

  await db
    .prepare(
      `INSERT INTO business_signup_requests (
        id,
        business_name,
        contact_email,
        website,
        phone,
        help_with,
        request_slack_channel,
        slack_connect_status,
        source_route,
        referral_code,
        referral_attribution_id,
        source_ref,
        created_at,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      record.id,
      record.businessName,
      record.contactEmail,
      record.website,
      record.phone,
      record.helpWith,
      record.requestSlackChannel ? 1 : 0,
      record.slackConnectStatus,
      record.sourceRoute,
      record.referralCode,
      record.referralAttributionId,
      record.sourceRef,
      record.createdAt,
      record.updatedAt,
    )
    .run()

  return record
}

// Persist the resolved pending attribution id onto the signup row after the
// referral spine binding has been recorded.
const updateBusinessSignupReferralAttribution = async (
  db: D1Database,
  input: Readonly<{
    id: string
    referralAttributionId: string
    updatedAt: string
  }>,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE business_signup_requests
          SET referral_attribution_id = ?,
              updated_at = ?
        WHERE id = ?
          AND referral_attribution_id IS NULL`,
    )
    .bind(input.referralAttributionId, input.updatedAt, input.id)
    .run()
}

const recordBusinessSignupFunnelEvent = async (
  db: D1Database,
  record: BusinessSignupRecord,
): Promise<void> => {
  await recordBusinessFunnelEvent(db, {
    eventRef: `business_signup:${record.id}`,
    stage: 'signup',
    sourceKind: businessSourceKindForSourceRef(record.sourceRef),
    sourceRef: record.sourceRef,
    occurredAt: record.createdAt,
  })
}

export const readBusinessSignupRequest = async (
  db: D1Database,
  id: string,
): Promise<BusinessSignupRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT
        id,
        business_name,
        contact_email,
        website,
        phone,
        help_with,
        request_slack_channel,
        slack_connect_status,
        source_route,
        referral_code,
        referral_attribution_id,
        source_ref,
        linked_pipeline_ref,
        fulfillment_status,
        fulfillment_ref,
        fulfillment_reason,
        created_at,
        updated_at
      FROM business_signup_requests
      WHERE id = ?`,
    )
    .bind(id)
    .first<BusinessSignupRow>()

  return row === null ? undefined : rowToRecord(row)
}

const pendingReferralCookieValue = (
  request: Request,
): string | undefined => {
  const value = parseCookies(request).get('oa_pending_referral_attribution')

  return value === undefined || value === '' ? undefined : value
}

// Resolve and bind a referral for a converted business signup, REUSING the
// existing referral spine. Last-touch wins, mirroring site-referral
// consumption: an already-captured pending attribution cookie (from a prior
// /r/<ref> redirect) is preferred; otherwise a bare ?ref=/referralCode is
// captured into a fresh pending attribution. Binding is consume-once and
// receipt-first (the business_signup_referral_attributions PRIMARY KEY plus the
// pending->claimed guard prevent double-credit). A referral failure never fails
// the signup; intake is the primary contract.
//
// Returns the bound attribution id (so the signup row can record it), or null
// when there was no resolvable referral.
const bindBusinessSignupReferral = async (
  request: Request,
  db: D1Database,
  runtime: BusinessSignupRuntime,
  input: Readonly<{
    businessSignupRequestId: string
    referralCode: string | null
  }>,
): Promise<string | null> => {
  const cookieAttributionId = pendingReferralCookieValue(request)

  let pendingAttributionId = cookieAttributionId

  if (pendingAttributionId === undefined && input.referralCode !== null) {
    const captured = await capturePendingReferralBySourceRef(db, runtime, {
      publicSourceRef: input.referralCode,
      capturePath: 'human',
      target: 'order',
    })

    if (captured._tag === 'captured') {
      pendingAttributionId = captured.attribution.id
    }
  }

  if (pendingAttributionId === undefined) {
    return null
  }

  const result = await linkPendingReferralToBusinessSignup(db, runtime, {
    businessSignupRequestId: input.businessSignupRequestId,
    pendingAttributionId,
  })

  if (result._tag === 'consumed' || result._tag === 'already_verified') {
    await updateBusinessSignupReferralAttribution(db, {
      id: input.businessSignupRequestId,
      referralAttributionId: result.attributionId,
      updatedAt: runtime.nowIso(),
    })

    return result.attributionId
  }

  return null
}

const publicResponseBody = (record: BusinessSignupRecord) => ({
  generatedAt: record.updatedAt,
  staleness: liveAtReadStaleness(['business_signup_request.insert']),
  request: {
    id: record.id,
    sourceRoute: record.sourceRoute,
    sourceRef: record.sourceRef,
    requestedSlackChannel: record.requestSlackChannel,
    slackConnectStatus: record.slackConnectStatus,
    // Public-safe: a boolean only; never echoes the referral code or the
    // internal attribution id.
    referralAttributed: record.referralAttributionId !== null,
    fulfillmentStatus: record.fulfillmentStatus,
    fulfillmentRef: record.fulfillmentRef,
    nextAction: record.requestSlackChannel
      ? 'operator_manual_slack_connect_invite'
      : record.fulfillmentStatus === 'invited'
        ? 'workspace_invite_sent'
        : 'operator_workspace_intake',
    authorityBoundary:
      'Intake receipt only; grants no Slack, workspace, spend, payout, or agent authority.',
  },
})

const successHtml = (record: BusinessSignupRecord): string => {
  const workspaceCopy =
    record.fulfillmentStatus === 'invited'
      ? 'We sent the workspace invite and queued the first scope confirmation.'
      : 'We queued the workspace intake handoff.'
  const slackCopy = record.requestSlackChannel
    ? 'We queued the Slack Connect invite handoff. Slack Connect still requires your workspace to accept the invite.'
    : workspaceCopy

  return `<!doctype html><html><head><meta charset="utf-8"><title>Request received</title></head><body><main><h1>Request received</h1><p>${slackCopy}</p><p>Reference: ${record.id}</p><p><a href="/business">Return to the business page</a></p></main></body></html>`
}

export const handleBusinessSignupApi = (
  request: Request,
  db: D1Database,
  runtime: BusinessSignupRuntime = systemBusinessSignupRuntime,
  options: BusinessSignupApiOptions = {},
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const contentLength = Number(request.headers.get('content-length') ?? '0')

    if (contentLength > maxSignupBodyBytes) {
      return payloadTooLarge(request)
    }

    const fields = yield* Effect.tryPromise({
      try: () => readSignupFields(request),
      catch: cause => new BusinessSignupIntakeFailure({ cause }),
    })
    const decoded = decodeSignupInput(fields, new URL(request.url))

    if ('reason' in decoded) {
      return validationError(decoded.reason, request)
    }
    const input = decoded.input

    const record = yield* Effect.tryPromise({
      try: () => insertBusinessSignupRequest(db, input, runtime),
      catch: cause => new BusinessSignupIntakeFailure({ cause }),
    })

    yield* Effect.tryPromise({
      try: () => recordBusinessSignupFunnelEvent(db, record),
      catch: cause => new BusinessSignupIntakeFailure({ cause }),
    }).pipe(Effect.catch(() => Effect.void))

    yield* Effect.tryPromise({
      try: () =>
        recordBusinessAffiliateSignupAttribution(
          db,
          {
            businessSignupRequestId: record.id,
            sourceRef: record.sourceRef,
          },
          runtime,
        ),
      catch: cause => new BusinessSignupIntakeFailure({ cause }),
    }).pipe(Effect.catch(() => Effect.void))

    // Bind the referral after the signup is durably stored. A referral failure
    // must not fail the intake, so it is caught and dropped to null.
    const referralAttributionId = yield* Effect.tryPromise({
      try: () =>
        bindBusinessSignupReferral(request, db, runtime, {
          businessSignupRequestId: record.id,
          referralCode: input.referralCode,
        }),
      catch: cause => new BusinessSignupIntakeFailure({ cause }),
    }).pipe(Effect.catch(() => Effect.succeed(null)))

    const referredRecord: BusinessSignupRecord =
      referralAttributionId === null
        ? record
        : { ...record, referralAttributionId }
    const fulfillment = yield* fulfillBusinessSignup(
      db,
      referredRecord,
      runtime,
      options,
    )
    const storedRecord: BusinessSignupRecord =
      {
        ...referredRecord,
        fulfillmentRef: fulfillment.id,
        fulfillmentReason: fulfillment.reason,
        fulfillmentStatus: fulfillment.status,
        updatedAt: fulfillment.updatedAt,
      }

    return wantsJsonResponse(request)
      ? noStoreJsonResponse(publicResponseBody(storedRecord), { status: 201 })
      : textResponse(successHtml(storedRecord), { status: 201 })
  }).pipe(
    Effect.catch(() => {
      return Effect.succeed(
        noStoreJsonResponse(
          { error: 'business_signup_storage_error' },
          { status: 500 },
        ),
      )
    }),
  )

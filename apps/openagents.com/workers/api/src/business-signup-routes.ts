import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  isRecord,
  optionalString,
  parseJsonUnknown,
  readJsonObject,
} from './json-boundary'
import { liveAtReadStaleness } from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

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
}>

export const systemBusinessSignupRuntime: BusinessSignupRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

export type BusinessSignupInput = Readonly<{
  businessName: string
  contactEmail: string
  website: string | null
  phone: string
  helpWith: string | null
  requestSlackChannel: boolean
}>

export type BusinessSignupRecord = BusinessSignupInput &
  Readonly<{
    id: string
    slackConnectStatus: SlackConnectStatus
    sourceRoute: '/business'
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
  created_at: string
  updated_at: string
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

const decodeSignupInput = (
  fields: Record<string, unknown>,
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

  return {
    input: {
      businessName,
      contactEmail,
      website: websiteResult.website,
      phone,
      helpWith: normalizeMultiline(fields.helpWith, 2_000) ?? null,
      requestSlackChannel: booleanFromUnknown(fields.requestSlackChannel),
    },
  }
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
    sourceRoute: '/business',
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
  sourceRoute: '/business',
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
        created_at,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
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
      record.createdAt,
      record.updatedAt,
    )
    .run()

  return record
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
        created_at,
        updated_at
      FROM business_signup_requests
      WHERE id = ?`,
    )
    .bind(id)
    .first<BusinessSignupRow>()

  return row === null ? undefined : rowToRecord(row)
}

const publicResponseBody = (record: BusinessSignupRecord) => ({
  generatedAt: record.updatedAt,
  staleness: liveAtReadStaleness(['business_signup_request.insert']),
  request: {
    id: record.id,
    sourceRoute: record.sourceRoute,
    requestedSlackChannel: record.requestSlackChannel,
    slackConnectStatus: record.slackConnectStatus,
    nextAction: record.requestSlackChannel
      ? 'operator_manual_slack_connect_invite'
      : 'operator_workspace_intake',
    authorityBoundary:
      'Intake receipt only; grants no Slack, workspace, spend, payout, or agent authority.',
  },
})

const successHtml = (record: BusinessSignupRecord): string => {
  const slackCopy = record.requestSlackChannel
    ? 'We queued the Slack Connect invite handoff. Slack Connect still requires your workspace to accept the invite.'
    : 'We queued the workspace intake handoff.'

  return `<!doctype html><html><head><meta charset="utf-8"><title>Request received</title></head><body><main><h1>Request received</h1><p>${slackCopy}</p><p>Reference: ${record.id}</p><p><a href="/business">Return to the business page</a></p></main></body></html>`
}

export const handleBusinessSignupApi = (
  request: Request,
  db: D1Database,
  runtime: BusinessSignupRuntime = systemBusinessSignupRuntime,
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
    const decoded = decodeSignupInput(fields)

    if ('reason' in decoded) {
      return validationError(decoded.reason, request)
    }

    const record = yield* Effect.tryPromise({
      try: () => insertBusinessSignupRequest(db, decoded.input, runtime),
      catch: cause => new BusinessSignupIntakeFailure({ cause }),
    })

    return wantsJsonResponse(request)
      ? noStoreJsonResponse(publicResponseBody(record), { status: 201 })
      : textResponse(successHtml(record), { status: 201 })
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

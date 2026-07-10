/**
 * PORTAL-1 (#8652): client portal routes.
 *
 * Client (browser OpenAuth cookie session, owner-scoped fail-closed):
 *   GET  /api/portal/engagement
 *     -> { engagement: null } | { engagement, items, kpis }
 *     Resolves ONLY through the caller's own verified session identity
 *     (client_user_id, or a pre-login client_email binding). There is no
 *     client-facing engagement-id lookup, so a client can never read another
 *     engagement.
 *   POST /api/portal/content/:id/decision  { decision: 'approve' | 'reject' }
 *     -> { ok: true, item, receiptRef, alreadyDecided }
 *     Cross-engagement items answer 404 (existence is not leaked). Every
 *     decision mints an immutable receipt ref
 *     (`portal_content_decision:<opaque id>`).
 *
 * Operator (admin bearer token, same guard posture as the CRM operator
 * surface — crm-sales-checkout-routes.ts):
 *   POST /api/portal/admin/engagements                     { name, clientEmail?, status? }
 *   POST /api/portal/admin/engagements/:id/bind            { clientUserId?, clientEmail? }
 *   POST /api/portal/admin/engagements/:id/content-items   { items: [...] }
 *   GET  /api/portal/admin/engagements/:id                 (verification read)
 *
 * KPI tiles are HONEST PLACEHOLDERS (#8652 scope 2): values are null with an
 * explicit note until the live funnel wiring exists. No fabricated numbers.
 */
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { isRecord } from './json-boundary'
import {
  PORTAL_DECISIONS,
  PortalValidationError,
  makeD1PortalStore,
  type PortalDecision,
  type PortalEngagement,
  type PortalSeedContentItemInput,
  type PortalStore,
} from './portal-store'

type HttpResponse = globalThis.Response

export type PortalSessionUser = Readonly<{
  userId: string
  email?: string | null
}>

export type PortalSession = Readonly<{ user: PortalSessionUser }>

export type PortalRouteDependencies<Bindings> = Readonly<{
  database: (env: Bindings) => D1Database
  makeStore?: ((env: Bindings) => PortalStore) | undefined
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<PortalSession | undefined>
}>

const CLIENT_ENGAGEMENT = /^\/api\/portal\/engagement$/
const CLIENT_DECISION = /^\/api\/portal\/content\/([A-Za-z0-9_.:-]{1,120})\/decision$/
const ADMIN_ENGAGEMENTS = /^\/api\/portal\/admin\/engagements$/
const ADMIN_ENGAGEMENT = /^\/api\/portal\/admin\/engagements\/([A-Za-z0-9_.:-]{1,120})$/
const ADMIN_BIND = /^\/api\/portal\/admin\/engagements\/([A-Za-z0-9_.:-]{1,120})\/bind$/
const ADMIN_CONTENT_ITEMS =
  /^\/api\/portal\/admin\/engagements\/([A-Za-z0-9_.:-]{1,120})\/content-items$/

const ALL: ReadonlyArray<RegExp> = [
  CLIENT_ENGAGEMENT,
  CLIENT_DECISION,
  ADMIN_ENGAGEMENTS,
  ADMIN_ENGAGEMENT,
  ADMIN_BIND,
  ADMIN_CONTENT_ITEMS,
]

/** Honest KPI placeholders until the live funnel wiring exists (#8652). */
export const PORTAL_KPI_PLACEHOLDERS = [
  {
    key: 'funnel_traffic',
    label: 'Funnel traffic',
    value: null,
    note: 'placeholder until live funnel wiring',
  },
  {
    key: 'leads',
    label: 'Leads',
    value: null,
    note: 'placeholder until live funnel wiring',
  },
  {
    key: 'conversions',
    label: 'Conversions',
    value: null,
    note: 'placeholder until live funnel wiring',
  },
] as const

const normalizedEmail = (value: string): string => value.trim().toLowerCase()

/** Owner-scoping law: a bound client_user_id is authoritative; the email
 * binding grants access only while no user id has been bound yet. */
export const portalEngagementOwnedBy = (
  engagement: PortalEngagement,
  identity: PortalSessionUser,
): boolean => {
  if (engagement.clientUserId !== null) {
    return engagement.clientUserId === identity.userId
  }
  const email = identity.email
  return (
    typeof email === 'string' &&
    email.trim() !== '' &&
    engagement.clientEmail !== null &&
    normalizedEmail(email) === engagement.clientEmail
  )
}

const badRequest = (reason: string): HttpResponse =>
  noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 })

const unauthorizedResponse = (): HttpResponse =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const notFoundResponse = (): HttpResponse =>
  noStoreJsonResponse({ error: 'not_found' }, { status: 404 })

const portalErrorResponse = (error: unknown): HttpResponse =>
  error instanceof PortalValidationError
    ? noStoreJsonResponse(
        { error: 'portal_validation_error', reason: error.message },
        { status: 422 },
      )
    : noStoreJsonResponse({ error: 'portal_error' }, { status: 500 })

const readJsonBody = async (request: Request): Promise<unknown> =>
  request.json().catch(() => null)

export const makePortalRoutes = <Bindings,>(
  dependencies: PortalRouteDependencies<Bindings>,
) => {
  const storeFor = (env: Bindings): PortalStore =>
    dependencies.makeStore?.(env) ??
    makeD1PortalStore(dependencies.database(env))

  const handleClientEngagement = async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }
    const session = await dependencies.requireBrowserSession(request, env, ctx)
    if (session === undefined) {
      return unauthorizedResponse()
    }
    const store = storeFor(env)
    const identity = {
      userId: session.user.userId,
      email: session.user.email ?? null,
    }
    let engagement = await store.readEngagementForClient(identity)
    if (engagement === null) {
      return noStoreJsonResponse({ engagement: null })
    }
    // First authenticated visit through an email-only binding pins the
    // authoritative user id so future access no longer depends on the email.
    if (engagement.clientUserId === null) {
      const bound = await store
        .bindClient({
          engagementId: engagement.id,
          clientUserId: identity.userId,
        })
        .catch(() => null)
      if (bound !== null) {
        engagement = bound
      }
    }
    const items = await store.listContentItems(engagement.id)
    return noStoreJsonResponse({
      engagement: {
        id: engagement.id,
        name: engagement.name,
        status: engagement.status,
        createdAt: engagement.createdAt,
      },
      items,
      kpis: PORTAL_KPI_PLACEHOLDERS,
    })
  }

  const handleClientDecision = async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
    itemId: string,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }
    const session = await dependencies.requireBrowserSession(request, env, ctx)
    if (session === undefined) {
      return unauthorizedResponse()
    }
    const body = await readJsonBody(request)
    if (!isRecord(body) || typeof body.decision !== 'string') {
      return badRequest('decision is required')
    }
    const decision = body.decision
    if (!(PORTAL_DECISIONS as ReadonlyArray<string>).includes(decision)) {
      return badRequest('decision must be approve or reject')
    }

    const store = storeFor(env)
    const item = await store.readContentItemById(itemId)
    if (item === null) {
      return notFoundResponse()
    }
    const engagement = await store.readEngagementById(item.engagementId)
    if (
      engagement === null ||
      !portalEngagementOwnedBy(engagement, {
        userId: session.user.userId,
        email: session.user.email ?? null,
      })
    ) {
      // Fail-closed cross-client isolation: never confirm the item exists.
      return notFoundResponse()
    }

    const result = await store.decideContentItem(
      itemId,
      decision as PortalDecision,
    )
    return noStoreJsonResponse({
      ok: true,
      item: result.item,
      receiptRef: result.receiptRef,
      alreadyDecided: result.alreadyDecided,
    })
  }

  const handleAdminCreateEngagement = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }
    const body = await readJsonBody(request)
    if (!isRecord(body) || typeof body.name !== 'string') {
      return badRequest('name is required')
    }
    const store = storeFor(env)
    const engagement = await store.createEngagement({
      name: body.name,
      status:
        typeof body.status === 'string'
          ? (body.status as PortalEngagement['status'])
          : undefined,
      clientEmail: typeof body.clientEmail === 'string' ? body.clientEmail : null,
    })
    return noStoreJsonResponse({ ok: true, engagement }, { status: 201 })
  }

  const handleAdminReadEngagement = async (
    request: Request,
    env: Bindings,
    engagementId: string,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }
    const store = storeFor(env)
    const engagement = await store.readEngagementById(engagementId)
    if (engagement === null) {
      return notFoundResponse()
    }
    const items = await store.listContentItems(engagementId)
    return noStoreJsonResponse({ ok: true, engagement, items })
  }

  const handleAdminBind = async (
    request: Request,
    env: Bindings,
    engagementId: string,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }
    const body = await readJsonBody(request)
    if (!isRecord(body)) {
      return badRequest('clientUserId or clientEmail is required')
    }
    const store = storeFor(env)
    const engagement = await store.bindClient({
      engagementId,
      clientUserId:
        typeof body.clientUserId === 'string' ? body.clientUserId : null,
      clientEmail:
        typeof body.clientEmail === 'string' ? body.clientEmail : null,
    })
    if (engagement === null) {
      return notFoundResponse()
    }
    return noStoreJsonResponse({ ok: true, engagement })
  }

  const handleAdminSeedContent = async (
    request: Request,
    env: Bindings,
    engagementId: string,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }
    const body = await readJsonBody(request)
    if (!isRecord(body) || !Array.isArray(body.items)) {
      return badRequest('items array is required')
    }
    const parsed: Array<PortalSeedContentItemInput> = []
    for (const raw of body.items) {
      if (
        !isRecord(raw) ||
        typeof raw.channel !== 'string' ||
        typeof raw.title !== 'string' ||
        typeof raw.body !== 'string'
      ) {
        return badRequest('each item needs channel, title, and body')
      }
      parsed.push({
        kind:
          typeof raw.kind === 'string'
            ? (raw.kind as PortalSeedContentItemInput['kind'])
            : undefined,
        channel: raw.channel,
        variant:
          typeof raw.variant === 'string'
            ? (raw.variant as PortalSeedContentItemInput['variant'])
            : undefined,
        pairRef: typeof raw.pairRef === 'string' ? raw.pairRef : null,
        title: raw.title,
        body: raw.body,
      })
    }
    const store = storeFor(env)
    const items = await store.seedContentItems(engagementId, parsed)
    return noStoreJsonResponse({ ok: true, items }, { status: 201 })
  }

  const routeAdmin = async (
    request: Request,
    env: Bindings,
    path: string,
  ): Promise<HttpResponse> => {
    const authorized = await dependencies
      .requireAdminApiToken(request, env)
      .catch(() => false)
    if (!authorized) {
      return unauthorizedResponse()
    }

    if (ADMIN_ENGAGEMENTS.test(path)) {
      return handleAdminCreateEngagement(request, env)
    }
    const bindMatch = ADMIN_BIND.exec(path)
    if (bindMatch !== null) {
      return handleAdminBind(request, env, bindMatch[1]!)
    }
    const seedMatch = ADMIN_CONTENT_ITEMS.exec(path)
    if (seedMatch !== null) {
      return handleAdminSeedContent(request, env, seedMatch[1]!)
    }
    const readMatch = ADMIN_ENGAGEMENT.exec(path)
    if (readMatch !== null) {
      return handleAdminReadEngagement(request, env, readMatch[1]!)
    }
    return notFoundResponse()
  }

  return {
    routePortalRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname
      if (!ALL.some(pattern => pattern.test(path))) {
        return undefined
      }

      const dispatch = async (): Promise<HttpResponse> => {
        if (path.startsWith('/api/portal/admin/')) {
          return routeAdmin(request, env, path)
        }
        if (CLIENT_ENGAGEMENT.test(path)) {
          return handleClientEngagement(request, env, ctx)
        }
        const decisionMatch = CLIENT_DECISION.exec(path)
        if (decisionMatch !== null) {
          return handleClientDecision(request, env, ctx, decisionMatch[1]!)
        }
        return notFoundResponse()
      }

      return Effect.promise(() =>
        dispatch().catch(error => portalErrorResponse(error)),
      )
    },
  }
}

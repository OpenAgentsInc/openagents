// PORTAL-1 (#8652): /portal data boundary.
//
// Same-origin fetchers against the Worker portal API (portal-routes.ts in
// workers/api — the authority boundary; this module is presentation-side
// only). All reads are fail-soft: network/parse failures return null and the
// surface renders an honest unavailable state, never fabricated data.

export const PORTAL_SESSION_URL = '/api/auth/session'
export const PORTAL_ENGAGEMENT_URL = '/api/portal/engagement'

export type PortalAuthMode = 'LoggedIn' | 'LoggedOut'

export const fetchPortalAuthMode = async (
  fetchFn: typeof fetch = fetch,
  url: string = PORTAL_SESSION_URL,
): Promise<PortalAuthMode> => {
  try {
    const response = await fetchFn(url, {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (!response.ok) return 'LoggedOut'
    const body = (await response.json()) as { authenticated?: unknown }
    return body.authenticated === true ? 'LoggedIn' : 'LoggedOut'
  } catch {
    return 'LoggedOut'
  }
}

export const portalLoginHref = (returnPath: string): string =>
  `/login/github?returnTo=${encodeURIComponent(returnPath)}`

export type PortalEngagementSummary = Readonly<{
  id: string
  name: string
  status: string
  createdAt: string
}>

export type PortalContentItem = Readonly<{
  id: string
  kind: string
  channel: string
  variant: string
  pairRef: string | null
  title: string
  body: string
  state: string
  decidedAt: string | null
  decisionReceiptRef: string | null
}>

export type PortalKpi = Readonly<{
  key: string
  label: string
  value: number | string | null
  note: string
}>

export type PortalEngagementSnapshot =
  | Readonly<{ kind: 'unauthorized' }>
  | Readonly<{ kind: 'none' }>
  | Readonly<{
      kind: 'ready'
      engagement: PortalEngagementSummary
      items: ReadonlyArray<PortalContentItem>
      kpis: ReadonlyArray<PortalKpi>
    }>

const stringOr = (value: unknown, fallback: string): string =>
  typeof value === 'string' ? value : fallback

const nullableString = (value: unknown): string | null =>
  typeof value === 'string' ? value : null

const parseContentItem = (raw: unknown): PortalContentItem | null => {
  if (typeof raw !== 'object' || raw === null) return null
  const record = raw as Record<string, unknown>
  if (typeof record.id !== 'string') return null
  return {
    id: record.id,
    kind: stringOr(record.kind, 'post'),
    channel: stringOr(record.channel, ''),
    variant: stringOr(record.variant, 'a'),
    pairRef: nullableString(record.pairRef),
    title: stringOr(record.title, ''),
    body: stringOr(record.body, ''),
    state: stringOr(record.state, 'draft'),
    decidedAt: nullableString(record.decidedAt),
    decisionReceiptRef: nullableString(record.decisionReceiptRef),
  }
}

const parseKpi = (raw: unknown): PortalKpi | null => {
  if (typeof raw !== 'object' || raw === null) return null
  const record = raw as Record<string, unknown>
  if (typeof record.key !== 'string' || typeof record.label !== 'string') {
    return null
  }
  return {
    key: record.key,
    label: record.label,
    value:
      typeof record.value === 'number' || typeof record.value === 'string'
        ? record.value
        : null,
    note: stringOr(record.note, ''),
  }
}

/** Null means the API was unreachable (honest unavailable state). */
export const fetchPortalEngagement = async (
  fetchFn: typeof fetch = fetch,
  url: string = PORTAL_ENGAGEMENT_URL,
): Promise<PortalEngagementSnapshot | null> => {
  try {
    const response = await fetchFn(url, {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (response.status === 401) {
      return { kind: 'unauthorized' }
    }
    if (!response.ok) {
      return null
    }
    const body = (await response.json()) as {
      engagement?: unknown
      items?: unknown
      kpis?: unknown
    }
    if (body.engagement === null || body.engagement === undefined) {
      return { kind: 'none' }
    }
    const engagementRecord = body.engagement as Record<string, unknown>
    if (typeof engagementRecord.id !== 'string') {
      return null
    }
    return {
      kind: 'ready',
      engagement: {
        id: engagementRecord.id,
        name: stringOr(engagementRecord.name, ''),
        status: stringOr(engagementRecord.status, 'preparing'),
        createdAt: stringOr(engagementRecord.createdAt, ''),
      },
      items: Array.isArray(body.items)
        ? body.items
            .map(parseContentItem)
            .filter((item): item is PortalContentItem => item !== null)
        : [],
      kpis: Array.isArray(body.kpis)
        ? body.kpis.map(parseKpi).filter((kpi): kpi is PortalKpi => kpi !== null)
        : [],
    }
  } catch {
    return null
  }
}

export type PortalDecision = 'approve' | 'reject'

export type PortalDecisionResult =
  | Readonly<{
      ok: true
      state: string
      receiptRef: string
      alreadyDecided: boolean
    }>
  | Readonly<{ ok: false; errorMessage: string }>

export const portalDecisionUrl = (itemId: string): string =>
  `/api/portal/content/${encodeURIComponent(itemId)}/decision`

export const submitPortalDecision = async (
  input: Readonly<{ itemId: string; decision: PortalDecision }>,
  fetchFn: typeof fetch = fetch,
): Promise<PortalDecisionResult> => {
  try {
    const response = await fetchFn(portalDecisionUrl(input.itemId), {
      method: 'POST',
      credentials: 'same-origin',
      headers: {
        'Content-Type': 'application/json',
        accept: 'application/json',
      },
      body: JSON.stringify({ decision: input.decision }),
    })
    const body = (await response.json().catch(() => ({}))) as {
      item?: { state?: unknown }
      receiptRef?: unknown
      alreadyDecided?: unknown
      reason?: unknown
      error?: unknown
    }
    if (!response.ok || typeof body.receiptRef !== 'string') {
      return {
        ok: false,
        errorMessage:
          typeof body.reason === 'string'
            ? body.reason
            : typeof body.error === 'string'
              ? body.error
              : 'Request failed',
      }
    }
    return {
      ok: true,
      state: stringOr(body.item?.state, input.decision === 'approve' ? 'approved' : 'rejected'),
      receiptRef: body.receiptRef,
      alreadyDecided: body.alreadyDecided === true,
    }
  } catch (error) {
    return {
      ok: false,
      errorMessage: error instanceof Error ? error.message : 'Request failed',
    }
  }
}

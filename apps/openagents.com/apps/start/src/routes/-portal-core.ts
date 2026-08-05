// PORTAL-1 (#8652): /portal client-portal state core — the phase machine, the
// login-gated load sequence, the optimistic decision transitions, and the
// account-identity copy the authenticated empty state renders.
//
// Converted from an Effect Native view tree to plain React (#9325): this
// module is the data/state half (no JSX, host-free, so the Cloud Run monolith
// browser entry can bundle it via src/portal-entry.ts) and -portal-page.tsx
// renders it.
//
// Login-gated: the surface probes /api/auth/session first. Logged-out yields
// the login gate phase (the page never renders engagement content in it);
// logged-in loads the caller's OWN engagement from the owner-scoped Worker
// API. Approve/reject POST /api/portal/content/:id/decision -> optimistic
// item state + the decision receipt ref rendered inline, rolled back on
// failure.
//
// KPI tiles are HONEST placeholders: values render as an em dash with the
// "placeholder until live funnel wiring" note. Nothing is fabricated
// client-side.

import {
  fetchPortalEngagement,
  fetchPortalSession,
  type PortalContentItem,
  type PortalDecision,
  type PortalDecisionResult,
  type PortalEngagementSummary,
  type PortalKpi,
  type PortalSessionIdentity,
} from './-portal-data'

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export type PortalDecisionPanel = Readonly<{
  phase: 'sending' | 'decided' | 'failed'
  message: string
  receiptRef?: string
}>

export type PortalPageState = Readonly<{
  phase: 'loading' | 'logged_out' | 'empty' | 'ready' | 'unavailable'
  /** Signed-in identity from /api/auth/session (null while logged out or
   * loading). Rendered on the authenticated empty state so a mismatched
   * engagement binding is self-diagnosable (#8652 reopen). */
  identity: PortalSessionIdentity | null
  engagement: PortalEngagementSummary | null
  items: ReadonlyArray<PortalContentItem>
  kpis: ReadonlyArray<PortalKpi>
  decisionPanels: Readonly<Record<string, PortalDecisionPanel>>
}>

export const initialPortalPageState: PortalPageState = {
  phase: 'loading',
  identity: null,
  engagement: null,
  items: [],
  kpis: [],
  decisionPanels: {},
}

// ---------------------------------------------------------------------------
// Account identity (empty-state contract:
// openagents_web.portal_empty_state_account_identity.v1)
// ---------------------------------------------------------------------------

/** Human-readable signed-in identity: prefer the session email, then the
 * provider login, then an honest fallback. Never blank. */
export const portalIdentityLabel = (
  identity: PortalSessionIdentity | null,
): string => {
  if (identity?.email !== null && identity?.email !== undefined && identity.email !== '') {
    return identity.email
  }
  if (identity?.login !== null && identity?.login !== undefined && identity.login !== '') {
    return identity.login
  }
  return 'your account (no email on this session)'
}

/** The authenticated empty state always names WHICH account is signed in, so
 * an engagement bound to a different email is self-diagnosable. */
export const portalEmptyStateIdentityLine = (
  identity: PortalSessionIdentity | null,
): string =>
  `No engagement is linked to this account yet. Signed in as ${portalIdentityLabel(identity)}.`

// ---------------------------------------------------------------------------
// Content layout
// ---------------------------------------------------------------------------

/** Group items into A/B pair rows (pairRef), unpaired items render alone. */
export const portalContentPairs = (
  items: ReadonlyArray<PortalContentItem>,
): ReadonlyArray<ReadonlyArray<PortalContentItem>> => {
  const byPair = new Map<string, Array<PortalContentItem>>()
  const rows: Array<ReadonlyArray<PortalContentItem>> = []
  for (const item of items) {
    if (item.pairRef === null) {
      rows.push([item])
      continue
    }
    const existing = byPair.get(item.pairRef)
    if (existing === undefined) {
      const group: Array<PortalContentItem> = [item]
      byPair.set(item.pairRef, group)
      rows.push(group)
      continue
    }
    existing.push(item)
  }
  return rows
}

/** KPI values are never fabricated: a missing value renders as an em dash. */
export const portalKpiValueLabel = (kpi: PortalKpi): string =>
  kpi.value === null ? '—' : String(kpi.value)

export const portalKpiNote = (kpis: ReadonlyArray<PortalKpi>): string =>
  kpis.every((kpi) => kpi.value === null)
    ? 'Honest placeholders: KPI values appear once the live funnel wiring exists — no fabricated numbers.'
    : 'Live values where wired; placeholders stay explicit.'

// ---------------------------------------------------------------------------
// Load: login gate first, then the owner-scoped engagement read
// ---------------------------------------------------------------------------

/**
 * Fail-soft by construction: any unreachable/unparseable read resolves to the
 * honest `unavailable` phase rather than stale or fabricated data. A 401 on
 * the engagement read falls back to the login gate.
 */
export const loadPortalPageState = async (
  fetchFn: typeof fetch = fetch,
): Promise<PortalPageState> => {
  try {
    const session = await fetchPortalSession(fetchFn)
    if (session.mode === 'LoggedOut') {
      return { ...initialPortalPageState, phase: 'logged_out' }
    }
    const identity = session.identity
    const snapshot = await fetchPortalEngagement(fetchFn)
    if (snapshot === null) {
      return { ...initialPortalPageState, phase: 'unavailable', identity }
    }
    if (snapshot.kind === 'unauthorized') {
      return { ...initialPortalPageState, phase: 'logged_out' }
    }
    if (snapshot.kind === 'none') {
      return { ...initialPortalPageState, phase: 'empty', identity }
    }
    return {
      ...initialPortalPageState,
      phase: 'ready',
      identity,
      engagement: snapshot.engagement,
      items: snapshot.items,
      kpis: snapshot.kpis,
    }
  } catch {
    return { ...initialPortalPageState, phase: 'unavailable' }
  }
}

// ---------------------------------------------------------------------------
// Decisions: optimistic flip, commit on success, roll back on failure
// ---------------------------------------------------------------------------

const withItem = (
  state: PortalPageState,
  itemId: string,
  update: (item: PortalContentItem) => PortalContentItem,
): ReadonlyArray<PortalContentItem> =>
  state.items.map((item) => (item.id === itemId ? update(item) : item))

const withPanel = (
  state: PortalPageState,
  itemId: string,
  panel: PortalDecisionPanel,
): Readonly<Record<string, PortalDecisionPanel>> => ({
  ...state.decisionPanels,
  [itemId]: panel,
})

/**
 * Flip the card immediately so the click feels instant. Returns null when the
 * decision is not eligible (unknown item, already decided, unknown decision) —
 * the caller must then NOT send the request.
 */
export const portalStateWithOptimisticDecision = (
  state: PortalPageState,
  itemId: string,
  decision: PortalDecision,
): PortalPageState | null => {
  const item = state.items.find((entry) => entry.id === itemId)
  if (item === undefined || item.state !== 'draft') return null
  if (decision !== 'approve' && decision !== 'reject') return null
  const optimisticState = decision === 'approve' ? 'approved' : 'rejected'
  return {
    ...state,
    items: withItem(state, itemId, (entry) => ({
      ...entry,
      state: optimisticState,
    })),
    decisionPanels: withPanel(state, itemId, {
      phase: 'sending',
      message: decision === 'approve' ? 'Approving…' : 'Rejecting…',
    }),
  }
}

/** Commit the server's state + minted receipt ref, or roll the item back. */
export const portalStateWithDecisionResult = (
  state: PortalPageState,
  itemId: string,
  result: PortalDecisionResult,
): PortalPageState => {
  if (result.ok) {
    return {
      ...state,
      items: withItem(state, itemId, (entry) => ({
        ...entry,
        state: result.state,
        decisionReceiptRef: result.receiptRef,
      })),
      decisionPanels: withPanel(state, itemId, {
        phase: 'decided',
        message: 'Decision recorded',
        receiptRef: result.receiptRef,
      }),
    }
  }
  return {
    ...state,
    items: withItem(state, itemId, (entry) => ({ ...entry, state: 'draft' })),
    decisionPanels: withPanel(state, itemId, {
      phase: 'failed',
      message: `Decision failed · ${result.errorMessage}`,
    }),
  }
}

// PORTAL-1 (#8652): the /portal client-portal surface.
//
// Converted from an Effect Native view tree to plain React (#9325). The state
// half — phase machine, login-gated load, optimistic decision transitions,
// A/B pairing, and the honest KPI/identity copy — lives in -portal-core.ts so
// the Cloud Run monolith browser entry (src/portal-entry.ts) shares exactly
// this component and exactly that logic.
//
// Privacy: the loading and logged-out phases render NO engagement content.
// That is contract openagents_web.portal_owner_scoped_engagement.v1.

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { useCallback, useEffect, useState, type ReactNode } from 'react'

import {
  initialPortalPageState,
  loadPortalPageState,
  portalContentPairs,
  portalEmptyStateIdentityLine,
  portalKpiNote,
  portalKpiValueLabel,
  portalStateWithDecisionResult,
  portalStateWithOptimisticDecision,
  type PortalDecisionPanel,
  type PortalPageState,
} from './-portal-core'
import {
  portalLoginHref,
  portalSignOutHref,
  submitPortalDecision,
  type PortalContentItem,
  type PortalDecision,
  type PortalKpi,
  type PortalSessionIdentity,
} from './-portal-data'

const headingClass = 'm-0 text-xl font-semibold leading-tight text-khala-text'
const titleClass = 'm-0 text-base font-semibold leading-6 text-khala-text'
const bodyClass = 'm-0 text-sm leading-6 text-khala-text-muted'
const captionClass = 'm-0 text-xs leading-5 text-khala-text-faint'
const cardClass = 'grid min-w-0 gap-3 bg-khala-surface p-6'
const linkButtonClass =
  'khala-focus inline-flex min-h-10 w-fit shrink-0 items-center justify-center gap-2 whitespace-nowrap border px-4 py-2 font-mono text-sm font-medium transition-colors'
const primaryLinkClass = `${linkButtonClass} border-khala-text bg-khala-text text-black hover:bg-white`
const secondaryLinkClass = `${linkButtonClass} border-khala-border bg-transparent text-khala-text-muted hover:bg-white/5 hover:text-khala-text`

type BannerTone = 'info' | 'warn' | 'danger'

const bannerToneClass: Readonly<Record<BannerTone, string>> = {
  info: 'border-khala-energy/30 bg-khala-energy/10 text-khala-text',
  warn: 'border-khala-warning/40 bg-khala-warning/10 text-khala-text',
  danger: 'border-khala-danger/40 bg-khala-danger/10 text-khala-text',
}

function StatusBanner({
  message,
  tone,
}: Readonly<{ message: string; tone: BannerTone }>) {
  return (
    <p
      className={`m-0 border px-3 py-2 text-sm leading-6 ${bannerToneClass[tone]}`}
      data-portal-banner={tone}
      role={tone === 'danger' ? 'alert' : 'status'}
    >
      {message}
    </p>
  )
}

const engagementStatusVariant = (
  status: string,
): 'default' | 'ready' | 'running' | 'warning' => {
  if (status === 'active') return 'ready'
  if (status === 'preparing') return 'running'
  if (status === 'paused') return 'warning'
  return 'default'
}

const itemStateVariant = (
  state: string,
): 'default' | 'ready' | 'running' | 'danger' => {
  if (state === 'approved') return 'ready'
  if (state === 'rejected') return 'danger'
  if (state === 'published') return 'running'
  return 'default'
}

function SurfaceCard({
  children,
  region,
}: Readonly<{ children: ReactNode; region: string }>) {
  return (
    <Card className={cardClass} data-portal-card={region}>
      {children}
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Phases that must never render engagement content
// ---------------------------------------------------------------------------

function LoginGate() {
  return (
    <SurfaceCard region="login-gate">
      <h1 className={headingClass}>Client portal</h1>
      <StatusBanner message="Log in to view your engagement." tone="info" />
      <p className={bodyClass}>
        Your engagement dashboard, content calendar, and approval queue are
        private to your account.
      </p>
      <a
        className={primaryLinkClass}
        data-portal-link="login"
        href={portalLoginHref('/portal')}
      >
        Log in with GitHub
      </a>
    </SurfaceCard>
  )
}

function EmptyState({
  identity,
}: Readonly<{ identity: PortalSessionIdentity | null }>) {
  return (
    <SurfaceCard region="empty">
      <h1 className={headingClass}>Your setup is being prepared</h1>
      <p className={bodyClass}>
        Your engagement has not been provisioned yet. Once your OpenAgents team
        activates it, your funnel status and content calendar appear here.
      </p>
      <StatusBanner
        message={portalEmptyStateIdentityLine(identity)}
        tone="info"
      />
      <p className={bodyClass}>
        If your engagement was set up under a different email, contact your
        OpenAgents team with the address above — or switch to the account it
        was set up with.
      </p>
      <a
        className={secondaryLinkClass}
        data-portal-link="signout"
        href={portalSignOutHref}
      >
        Sign out / switch account
      </a>
    </SurfaceCard>
  )
}

function UnavailableState() {
  return (
    <SurfaceCard region="unavailable">
      <h1 className={headingClass}>Portal unavailable</h1>
      <StatusBanner
        message="The portal API is unreachable right now. Nothing is shown rather than showing stale or fabricated data."
        tone="warn"
      />
    </SurfaceCard>
  )
}

// ---------------------------------------------------------------------------
// Ready phase
// ---------------------------------------------------------------------------

function KpiTiles({ kpis }: Readonly<{ kpis: ReadonlyArray<PortalKpi> }>) {
  return (
    <section className="grid min-w-0 gap-3" data-portal-region="kpis">
      <h2 className={titleClass}>Funnel KPIs</h2>
      <div
        className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
        data-portal-kpis=""
      >
        {kpis.map(kpi => (
          <div
            className="min-w-0 border border-khala-border bg-khala-surface p-4"
            data-portal-kpi={kpi.key}
            key={kpi.key}
          >
            <p className={captionClass}>{kpi.label}</p>
            <p className="m-0 mt-1 font-mono text-2xl font-semibold tabular-nums text-khala-text">
              {portalKpiValueLabel(kpi)}
            </p>
          </div>
        ))}
      </div>
      <p className={captionClass}>{portalKpiNote(kpis)}</p>
    </section>
  )
}

function DecisionPanel({
  item,
  panel,
}: Readonly<{
  item: PortalContentItem
  panel: PortalDecisionPanel | undefined
}>) {
  if (panel?.phase === 'sending') {
    return <p className={captionClass}>{panel.message}</p>
  }
  if (panel?.phase === 'failed') {
    return <StatusBanner message={panel.message} tone="danger" />
  }
  const receiptRef = panel?.receiptRef ?? item.decisionReceiptRef
  if (item.state !== 'draft' && receiptRef !== null && receiptRef !== undefined) {
    return (
      <p className={captionClass} data-portal-receipt={receiptRef}>
        receipt: {receiptRef}
      </p>
    )
  }
  return null
}

function ContentItemCard({
  item,
  onDecide,
  panel,
}: Readonly<{
  item: PortalContentItem
  onDecide: (itemId: string, decision: PortalDecision) => void
  panel: PortalDecisionPanel | undefined
}>) {
  return (
    <Card
      className="grid min-w-0 gap-2 bg-khala-surface p-4"
      data-portal-item={item.id}
    >
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <Badge variant="outline">
          channel <span className="ml-1 normal-case">{item.channel}</span>
        </Badge>
        <Badge>variant {item.variant.toUpperCase()}</Badge>
        <Badge variant={itemStateVariant(item.state)}>{item.state}</Badge>
      </div>
      <h3 className={titleClass}>{item.title}</h3>
      <p className={bodyClass}>{item.body}</p>
      {item.state === 'draft' && panel?.phase !== 'sending' ? (
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <Button onClick={() => onDecide(item.id, 'approve')} type="button">
            Approve
          </Button>
          <Button
            onClick={() => onDecide(item.id, 'reject')}
            type="button"
            variant="secondary"
          >
            Reject
          </Button>
        </div>
      ) : null}
      <DecisionPanel item={item} panel={panel} />
    </Card>
  )
}

function ContentCalendar({
  onDecide,
  state,
}: Readonly<{
  onDecide: (itemId: string, decision: PortalDecision) => void
  state: PortalPageState
}>) {
  return (
    <section className="grid min-w-0 gap-3" data-portal-region="calendar">
      <h2 className={titleClass}>Content calendar</h2>
      <p className={bodyClass}>
        Agent-drafted posts awaiting your decision. A/B variants render side by
        side; every approve or reject mints a receipt.
      </p>
      {state.items.length === 0 ? (
        <StatusBanner
          message="No content items yet — drafts appear here as your team publishes the calendar."
          tone="info"
        />
      ) : (
        portalContentPairs(state.items).map((pair, index) => (
          <div
            className="grid min-w-0 gap-3 lg:grid-cols-2"
            data-portal-pair=""
            key={pair[0]?.pairRef ?? pair[0]?.id ?? index}
          >
            {pair.map(item => (
              <ContentItemCard
                item={item}
                key={item.id}
                onDecide={onDecide}
                panel={state.decisionPanels[item.id]}
              />
            ))}
          </div>
        ))
      )}
    </section>
  )
}

function ReadyState({
  onDecide,
  state,
}: Readonly<{
  onDecide: (itemId: string, decision: PortalDecision) => void
  state: PortalPageState
}>) {
  const engagement = state.engagement
  if (engagement === null) {
    return <EmptyState identity={state.identity} />
  }
  return (
    <>
      <SurfaceCard region="header">
        <div className="flex min-w-0 flex-wrap items-center gap-3">
          <h1 className={headingClass}>{engagement.name}</h1>
          <Badge variant={engagementStatusVariant(engagement.status)}>
            {engagement.status}
          </Badge>
        </div>
        <p className={bodyClass}>
          Your engagement at a glance: funnel status, the content calendar, and
          your approval queue.
        </p>
      </SurfaceCard>
      <KpiTiles kpis={state.kpis} />
      <hr className="m-0 border-0 border-t border-khala-border" />
      <ContentCalendar onDecide={onDecide} state={state} />
    </>
  )
}

// ---------------------------------------------------------------------------
// Surface: a pure function of PortalPageState, so every phase — including the
// two that must never leak engagement content — is provable on server-
// rendered markup.
// ---------------------------------------------------------------------------

const noDecision = (): void => undefined

export function PortalSurface({
  onDecide = noDecision,
  state,
}: Readonly<{
  onDecide?: (itemId: string, decision: PortalDecision) => void
  state: PortalPageState
}>) {
  return (
    <main
      aria-label="OpenAgents client portal"
      className="portal-host min-h-dvh bg-khala-void text-khala-text"
      data-portal=""
      data-route="portal"
    >
      <section
        className="mx-auto grid w-full max-w-5xl gap-6 px-4 py-12 sm:px-6"
        data-portal-phase={state.phase}
        data-portal-root=""
      >
        {state.phase === 'loading' ? (
          <p className={bodyClass}>Loading your portal…</p>
        ) : state.phase === 'logged_out' ? (
          <LoginGate />
        ) : state.phase === 'empty' ? (
          <EmptyState identity={state.identity} />
        ) : state.phase === 'unavailable' ? (
          <UnavailableState />
        ) : (
          <ReadyState onDecide={onDecide} state={state} />
        )}
      </section>
    </main>
  )
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export function PortalPage() {
  const [state, setState] = useState<PortalPageState>(initialPortalPageState)

  useEffect(() => {
    let cancelled = false
    void loadPortalPageState().then(loaded => {
      if (!cancelled) {
        setState(previous => ({ ...previous, ...loaded }))
      }
    })
    return () => {
      cancelled = true
    }
  }, [])

  const onDecide = useCallback(
    (itemId: string, decision: PortalDecision) => {
      // Eligibility is decided against the state these buttons were rendered
      // from: a null result means the item is unknown or already decided, and
      // no request may be sent.
      const optimistic = portalStateWithOptimisticDecision(
        state,
        itemId,
        decision,
      )
      if (optimistic === null) return
      setState(optimistic)
      void submitPortalDecision({ itemId, decision }).then(result => {
        setState(previous =>
          portalStateWithDecisionResult(previous, itemId, result),
        )
      })
    },
    [state],
  )

  return <PortalSurface onDecide={onDecide} state={state} />
}

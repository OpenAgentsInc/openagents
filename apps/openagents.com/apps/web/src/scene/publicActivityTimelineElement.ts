import {
  type PublicActivityTimelineEnvelope,
  type PublicActivityTimelineEvent,
  type PublicActivityTimelineSourceLag,
  assertPublicActivityTimelineEnvelopeSafe,
  publicActivityTimelineHasUnsafeMaterial,
} from '@openagentsinc/public-activity-timeline'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

export const PUBLIC_ACTIVITY_TIMELINE_TAG = 'oa-public-activity-timeline'
export const PUBLIC_ACTIVITY_TIMELINE_ENDPOINT =
  '/api/public/activity-timeline?limit=100'
export const PUBLIC_ACTIVITY_TIMELINE_REFRESH_MS = 15_000

type ActivityPaneId = 'fleet' | 'forum' | 'money' | 'proof' | 'timeline'
type ActivityFilter =
  | 'all'
  | 'boot'
  | 'forum'
  | 'operator'
  | 'settle'
  | 'verify'
  | 'work'
type ActivityTimelineDataState = 'loading' | 'ok' | 'empty' | 'error'

export type PublicActivityEventUrl = Readonly<{
  href: string
  label: string
  ref: string
}>

export type PublicActivityTimelineHandle = Readonly<{
  dispose: () => void
  refresh: () => Promise<void>
}>

export type PublicActivityTimelineMountOptions = Readonly<{
  endpoint?: string
  fetchFn?: typeof fetch
  onState?: (state: ActivityTimelineDataState) => void
  refreshIntervalMs?: number
}>

type PublicActivityTimelineRenderState = {
  activeFilter: ActivityFilter
  endpoint: string
  selectedCursor: string | null
}

const hostCss = `
:host {
  display: block;
  min-height: 100dvh;
  background: #08090a;
  color: #f4f2ea;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  color-scheme: dark;
}
* { box-sizing: border-box; }
.activity {
  min-height: 100dvh;
  display: grid;
  gap: 16px;
  padding: clamp(16px, 3vw, 32px);
  background:
    linear-gradient(180deg, rgba(22, 24, 25, 0.92), rgba(8, 9, 10, 1) 34%),
    #08090a;
}
.hero {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(18rem, 34rem);
  gap: 16px;
  align-items: end;
  border-bottom: 1px solid rgba(244, 242, 234, 0.14);
  padding-bottom: 16px;
}
.eyebrow {
  margin: 0 0 6px;
  color: #8dd3c7;
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
h1, h2, h3, p, dl, dd, ol, pre { margin: 0; }
h1 {
  max-width: 16ch;
  color: #fff8e6;
  font-size: clamp(2rem, 5vw, 4.25rem);
  font-weight: 760;
  line-height: 0.96;
  letter-spacing: 0;
}
h2 {
  color: #fff8e6;
  font-size: 0.95rem;
  font-weight: 760;
  letter-spacing: 0;
}
h3 {
  color: #dfe8ff;
  font-size: 0.78rem;
  font-weight: 720;
  letter-spacing: 0;
}
.boundary {
  margin-top: 10px;
  max-width: 76ch;
  color: rgba(244, 242, 234, 0.68);
  font-size: 0.9rem;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}
.metric {
  min-width: 0;
  border: 1px solid rgba(244, 242, 234, 0.12);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.035);
  padding: 10px;
}
.metric dt {
  margin: 0 0 5px;
  color: rgba(244, 242, 234, 0.5);
  font-size: 0.68rem;
  font-weight: 720;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.metric dd {
  min-width: 0;
  color: #f4f2ea;
  font-size: 0.95rem;
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
}
.source-strip {
  display: grid;
  gap: 10px;
  border: 1px solid rgba(244, 242, 234, 0.12);
  border-radius: 8px;
  background: rgba(14, 15, 16, 0.84);
  padding: 12px;
}
.source-strip-head {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  align-items: baseline;
  justify-content: space-between;
}
.source-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.source-status {
  max-width: 100%;
  display: inline-flex;
  gap: 6px;
  align-items: center;
  min-height: 28px;
  border: 1px solid rgba(244, 242, 234, 0.12);
  border-radius: 999px;
  padding: 5px 9px;
  color: rgba(244, 242, 234, 0.78);
  font-size: 0.72rem;
  line-height: 1.2;
  overflow-wrap: anywhere;
}
.source-status strong {
  color: #f4f2ea;
  font-weight: 720;
}
.source-status-current { border-color: rgba(115, 214, 168, 0.32); background: rgba(115, 214, 168, 0.09); }
.source-status-stale { border-color: rgba(245, 179, 80, 0.36); background: rgba(245, 179, 80, 0.1); }
.source-status-unavailable { border-color: rgba(255, 126, 126, 0.36); background: rgba(255, 126, 126, 0.1); }
.source-status-projection_gap { border-color: rgba(155, 180, 255, 0.36); background: rgba(155, 180, 255, 0.1); }
.dashboard-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(20rem, 0.74fr);
  gap: 16px;
  align-items: start;
}
.pane-grid {
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}
.pane,
.proof-drawer {
  min-width: 0;
  overflow: hidden;
  border: 1px solid rgba(244, 242, 234, 0.12);
  border-radius: 8px;
  background: rgba(16, 17, 18, 0.9);
}
.pane[data-activity-pane="timeline"] {
  grid-column: 1 / -1;
}
.pane-head,
.proof-head {
  display: flex;
  min-width: 0;
  gap: 10px;
  align-items: baseline;
  justify-content: space-between;
  border-bottom: 1px solid rgba(244, 242, 234, 0.1);
  padding: 12px;
}
.count {
  color: rgba(244, 242, 234, 0.55);
  font-size: 0.74rem;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.event-list,
.timeline-list {
  display: grid;
  list-style: none;
  margin: 0;
  padding: 0;
}
.event-item,
.timeline-row {
  min-width: 0;
  border-top: 1px solid rgba(244, 242, 234, 0.08);
}
.event-item:first-child,
.timeline-row:first-child {
  border-top: 0;
}
.event-button {
  width: 100%;
  display: grid;
  gap: 8px;
  min-width: 0;
  border: 0;
  border-left: 3px solid transparent;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
  padding: 12px;
  text-align: left;
}
.event-button:hover,
.event-button:focus-visible {
  background: rgba(255, 255, 255, 0.05);
  outline: none;
}
.event-button[aria-pressed="true"] {
  border-left-color: #8dd3c7;
  background: rgba(141, 211, 199, 0.08);
}
.event-head,
.event-meta,
.proof-meta {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  gap: 6px 10px;
  align-items: center;
}
.kind {
  color: #dfe8ff;
  font-size: 0.74rem;
  font-weight: 720;
  overflow-wrap: anywhere;
}
time,
.muted {
  color: rgba(244, 242, 234, 0.5);
  font-size: 0.72rem;
}
.event-text {
  color: rgba(244, 242, 234, 0.82);
  font-size: 0.86rem;
  line-height: 1.45;
  overflow-wrap: anywhere;
}
.tag,
.ref {
  max-width: 100%;
  display: inline-flex;
  min-width: 0;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.055);
  color: rgba(244, 242, 234, 0.7);
  font-size: 0.68rem;
  line-height: 1.25;
  padding: 3px 7px;
  overflow-wrap: anywhere;
}
.refs {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  gap: 5px;
}
.filter-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  border-bottom: 1px solid rgba(244, 242, 234, 0.08);
  padding: 10px 12px;
}
.filter-button {
  min-height: 30px;
  border: 1px solid rgba(244, 242, 234, 0.14);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.035);
  color: rgba(244, 242, 234, 0.72);
  cursor: pointer;
  font: inherit;
  font-size: 0.72rem;
  font-weight: 720;
  padding: 5px 10px;
}
.filter-button:hover,
.filter-button:focus-visible {
  border-color: rgba(141, 211, 199, 0.44);
  color: #f4f2ea;
  outline: none;
}
.filter-button[aria-pressed="true"] {
  border-color: rgba(141, 211, 199, 0.62);
  background: rgba(141, 211, 199, 0.12);
  color: #fff8e6;
}
.timeline-button {
  padding: 11px 12px;
}
.proof-drawer {
  position: sticky;
  top: 16px;
  display: grid;
}
.proof-body {
  display: grid;
  gap: 14px;
  padding: 12px;
}
.proof-fields {
  display: grid;
  grid-template-columns: minmax(8rem, 0.36fr) minmax(0, 1fr);
  gap: 8px 12px;
}
.proof-fields dt {
  color: rgba(244, 242, 234, 0.48);
  font-size: 0.68rem;
  font-weight: 720;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}
.proof-fields dd {
  min-width: 0;
  color: rgba(244, 242, 234, 0.82);
  font-size: 0.78rem;
  line-height: 1.45;
  overflow-wrap: anywhere;
}
.proof-section {
  display: grid;
  gap: 8px;
  min-width: 0;
}
.proof-json {
  max-height: 18rem;
  overflow: auto;
  border: 1px solid rgba(244, 242, 234, 0.1);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.34);
  color: rgba(244, 242, 234, 0.78);
  font: 0.72rem/1.45 ui-monospace, SFMono-Regular, Menlo, monospace;
  padding: 10px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.url-list {
  display: grid;
  gap: 6px;
  min-width: 0;
}
.proof-link {
  min-width: 0;
  color: #8dd3c7;
  font-size: 0.76rem;
  line-height: 1.45;
  overflow-wrap: anywhere;
  text-underline-offset: 0.18rem;
}
.proof-link:hover,
.proof-link:focus-visible {
  color: #fff8e6;
  outline: none;
}
.empty,
.error {
  padding: 12px;
  color: rgba(244, 242, 234, 0.58);
  font-size: 0.84rem;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.error {
  border: 1px solid rgba(255, 126, 126, 0.28);
  border-radius: 8px;
  background: rgba(255, 126, 126, 0.08);
  color: #ffd1d1;
}
.loading {
  min-height: 20rem;
  display: grid;
  place-items: center;
  border: 1px solid rgba(244, 242, 234, 0.12);
  border-radius: 8px;
  color: rgba(244, 242, 234, 0.62);
}
@media (max-width: 1080px) {
  .hero,
  .dashboard-grid,
  .pane-grid {
    grid-template-columns: 1fr;
  }
  .proof-drawer {
    position: static;
  }
}
@media (max-width: 640px) {
  .activity {
    padding: 12px;
  }
  .metrics,
  .proof-fields {
    grid-template-columns: 1fr;
  }
  .pane-head,
  .proof-head {
    align-items: flex-start;
    flex-direction: column;
  }
}
`

const element = defineCustomElement({
  events: {},
  properties: {},
  tag: PUBLIC_ACTIVITY_TIMELINE_TAG,
})

const fleetKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'assignment_ready',
  'capacity_snapshot',
  'pylon_heartbeat',
  'pylon_registered',
  'wallet_ready',
])

const moneyKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'real_bitcoin_moved',
  'settlement_recorded',
])

const forumKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'forum_posted',
  'forum_topic_created',
])

const workKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'trace_submitted',
  'window_closed',
  'window_opened',
  'work_claimed',
])

const verifyKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'verification_queued',
  'verification_rejected',
  'verification_verified',
])

const operatorKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'artanis_tick',
  'projection_gap',
])

const activityFilters: ReadonlyArray<
  Readonly<{ id: ActivityFilter; label: string }>
> = [
  { id: 'all', label: 'All' },
  { id: 'boot', label: 'Boot' },
  { id: 'work', label: 'Work' },
  { id: 'verify', label: 'Verify' },
  { id: 'settle', label: 'Settle' },
  { id: 'forum', label: 'Forum' },
  { id: 'operator', label: 'Operator' },
]

const filterForEvent = (event: PublicActivityTimelineEvent): ActivityFilter => {
  if (
    fleetKinds.has(event.kind) ||
    event.sourceKind === 'pylon_api' ||
    event.sourceKind === 'pylon_presence' ||
    event.sourceKind === 'capacity_funnel'
  ) {
    return 'boot'
  }

  if (
    workKinds.has(event.kind) ||
    event.sourceKind === 'training_window' ||
    event.sourceKind === 'training_trace'
  ) {
    return 'work'
  }

  if (
    verifyKinds.has(event.kind) ||
    event.sourceKind === 'training_verification'
  ) {
    return 'verify'
  }

  if (moneyKinds.has(event.kind) || event.sourceKind === 'settlement_receipt') {
    return 'settle'
  }

  if (forumKinds.has(event.kind) || event.sourceKind === 'forum') {
    return 'forum'
  }

  if (
    operatorKinds.has(event.kind) ||
    event.sourceKind === 'artanis' ||
    event.sourceKind === 'projection_gap'
  ) {
    return 'operator'
  }

  return 'operator'
}

const filterMatchesEvent = (
  filter: ActivityFilter,
  event: PublicActivityTimelineEvent,
): boolean => filter === 'all' || filterForEvent(event) === filter

const filterLabel = (filter: ActivityFilter): string =>
  activityFilters.find(item => item.id === filter)?.label ??
  displayLabel(filter)

const create = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const setDataState = (
  root: HTMLElement,
  state: ActivityTimelineDataState,
  options: PublicActivityTimelineMountOptions,
): void => {
  root.dataset.state = state
  options.onState?.(state)
}

const displayLabel = (value: string): string => value.replaceAll('_', ' ')

const formatCount = (value: number): string => value.toLocaleString('en-US')

const formatTime = (value: string | null | undefined): string => {
  if (value === null || value === undefined || value.trim() === '') {
    return 'unknown'
  }

  const timestamp = Date.parse(value)
  if (Number.isNaN(timestamp)) return value

  return new Intl.DateTimeFormat('en-US', {
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    month: 'short',
    timeZoneName: 'short',
  }).format(timestamp)
}

const lagLabel = (lag: PublicActivityTimelineSourceLag): string => {
  if (lag.lagSeconds === null) return `${lag.status} / no source timestamp`
  return `${lag.status} / ${formatCount(Math.round(lag.lagSeconds))}s lag`
}

const sourceStatusBadge = (
  lag: PublicActivityTimelineSourceLag,
): HTMLElement => {
  const badge = create('span', `source-status source-status-${lag.status}`)
  badge.dataset.sourceStatus = lag.status
  badge.dataset.sourceKind = lag.sourceKind
  const source = create('strong', undefined, displayLabel(lag.sourceKind))
  const status = create('span', undefined, lagLabel(lag))
  badge.append(source, status)
  return badge
}

const appendMetric = (
  parent: HTMLElement,
  label: string,
  value: string,
): void => {
  const metric = create('div', 'metric')
  metric.append(create('dt', undefined, label), create('dd', undefined, value))
  parent.append(metric)
}

const refsView = (
  refs: ReadonlyArray<string>,
  limit = refs.length,
): HTMLElement => {
  const wrapper = create('div', 'refs')
  const visible = refs.slice(0, limit)

  if (visible.length === 0) {
    wrapper.append(create('span', 'muted', 'none'))
    return wrapper
  }

  for (const ref of visible) {
    wrapper.append(create('span', 'ref', ref))
  }

  if (refs.length > visible.length) {
    wrapper.append(create('span', 'ref', `+${refs.length - visible.length}`))
  }

  return wrapper
}

const eventAmountLabel = (event: PublicActivityTimelineEvent): string | null =>
  typeof event.amountSats === 'number' && Number.isFinite(event.amountSats)
    ? `${formatCount(event.amountSats)} sats`
    : null

const eventMetaItems = (
  event: PublicActivityTimelineEvent,
): ReadonlyArray<string> =>
  [
    filterLabel(filterForEvent(event)),
    displayLabel(event.sourceKind),
    event.state,
    eventAmountLabel(event),
    event.realBitcoinMoved === true
      ? 'real Bitcoin'
      : event.realBitcoinMoved === false
        ? 'simulated'
        : null,
  ].filter((item): item is string => item !== null && item !== undefined)

const safePublicPath = (href: string): string | null => {
  const trimmed = href.trim()
  if (
    trimmed.length === 0 ||
    !trimmed.startsWith('/') ||
    trimmed.startsWith('//') ||
    trimmed.includes('{') ||
    trimmed.includes('}') ||
    publicActivityTimelineHasUnsafeMaterial(trimmed)
  ) {
    return null
  }

  return trimmed
}

const publicHrefForRef = (
  ref: string,
  event: PublicActivityTimelineEvent,
): string | null => {
  const trimmed = ref.trim()
  if (trimmed.startsWith('route:')) {
    return safePublicPath(trimmed.slice('route:'.length))
  }

  if (trimmed.startsWith('/')) {
    return safePublicPath(trimmed)
  }

  const githubIssue = /^issue\.public\.github\.(\d+)$/i.exec(trimmed)
  if (githubIssue !== null) {
    return `https://github.com/OpenAgentsInc/openagents/issues/${githubIssue[1]}`
  }

  if (/^receipt\.forum\./i.test(trimmed)) {
    return safePublicPath(`/api/forum/receipts/${encodeURIComponent(trimmed)}`)
  }

  if (/^receipt\.(nexus\.|nexus_|nexus-pylon\.|public\.)/i.test(trimmed)) {
    return safePublicPath(
      `/api/public/nexus-pylon/receipts/${encodeURIComponent(trimmed)}`,
    )
  }

  if (/^training\.verification\.challenge\./i.test(trimmed)) {
    return safePublicPath(
      `/api/public/training/verification-challenges/${encodeURIComponent(
        trimmed,
      )}`,
    )
  }

  if (
    /^training\.window\./i.test(trimmed) ||
    /^trace\.public\./i.test(trimmed)
  ) {
    const runRef = event.runRef?.trim()
    if (runRef !== undefined && runRef.length > 0) {
      return safePublicPath(
        `/api/public/training/runs/${encodeURIComponent(
          runRef,
        )}?focusRef=${encodeURIComponent(trimmed)}`,
      )
    }
  }

  if (/^run\./i.test(trimmed)) {
    return safePublicPath(
      `/api/public/training/runs/${encodeURIComponent(trimmed)}`,
    )
  }

  if (/^pylon\.|^pylon_/i.test(trimmed)) {
    return safePublicPath('/api/public/pylon-stats')
  }

  if (/^forum\./i.test(trimmed)) {
    return safePublicPath('/forum')
  }

  if (/^artanis\./i.test(trimmed)) {
    return safePublicPath('/api/public/artanis/admin-ticks')
  }

  if (/capacity/i.test(trimmed)) {
    return safePublicPath('/api/public/pylon-capacity-funnel/history')
  }

  return null
}

export const publicActivityEventUrls = (
  event: PublicActivityTimelineEvent,
): ReadonlyArray<PublicActivityEventUrl> => {
  const seen = new Set<string>()
  const urls: PublicActivityEventUrl[] = []
  for (const ref of [
    ...event.sourceRefs,
    ...event.refs,
    ...event.blockerRefs,
    ...event.caveatRefs,
  ]) {
    const href = publicHrefForRef(ref, event)
    if (href === null || seen.has(href)) continue
    seen.add(href)
    urls.push({ href, label: href, ref })
  }

  return urls
}

const productPromiseBlockerRefsFor = (
  event: PublicActivityTimelineEvent,
  lag: PublicActivityTimelineSourceLag | null,
): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const refs = [
    ...event.blockerRefs,
    ...event.caveatRefs,
    ...event.refs,
    ...(lag?.blockerRefs ?? []),
    ...(lag?.caveatRefs ?? []),
  ].filter(ref => /product[-_.]?promise|promise/i.test(ref))

  return refs.filter(ref => {
    if (seen.has(ref)) return false
    seen.add(ref)
    return true
  })
}

const eventContent = (
  event: PublicActivityTimelineEvent,
  options: Readonly<{ refLimit?: number }> = {},
): ReadonlyArray<HTMLElement> => {
  const head = create('div', 'event-head')
  head.append(
    create('span', 'kind', displayLabel(event.kind)),
    create('time', undefined, formatTime(event.ts)),
  )

  const meta = create('div', 'event-meta')
  for (const metaItem of eventMetaItems(event)) {
    meta.append(create('span', 'tag', metaItem))
  }

  return [
    head,
    create('p', 'event-text', event.text),
    meta,
    refsView(
      event.refs.length > 0 ? event.refs : event.sourceRefs,
      options.refLimit ?? 3,
    ),
  ]
}

const selectableEventItem = (
  input: Readonly<{
    envelope: PublicActivityTimelineEnvelope
    event: PublicActivityTimelineEvent
    refLimit?: number
    root: HTMLElement
    state: PublicActivityTimelineRenderState
    timeline?: boolean
  }>,
): HTMLElement => {
  const item = create(
    'li',
    input.timeline === true ? 'timeline-row' : 'event-item',
  )
  item.dataset.eventKind = input.event.kind
  item.dataset.eventSource = input.event.sourceKind

  const button = create(
    'button',
    input.timeline === true ? 'event-button timeline-button' : 'event-button',
  ) as HTMLButtonElement
  button.type = 'button'
  button.dataset.activityEvent = input.event.cursor
  button.setAttribute(
    'aria-pressed',
    String(
      input.event.cursor ===
        selectedEventFor(input.envelope, input.state.selectedCursor)?.cursor,
    ),
  )
  button.addEventListener('click', () => {
    input.state.selectedCursor = input.event.cursor
    renderEnvelope(input.root, input.state, input.envelope)
  })
  button.append(
    ...eventContent(
      input.event,
      input.refLimit === undefined ? {} : { refLimit: input.refLimit },
    ),
  )

  item.append(button)
  return item
}

const latestFirst = (
  events: ReadonlyArray<PublicActivityTimelineEvent>,
): ReadonlyArray<PublicActivityTimelineEvent> => [...events].reverse()

const eventsForPane = (
  envelope: PublicActivityTimelineEnvelope,
  pane: Exclude<ActivityPaneId, 'proof' | 'timeline'>,
): ReadonlyArray<PublicActivityTimelineEvent> => {
  const events = latestFirst(envelope.events)

  if (pane === 'fleet') {
    return events.filter(
      event =>
        fleetKinds.has(event.kind) ||
        event.sourceKind === 'pylon_api' ||
        event.sourceKind === 'pylon_presence' ||
        event.sourceKind === 'capacity_funnel',
    )
  }

  if (pane === 'money') {
    return events.filter(
      event =>
        moneyKinds.has(event.kind) || event.sourceKind === 'settlement_receipt',
    )
  }

  return events.filter(
    event => forumKinds.has(event.kind) || event.sourceKind === 'forum',
  )
}

const renderPane = (
  input: Readonly<{
    countLabel: string
    emptyText: string
    envelope: PublicActivityTimelineEnvelope
    events: ReadonlyArray<PublicActivityTimelineEvent>
    pane: Exclude<ActivityPaneId, 'proof' | 'timeline'>
    root: HTMLElement
    state: PublicActivityTimelineRenderState
    title: string
  }>,
): HTMLElement => {
  const section = create('section', 'pane')
  section.dataset.activityPane = input.pane

  const head = create('div', 'pane-head')
  head.append(
    create('h2', undefined, input.title),
    create('span', 'count', input.countLabel),
  )
  section.append(head)

  if (input.events.length === 0) {
    section.append(create('p', 'empty', input.emptyText))
    return section
  }

  const list = create('ol', 'event-list')
  for (const event of input.events.slice(0, 5)) {
    list.append(
      selectableEventItem({
        envelope: input.envelope,
        event,
        refLimit: 3,
        root: input.root,
        state: input.state,
      }),
    )
  }
  section.append(list)
  return section
}

const selectedEventFor = (
  envelope: PublicActivityTimelineEnvelope,
  selectedCursor: string | null,
): PublicActivityTimelineEvent | null =>
  envelope.events.find(event => event.cursor === selectedCursor) ??
  latestFirst(envelope.events)[0] ??
  null

const sourceLagFor = (
  envelope: PublicActivityTimelineEnvelope,
  event: PublicActivityTimelineEvent,
): PublicActivityTimelineSourceLag | null =>
  envelope.sourceLag.find(lag => lag.sourceKind === event.sourceKind) ?? null

const eventsForFilter = (
  envelope: PublicActivityTimelineEnvelope,
  filter: ActivityFilter,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  latestFirst(envelope.events).filter(event =>
    filterMatchesEvent(filter, event),
  )

const publicEventProofPayload = (
  event: PublicActivityTimelineEvent,
): Record<string, unknown> => ({
  actorRef: event.actorRef,
  amountSats: event.amountSats,
  blockerRefs: event.blockerRefs,
  caveatRefs: event.caveatRefs,
  cursor: event.cursor,
  eventRef: event.eventRef,
  kind: event.kind,
  realBitcoinMoved: event.realBitcoinMoved,
  refs: event.refs,
  runRef: event.runRef,
  sourceKind: event.sourceKind,
  sourceRefs: event.sourceRefs,
  state: event.state,
  targetRef: event.targetRef,
  text: event.text,
  ts: event.ts,
  windowRef: event.windowRef,
})

const appendField = (
  list: HTMLElement,
  label: string,
  value: string,
  dataAttribute?: string,
): void => {
  list.append(create('dt', undefined, label))
  const dd = create('dd', undefined, value)
  if (dataAttribute !== undefined) dd.setAttribute(dataAttribute, '')
  list.append(dd)
}

const proofRefsSection = (
  title: string,
  refs: ReadonlyArray<string>,
): HTMLElement => {
  const section = create('div', 'proof-section')
  section.append(create('h3', undefined, title), refsView(refs))
  return section
}

const proofUrlsSection = (
  urls: ReadonlyArray<PublicActivityEventUrl>,
): HTMLElement => {
  const section = create('div', 'proof-section')
  section.append(create('h3', undefined, 'Public URLs'))
  if (urls.length === 0) {
    section.append(create('p', 'muted', 'none'))
    return section
  }

  const list = create('div', 'url-list')
  for (const url of urls) {
    const link = create('a', 'proof-link', url.label) as HTMLAnchorElement
    link.href = url.href
    link.dataset.proofUrl = url.ref
    list.append(link)
  }
  section.append(list)
  return section
}

const renderProofDrawer = (
  envelope: PublicActivityTimelineEnvelope,
  state: PublicActivityTimelineRenderState,
): HTMLElement => {
  const selected = selectedEventFor(envelope, state.selectedCursor)
  const drawer = create('aside', 'proof-drawer')
  drawer.dataset.activityPane = 'proof'
  drawer.setAttribute('data-proof-drawer', '')

  const head = create('div', 'proof-head')
  head.append(create('h2', undefined, 'Proof Drawer'))
  drawer.append(head)

  if (selected === null) {
    drawer.append(create('p', 'empty', 'No public activity events in range.'))
    return drawer
  }

  const lag = sourceLagFor(envelope, selected)
  const productPromiseBlockers = productPromiseBlockerRefsFor(selected, lag)
  const body = create('div', 'proof-body')
  const fields = create('dl', 'proof-fields')
  appendField(fields, 'Event', selected.eventRef)
  appendField(fields, 'Cursor', selected.cursor)
  appendField(fields, 'Source API', state.endpoint, 'data-proof-source-api')
  appendField(fields, 'Category', filterLabel(filterForEvent(selected)))
  appendField(fields, 'Kind', displayLabel(selected.kind))
  appendField(fields, 'Source', displayLabel(selected.sourceKind))
  appendField(fields, 'Observed', formatTime(selected.ts))
  appendField(fields, 'Generated', formatTime(envelope.generatedAt))
  appendField(fields, 'Staleness', envelope.staleness.composition)
  appendField(fields, 'State', selected.state ?? 'none')
  appendField(fields, 'Actor', selected.actorRef ?? 'none')
  appendField(fields, 'Target', selected.targetRef ?? 'none')
  appendField(fields, 'Run', selected.runRef ?? 'none')
  appendField(fields, 'Window', selected.windowRef ?? 'none')
  appendField(fields, 'Amount', eventAmountLabel(selected) ?? 'none')
  appendField(
    fields,
    'Real Bitcoin',
    selected.realBitcoinMoved === true
      ? 'true'
      : selected.realBitcoinMoved === false
        ? 'false'
        : 'not stated',
  )

  body.append(fields)
  body.append(
    proofRefsSection('Refs', selected.refs),
    proofRefsSection('Source refs', selected.sourceRefs),
    proofRefsSection('Blocker refs', selected.blockerRefs),
    proofRefsSection('Caveat refs', selected.caveatRefs),
    proofRefsSection('Product-promise blockers', productPromiseBlockers),
    proofUrlsSection(publicActivityEventUrls(selected)),
  )

  const lagSection = create('div', 'proof-section')
  lagSection.append(create('h3', undefined, 'Source lag'))
  if (lag === null) {
    lagSection.append(create('p', 'muted', 'none'))
  } else {
    const lagFields = create('dl', 'proof-fields')
    appendField(lagFields, 'Status', lag.status)
    appendField(lagFields, 'Latest source', formatTime(lag.latestSourceEventAt))
    appendField(lagFields, 'Observed', formatTime(lag.observedAt))
    appendField(
      lagFields,
      'Lag seconds',
      lag.lagSeconds === null
        ? 'none'
        : formatCount(Math.round(lag.lagSeconds)),
    )
    appendField(
      lagFields,
      'Max staleness',
      `${formatCount(Math.round(lag.maxStalenessSeconds))}s`,
    )
    lagSection.append(lagFields)
    lagSection.append(
      proofRefsSection('Lag source refs', lag.sourceRefs),
      proofRefsSection('Lag blocker refs', lag.blockerRefs),
      proofRefsSection('Lag caveat refs', lag.caveatRefs),
    )
  }
  body.append(lagSection)

  const jsonSection = create('div', 'proof-section')
  jsonSection.append(create('h3', undefined, 'Event JSON'))
  const pre = create('pre', 'proof-json')
  pre.setAttribute('data-proof-event-json', '')
  pre.textContent = JSON.stringify(publicEventProofPayload(selected), null, 2)
  jsonSection.append(pre)
  body.append(jsonSection)
  drawer.append(body)
  return drawer
}

const renderTimelinePane = (
  envelope: PublicActivityTimelineEnvelope,
  state: PublicActivityTimelineRenderState,
  root: HTMLElement,
): HTMLElement => {
  const section = create('section', 'pane')
  section.dataset.activityPane = 'timeline'

  const events = eventsForFilter(envelope, state.activeFilter)
  const head = create('div', 'pane-head')
  head.append(
    create('h2', undefined, 'Timeline'),
    create(
      'span',
      'count',
      `${formatCount(events.length)} of ${formatCount(envelope.events.length)} events`,
    ),
  )
  section.append(head)

  const filterBar = create('div', 'filter-bar')
  for (const filter of activityFilters) {
    const button = create(
      'button',
      'filter-button',
      filter.label,
    ) as HTMLButtonElement
    button.type = 'button'
    button.dataset.activityFilter = filter.id
    button.setAttribute(
      'aria-pressed',
      String(state.activeFilter === filter.id),
    )
    button.addEventListener('click', () => {
      state.activeFilter = filter.id
      const nextEvents = eventsForFilter(envelope, state.activeFilter)
      if (
        state.selectedCursor !== null &&
        !nextEvents.some(event => event.cursor === state.selectedCursor)
      ) {
        state.selectedCursor = nextEvents[0]?.cursor ?? null
      }
      renderEnvelope(root, state, envelope)
    })
    filterBar.append(button)
  }
  section.append(filterBar)

  if (events.length === 0) {
    section.append(
      create(
        'p',
        'empty',
        `No public ${filterLabel(state.activeFilter).toLowerCase()} events in range.`,
      ),
    )
    return section
  }

  const list = create('ol', 'timeline-list')
  for (const event of events) {
    list.append(
      selectableEventItem({
        envelope,
        event,
        refLimit: 4,
        root,
        state,
        timeline: true,
      }),
    )
  }

  section.append(list)
  return section
}

const renderSourceStrip = (
  envelope: PublicActivityTimelineEnvelope,
): HTMLElement => {
  const staleCount = envelope.sourceLag.filter(
    lag => lag.status !== 'current',
  ).length
  const section = create('section', 'source-strip')
  section.dataset.activitySourceLag = ''

  const head = create('div', 'source-strip-head')
  head.append(
    create('h2', undefined, 'Source lag'),
    create(
      'span',
      'count',
      staleCount === 0
        ? 'all current'
        : `${formatCount(staleCount)} stale or blocked`,
    ),
  )

  const list = create('div', 'source-list')
  if (envelope.sourceLag.length === 0) {
    list.append(create('span', 'muted', 'No source lag rows.'))
  } else {
    for (const lag of envelope.sourceLag) {
      list.append(sourceStatusBadge(lag))
    }
  }

  section.append(head, list)
  return section
}

const renderEnvelope = (
  root: HTMLElement,
  state: PublicActivityTimelineRenderState,
  envelope: PublicActivityTimelineEnvelope,
): void => {
  root.replaceChildren()
  root.dataset.state = envelope.events.length === 0 ? 'empty' : 'ok'

  const activity = create('div', 'activity')
  const hero = create('header', 'hero')
  const titleBlock = create('div')
  titleBlock.append(
    create('p', 'eyebrow', 'OpenAgents activity'),
    create('h1', undefined, 'Live public activity'),
    create(
      'p',
      'boundary',
      'Read-only public projection. No settlement, payout, deployment, accepted-work, provider, wallet, or public-claim authority is available here.',
    ),
  )

  const metrics = create('dl', 'metrics')
  const staleCount = envelope.sourceLag.filter(
    lag => lag.status !== 'current',
  ).length
  appendMetric(metrics, 'Generated', formatTime(envelope.generatedAt))
  appendMetric(metrics, 'Events', formatCount(envelope.events.length))
  appendMetric(metrics, 'Stale sources', formatCount(staleCount))
  appendMetric(metrics, 'Next cursor', envelope.nextCursor ?? 'none')
  hero.append(titleBlock, metrics)

  const paneGrid = create('div', 'pane-grid')
  const fleetEvents = eventsForPane(envelope, 'fleet')
  const moneyEvents = eventsForPane(envelope, 'money')
  const forumEvents = eventsForPane(envelope, 'forum')
  paneGrid.append(
    renderPane({
      countLabel: `${formatCount(fleetEvents.length)} events`,
      emptyText: 'No public fleet events in range.',
      envelope,
      events: fleetEvents,
      pane: 'fleet',
      root,
      state,
      title: 'Fleet',
    }),
    renderPane({
      countLabel: `${formatCount(moneyEvents.length)} events`,
      emptyText: 'No public money-loop events in range.',
      envelope,
      events: moneyEvents,
      pane: 'money',
      root,
      state,
      title: 'Money Loop',
    }),
    renderPane({
      countLabel: `${formatCount(forumEvents.length)} events`,
      emptyText: 'No public Forum events in range.',
      envelope,
      events: forumEvents,
      pane: 'forum',
      root,
      state,
      title: 'Forum',
    }),
    renderTimelinePane(envelope, state, root),
  )

  const dashboard = create('div', 'dashboard-grid')
  dashboard.append(paneGrid, renderProofDrawer(envelope, state))

  activity.append(hero, renderSourceStrip(envelope), dashboard)
  root.append(activity)
}

const renderLoading = (
  root: HTMLElement,
  options: PublicActivityTimelineMountOptions,
): void => {
  setDataState(root, 'loading', options)
  root.replaceChildren(create('div', 'loading', 'Loading public activity...'))
}

const renderError = (
  root: HTMLElement,
  options: PublicActivityTimelineMountOptions,
): void => {
  setDataState(root, 'error', options)
  const wrapper = create('div', 'activity')
  wrapper.append(
    create(
      'section',
      'error',
      'Timeline unavailable. The public activity payload could not be read safely.',
    ),
  )
  root.replaceChildren(wrapper)
}

const readTimelineEnvelope = async (
  fetchFn: typeof fetch,
  endpoint: string,
): Promise<PublicActivityTimelineEnvelope> => {
  const response = await fetchFn(endpoint, {
    cache: 'no-store',
    headers: { accept: 'application/json' },
  })

  if (!response.ok) {
    throw new Error(
      `Public activity timeline returned HTTP ${response.status}.`,
    )
  }

  const payload = await response.json()
  if (publicActivityTimelineHasUnsafeMaterial(payload)) {
    throw new Error(
      'Public activity timeline payload contains unsafe material.',
    )
  }

  return assertPublicActivityTimelineEnvelopeSafe(payload)
}

export const mountPublicActivityTimeline = (
  root: HTMLElement,
  options: PublicActivityTimelineMountOptions = {},
): PublicActivityTimelineHandle => {
  const endpoint = options.endpoint ?? PUBLIC_ACTIVITY_TIMELINE_ENDPOINT
  const fetchFn = options.fetchFn ?? fetch
  const state: PublicActivityTimelineRenderState = {
    activeFilter: 'all',
    endpoint,
    selectedCursor: null,
  }
  let disposed = false

  const refresh = async (): Promise<void> => {
    if (disposed) return
    if (root.dataset.state !== 'ok' && root.dataset.state !== 'empty') {
      renderLoading(root, options)
    }

    try {
      const envelope = await readTimelineEnvelope(fetchFn, endpoint)
      if (disposed) return
      renderEnvelope(root, state, envelope)
      setDataState(root, envelope.events.length === 0 ? 'empty' : 'ok', options)
    } catch {
      if (disposed) return
      renderError(root, options)
    }
  }

  void refresh()

  const refreshIntervalMs =
    options.refreshIntervalMs ?? PUBLIC_ACTIVITY_TIMELINE_REFRESH_MS
  const timer =
    refreshIntervalMs > 0
      ? setInterval(() => {
          void refresh()
        }, refreshIntervalMs)
      : null

  return {
    dispose: () => {
      disposed = true
      if (timer !== null) clearInterval(timer)
    },
    refresh,
  }
}

const makePublicActivityTimelineElement = (): CustomElementConstructor =>
  class PublicActivityTimelineElement extends HTMLElement {
    #handle: PublicActivityTimelineHandle | null = null

    connectedCallback(): void {
      if (this.#handle !== null) return

      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = hostCss
      const app = document.createElement('div')
      shadow.append(style, app)

      this.#handle = mountPublicActivityTimeline(app, {
        onState: state => {
          this.dataset.state = state
        },
      })
    }

    disconnectedCallback(): void {
      this.#handle?.dispose()
      this.#handle = null
    }
  }

export const registerPublicActivityTimelineElement = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(PUBLIC_ACTIVITY_TIMELINE_TAG) !== undefined) return
  customElements.define(
    PUBLIC_ACTIVITY_TIMELINE_TAG,
    makePublicActivityTimelineElement(),
  )
}

export const publicActivityTimelineView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  registerPublicActivityTimelineElement()
  const customElement = element.withMessage<Message>()
  return customElement(attributes, [])
}

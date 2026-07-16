import {
  ChevronLeft,
  ChevronRight,
  MessageCircle,
  House,
  PanelLeft,
  Search,
  Settings,
  SquarePen,
  X,
  type LucideIcon,
} from "lucide-react"
import { forwardRef, type KeyboardEvent, type ReactElement, type ReactNode } from "react"

export type DesktopRailIcon = "chat" | "home" | "new-session" | "settings"

export type DesktopRailDestination = Readonly<{
  id: string
  label: string
  icon: DesktopRailIcon
  selected?: boolean
  current?: "page" | "step"
  indicator?: "current" | null
  accessibilityLabel?: string
}>

export type DesktopRailSession = Readonly<{
  id: string
  title: string
  meta: string
  selected?: boolean
}>

const railIcons: Readonly<Record<DesktopRailIcon, LucideIcon>> = {
  chat: MessageCircle,
  home: House,
  "new-session": SquarePen,
  settings: Settings,
}

const railIconNames: Readonly<Record<DesktopRailIcon, string>> = {
  chat: "Chats",
  home: "Home",
  "new-session": "ChatCompose",
  settings: "Settings",
}

export type DesktopSessionRailProps = Readonly<{
  stageLabel?: string
  open: boolean
  canGoBack?: boolean
  canGoForward?: boolean
  backLabel?: string
  forwardLabel?: string
  searchOpen: boolean
  searchQuery: string
  searchPending?: boolean
  hydrated?: boolean
  destinations: ReadonlyArray<DesktopRailDestination>
  settingsDestination?: DesktopRailDestination
  sessions: ReadonlyArray<DesktopRailSession>
  canLoadMore?: boolean
  footer?: ReactNode
  onCollapse: () => void
  onBack: () => void
  onForward: () => void
  onSearchOpenChange: (open: boolean) => void
  onSearchQueryChange: (query: string) => void
  onDestinationSelect: (destination: DesktopRailDestination) => void
  onSessionSelect: (session: DesktopRailSession) => void
  onLoadMore?: () => void
}>

const focusAdjacentSession = (event: KeyboardEvent<HTMLElement>): void => {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return
  const rows = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("[data-session-row]")]
  const index = rows.indexOf(event.target as HTMLButtonElement)
  if (index < 0 || rows.length === 0) return
  event.preventDefault()
  rows[(index + (event.key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length]?.focus()
}

export const DesktopSessionRail = forwardRef<HTMLElement, DesktopSessionRailProps>(({
  stageLabel,
  open,
  canGoBack = false,
  canGoForward = false,
  backLabel = "Back",
  forwardLabel = "Forward",
  searchOpen,
  searchQuery,
  searchPending = false,
  hydrated = true,
  destinations,
  settingsDestination,
  sessions,
  canLoadMore = false,
  footer,
  onCollapse,
  onBack,
  onForward,
  onSearchOpenChange,
  onSearchQueryChange,
  onDestinationSelect,
  onSessionSelect,
  onLoadMore,
}, ref): ReactElement => {
  const renderDestination = (destination: DesktopRailDestination): ReactElement => {
    const Icon = railIcons[destination.icon]
    return <button
      aria-current={destination.current}
      aria-label={destination.accessibilityLabel}
      className="oa-react-primary-destination justify-start text-left"
      data-selected={destination.selected ? "true" : "false"}
      data-sidebar-destination-id={destination.id}
      key={destination.id}
      onClick={() => onDestinationSelect(destination)}
      type="button"
    >
      <Icon aria-hidden="true" data-icon-name={railIconNames[destination.icon]} />
      <span>{destination.label}</span>
      {destination.indicator === null || destination.indicator === undefined
        ? null
        : <i aria-hidden="true" data-destination-indicator={destination.indicator} />}
    </button>
  }

  return <aside aria-label="Sessions" className="oa-react-session-rail" data-open={open ? "true" : "false"} onKeyDown={focusAdjacentSession} ref={ref}>
    <div aria-label="Sidebar controls" className="oa-react-rail-windowbar">
      <button aria-label="Collapse sidebar" className="oa-react-icon-button oa-react-rail-collapse" onClick={onCollapse} title="Collapse sidebar" type="button">
        <PanelLeft aria-hidden="true" data-icon-name="Menu" />
      </button>
      <div aria-label="Session navigation" className="oa-react-history-controls">
        <button aria-label={backLabel} className="oa-react-icon-button" disabled={!canGoBack} onClick={onBack} title={backLabel} type="button">
          <ChevronLeft aria-hidden="true" data-icon-name="ChevronLeft" />
        </button>
        <button aria-label={forwardLabel} className="oa-react-icon-button" disabled={!canGoForward} onClick={onForward} title={forwardLabel} type="button">
          <ChevronRight aria-hidden="true" data-icon-name="ChevronRight" />
        </button>
      </div>
    </div>
    <div className="oa-react-rail-titlebar">
      <div aria-label={stageLabel === undefined ? "OpenAgents" : `OpenAgents ${stageLabel}`} className="oa-react-rail-brand">
        <strong>OpenAgents</strong>
        {stageLabel === undefined ? null : <span className="oa-react-rail-stage" data-app-stage={stageLabel.toLocaleLowerCase()}>{stageLabel}</span>}
      </div>
      <button
        aria-expanded={searchOpen}
        aria-label={searchOpen ? "Close session search" : "Search sessions"}
        className="oa-react-icon-button oa-react-search-trigger"
        onClick={() => onSearchOpenChange(!searchOpen)}
        title={searchOpen ? "Close search" : "Search sessions"}
        type="button"
      >
        {searchOpen ? <X aria-hidden="true" /> : <Search aria-hidden="true" data-icon-name="Search" />}
      </button>
    </div>
    {searchOpen ? <label className="oa-react-search">
      <span className="oa-react-sr-only">Search sessions</span>
      <input
        autoFocus
        onInput={event => onSearchQueryChange(event.currentTarget.value)}
        onKeyDown={event => {
          if (event.key !== "Escape") return
          event.preventDefault()
          onSearchOpenChange(false)
        }}
        placeholder="Search sessions"
        type="search"
        value={searchQuery}
      />
    </label> : null}
    <nav aria-label="Primary" className="oa-react-primary-nav">{destinations.map(renderDestination)}</nav>
    <p className="oa-react-section-label">Recent</p>
    <div className="oa-react-session-scroll">
      <nav aria-label="Recent sessions" className="oa-react-session-list">
        {!hydrated && sessions.length === 0
          ? <p role="status">Scanning sessions…</p>
          : sessions.length === 0
            ? <p>{searchPending ? "Searching…" : "No sessions found"}</p>
            : sessions.map(session => <button
                aria-current={session.selected ? "page" : undefined}
                className="oa-react-session-row justify-start text-left"
                data-en-key={`sidebar-thread-${session.id}`}
                data-en-tag="Button"
                data-selected={session.selected ? "true" : "false"}
                data-session-row
                key={session.id}
                onClick={() => onSessionSelect(session)}
                type="button"
              >
                <span className="oa-react-session-title">{session.title}</span>
                <small className="oa-react-session-meta" data-en-role="meta">{session.meta}</small>
              </button>)}
        {canLoadMore ? <button className="oa-react-load-more" onClick={onLoadMore} type="button">Load more sessions</button> : null}
      </nav>
    </div>
    {settingsDestination === undefined ? null : <nav aria-label="Settings" className="oa-react-sidebar-footer">
      {renderDestination(settingsDestination)}
    </nav>}
    {footer}
  </aside>
})
DesktopSessionRail.displayName = "DesktopSessionRail"

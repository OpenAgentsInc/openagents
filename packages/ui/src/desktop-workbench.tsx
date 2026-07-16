import type { Theme } from "@effect-native/core"
import {
  ChevronLeft,
  ChevronRight,
  House,
  MessageCircle,
  PanelLeft,
  Search,
  Settings,
  SquarePen,
  X,
  type LucideIcon,
} from "lucide-react"
import {
  forwardRef,
  useState,
  type ComponentPropsWithoutRef,
  type CSSProperties,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
} from "react"

import "./desktop-workbench.css"

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

const cx = (...values: ReadonlyArray<string | false | null | undefined>): string =>
  values.filter(Boolean).join(" ")

const px = (value: string | number): string => typeof value === "number" ? `${value}px` : value

export type DesktopThemeCssVariables = CSSProperties & Readonly<Record<`--en-${string}`, string | number>>

/** The same token bridge used by Electron, scoped for embedded web workbenches. */
export const desktopThemeCssVariables = (theme: Theme): DesktopThemeCssVariables => {
  const variables: Record<string, string | number> = {
    backgroundColor: theme.color.background,
    color: theme.color.textPrimary,
  }
  for (const [key, value] of Object.entries(theme.color)) variables[`--en-color-${key}`] = value
  for (const [key, value] of Object.entries(theme.spacing)) variables[`--en-spacing-${key}`] = px(value)
  for (const [key, value] of Object.entries(theme.radius)) variables[`--en-radius-${key}`] = px(value)
  for (const [key, value] of Object.entries(theme.dimension)) variables[`--en-dimension-${key}`] = px(value)
  for (const [key, value] of Object.entries(theme.typeScale)) {
    variables[`--en-type-${key}-fontSize`] = px(value.fontSize)
    variables[`--en-type-${key}-lineHeight`] = px(value.lineHeight)
    variables[`--en-type-${key}-fontWeight`] = value.fontWeight
  }
  for (const [key, value] of Object.entries(theme.control)) {
    variables[`--en-control-${key}-height`] = px(value.height)
    variables[`--en-control-${key}-gutter`] = px(value.gutter)
    variables[`--en-control-${key}-radius`] = px(value.radius)
    variables[`--en-control-${key}-font-size`] = px(value.fontSize)
    variables[`--en-control-${key}-icon`] = px(value.icon)
  }
  variables["--en-motion-fast"] = `${theme.motion.durationFastMs}ms`
  variables["--en-motion-enter"] = `${theme.motion.durationEnterMs}ms`
  variables["--en-motion-exit"] = `${theme.motion.durationExitMs}ms`
  variables["--en-motion-loop"] = `${theme.motion.durationLoopMs}ms`
  variables["--en-ease-basic"] = theme.motion.easeBasic
  variables["--en-ease-enter"] = theme.motion.easeEnter
  variables["--en-ease-exit"] = theme.motion.easeExit
  variables["--en-ease-exit-snappy"] = theme.motion.easeExitSnappy
  variables["--en-ease-move"] = theme.motion.easeMove
  variables["--en-elevation-overlay-shadow"] = theme.elevation.overlayShadow
  variables["--en-elevation-raised-shadow"] = theme.elevation.overlayShadow
  variables["--en-elevation-hairline"] = `0 0 0 ${px(theme.elevation.hairlineWidth)} ${theme.color.borderSubtle}`
  return variables as DesktopThemeCssVariables
}

export const DesktopWorkbench = ({
  children,
  className,
  railCollapsed = false,
  ...props
}: ComponentPropsWithoutRef<"div"> & Readonly<{ railCollapsed?: boolean }>): ReactElement =>
  <div
    {...props}
    className={cx("oa-react-workbench", className)}
    data-en-react-surface="true"
    data-rail-collapsed={railCollapsed ? "true" : "false"}
  >
    {children}
  </div>

export const DesktopSidebarExpand = forwardRef<HTMLButtonElement, ComponentPropsWithoutRef<"button">>(({ className, ...props }, ref): ReactElement =>
  <button {...props} className={cx("oa-react-sidebar-expand", className)} ref={ref} type="button">
    <PanelLeft aria-hidden="true" data-icon-name="Menu" />
  </button>)
DesktopSidebarExpand.displayName = "DesktopSidebarExpand"

export const DesktopRailScrim = (props: ComponentPropsWithoutRef<"button">): ReactElement =>
  <button {...props} className={cx("oa-react-rail-scrim", props.className)} type="button" />

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
                data-selected={session.selected ? "true" : "false"}
                data-session-row
                key={session.id}
                onClick={() => onSessionSelect(session)}
                type="button"
              >
                <span className="oa-react-session-title">{session.title}</span>
                <small className="oa-react-session-meta">{session.meta}</small>
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

export const DesktopConversationHeader = ({
  title,
  lifecycle,
  secondary,
}: Readonly<{ title: string; lifecycle: string; secondary?: string }>): ReactElement =>
  <header className="oa-react-conversation-header">
    <div className="oa-react-conversation-heading">
      <h1>{title}</h1>
      <div aria-label="Session status" className="oa-react-conversation-meta">
        <span data-lifecycle={lifecycle.toLocaleLowerCase().replaceAll(" ", "-")}>{lifecycle}</span>
        {secondary === undefined ? null : <span>{secondary}</span>}
      </div>
    </div>
  </header>

export const DesktopConversation = ({
  header,
  notices,
  timeline,
  composer,
}: Readonly<{ header: ReactNode; notices?: ReactNode; timeline: ReactNode; composer: ReactNode }>): ReactElement =>
  <main className="oa-react-conversation" data-react-workspace="chat">
    {header}
    <div className="oa-react-conversation-body">{notices}{timeline}</div>
    {composer}
  </main>

export const DesktopTimeline = ({ children, working = false }: Readonly<{ children: ReactNode; working?: boolean }>): ReactElement =>
  <section aria-label="Conversation timeline" className="oa-react-timeline-region">
    <div className="oa-react-timeline-scroll">
      <div aria-busy={working} className="oa-react-timeline-content" role="list">
        {children}
        {working ? <div className="oa-react-working" role="status" aria-label="Codex is working"><span>Working</span><i /><i /><i /></div> : null}
      </div>
    </div>
  </section>

export const DesktopTimelineMessage = ({
  children,
  itemKey,
  kind,
  label,
  sequence,
  tone,
}: Readonly<{
  children: ReactNode
  itemKey: string
  kind?: string
  label: string
  sequence: number
  tone: "assistant" | "user"
}>): ReactElement =>
  <article
    aria-label={`${label}. Item ${sequence + 1}`}
    className="oa-react-timeline-item"
    data-kind={kind ?? (tone === "user" ? "user_message" : "assistant_message")}
    data-timeline-key={itemKey}
    data-tone={tone}
    role="listitem"
  >{children}</article>

export const DesktopTimelineNotice = ({
  body,
  danger = false,
  itemKey,
  kind,
  label = "Update",
}: Readonly<{ body: ReactNode; danger?: boolean; itemKey: string; kind?: string; label?: string }>): ReactElement =>
  <article className="oa-react-notice" data-danger={danger ? "true" : "false"} data-kind={kind ?? (danger ? "error" : "notice")} data-timeline-key={itemKey} role="listitem">
    <strong>{label}</strong><span>{body}</span>
  </article>

export const DesktopWorkEntry = ({
  body,
  itemKey,
  kind = "tool_call",
  label,
  preview,
  status = "completed",
  statusLabel,
}: Readonly<{ body: ReactNode; itemKey: string; kind?: string; label: string; preview: string; status?: string; statusLabel?: string }>): ReactElement =>
  <details className="oa-react-work-entry" data-kind={kind} data-timeline-key={itemKey} role="listitem">
    <summary>
      <span className="oa-react-work-label">{label}</span>
      <span className="oa-react-work-preview">{preview}</span>
      <span className="oa-react-work-status" data-status={status}>{statusLabel ?? (status === "running" ? "Running" : "Done")}</span>
    </summary>
    <div className="oa-react-work-detail">{body}</div>
  </details>

export const DesktopWorkGroup = ({ children, count, running = false }: Readonly<{ children: ReactNode; count: number; running?: boolean }>): ReactElement => {
  const [expanded, setExpanded] = useState(false)
  return <div className="oa-react-work-group" role="listitem">
    <button aria-expanded={expanded} className="oa-react-work-group-summary" onClick={() => setExpanded(value => !value)} type="button">
      <ChevronRight aria-hidden="true" data-expanded={expanded ? "true" : "false"} />
      <strong>{running ? `+${count} previous` : "Worked"}</strong>
      <span>{count} {count === 1 ? "activity" : "activities"}</span>
    </button>
    {expanded ? <div role="list">{children}</div> : null}
  </div>
}

export const DesktopComposerFrame = forwardRef<HTMLElement, ComponentPropsWithoutRef<"section">>(({
  children,
  className,
  ...props
}, ref): ReactElement => <section
  {...props}
  className={cx("oa-react-composer", className)}
  data-chat-composer="true"
  data-en-key="shell-composer"
  ref={ref}
>{children}</section>)
DesktopComposerFrame.displayName = "DesktopComposerFrame"

export const DesktopComposerInput = ({ children }: Readonly<{ children: ReactNode }>): ReactElement =>
  <div className="oa-react-composer-input" data-en-key="shell-input">{children}</div>

export const DesktopComposerBar = ({ children }: Readonly<{ children: ReactNode }>): ReactElement =>
  <div className="oa-react-composer-bar" data-chat-composer-footer="true">{children}</div>

import type { LucideIcon } from "lucide-react"
import { useState, type ReactElement, type ReactNode } from "react"

import { activityStatusIcon, activityStatusLabel, type DesktopActivityStatus } from "./activity-status.tsx"

/**
 * Private shared shell behind command/file-change/tool-call cards. Not part
 * of the public `@openagentsinc/ui/desktop-workbench` surface.
 */
export type DesktopProtocolCardProps = Readonly<{
  body: ReactNode
  defaultOpen?: boolean | undefined
  icon: LucideIcon
  itemKey: string
  meta?: string | undefined
  status: DesktopActivityStatus
  summary: ReactNode
  title: string
  variant: string
}>

export const DesktopProtocolCard = ({ body, defaultOpen = false, icon: Icon, itemKey, meta, status, summary, title, variant }: DesktopProtocolCardProps): ReactElement => {
  const [open, setOpen] = useState(defaultOpen)
  return <details className="oa-react-protocol-card" data-kind={variant} data-status={status} data-timeline-key={itemKey} onToggle={event => setOpen(event.currentTarget.open)} open={open} role="listitem">
    <summary>
      <span className="oa-react-event-icon"><Icon aria-hidden="true" /></span>
      <span className="oa-react-event-heading"><strong>{title}</strong><span>{summary}</span></span>
      {meta === undefined ? null : <small>{meta}</small>}
      <span className="oa-react-event-status" data-status={status}>{activityStatusIcon(status)}{activityStatusLabel(status)}</span>
    </summary>
    <div className="oa-react-protocol-detail">{body}</div>
  </details>
}

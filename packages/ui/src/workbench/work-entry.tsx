import { ChevronRight } from "lucide-react"
import { useState, type ReactElement, type ReactNode } from "react"

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

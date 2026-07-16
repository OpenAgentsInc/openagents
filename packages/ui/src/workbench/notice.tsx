import type { ReactElement, ReactNode } from "react"

export const DesktopTimelineNotice = ({
  body,
  danger = false,
  itemKey,
  kind,
  label = "Update",
}: Readonly<{ body: ReactNode; danger?: boolean | undefined; itemKey: string; kind?: string | undefined; label?: string | undefined }>): ReactElement =>
  <article className="oa-react-notice" data-danger={danger ? "true" : "false"} data-kind={kind ?? (danger ? "error" : "notice")} data-timeline-key={itemKey} role="listitem">
    <strong>{label}</strong><span>{body}</span>
  </article>

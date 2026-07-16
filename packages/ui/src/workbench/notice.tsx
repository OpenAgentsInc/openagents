import type { ReactElement, ReactNode } from "react"

/**
 * Notice severity (#8869, T12 epic #8857 wave 2). `danger` is the legacy
 * boolean every pre-existing call site still passes (history-projected
 * error/lifecycle rows); `severity` is the typed field a `notice`
 * `WorkbenchItem` carries. When both are given, `severity` wins so the
 * caller's exact classification survives — `danger` is only the fallback
 * shape for callers that predate severity.
 */
export type DesktopTimelineNoticeSeverity = "info" | "warning" | "error"

const resolveSeverity = (
  severity: DesktopTimelineNoticeSeverity | undefined,
  danger: boolean,
): DesktopTimelineNoticeSeverity => severity ?? (danger ? "error" : "info")

const severityTag = (severity: DesktopTimelineNoticeSeverity): string =>
  severity === "error" ? "ERROR" : severity === "warning" ? "WARN" : "INFO"

/**
 * Autopilot muted-red/notice grammar (#8869): a quiet, single-line system-log
 * row — never a chat bubble, never a bright alarm wall. Design-spec
 * discipline (§2.3): red stays muted brick (~45-55% perceived brightness)
 * even for errors; the only saturated mark permitted is the tiny tick-scale
 * status rectangle (§5.7), which reuses the same hairline-adjacent solid-
 * rectangle convention as the sidebar's "current" tick. `severity`
 * "warning"/"info" stay on the grey luminance ladder — the design spec bans
 * amber/yellow for advisory rows ("no yellow/orange/amber anywhere").
 */
export const DesktopTimelineNotice = ({
  body,
  danger = false,
  itemKey,
  kind,
  label = "Update",
  severity,
}: Readonly<{
  body: ReactNode
  danger?: boolean | undefined
  itemKey: string
  kind?: string | undefined
  label?: string | undefined
  severity?: DesktopTimelineNoticeSeverity | undefined
}>): ReactElement => {
  const resolved = resolveSeverity(severity, danger)
  return <article
    className="oa-react-notice"
    data-danger={resolved === "error" ? "true" : "false"}
    data-kind={kind ?? (resolved === "error" ? "error" : "notice")}
    data-severity={resolved}
    data-timeline-key={itemKey}
    role="listitem"
  >
    <i aria-hidden="true" className="oa-react-notice-tick" data-severity={resolved} />
    <span className="oa-react-notice-tag">[{severityTag(resolved)}]</span>
    <strong>{label}</strong>
    <span className="oa-react-notice-body">{body}</span>
  </article>
}

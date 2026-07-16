import type { ReactElement, ReactNode } from "react"

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

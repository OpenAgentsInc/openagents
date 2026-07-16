import type { ReactElement, ReactNode } from "react"

import {
  ContextMeter,
  type ContextMeterRateLimitWindow,
  type ContextMeterUsage,
} from "./context-meter.tsx"

/**
 * Live context/usage meter mount point (#8868, epic #8857 T11). The
 * conversation header is a persistent-per-turn widget host — unlike every
 * other workbench item, a meter is not one chat message, so it is driven
 * directly by the active thread's latest snapshot rather than through the
 * per-record timeline dispatch (`dispatchWorkbenchItem`'s "meter" branch
 * still renders a historical/inspector view of a past snapshot separately).
 */
export type DesktopConversationHeaderMeter = Readonly<{
  usage?: ContextMeterUsage
  rateLimits?: ReadonlyArray<ContextMeterRateLimitWindow>
}>

export const DesktopConversationHeader = ({
  title,
  lifecycle,
  secondary,
  meter,
}: Readonly<{ title: string; lifecycle: string; secondary?: string; meter?: DesktopConversationHeaderMeter }>): ReactElement =>
  <header className="oa-react-conversation-header">
    <div className="oa-react-conversation-heading">
      <h1>{title}</h1>
      <div aria-label="Session status" className="oa-react-conversation-meta">
        <span data-lifecycle={lifecycle.toLocaleLowerCase().replaceAll(" ", "-")}>{lifecycle}</span>
        {secondary === undefined ? null : <span>{secondary}</span>}
      </div>
    </div>
    {meter === undefined ? null : <ContextMeter
      {...(meter.usage === undefined ? {} : { usage: meter.usage })}
      {...(meter.rateLimits === undefined ? {} : { rateLimits: meter.rateLimits })}
    />}
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

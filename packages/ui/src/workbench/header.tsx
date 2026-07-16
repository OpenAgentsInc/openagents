import type { ReactElement, ReactNode } from "react"

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

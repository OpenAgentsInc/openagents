import { Bot, GitBranch, Network } from "lucide-react"
import { useState, type ReactElement } from "react"

import { activityStatusIcon, activityStatusLabel } from "./activity-status.tsx"

export type DesktopAgentStatus = "completed" | "failed" | "running" | "waiting"

export type DesktopAgentActivity = Readonly<{
  agentKey: string
  depth?: number
  detail: string
  name: string
  parent?: string
  role: string
  status: DesktopAgentStatus
  transcript?: ReadonlyArray<Readonly<{ label: string; text: string }>>
}>

const DesktopAgentRow = ({ agent }: Readonly<{ agent: DesktopAgentActivity }>): ReactElement => {
  const [open, setOpen] = useState((agent.transcript?.length ?? 0) > 0)
  return <details
    className="oa-react-agent-card"
    data-depth={agent.depth ?? 0}
    data-status={agent.status}
    onToggle={event => setOpen(event.currentTarget.open)}
    open={open}
    role="listitem"
  >
    <summary>
      <span className="oa-react-agent-avatar"><Bot aria-hidden="true" /></span>
      <span className="oa-react-agent-heading"><strong>{agent.name}</strong><small>{agent.role}</small></span>
      <span className="oa-react-agent-task">{agent.detail}</span>
      <span className="oa-react-event-status" data-status={agent.status}>{activityStatusIcon(agent.status)}{activityStatusLabel(agent.status)}</span>
    </summary>
    {agent.transcript === undefined || agent.transcript.length === 0 ? null : <div className="oa-react-agent-transcript">
      {agent.parent === undefined ? null : <p className="oa-react-agent-parent"><GitBranch aria-hidden="true" />spawned by {agent.parent}</p>}
      {agent.transcript.map((line, index) => <p key={`${agent.agentKey}:line:${index}`}><strong>{line.label}</strong><span>{line.text}</span></p>)}
    </div>}
  </details>
}

export const DesktopAgentGroup = ({ agents, itemKey, title = "Delegated agents" }: Readonly<{
  agents: ReadonlyArray<DesktopAgentActivity>
  itemKey: string
  title?: string | undefined
}>): ReactElement => {
  const completed = agents.filter(agent => agent.status === "completed").length
  const running = agents.filter(agent => agent.status === "running").length
  return <section className="oa-react-agent-group" data-kind="collabAgentToolCall" data-timeline-key={itemKey} role="listitem">
    <header>
      <span className="oa-react-event-title"><Network aria-hidden="true" /><strong>{title}</strong></span>
      <span>{completed} done{running > 0 ? ` · ${running} running` : ""}</span>
    </header>
    <div className="oa-react-agent-list" role="list">
      {agents.map(agent => <DesktopAgentRow agent={agent} key={agent.agentKey} />)}
    </div>
  </section>
}

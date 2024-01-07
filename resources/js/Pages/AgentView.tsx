import { usePage } from "@inertiajs/react"
import { Agent } from "@/types/agents"
import { NavLayout } from "@/Layouts/NavLayout"
import { AgentView } from "@/Components/agent/AgentView"

function AgentViewPage() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  return (
    <AgentView agent={agent} />
  )
}

AgentViewPage.layout = (page) => <NavLayout children={page} />

export default AgentViewPage

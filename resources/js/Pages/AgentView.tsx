import { usePage } from "@inertiajs/react"
import { Agent } from "@/types/agents"
import { NavLayout } from "@/Layouts/NavLayout"
import { AgentView } from "@/Components/agent/AgentView"

function AgentViewPage() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const conversation = props.conversation as any
  return (
    <AgentView agent={agent} conversation={conversation} />
  )
}

AgentViewPage.layout = (page) => <NavLayout children={page} />

export default AgentViewPage

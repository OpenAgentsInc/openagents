import { usePage } from "@inertiajs/react"
import { Agent } from "@/types/agents"
import { NavLayout } from "@/Layouts/NavLayout"
import { AgentView } from "@/Components/agent/AgentView"

function AgentViewPage() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const conversation = props.conversation as any
  const files = props.files as any
  const owner = props.owner as string
  return (
    <AgentView agent={agent} conversation={conversation} files={files} owner={owner} />
  )
}

AgentViewPage.layout = (page) => <NavLayout children={page} />

export default AgentViewPage

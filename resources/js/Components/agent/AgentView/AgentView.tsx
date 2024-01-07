import { ChatPane } from "@/Components/builder/ChatPane"
import { Agent } from "@/types/agents"

interface AgentViewProps {
  agent: Agent
}

export const AgentView = ({ agent }: AgentViewProps) => {
  return (
    <div className="h-full">
      <ChatPane />
    </div>
  )
}

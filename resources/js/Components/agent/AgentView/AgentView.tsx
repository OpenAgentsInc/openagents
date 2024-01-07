import { ChatPane } from "@/Components/builder/ChatPane"
import { Agent } from "@/types/agents"

interface AgentViewProps {
  agent: Agent
}

export const AgentView = ({ agent }: AgentViewProps) => {
  const initialMessages = [{ id: 0, role: "assistant", content: agent.welcome_message, tokens: [] }]
  return (
    <div className="h-full">
      <ChatPane initialMessages={initialMessages} />
    </div>
  )
}

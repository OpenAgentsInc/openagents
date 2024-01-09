import { ChatPane } from "@/Components/builder/ChatPane"
import { Agent } from "@/types/agents"
import { AgentIntro } from "../AgentIntro"

interface AgentViewProps {
  agent: Agent
  conversation: {
    id: number
  }
}

export const AgentView = ({ agent, conversation }: AgentViewProps) => {
  const initialMessages = [{ id: 0, role: "assistant", content: agent.welcome_message, tokens: [] }]
  return (
    <div className="h-full">
      <ChatPane agent={agent} conversationId={conversation.id} initialMessages={initialMessages} />
    </div>
  )
}

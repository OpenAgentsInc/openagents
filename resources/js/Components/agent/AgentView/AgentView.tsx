import { ChatPane } from "@/Components/builder/ChatPane"
import { Agent } from "@/types/agents"
import { AgentSidebar } from "../AgentSidebar"

interface Message {
  id: number
  body: string
  sender: string
  created_at: string
}

interface AgentViewProps {
  agent: Agent
  conversation: {
    id: number
    messages: Message[]
  }
  files: any[]
  owner: string // username of agent's owner
}

export const AgentView = ({ agent, conversation, files, owner }: AgentViewProps) => {
  const initialMessages = [
    { id: 0, role: "assistant", content: agent.welcome_message, tokens: [] },
    ...conversation.messages.map((m) => ({
      id: m.id,
      role: m.sender,
      content: m.body,
      tokens: [],
    })),
  ]
  return (
    <div className="h-full">
      <AgentSidebar agent={agent} files={files}>
        <ChatPane agent={agent} conversationId={conversation.id} initialMessages={initialMessages} owner={owner} />
      </AgentSidebar>
    </div>
  )
}

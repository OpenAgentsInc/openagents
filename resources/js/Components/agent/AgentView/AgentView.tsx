import { ChatPane } from "@/Components/builder/ChatPane"
import { Agent } from "@/types/agents"
import { AgentSidebar } from "../AgentSidebar"
import { usePage } from "@inertiajs/react"

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
  // const user = usePage().props.auth.user
  const props = usePage().props as any
  const user = props.auth.user
  console.log(user)
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
      <AgentSidebar agent={agent} files={files} isOwner={!!user && (owner === user?.github_nickname || owner === user?.twitter.nickname)}>
        <ChatPane agent={agent} conversationId={conversation.id} initialMessages={initialMessages} owner={owner} />
      </AgentSidebar>
    </div>
  )
}

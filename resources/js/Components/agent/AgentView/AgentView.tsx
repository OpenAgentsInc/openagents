import { ChatPane } from "@/Components/builder/ChatPane"
import { Card, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card"
import { Agent } from "@/types/agents"

interface AgentViewProps {
  agent: Agent
  conversation: any
}

export const AgentView = ({ agent, conversation }: AgentViewProps) => {
  const initialMessages = [{ id: 0, role: "assistant", content: agent.welcome_message, tokens: [] }]
  return (
    <div className="h-full">
      <Card className="fixed left-[20px] top-[20%] w-[350px]">
        <CardHeader>
          <CardTitle>{agent.name}</CardTitle>
          <CardDescription>{agent.description}</CardDescription>
          {/* <CardDescription>By [author]</CardDescription> */}
        </CardHeader>
      </Card>
      <ChatPane agentId={agent.id} conversationId={conversation.id} initialMessages={initialMessages} />
    </div>
  )
}

import { Card, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card"
import { Agent } from "@/types/agents"

interface AgentKnowledgeProps {
  agent: Agent
}

export const AgentKnowledge = ({ agent }: AgentKnowledgeProps) => {
  return (
    <div className="w-full p-4">
      <Card className="w-full mx-auto">
        <CardHeader>
          <CardTitle className="text-xl">{agent.name} Knowledge</CardTitle>
          <CardDescription className="text-base">{agent.description}</CardDescription>
        </CardHeader>
      </Card>
    </div>
  )
}

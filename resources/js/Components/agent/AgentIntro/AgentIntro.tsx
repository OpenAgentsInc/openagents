import { Card, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card"
import { Agent } from "@/types/agents"

interface AgentViewProps {
  agent: Agent
}

export const AgentIntro = ({ agent }: AgentViewProps) => {
  return (
    <Card className="w-full mx-auto">
      <CardHeader>
        <CardTitle className="text-xl">{agent.name}</CardTitle>
        <CardDescription>{agent.description}</CardDescription>
        <CardDescription>By [author]</CardDescription>
      </CardHeader>
    </Card>
  )
}

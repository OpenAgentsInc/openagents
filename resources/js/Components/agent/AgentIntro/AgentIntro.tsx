import { Card, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card"
import { Agent } from "@/types/agents"

interface AgentViewProps {
  agent: Agent
  owner: string // username of agent's owner
}

export const AgentIntro = ({ agent, owner }: AgentViewProps) => {
  return (
    <Card className="w-full mx-auto">
      <CardHeader>
        <CardTitle className="text-xl">{agent.name}</CardTitle>
        <CardDescription className="text-base">{agent.description}</CardDescription>
        <CardDescription>By {owner}</CardDescription>
      </CardHeader>
    </Card>
  )
}

import { Card, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card"
import { Agent } from "@/types/agents"

interface AgentKnowledgeProps {
  agent: Agent
}

const knowledge = [
  {
    id: 1,
    name: "Deposition.pdf",
    size: "13 MB",
  },
  {
    id: 2,
    name: "Deposition2.pdf",
    size: "4 MB",
  },
]

export const AgentKnowledge = ({ agent }: AgentKnowledgeProps) => {
  return (
    <div className="w-full p-5">
      <div className="mb-4">
        <h2 className="text-xl">Knowledge</h2>
        <p className="font-light">See the knowledge base of {agent.name}</p>
      </div>

      {knowledge.map((file) => (
        <Card key={file.id} className="w-full mx-auto my-3">
          <CardHeader className="px-4 py-3">
            <CardTitle className="text-lg">{file.name}</CardTitle>
            <CardDescription className="text-base">{file.size}</CardDescription>
          </CardHeader>
        </Card>
      ))}
    </div>
  )
}

import { Card, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card"
import { Agent } from "@/types/agents"
import { KnowledgeUploader } from "../KnowledgeUploader"

interface AgentKnowledgeProps {
  agent: Agent
  files?: any[]
  isOwner: boolean
}

const dummyKnowledge = [
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

export const AgentKnowledge = ({ agent, files, isOwner }: AgentKnowledgeProps) => {
  const knowledge = files ?? dummyKnowledge
  return (
    <div className="overflow-y-auto w-full p-5">
      <div className="mb-4">
        <h2 className="text-xl">Knowledge</h2>
        <p className="font-light">See the knowledge base of {agent.name}</p>
      </div>

      {isOwner && <KnowledgeUploader agent={agent} />}

      {knowledge.map((file) => (
        <Card key={file.id} className="w-full mx-auto my-3">
          <CardHeader className="px-4 py-3">
            <CardTitle className="text-lg truncate">{file.name}</CardTitle>
            <CardDescription className="text-base">{file.size} bytes {file.mime_type === 'application/pdf' ? "PDF" : ""}</CardDescription>
          </CardHeader>
        </Card>
      ))}

      {/* If no knowledge, show a message */}

      {knowledge.length === 0 && (
        <div className="text-center">
          <p className="text-sm italic">No files uploaded</p>
        </div>
      )}
    </div>
  )
}
